"""Independent acceptance of two downloaded RGB COGs, using Rasterio only for QA.

Reuses checksum-verified original downloads in a new isolated store. It never
downloads files, edits the input store, or treats fixtures as provider delivery.
Requires Rasterio/NumPy/Shapely; these are not product runtime dependencies.
"""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time
from urllib.error import URLError
from urllib.request import Request, urlopen

import numpy as np
import rasterio
from rasterio.warp import transform, transform_bounds
from rasterio.windows import Window
from shapely import contains_xy
from shapely.geometry import shape


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write_json(path, data):
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def run(args):
    source_root = args.source_store.resolve(strict=True)
    destination = args.output.resolve()
    destination.mkdir(parents=True, exist_ok=False)
    assets = destination / "assets"
    assets.mkdir()
    jobs = json.loads((source_root / "jobs.json").read_text(encoding="utf-8"))
    projects = json.loads((source_root / "projects.json").read_text(encoding="utf-8"))
    original_project = projects[args.project]
    selected = [copy.deepcopy(jobs[job_id]) for job_id in args.sources]
    assert len(selected) == 2 and len(set(args.sources)) == 2, "Exactly two distinct RGB sources required"
    selected.sort(key=lambda job: next((scene["date"], scene["itemId"])
                  for scene in original_project["scenes"] if scene["itemId"] == job["itemId"]))
    originals = []
    for job in selected:
        assert job["status"] == "succeeded" and job["kind"] == "download" and job["assetKey"] == "visual"
        path = Path(job["outputPath"]).resolve(strict=True)
        assert path.parent.samefile(source_root / "assets")
        assert digest(path) == job["sha256"]
        originals.append(path)
        target = assets / path.name
        # Both product and oracle open these verified originals read-only.
        os.link(path, target)
        job["outputPath"] = str(target)
    write_json(destination / "jobs.json", {job["id"]: job for job in selected})
    with rasterio.open(originals[0]) as first, rasterio.open(originals[1]) as second:
        assert first.crs == second.crs and first.res == second.res and first.count == second.count == 3
        assert first.dtypes == second.dtypes == ("uint8", "uint8", "uint8")
        crs = first.crs
    west, south, east, north = args.projected_bounds
    assert west < east and south < north
    bounds = list(transform_bounds(crs, "EPSG:4326", west, south, east, north))
    # A nonrectangular area exercises polygon masking and spans the tile seam.
    xs, ys = transform(crs, "EPSG:4326", [west, east, west, west], [south, north, north, south])
    polygon = {"type": "Polygon", "coordinates": [list(map(list, zip(xs, ys)))]}
    scenes = [scene for scene in original_project["scenes"] if scene["itemId"] in {job["itemId"] for job in selected}]
    request = {"name": "Real RGB seam and polygon acceptance", "bounds": bounds,
               "geometry": polygon, "scenes": scenes}
    write_json(destination / "project-request.json", request)
    base = f"http://127.0.0.1:{args.port}"

    def api(path, payload=None):
        body = None if payload is None else json.dumps(payload).encode()
        headers = {} if body is None else {"Content-Type": "application/json", "X-GeoD-Client": "geod-global"}
        with urlopen(Request(base + path, data=body, headers=headers), timeout=60) as response:
            return json.load(response)

    with (destination / "service.out.log").open("wb") as out, (destination / "service.err.log").open("wb") as err:
        process = subprocess.Popen([str(args.runtime.resolve(strict=True)), "serve", "--data-dir", str(destination),
                                    "--port", str(args.port)], stdout=out, stderr=err)
        try:
            deadline = time.monotonic() + 20
            while True:
                assert process.poll() is None, "Acceptance runtime exited before becoming ready"
                try:
                    assert api("/health")["status"] == "ok"
                    break
                except URLError:
                    assert time.monotonic() < deadline, "Acceptance runtime did not start"
                    time.sleep(0.15)
            project = api("/projects", request)
            job = api(f"/projects/{project['id']}/mosaics", {"assetKey": "visual"})
            deadline = time.monotonic() + 180
            while job["status"] in {"queued", "running"}:
                assert time.monotonic() < deadline, "RGB processing did not settle in time"
                time.sleep(0.2)
                job = api(f"/jobs/{job['id']}")
            assert job["status"] == "succeeded", job.get("error")
            output = Path(job["outputPath"])
            assert output.resolve().parent.samefile(assets) and digest(output) == job["sha256"]
            with rasterio.open(output) as result:
                assert result.crs == crs and result.count == 3 and result.dtypes == ("uint8",) * 3
                assert result.res == (10.0, 10.0) and result.nodata == 0
                actual = result.read()
                expected = np.zeros_like(actual)
                owner = np.full((result.height, result.width), -1, dtype="int8")
                for index, source in enumerate(originals):
                    with rasterio.open(source) as raster:
                        col, row = (~raster.transform) * (result.bounds.left, result.bounds.top)
                        assert abs(col - round(col)) < 1e-7 and abs(row - round(row)) < 1e-7
                        pixels = raster.read(window=Window(round(col), round(row), result.width, result.height),
                                             boundless=True, fill_value=0)
                        valid = np.any(pixels != 0, axis=0)
                        expected[:, valid] = pixels[:, valid]
                        owner[valid] = index
                # GeoJSON edges are straight in WGS84, not straight chords in UTM.
                # Use GDAL inverse projection and GEOS point inclusion independently
                # rather than changing the saved polygon while constructing the oracle.
                rows, cols = np.indices(owner.shape)
                x = result.bounds.left + (cols + 0.5) * result.res[0]
                y = result.bounds.top - (rows + 0.5) * result.res[1]
                longitude, latitude = transform(crs, "EPSG:4326", x.ravel(), y.ravel())
                inside = contains_xy(shape(polygon), longitude, latitude).reshape(owner.shape)
                expected[:, ~inside] = 0
                owner[~inside] = -1
                contributors = [int(np.count_nonzero(owner == index)) for index in range(2)]
                assert all(count > 0 for count in contributors), "Saved area did not exercise both real tiles"
                assert np.any(~inside), "Polygon did not mask any output pixels"
                np.testing.assert_array_equal(actual, expected)
                metadata = json.loads(Path(job["manifestPath"]).read_text(encoding="utf-8"))
                assert {source["sha256"] for source in metadata["sources"]} == {job["sha256"] for job in selected}
                report = {"independentDecoder": "Rasterio / GDAL", "polygonOracle": "GEOS / Shapely in WGS84",
                          "realSourceReuse": True,
                          "sourceFiles": [{"jobId": job["id"], "itemId": job["itemId"], "sha256": job["sha256"],
                                           "href": job["href"], "contributingPixels": count}
                                          for job, count in zip(selected, contributors)],
                          "outputJobId": job["id"], "outputSha256": job["sha256"],
                          "width": result.width, "height": result.height, "bands": result.count,
                          "verifiedSamples": int(actual.size), "crs": str(result.crs),
                          "bounds": list(result.bounds), "pixelSize": list(result.res),
                          "polygonMaskedPixels": int(np.count_nonzero(~inside)),
                          "allSamplesExact": True, "sourcesUnchanged": True,
                          "runtimeBinarySha256": digest(args.runtime)}
            assert all(digest(path) == job["sha256"] for path, job in zip(originals, selected))
            write_json(destination / "independent-verification.json", report)
            print(json.dumps(report, ensure_ascii=False, indent=2))
        finally:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=10)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source_store", type=Path)
    parser.add_argument("project")
    parser.add_argument("sources", nargs=2)
    parser.add_argument("--projected-bounds", nargs=4, type=float, required=True, metavar=("WEST", "SOUTH", "EAST", "NORTH"))
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--port", type=int, default=4319)
    run(parser.parse_args())
