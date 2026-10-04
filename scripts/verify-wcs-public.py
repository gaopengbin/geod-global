"""Independently verify one bounded real WCS subset through the native local API.

No public endpoint is selected by default. This script never directly calls upstream
services. A successful result is a byte-preserved server subset, not a source archive.
"""
import argparse
import base64
from datetime import datetime, timezone
import importlib.util
import json
import math
from pathlib import Path
import sys
import time
from urllib.parse import parse_qs, urlsplit
from xml.etree import ElementTree as ET

SPEC = importlib.util.spec_from_file_location('stac_qa', Path(__file__).with_name('verify-stac-public.py'))
qa = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(qa)
require, sha, read_json, save_json = qa.require, qa.sha, qa.read_json, qa.save_json
NS = {'w': 'http://www.opengis.net/wcs/2.0', 'g': 'http://www.opengis.net/gml/3.2',
      'o': 'http://www.opengis.net/ows/2.0', 's': 'http://www.opengis.net/swe/2.0',
      'c': 'http://www.opengis.net/gmlcov/1.0'}
XLINK = '{http://www.w3.org/1999/xlink}href'


def close_sequence(actual, expected, message, tolerance=1e-8):
    require(len(actual) == len(expected) and all(math.isclose(a, b, rel_tol=1e-12, abs_tol=tolerance)
                                              for a, b in zip(actual, expected)), message)


def numbers(element):
    require(element is not None and element.text, 'Missing numeric grid component')
    return [float(n) for n in element.text.split()]


def source_grid(xml, ident):
    """Interpret GML grid independently of the native planner. Strict north-up 2D path."""
    tree = ET.fromstring(xml)
    entries = tree.findall('w:CoverageDescription', NS)
    require(len(entries) == 1 and entries[0].findtext('w:CoverageId', namespaces=NS) == ident, 'Wrong original coverage identity')
    source = entries[0]
    envelope = source.find('g:boundedBy/g:Envelope', NS)
    grid = source.find('g:domainSet/g:RectifiedGrid', NS)
    require(envelope is not None and grid is not None and grid.get('dimension') == '2', 'Only an explicit 2D RectifiedGrid can be independently checked')
    crs = envelope.get('srsName', '')
    if crs.endswith('/4326') or crs.upper().endswith(':4326'):
        x_axis, y_axis, raster_crs = 1, 0, 'EPSG:4326'
    elif crs.endswith('/CRS84') or crs.upper().endswith(':CRS84'):
        x_axis, y_axis, raster_crs = 0, 1, 'EPSG:4326'
    elif crs.endswith('/3857') or crs.upper().endswith(':3857'):
        x_axis, y_axis, raster_crs = 0, 1, 'EPSG:3857'
    else:
        raise ValueError('Independent acceptance supports explicit EPSG4326/CRS84/3857 grids only')
    axes = envelope.get('axisLabels', '').split()
    require(len(axes) == 2 and axes[0] != axes[1], 'Invalid source axis labels')
    low = numbers(grid.find('g:limits/g:GridEnvelope/g:low', NS))
    high = numbers(grid.find('g:limits/g:GridEnvelope/g:high', NS))
    origin = numbers(grid.find('g:origin/g:Point/g:pos', NS))
    vectors = [numbers(e) for e in grid.findall('g:offsetVector', NS)]
    require(len(low) == len(high) == len(origin) == len(vectors) == 2 and all(len(v) == 2 for v in vectors), 'Unexpected grid dimensionality')
    require(all(x.is_integer() for x in low + high), 'Grid limits are not integers')
    dx, dy = vectors[0][x_axis], vectors[1][y_axis]
    require(dx > 0 and dy < 0 and vectors[0][y_axis] == vectors[1][x_axis] == 0, 'Independent check requires regular north-up grid axes')
    width, height = int(high[0] - low[0] + 1), int(high[1] - low[1] + 1)
    left = origin[x_axis] + low[0] * dx - dx / 2
    top = origin[y_axis] + low[1] * dy - dy / 2
    fields = []
    for field in source.findall('c:rangeType/s:DataRecord/s:field', NS):
        quantity = field.find('s:Quantity', NS)
        require(quantity is not None, 'Only explicit numeric Quantity fields are checked')
        unit = quantity.find('s:uom', NS)
        fields.append({'name': field.get('name'), 'description': quantity.findtext('s:description', default='', namespaces=NS),
                       'unit': unit.get('code') if unit is not None else None,
                       'nilValues': [{'value': n.text.strip(), 'reason': n.get('reason')}
                                     for n in quantity.findall('s:nilValues/s:NilValues/s:nilValue', NS)]})
    require(fields and width > 0 and height > 0, 'Invalid range or grid size')
    return {'declaredCrs': crs, 'crs': raster_crs, 'axisLabels': axes,
            'gridAxisLabels': grid.findtext('g:axisLabels', default='', namespaces=NS).split(),
            'xAxis': x_axis, 'yAxis': y_axis, 'width': width, 'height': height,
            'low': low, 'high': high, 'origin': origin, 'offsetVectors': vectors,
            'transform': [dx, 0, left, 0, dy, top], 'nativeBounds': [left, top + height * dy, left + width * dx, top],
            'fields': fields, 'metadataLinks': [e.get(XLINK) for e in source.findall('.//o:Metadata', NS) if e.get(XLINK)]}


