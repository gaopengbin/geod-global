"""Independent manual QA of a completed real crop job.

Requires Rasterio/GDAL and numpy for verification only, never for app execution.
Reconstructs the pixel window from the saved recipe, reads both files, and checks
every output sample plus georeferencing and hashes. Does not trust the job's plan
as the reference window.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
from uuid import UUID

import numpy as np
import rasterio
from rasterio.warp import transform_bounds
from rasterio.windows import Window

parser = argparse.ArgumentParser()
parser.add_argument("store", type=Path)
parser.add_argument("job_id", type=UUID)
parser.add_argument("--report", type=Path, required=True)
args = parser.parse_args()
jobs = json.loads((args.store / "jobs.json").read_text(encoding="utf-8"))
job = jobs[str(args.job_id)]
assert job["status"] == "succeeded", "Crop job has not succeeded"
assert job["kind"] == "raster_clip", "Expected an actual raster processing job"
recipe = job["recipe"]
assert recipe["schemaVersion"] == "geod-raster-recipe/v1"
source_job = jobs[recipe["source"]["jobId"]]
source_file, output_file = Path(source_job["outputPath"]), Path(job["outputPath"])
assert source_file != output_file
assert source_file.parent.samefile(args.store / "assets")
assert output_file.parent.samefile(args.store / "assets")
source_hash = hashlib.sha256(source_file.read_bytes()).hexdigest()
output_hash = hashlib.sha256(output_file.read_bytes()).hexdigest()
assert source_hash == recipe["source"]["sha256"] == source_job["sha256"], "Source changed"
assert output_hash == job["sha256"]
assert output_file.stat().st_size == job["bytesDownloaded"]
manifest_file = Path(job["manifestPath"])
assert manifest_file.parent.samefile(args.store / "assets")
assert manifest_file.name == f'{job["id"]}.metadata.json'
manifest = json.loads(manifest_file.read_text(encoding="utf-8"))
assert manifest["schemaVersion"] == "geod-raster-artifact/v1"
assert manifest["output"] == {
    "file": output_file.name, "format": "GeoTIFF",
    "bytes": output_file.stat().st_size, "sha256": output_hash,
}
assert manifest["recipe"] == recipe
assert manifest["source"]["sha256"] == source_hash
assert manifest["source"]["jobId"] == source_job["id"]
assert manifest["source"]["href"] == source_job["href"]
assert manifest["crop"] == job["crop"]
requested = recipe["operation"]["bounds"]
with rasterio.open(source_file) as source, rasterio.open(output_file) as result:
    if recipe["operation"]["crs"] == "EPSG:4326":
        projected = transform_bounds("EPSG:4326", source.crs, *requested, densify_pts=63)
    else:
        assert recipe["operation"]["crs"] == "source"
        projected = requested
    left, bottom, right, top = source.bounds
    dx, dy = source.res
    x0 = max(0, math.floor((projected[0] - left) / dx))
    x1 = min(source.width, math.ceil((projected[2] - left) / dx))
    y0 = max(0, math.floor((top - projected[3]) / dy))
    y1 = min(source.height, math.ceil((top - projected[1]) / dy))
    assert x1 > x0 and y1 > y0
    window = Window(x0, y0, x1 - x0, y1 - y0)
    expected = source.read(window=window)
    pixels = result.read()
    assert np.array_equal(pixels, expected), "Output samples differ from source window"
    assert result.crs == source.crs
    assert result.res == source.res
    assert result.count == source.count == 1
    assert result.dtypes == source.dtypes == ("uint8",)
    assert result.nodata == source.nodata
    assert np.allclose(list(result.transform), list(source.window_transform(window)), rtol=0, atol=1e-9)
    assert result.driver == "GTiff"
    if job.get("crop"):
        assert job["crop"]["window"] == [x0, y0, x1 - x0, y1 - y0]
    report = {
        "jobId": job["id"], "sourceJobId": source_job["id"], "itemId": job["itemId"],
        "recipe": recipe, "sourceSha256": source_hash, "outputSha256": output_hash,
        "outputBytes": output_file.stat().st_size,
        "independentWindow": [x0, y0, x1 - x0, y1 - y0],
        "width": result.width, "height": result.height, "bands": result.count,
        "crs": str(result.crs), "resolution": list(result.res), "nodata": result.nodata,
        "transform": list(result.transform), "bounds": list(result.bounds),
        "dtype": result.dtypes[0], "driver": result.driver,
        "compression": str(result.compression), "comparedSamples": int(pixels.size),
        "source": source_job["href"],
        "independentChecks": {
            "decoder": "Rasterio / GDAL",
            "projection": "GDAL transform_bounds" if recipe["operation"]["crs"] == "EPSG:4326" else "Not required; source CRS coordinates",
            "everyOutputSample": "passed", "georeferencing": "passed",
            "sourceUnchanged": "passed", "outputHashAndBytes": "passed",
            "portableProvenanceSidecar": "passed",
        },
        "scope": "Pixel-aligned projected rectangle; no reprojection/resampling or geographic polygon mask",
    }
args.report.parent.mkdir(parents=True, exist_ok=True)
args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report, indent=2))
