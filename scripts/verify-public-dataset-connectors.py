"""Live acceptance of reviewed public dataset connectors through the native API.

Freezes one repository-owned executable and uses a new isolated loopback store.
Original transfers are started only after resolve, metadata connection and explicit
source selection. Rasterio independently decodes every block and checks native raw
pixel readback. This script uses no credentials, model or Earth Engine computation.
It does not operate the user's desktop, publish anything, or install dependencies.
"""
import argparse
import base64
from datetime import datetime, timezone
import hashlib
import io
import json
import math
from pathlib import Path
import shutil
import socket
import subprocess
import time
import traceback
from urllib.error import HTTPError
from urllib.request import Request, urlopen

import numpy as np
from PIL import Image
import rasterio
from rasterio.windows import Window, from_bounds
from rasterio.warp import transform_bounds


CASES = [
    {"datasetId": "worldcover", "bounds": [15.1, 78.1, 15.2, 78.2], "year": 2021, "layer": "map"},
    {"datasetId": "worldcover-composites", "bounds": [10.1, 78.1, 10.2, 78.2], "year": 2021,
     "layer": "swir", "assetContains": "_SWIR.tif"},
    {"datasetId": "worldpop", "bounds": [-169.95, -19.2, -169.75, -18.95], "year": 2020, "country": "NIU"},
    {"datasetId": "jrc-water", "bounds": [115.1, 45.1, 115.2, 45.2],
     "layer": "occurrence", "assetContains": "/occurrence/"},
    {"datasetId": "glad-forest", "bounds": [115.1, 45.1, 115.2, 45.2],
     "layer": "lossyear", "assetContains": "_lossyear_"},
    {"datasetId": "etopo", "bounds": [2.1, 42.1, 2.2, 42.2],
     "layer": "surface", "assetContains": "_surface.tif"},
]


def sha256_file(path):
    checksum = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            checksum.update(chunk)
    return checksum.hexdigest()


def scalar(value):
    value = value.item() if hasattr(value, "item") else value
    return str(value) if isinstance(value, float) and not math.isfinite(value) else value


def inspect_all(path, publisher_nodata=None):
    """Decode all original pixels blockwise, without a remote raster shortcut."""
    started = time.monotonic()
    with rasterio.open(path) as dataset:
        bands = []
        for band in range(1, dataset.count + 1):
            valid_count, all_count, minimum, maximum = 0, 0, None, None
            publisher_count, publisher_minimum, publisher_maximum = 0, None, None
            first_usable = None
            counts = {} if np.issubdtype(np.dtype(dataset.dtypes[band - 1]), np.integer) else None
            for _, window in dataset.block_windows(band):
                values = dataset.read(band, window=window, masked=True)
                assert values.size == int(window.width * window.height)
                all_count += values.size
                valid = values.compressed()
                valid = valid[np.isfinite(valid)]
                if first_usable is None:
                    usable = ~np.ma.getmaskarray(values) & np.isfinite(values.data)
                    if publisher_nodata is not None:
                        usable &= values.data != publisher_nodata["value"]
                    indices = np.flatnonzero(usable)
                    if len(indices):
                        row, column = np.unravel_index(indices[0], values.shape)
                        first_usable = {"column": int(window.col_off) + int(column),
                                        "row": int(window.row_off) + int(row),
                                        "value": scalar(values.data[row, column])}
                valid_count += len(valid)
                if len(valid):
                    lo, hi = scalar(valid.min()), scalar(valid.max())
                    minimum = lo if minimum is None else min(minimum, lo)
                    maximum = hi if maximum is None else max(maximum, hi)
                    if counts is not None and hi - lo <= 256 and len(counts) <= 256:
                        unique, frequency = np.unique(valid, return_counts=True)
                        for value, number in zip(unique, frequency):
                            key = str(scalar(value)); counts[key] = counts.get(key, 0) + int(number)
                    else:
                        counts = None
                if publisher_nodata is not None:
                    interpreted = valid[valid != publisher_nodata["value"]]
                    publisher_count += len(interpreted)
                    if len(interpreted):
                        lo, hi = scalar(interpreted.min()), scalar(interpreted.max())
                        publisher_minimum = lo if publisher_minimum is None else min(publisher_minimum, lo)
                        publisher_maximum = hi if publisher_maximum is None else max(publisher_maximum, hi)
            assert all_count == dataset.width * dataset.height
            bands.append({"index": band, "dtype": dataset.dtypes[band - 1],
                          "nodata": scalar(dataset.nodatavals[band - 1]), "allPixelsDecoded": all_count,
                          "validPixels": valid_count, "validFraction": valid_count / all_count,
                          "minimum": minimum, "maximum": maximum, "valueCounts": counts,
                          "firstUsableSample": first_usable})
            if publisher_nodata is not None and dataset.nodatavals[band - 1] is None:
                bands[-1]["publisherSentinelStats"] = {
                    "kind": "Auxiliary declaration-based sentinel statistics; original TIFF mask and values remain unchanged",
                    "value": publisher_nodata["value"], "sourceUrl": publisher_nodata["sourceUrl"],
                    "declarationKind": publisher_nodata.get("declarationKind", "explicit acceptance-case declaration"),
                    "embeddedNoDataAgrees": dataset.nodatavals[band - 1] == publisher_nodata["value"],
                    "validPixels": publisher_count, "validFraction": publisher_count / all_count,
                    "minimum": publisher_minimum, "maximum": publisher_maximum,
                    "additionalSentinelPixelsExcluded": valid_count - publisher_count,
                }
        return {"width": dataset.width, "height": dataset.height, "count": dataset.count,
                "crs": str(dataset.crs), "bounds": list(dataset.bounds),
                "transform": list(dataset.transform)[:6], "resolution": list(dataset.res),
                "scales": list(dataset.scales), "offsets": list(dataset.offsets),
                "units": list(dataset.units), "tags": dataset.tags(), "bands": bands,
                "seconds": round(time.monotonic() - started, 3), "completeDecode": True}