def predicted_subset(grid, bounds):
    from rasterio.warp import transform_bounds
    west, south, east, north = transform_bounds('EPSG:4326', grid['crs'], *bounds, densify_pts=21)
    dx, _, left, _, dy, top = grid['transform']
    # Input rectangle is snapped outwards to native cell edges, without resampling.
    x0, x1 = math.floor((west - left) / dx + 1e-7), math.ceil((east - left) / dx - 1e-7)
    y0, y1 = math.floor((top - north) / -dy + 1e-7), math.ceil((top - south) / -dy - 1e-7)
    require(0 <= x0 < x1 <= grid['width'] and 0 <= y0 < y1 <= grid['height'], 'Acceptance area falls outside original grid')
    subset_left, subset_top = left + x0 * dx, top + y0 * dy
    native = [subset_left, top + y1 * dy, left + x1 * dx, subset_top]
    return {'width': x1 - x0, 'height': y1 - y0, 'nativeBounds': native,
            'transform': [dx, 0, subset_left, 0, dy, subset_top],
            'bounds': list(transform_bounds(grid['crs'], 'EPSG:4326', *native, densify_pts=21))}


def retain_metadata(root, connection, description, plan, output):
    targets = [('capabilities', 'xml', connection['capabilitiesSha256']),
               ('coverage', 'xml', description['descriptionSha256']),
               ('description', 'json', description['id']), ('plan', 'json', plan['id'])]
    directory = output / 'metadata'
    directory.mkdir(exist_ok=True)
    receipts = []
    for prefix, extension, digest in targets:
        name = f'{prefix}-{digest}.{extension}'
        raw = (root / 'wcs' / name).read_bytes()
        require(sha(raw) == digest, 'Immutable WCS metadata hash mismatch')
        (directory / name).write_bytes(raw)
        receipts.append({'file': 'metadata/' + name, 'sha256': digest, 'bytes': len(raw), 'kind': prefix})
    cap = ET.fromstring((root / 'wcs' / targets_name(targets[0])).read_bytes())
    require(cap.get('version') == connection['version'] == '2.0.1', 'WCS version differs from original')
    summaries = cap.findall('w:Contents/w:CoverageSummary', NS)
    require([s.findtext('w:CoverageId', namespaces=NS) for s in summaries] == [c['id'] for c in connection['coverages']], 'Connection omitted/reordered coverage identities')
    require(cap.findtext('o:ServiceIdentification/o:Fees', default='', namespaces=NS) == connection['fees'], 'Service fees changed')
    require(cap.findtext('o:ServiceIdentification/o:AccessConstraints', default='', namespaces=NS) == connection['accessConstraints'], 'Access constraints changed')
    raw_description = (root / 'wcs' / targets_name(targets[1])).read_bytes()
    grid = source_grid(raw_description, description['coverageId'])
    for key in ('declaredCrs', 'crs', 'axisLabels', 'gridAxisLabels', 'width', 'height', 'fields', 'metadataLinks'):
        require(description[key] == grid[key], 'Description differs from source XML: ' + key)
    close_sequence(description['transform'], grid['transform'], 'Description transform differs from original grid')
    close_sequence(description['nativeBounds'], grid['nativeBounds'], 'Description bounds differ from original grid')
    original = read_json(root / 'wcs' / targets_name(targets[2]))
    require(original['connection'] == connection and original['coverageId'] == description['coverageId'] and original['descriptionSha256'] == description['descriptionSha256'], 'Description record changed source identity')
    pinned = read_json(root / 'wcs' / targets_name(targets[3]))
    require(pinned['descriptionId'] == description['id'] and pinned['bounds'] == plan['requestedBounds'], 'Plan lost its original request')
    require(plan['description'] == description, 'Plan uses a different description')
    return receipts, grid


