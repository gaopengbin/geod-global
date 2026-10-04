"""Independent verification of actual native RTC project outputs; no GUI control.

Requires an isolated running native service and completed original downloads.
Python/GDAL are verification tools, not desktop application dependencies.
"""
import argparse
import hashlib
import json
import time
import urllib.request
from pathlib import Path

import numpy as np
import rasterio
from rasterio.windows import Window
from pyproj import Transformer
from shapely import contains_xy
from shapely.geometry import shape


def request(server, path, body=None):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(server + path, data=data, headers={
        'Content-Type': 'application/json', 'X-GeoD-Client': 'geod-global',
    })
    with urllib.request.urlopen(req, timeout=90) as response:
        return json.load(response)


def digest(path):
    h = hashlib.sha256()
    with open(path, 'rb') as source:
        while part := source.read(1024 * 1024):
            h.update(part)
    return h.hexdigest()


def wait(server, job_id):
    previous = None
    for _ in range(240):
        job = request(server, '/jobs/' + job_id)
        key = (job['status'], job['bytesDownloaded'], job['validation'])
        if key != previous:
            print(json.dumps({'jobId':job_id,'status':key[0], 'progress':key[1], 'stage':key[2]}),flush=True)
            previous = key
        if job['status'] in ['succeeded','failed','cancelled','interrupted']:
            assert job['status'] == 'succeeded', job.get('error')
            return job
        time.sleep(2)
    raise TimeoutError('Native radar job is still active; poll its existing ID before continuing')


def scene(job, catalogue):
    item = next(i for i in catalogue['features'] if i['id'] == job['itemId'])
    return {'itemId':item['id'],'date':item['properties']['datetime'],'cloud':None,
            'crs':f"EPSG:{item['properties']['proj:epsg']}",'gridCode':None,
            'bbox':item['bbox'], 'assets':{'vv':{'href':job['href'],'mediaType':job['mediaType']}}}