def aoi_usable_sample(dataset, bounds, sentinel=None):
    """Pick one finite, unmasked original pixel near the requested AOI, if any.

    The point is evidence of one sample, not a valid-data coverage measurement.
    Coordinates and numerical samples are not resampled or rewritten.
    """
    if not dataset.crs or not dataset.transform.is_rectilinear:
        return None
    west, south, east, north = transform_bounds("EPSG:4326", dataset.crs, *bounds)
    extent = dataset.bounds
    west, south, east, north = max(west, extent.left), max(south, extent.bottom), min(east, extent.right), min(north, extent.top)
    if west >= east or south >= north:
        return None
    requested = from_bounds(west, south, east, north, transform=dataset.transform)
    # Use pixel centers so a partially intersecting edge cell outside the AOI
    # is not presented as an in-area sample. Extremely small AOIs may have none.
    left, top = max(0, math.ceil(requested.col_off - .5)), max(0, math.ceil(requested.row_off - .5))
    right = min(dataset.width, math.floor(requested.col_off + requested.width - .5) + 1)
    bottom = min(dataset.height, math.floor(requested.row_off + requested.height - .5) + 1)
    for row in range(top, bottom, 512):
        for column in range(left, right, 512):
            window = Window(column, row, min(512, right - column), min(512, bottom - row))
            values = dataset.read(1, window=window, masked=True)
            usable = ~np.ma.getmaskarray(values) & np.isfinite(values.data)
            if sentinel is not None:
                usable &= values.data != sentinel
            indices = np.flatnonzero(usable)
            if len(indices):
                sample_row, sample_column = np.unravel_index(indices[0], values.shape)
                return {"column": column + int(sample_column), "row": row + int(sample_row),
                        "value": scalar(values.data[sample_row, sample_column]),
                        "scope": "one original pixel within a requested-area window; not AOI coverage validation"}
    return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exe", required=True, type=Path)
    parser.add_argument("--run", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--port", type=int, default=4658)
    parser.add_argument("--cases", type=Path)
    parser.add_argument("--recheck", action="store_true", help="Recheck settled originals in this private run with a new frozen binary; download only cases without a settled job")
    args = parser.parse_args()
    repository = Path(__file__).resolve().parents[1]
    run = args.run.resolve()
    assert run.parent == repository / ".verification" and run.name.startswith("public-datasets-")
    assert args.recheck or not run.exists(), "Acceptance requires a new isolated directory"
    assert args.exe.resolve().is_relative_to(repository), "Use this repository's native binary"
    assert 1024 <= args.port <= 65535
    with socket.socket() as check:
        check.bind(("127.0.0.1", args.port))
    run.mkdir(exist_ok=args.recheck)
    previous = None
    if args.recheck:
        previous = json.loads((run / "verification.json").read_text(encoding="utf-8"))
        assert Path(previous["run"]).samefile(run)
        old = run / ("verification-before-recheck-" + str(time.time_ns()) + ".json")
        shutil.copyfile(run / "verification.json", old)
    binary_hash = sha256_file(args.exe)
    frozen = run / ("runtime-" + binary_hash[:16] + args.exe.suffix)
    shutil.copyfile(args.exe, frozen)
    assert sha256_file(frozen) == binary_hash, "Executable changed while freezing the run"
    store = run / "core"
    base = f"http://127.0.0.1:{args.port}"
    cases = json.loads(args.cases.read_text(encoding="utf-8")) if args.cases else CASES
    assert isinstance(cases, list) and cases, "Provide at least one explicit case"
    for case in cases:
        if "publisherNoData" in case:
            declaration = case["publisherNoData"]
            assert math.isfinite(declaration["value"]) and declaration["sourceUrl"].startswith("https://"), "Publisher sentinel needs a finite value and documentation URL"
    report = {"schema": "geod-public-dataset-native-acceptance/v1", "status": "running",
              "checkedAt": datetime.now(timezone.utc).isoformat(), "binarySha256": binary_hash,
              "reader": f"Rasterio {rasterio.__version__} / GDAL {rasterio.__gdal_version__}",
              "credentialsRead": False, "modelCalls": 0, "earthEngineComputeRequests": 0,
              "nativeWindowTested": False, "run": str(run), "cases": [], "errors": []}
    if previous:
        report["previousBinarySha256"] = previous["binarySha256"]
    args.report.parent.mkdir(parents=True, exist_ok=True)

    def save():
        (run / "verification.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    def api(route, payload=None):
        request = Request(base + route, data=None if payload is None else json.dumps(payload).encode(),
                          headers={"X-GeoD-Client": "geod-global", "Content-Type": "application/json"},
                          method="GET" if payload is None else "POST")
        try:
            with urlopen(request, timeout=90) as response:
                return json.loads(response.read())
        except HTTPError as error:
            detail = error.read().decode("utf-8", "replace")
            raise RuntimeError(f"{route}: HTTP {error.code} {detail}") from error

    def start():
        out = (run / "runtime.stdout.log").open("ab")
        err = (run / "runtime.stderr.log").open("ab")
        process = subprocess.Popen([str(frozen), "serve", "--data-dir", str(store), "--port", str(args.port)],
                                   cwd=repository, stdin=subprocess.DEVNULL, stdout=out, stderr=err,
                                   creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        out.close(); err.close()
        for _ in range(150):
            try:
                api("/health"); return process
            except Exception:
                assert process.poll() is None, "Private runtime failed; inspect its log"
                time.sleep(.1)
        process.terminate(); process.wait(timeout=10)
        raise RuntimeError("Private runtime did not become ready")

    process = start()
    try:
        report["datasets"] = api("/datasets")
        known = {dataset["id"] for dataset in report["datasets"]}
        for case_number, case in enumerate(cases, 1):
            record = {"request": case, "status": "running"}
            report["cases"].append(record); save()
            try:
                assert case["datasetId"] in known, "Dataset ID missing from actual runtime"
                # Case selectors and independent evidence are QA instructions,
                # not native ResolveRequest fields. Keep the production API's
                # deny_unknown_fields contract instead of leaking new QA keys.
                request = {key: case[key] for key in (
                    "datasetId", "bounds", "year", "month", "country", "layer", "region", "season"
                ) if key in case}
                prior = next((row for row in previous["cases"] if row["request"] == case and row.get("job")), None) if previous else None
                before = api("/jobs")
                record["plan"] = api("/datasets/resolve", request)
                record["resolvedPlan"] = record["plan"]
                connected = prior["connectionResult"] if prior else api("/datasets/connect", request)
                record["connectionResult"] = connected
                assert api("/jobs") == before, "Metadata connection unexpectedly started a transfer"
                assert connected["complete"] and not connected["failures"], "Some native metadata probes failed"
                record["discoveryStartedNoDownloads"] = True
                assert connected["stacConnections"], "Expected public raster metadata connections"
                items = []
                for connection in connected["stacConnections"]:
                    for snapshot in connection["snapshotIds"]:
                        item = api("/stac/snapshots/" + snapshot)
                        for asset in item["assets"]:
                            if asset["eligible"] and case.get("assetContains", "") in asset["href"] \
                                    and ("assetKey" not in case or asset["key"] == case["assetKey"]):
                                items.append((item, asset))
                assert items, "No eligible native numeric asset matched explicit selection"
                item, asset = items[0]
                # Direct raster receipts carry the client preset in their
                # generated properties; publisher STAC properties stay original
                # and bind the reviewed preset separately in provenance.
                preset = item["properties"].get("datasetPreset")
                if preset is None:
                    preset = item["provenance"]["reviewedDataset"]
                assert isinstance(preset, dict), "Native source has no reviewed client preset"
                assert preset["dataset"] == record["plan"]["dataset"]
                assert preset["dataset"]["id"] == case["datasetId"]
                assert preset["request"] == record["plan"]["request"]
                selected_layer = next(layer for layer in record["plan"]["dataset"]["layers"]
                                      if layer["id"] == record["plan"]["request"].get("layer", record["plan"]["dataset"]["defaultLayer"]))
                assert preset["layer"] == selected_layer
                assert preset["declarationKind"].startswith("reviewed-client-preset;")
                record["source"] = {"snapshot": item, "asset": asset}
                selection = {"snapshotId": item["id"], "assetKey": asset["key"]}
                project = next(item for item in api("/projects") if item["id"] == prior["project"]["id"]) if prior else api("/stac/project", {"name": "Live acceptance · " + case["datasetId"],
                                                "bounds": case["bounds"], "selections": [selection]})
                record["project"] = project
                assert len(project["stacItems"]) == 1
                download_started = time.monotonic()
                queued = {"jobs": [api("/jobs/" + prior["job"]["id"])]} if prior else api("/stac/downloads", {"projectId": project["id"], "selections": [selection]})
                assert len(queued["jobs"]) == 1
                job = queued["jobs"][0]
                deadline = time.monotonic() + 900
                while time.monotonic() < deadline:
                    job = api("/jobs/" + job["id"])
                    if job["status"] in {"failed", "cancelled", "interrupted"}:
                        raise RuntimeError("Actual source transfer failed: " + str(job.get("error")))
                    if job["status"] == "succeeded" and job.get("settled"):
                        break
                    time.sleep(.4)
                else:
                    record["unfinishedJob"] = job
                    raise RuntimeError("Transfer did not settle within 15-minute observation")
                record["downloadSeconds"] = prior["downloadSeconds"] if prior else round(time.monotonic() - download_started, 3)
                record["settledTransferReusedForFinalInspection"] = bool(prior)
                record["job"] = job
                path = Path(job["outputPath"]).resolve(strict=True)
                assert path.parent.samefile(store / "assets"), "Original output escaped managed assets"
                assert path.stat().st_size == job["bytesDownloaded"]
                assert job["totalBytes"] in {None, path.stat().st_size}
                assert sha256_file(path) == job["sha256"]
                if "expectedBytes" in case:
                    assert path.stat().st_size == case["expectedBytes"], "Original differs from independently measured byte count"
                if "expectedSha256" in case:
                    assert job["sha256"] == case["expectedSha256"], "Original differs from the independent reference SHA-256"
                    record["independentReferenceSha256Matched"] = True
                if "expectedMd5" in case:
                    with path.open("rb") as original:
                        digest = hashlib.file_digest(original, "md5").hexdigest()
                    assert digest == case["expectedMd5"], "Original differs from publisher-record MD5"
                    record["publisherIntegrityCheck"] = {"algorithm": "md5", "expected": case["expectedMd5"],
                        "actual": digest, "matched": True, "scope": "Independent acceptance reader; not a native direct-source checksum contract"}
                declaration = case.get("publisherNoData")
                if declaration is None and preset["layer"].get("nodata") is not None:
                    declaration = {"value": preset["layer"]["nodata"], "sourceUrl": preset["dataset"]["documentationUrl"],
                                   "declarationKind": preset["declarationKind"]}
                record["independentRaster"] = inspect_all(path, declaration)
                if "expectedShape" in case:
                    assert [record["independentRaster"]["height"], record["independentRaster"]["width"]] == case["expectedShape"]
                native = api("/stac/jobs/" + job["id"] + "/inspect")
                record["nativeInspection"] = {key: value for key, value in native.items() if key != "previewDataUrl"}
                record["nativeInspection"]["previewAvailable"] = bool(native.get("previewDataUrl"))
                assert native["sha256"] == job["sha256"] and native["jobId"] == job["id"]
                assert [native["width"], native["height"]] == [record["independentRaster"]["width"], record["independentRaster"]["height"]]
                assert len(native["bands"]) == record["independentRaster"]["count"]
                assert native["crs"] and native["bounds"] and native["transform"], "Native georeferencing missing"
                assert all(math.isclose(left, right, rel_tol=1e-10, abs_tol=1e-6)
                           for left, right in zip(native["bounds"], record["independentRaster"]["bounds"])), "Native file bounds differ from independent decoder"
                if case["datasetId"] == "etopo":
                    assert record["independentRaster"]["crs"] == "EPSG:9518"
                    assert native["horizontalCrs"] == "EPSG:4326"
                    assert native["verticalCrs"] == "EPSG:3855"
                    assert native["sourceModelPixelScale"] == [1 / 240, 1 / 240, 1]
                    assert native["sourceModelTiepoint"] == [0, 0, 0, 0, 45, 0]
                assert native["previewDataUrl"], "Native raw preview missing"
                preview = base64.b64decode(native["previewDataUrl"].split(",", 1)[1], validate=True)
                with Image.open(io.BytesIO(preview)) as image:
                    assert list(image.size) == [native["previewWidth"], native["previewHeight"]]
                    image.verify()
                preview_name = f"{case_number:02d}-{case['datasetId']}-{request.get('layer', 'default')}-{asset['key']}-preview.png"
                (run / preview_name).write_bytes(preview)
                record["previewFile"] = preview_name
                record["previewSha256"] = hashlib.sha256(preview).hexdigest()
                native["previewDataUrl"] = "saved in private run directory"
                record["nativeInspection"] = native
                record["pixels"] = []
                with rasterio.open(path) as dataset:
                    coords = {(0, 0), (dataset.width - 1, dataset.height - 1),
                              (dataset.width // 2, dataset.height // 2),
                              (dataset.width // 3, dataset.height // 3),
                              (dataset.width * 2 // 3, dataset.height * 2 // 3)}
                    useful = aoi_usable_sample(dataset, case["bounds"], declaration["value"] if declaration else None)
                    if useful is None:
                        first = record["independentRaster"]["bands"][0]["firstUsableSample"]
                        useful = {**first, "scope": "one original pixel elsewhere in the file; requested-area validity was not established"} if first else None
                    assert useful, "The representative original contains no finite, unmasked first-band sample after declared sentinel exclusion"
                    record["usefulPixelSample"] = {**useful, "validityBasis": "Embedded file mask plus finite values and any reviewed declaration-based sentinel; no scientific coverage claim"}
                    coords.add((useful["column"], useful["row"]))
                    for coordinate in case.get("sampleCoordinates", []):
                        column, row = coordinate
                        assert isinstance(column, int) and isinstance(row, int)
                        assert 0 <= column < dataset.width and 0 <= row < dataset.height
                        coords.add((column, row))
                    for column, row in sorted(coords):
                        actual = api(f"/stac/jobs/{job['id']}/pixel?column={column}&row={row}")
                        expected = dataset.read(window=((row, row + 1), (column, column + 1)))[:, 0, 0]
                        assert actual["sha256"] == job["sha256"]
                        assert len(actual["values"]) == dataset.count
                        for index, value in enumerate(expected):
                            sample = scalar(value)
                            got = actual["values"][index]
                            assert got == sample or (isinstance(sample, float) and math.isclose(float(got), sample, rel_tol=1e-7, abs_tol=1e-7))
                            nodata = dataset.nodatavals[index]
                            missing = nodata is not None and (value == nodata or np.isnan(value) and np.isnan(nodata))
                            assert actual["noData"][index] == bool(missing)
                        record["pixels"].append(actual)
                assert sha256_file(path) == job["sha256"], "Native inspection changed the original bytes"
                record["originalSha256Verified"] = True
                if prior:
                    assert api("/jobs") == before, "Final reinspection unexpectedly created or changed a transfer"
                record["status"] = "passed"
                print(json.dumps({"dataset": case["datasetId"], "status": "passed", "bytes": path.stat().st_size,
                                  "dimensions": [native["width"], native["height"]], "bands": len(native["bands"]),
                                  "downloadSeconds": record["downloadSeconds"]}), flush=True)
            except Exception as error:
                record["status"] = "failed"; record["error"] = f"{type(error).__name__}: {error}"
                record["traceback"] = traceback.format_exc()
                report["errors"].append({"dataset": case["datasetId"], "error": record["error"]})
                print(json.dumps(report["errors"][-1]), flush=True)
            save()
        # Reopen only after jobs have settled; restarting never touches a user's store.
        assert not any(job["status"] in {"queued", "running"} for job in api("/jobs")), "Private run still has live transfers"
        process.terminate(); process.wait(timeout=15)
        process = start()
        projects = {item["id"]: item for item in api("/projects")}
        for record in report["cases"]:
            if record["status"] != "passed":
                continue
            assert projects[record["project"]["id"]]["stacItems"] == record["project"]["stacItems"]
            assert api("/jobs/" + record["job"]["id"])["sha256"] == record["job"]["sha256"]
            probe = record["pixels"][0]
            again = api(f"/stac/jobs/{record['job']['id']}/pixel?column={probe['column']}&row={probe['row']}")
            assert again == probe
            record["reopenedProjectAndSourceVerified"] = True
            reopened = api("/stac/jobs/" + record["job"]["id"] + "/inspect")
            reopened_preview = base64.b64decode(reopened["previewDataUrl"].split(",", 1)[1], validate=True)
            assert hashlib.sha256(reopened_preview).hexdigest() == record["previewSha256"], "Reopened native original preview changed"
            reopened["previewDataUrl"] = "saved in private run directory"
            assert reopened == record["nativeInspection"], "Reopened native original metadata changed"
            record["reopenedOriginalPreviewVerified"] = True
        report["status"] = "passed" if not report["errors"] else "partial"
        report["totalOriginalBytes"] = sum(row.get("job", {}).get("bytesDownloaded", 0) for row in report["cases"])
        report["totalDecodedOriginalPixels"] = sum(row["independentRaster"]["width"] * row["independentRaster"]["height"] * row["independentRaster"]["count"]
                                                   for row in report["cases"] if row.get("status") == "passed")
        report["totalNativePixelCoordinates"] = sum(len(row.get("pixels", [])) for row in report["cases"])
        report["sourceZipPackaging"] = "Not supported by existing derived-raster package API; no package was claimed."
        save()
    finally:
        if process.poll() is None:
            process.terminate(); process.wait(timeout=15)
        report["ownRuntimeStopped"] = process.poll() is not None
        save()
    assert report["status"] == "passed", json.dumps(report["errors"])


if __name__ == "__main__":
    main()