def targets_name(target):
    return f'{target[0]}-{target[2]}.{target[1]}'


def verify_plan(plan, grid, bounds, endpoint):
    require(plan['requestedBounds'] == bounds, 'Original requested area changed')
    expected = predicted_subset(grid, bounds)
    require(plan['width'] == expected['width'] and plan['height'] == expected['height'], 'Native subset dimensions differ from independent prediction')
    for key in ('transform', 'nativeBounds', 'bounds'):
        close_sequence(plan[key], expected[key], 'Subset grid differs from independent prediction: ' + key)
    require(plan['width'] * plan['height'] <= 65536, 'Acceptance rejects more than 65,536 pixels before any transfer')
    request, base = urlsplit(plan['requestUrl']), urlsplit(endpoint)
    require((request.scheme, request.netloc) == (base.scheme, base.netloc), 'Subset request left the explicit service origin')
    params = parse_qs(request.query, keep_blank_values=True)
    require(set(params) == {'service', 'version', 'request', 'coverageId', 'format', 'subset'}, 'Subset request adds unexpected processing or omits required parameters')
    require(params['service'] == ['WCS'] and params['version'] == ['2.0.1'] and params['request'] == ['GetCoverage'], 'Invalid protocol request')
    require(params['coverageId'] == [plan['description']['coverageId']] and params['format'] == [plan['format']] and 'tiff' in plan['format'].lower(), 'Wrong coverage or response encoding')
    require(len(params['subset']) == 2, 'Both spatial axes must be trimmed exactly once')
    subsets = {}
    for subset in params['subset']:
        label, interval = subset.split('(', 1)
        require(label not in subsets and interval.endswith(')'), 'Invalid or duplicate trim axis')
        subsets[label] = [float(n) for n in interval[:-1].split(',')]
    b = expected['nativeBounds']
    close_sequence(subsets[grid['axisLabels'][grid['xAxis']]], [b[0], b[2]], 'Longitude/easting trim differs')
    close_sequence(subsets[grid['axisLabels'][grid['yAxis']]], [b[1], b[3]], 'Latitude/northing trim differs')
    return expected