def verify(server, job, jobs, project, catalogue):
    path = Path(job['outputPath']); plan = job['mosaicOutput']
    assert digest(path) == job['sha256']
    profile = {'product':'sentinel-1-iw-rtc','polarization':'VV','quantity':'gamma0','unit':'linear'}
    assert plan['radar'] == profile
    assert all(plan.get(k) is None for k in ['elevation','calibration','aerial'])
    with rasterio.open(path) as output:
        assert output.count == 1 and output.dtypes == ('float32',) and output.nodata == -32768
        assert output.crs.to_string() == plan['crs'] and list(output.bounds) == plan['bounds']
        assert list(output.res) == [10,10] and output.tags().get('AREA_OR_POINT') == 'Area'
        assert output.tags(1).get('POLARIZATION') == 'VV'
        assert output.units == ('gamma0 (linear)',)
        actual = output.read(1)
        expected = np.full(actual.shape,-32768,dtype=np.float32)
        acquisitions = {item['id']:item['properties']['datetime'] for item in catalogue['features']}
        ordered = sorted(job['mosaic']['sources'],key=lambda pin:(
            acquisitions[jobs[pin['jobId']]['itemId']],jobs[pin['jobId']]['itemId']))
        assert job['mosaic']['sources'] == ordered, 'Native source order differs from official acquisition chronology'
        overlap = {'olderValidUnderNewerNoData':0,'newerValidReplacesDifferentOlderValue':0}
        for pin in ordered:
            original = jobs[pin['jobId']]
            assert original['sha256'] == pin['sha256']
            with rasterio.open(original['outputPath']) as source:
                assert source.crs == output.crs and source.res == output.res
                col, row = (~source.transform) * (output.bounds.left,output.bounds.top)
                assert abs(col-round(col)) < 1e-5 and abs(row-round(row)) < 1e-5
                values = source.read(1,window=Window(round(col),round(row),output.width,output.height),
                                     boundless=True,fill_value=-32768)
                assert np.isfinite(values).all() and ((values>=0)|(values==-32768)).all()
                previous_valid = expected != -32768
                overlap['olderValidUnderNewerNoData'] += int((previous_valid & (values==-32768)).sum())
                overlap['newerValidReplacesDifferentOlderValue'] += int((previous_valid & (values!=-32768)
                    & (values.view(np.uint32)!=expected.view(np.uint32))).sum())
                np.copyto(expected,values,where=values != -32768)
        masked = 0
        if project.get('geometry'):
            # The saved polygon has straight edges in WGS84, not in UTM.
            # Projecting only its vertices changes those edges. Independently
            # classify actual pixel centres in the original geographic polygon
            # with PROJ and GEOS, without using the native projection or mask.
            transform = Transformer.from_crs(output.crs,'EPSG:4326',always_xy=True)
            columns,rows = np.meshgrid(np.arange(output.width)+0.5,np.arange(output.height)+0.5)
            x,y = output.transform * (columns,rows)
            longitude,latitude = transform.transform(x,y)
            outside = ~contains_xy(shape(project['geometry']),longitude,latitude)
            masked = int(outside.sum()); expected[outside] = -32768
        # Compare the actual Float32 bit patterns, never a display image or dB values.
        assert np.array_equal(actual.view(np.uint32),expected.view(np.uint32))
        assert plan['coveredPixels'] == int((expected != -32768).sum())
        assert plan['maskedPixels'] == masked
        points = []
        for row,col in [(0,0),(output.height//2,output.width//2),(output.height-1,output.width-1)]:
            x,y=output.xy(row,col); value=float(actual[row,col])
            pixel=request(server,f"/jobs/{job['id']}/pixel?x={x}&y={y}")
            assert pixel['value'] == value and pixel['pixel'] == [col,row] and pixel['sha256'] == job['sha256']
            points.append({'pixel':[col,row],'gamma0':value,'exactMatch':True})
    manifest=json.loads(Path(job['manifestPath']).read_text())
    assert manifest['plan'] == plan and manifest['output']['sha256'] == job['sha256']
    assert all(source['radar']==profile and source['additionalCalibrationApplied'] is False
               and source['speckleFilteringApplied'] is False for source in manifest['sources'])
    preview=request(server,f"/jobs/{job['id']}/raster")
    assert preview['radar']['unit']=='linear' and preview['radar']['polarization']=='VV'
    thumb=request(server,f"/jobs/{job['id']}/thumbnail")
    assert thumb['sha256']==job['sha256']
    return {'jobId':job['id'],'projectId':project['id'],'projectName':project['name'],
            'bytes':job['bytesDownloaded'],'sha256':job['sha256'],'plan':plan,
            'float32ValuesCompared':int(actual.size),'allFloat32BitsMatch':True,
            'independentDecoder':'GDAL/rasterio; PROJ/pyproj; GEOS/Shapely WGS84 pixel-centre polygon mask',
            'acquisitionOrder':[{'itemId':jobs[pin['jobId']]['itemId'],
                                 'datetime':acquisitions[jobs[pin['jobId']]['itemId']]} for pin in ordered],
            'overlapChecks':overlap,
            'pixelChecks':points,'nativePreviewVerified':True,'persistentThumbnailVerified':True}


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--server',default='http://127.0.0.1:4328')
    parser.add_argument('--catalogue',default='.verification/sentinel-1-rtc-all.json')
    parser.add_argument('--jobs',nargs='+',required=True)
    parser.add_argument('--out',default='.verification/radar-processing-verification.json')
    args=parser.parse_args(); server=args.server
    catalogue=json.loads(Path(args.catalogue).read_text())
    originals=[wait(server,jid) for jid in args.jobs]
    assert len(originals)==2
    for job in originals:
        assert job['kind']=='download' and job['assetKey']=='vv'
        assert digest(job['outputPath'])==job['sha256']
    jobs={job['id']:job for job in originals}
    bounds=[-122.55,37.65,-122.4,37.8]
    outer=[[bounds[0],bounds[1]],[bounds[2],bounds[1]],[bounds[2],bounds[3]],[bounds[0],bounds[3]],[bounds[0],bounds[1]]]
    hole=[[-122.49,37.71],[-122.46,37.71],[-122.46,37.74],[-122.49,37.74],[-122.49,37.71]]
    cases=[('RTC · 实际矩形裁剪',originals[:1],None),
           ('RTC · 实际孔洞多边形裁剪',originals[:1],{'type':'Polygon','coordinates':[outer,hole]}),
           ('RTC · 实际双景拼接',originals,None)]
    reports=[]
    for name,sources,geometry in cases:
        project=next((p for p in request(server,'/projects') if p['name']==name),None)
        if project is None:
            project=request(server,'/projects',{'name':name,'bounds':bounds,'geometry':geometry,
                                               'scenes':[scene(job,catalogue) for job in sources]})
        done=next((j for j in request(server,'/jobs') if j['status']=='succeeded'
                   and j.get('mosaic',{} ) and j['mosaic']['projectId']==project['id']),None)
        job=done or wait(server,request(server,f"/projects/{project['id']}/mosaics",{'assetKey':'vv'})['id'])
        reports.append(verify(server,job,jobs,project,catalogue))
    result={'checkedAt':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),
            'scope':'Two actual Sentinel-1 VV RTC originals; rectangular and polygon-with-hole clips plus aligned two-scene mosaic; all output Float32 values checked',
            'originals':[{k:j[k] for k in ['id','itemId','href','bytesDownloaded','sha256']} for j in originals],
            'results':reports,'usedUserDesktop':False,'otherPolarizationsAccepted':False,
            'calibrationApplied':False,'speckleFilteringApplied':False}
    path=Path(args.out); path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(json.dumps(result,indent=2),encoding='utf8')
    print(json.dumps({'report':str(path),'outputs':len(reports),'valuesCompared':sum(r['float32ValuesCompared'] for r in reports)}),flush=True)


if __name__=='__main__': main()