def compare_research(reference, research):
    require((reference['width'], reference['height'], reference['bandCount'], reference['crs']) ==
            (research['width'], research['height'], research['bandCount'], research['crs']), 'Application subset differs from research grid')
    close_sequence(reference['transform'], research['transform'], 'Application subset transform differs from research')
    for actual, prior in zip(reference['bands'], research['bands']):
        for key in ('dataType', 'sampleSha256', 'maskSha256', 'samples'):
            require(actual[key] == prior[key], 'Application subset decoded samples differ from research: ' + key)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--server', required=True)
    parser.add_argument('--data-dir', required=True, type=Path)
    parser.add_argument('--endpoint', required=True)
    parser.add_argument('--coverage', required=True)
    parser.add_argument('--bounds', required=True, type=lambda s: [float(x) for x in s.split(',')])
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--connection-id')
    parser.add_argument('--description-id')
    parser.add_argument('--plan-id')
    parser.add_argument('--project-id')
    parser.add_argument('--job-id')
    parser.add_argument('--research-reference', type=Path)
    args = parser.parse_args()
    root, output = args.data_dir.resolve(), args.output.resolve()
    require(root.is_dir() and not output.exists() and not output.is_relative_to(root), 'Use existing storage and new separate evidence directory')
    require(len(args.bounds) == 4 and args.bounds[0] < args.bounds[2] and args.bounds[1] < args.bounds[3], 'Use increasing W,S,E,N bounds')
    require(not args.job_id or args.plan_id and args.project_id, 'Existing job verification requires its saved plan and project IDs')
    output.mkdir(parents=True)
    report = {'status': 'running', 'startedAt': datetime.now(timezone.utc).isoformat(), 'server': args.server,
              'dataDirectory': str(root), 'endpoint': args.endpoint, 'coverageId': args.coverage, 'requestedBounds': args.bounds,
              'productKind': 'server-generated WCS coverage subset', 'originalProviderArchiveClaimed': False,
              'syntheticData': False, 'automaticRetries': 0, 'desktopWebViewAccepted': False,
              'wholeRasterNativePixelComparisonClaimed': False}
    runtime = qa.Runtime(args.server, output)
    save_json(output / 'report.json', report)
    try:
        connections = runtime.call('/wcs/connections')
        matches = [c for c in connections if c['url'].rstrip('/') == args.endpoint.rstrip('/') and
                   (not args.connection_id or c['id'] == args.connection_id)]
        require(not args.connection_id or len(matches) == 1, 'Requested connection does not match explicit endpoint')
        require(len(matches) <= 1, 'Ambiguous existing connections')
        require(matches or not (args.plan_id or args.description_id or args.job_id), 'Saved-source verification must not create a new upstream connection')
        connection = matches[0] if matches else runtime.call('/wcs/connections', {'name': 'Public WCS subset verification', 'url': args.endpoint})
        report['connectionId'] = connection['id']
        save_json(output / 'connection.json', connection)
        save_json(output / 'report.json', report)
        require(read_json(root / 'wcs-connections.json')['connections'][connection['id']] == connection, 'Runtime and storage disagree')
        if args.plan_id:
            plan = runtime.call('/wcs/plans/' + args.plan_id)
            report.update(planId=plan['id'], descriptionId=plan['description']['id'], requestUrl=plan['requestUrl'])
            save_json(output / 'plan.json', plan)
            save_json(output / 'report.json', report)
            description = runtime.call('/wcs/descriptions/' + plan['description']['id'])
        else:
            description = runtime.call('/wcs/descriptions/' + args.description_id) if args.description_id else runtime.call('/wcs/describe', {'connectionId': connection['id'], 'coverageId': args.coverage})
            report['descriptionId'] = description['id']
            save_json(output / 'description.json', description)
            save_json(output / 'report.json', report)
            plan = runtime.call('/wcs/plan', {'descriptionId': description['id'], 'bounds': args.bounds})
        save_json(output / 'description.json', description)
        save_json(output / 'plan.json', plan)
        report.update(descriptionId=description['id'], planId=plan['id'], requestUrl=plan['requestUrl'])
        save_json(output / 'report.json', report)
        require(description['connectionId'] == connection['id'] and description['coverageId'] == args.coverage, 'Wrong source description')
        receipts, grid = retain_metadata(root, connection, description, plan, output)
        report['metadata'] = receipts
        expected = verify_plan(plan, grid, args.bounds, args.endpoint)
        save_json(output / 'independent-source-grid.json', grid)
        save_json(output / 'independent-subset-grid.json', expected)
        pin = {'planId': plan['id']}
        if args.project_id:
            projects = runtime.call('/projects')
            project = next(p for p in projects if p['id'] == args.project_id)
        else:
            project = runtime.call('/wcs/project', {'name': 'Public WCS subset verification', 'bounds': args.bounds, 'selections': [pin]})
        report['projectId'] = project['id']
        save_json(output / 'project.json', project)
        save_json(output / 'report.json', report)
        require(any(i['planId'] == plan['id'] for i in project['wcsItems']), 'Project lost the immutable plan')
        if args.job_id:
            job = runtime.call('/jobs/' + args.job_id)
        else:
            queued = runtime.call('/wcs/downloads', {'projectId': project['id'], 'selections': [pin]})
            save_json(output / 'download-response.json', queued)
            require(len(queued['jobs']) == 1, 'Expected one bounded subset job')
            job = queued['jobs'][0]
            save_json(output / 'queued-job.json', job)
        report['jobId'] = job['id']
        save_json(output / 'report.json', report)
        deadline = time.monotonic() + 300
        while job['status'] in ('queued', 'running') or job.get('settled') is False:
            require(time.monotonic() < deadline, 'Subset did not settle within five minutes')
            time.sleep(1)
            job = runtime.call('/jobs/' + job['id'])
        job.pop('settled', None)
        save_json(output / 'job.json', job)
        require(job['status'] == 'succeeded', 'Coverage transfer failed: ' + str(job.get('error')))
        require(job['wcsSource'] == pin and job['href'] == plan['requestUrl'] and job['itemId'] == args.coverage and job['assetKey'] == 'wcs_coverage', 'Job no longer matches exact planned subset')
        path = Path(job['outputPath'])
        require(path.samefile(root / 'assets' / f"{job['id']}.tif"), 'Result is not the managed subset file')
        require(path.stat().st_size <= 1024 * 1024, 'Acceptance subset exceeds one MiB')
        raw = path.read_bytes()
        require(len(raw) == job['bytesDownloaded'] == job['totalBytes'] and sha(raw) == job['sha256'], 'Returned subset length/hash mismatch')
        (output / 'subset.tif').write_bytes(raw)
        reference = qa.raster_reference(path)
        require(reference['width'] == expected['width'] and reference['height'] == expected['height'] and reference['crs'] == grid['crs'], 'Returned subset has wrong native grid')
        require(reference['bandCount'] == len(grid['fields']), 'Returned subset omitted a range field')
        close_sequence(reference['transform'], expected['transform'], 'Returned subset was shifted/resampled')
        close_sequence(reference['bounds'], expected['nativeBounds'], 'Returned subset has wrong native bounds')
        save_json(output / 'rasterio-reference.json', reference)
        if args.research_reference:
            compare_research(reference, read_json(args.research_reference))
            report['allDecodedSamplesMatchIndependentResearch'] = True
        inspection = runtime.call('/wcs/jobs/' + job['id'] + '/inspect')
        qa.verify_inspection(inspection, reference, job)
        save_json(output / 'native-inspection.json', inspection)
        preview = inspection.get('previewDataUrl')
        if preview:
            (output / 'native-preview.png').write_bytes(base64.b64decode(preview.split(',', 1)[1], validate=True))
        probes = []
        for sample in reference['samples']:
            pixel = runtime.call(f"/wcs/jobs/{job['id']}/pixel?column={sample['column']}&row={sample['row']}")
            qa.verify_pixel(pixel, sample, job)
            probes.append(pixel)
        save_json(output / 'native-pixels.json', probes)
        report.update(status='passed', bytes=len(raw), sha256=sha(raw), width=reference['width'], height=reference['height'],
                      crs=reference['crs'], bands=reference['bands'], nativePixelProbeCount=len(probes),
                      independentDecodedSamples=sum(b['samples'] for b in reference['bands']),
                      declaredFields=grid['fields'], observedFileNodata=[b['nodata'] for b in reference['bands']],
                      sourceWarnings=description['warnings'], planWarnings=plan['warnings'],
                      metadataLinks=grid['metadataLinks'], linkedRightsMetadataFetched=False,
                      serviceFees=connection['fees'], serviceAccessConstraints=connection['accessConstraints'])
    except Exception as error:
        report.update(status='failed', error=f'{type(error).__name__}: {error}')
        raise
    finally:
        report['finishedAt'] = datetime.now(timezone.utc).isoformat()
        save_json(output / 'report.json', report)
    print(json.dumps({'status': report['status'], 'report': str(output / 'report.json'), 'projectId': report.get('projectId'),
                      'jobId': report.get('jobId'), 'bytes': report.get('bytes'), 'samples': report.get('independentDecodedSamples')}))


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
