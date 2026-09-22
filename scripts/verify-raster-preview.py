"""Compare the native raster API with an independent full raster decode.

Manual QA only: requires numpy, rasterio and Pillow. None are app dependencies.
Run with the local runtime service active. The API receives a job ID, not a path.
"""
import argparse
import base64
import hashlib
import io
import json
from pathlib import Path
from urllib.request import urlopen
from uuid import UUID

import numpy as np
import rasterio
from PIL import Image

parser = argparse.ArgumentParser()
parser.add_argument("store", type=Path)
parser.add_argument("job_id", type=UUID)
parser.add_argument("--report", type=Path, required=True)
parser.add_argument("--preview", type=Path)
args = parser.parse_args()
job = json.loads((args.store / "jobs.json").read_text(encoding="utf-8"))[str(args.job_id)]
assert job["status"] == "succeeded"
source = Path(job["outputPath"])
assert source.parent.samefile(args.store / "assets")
sha256 = hashlib.sha256(source.read_bytes()).hexdigest()
assert sha256 == job["sha256"]

with urlopen(f"http://127.0.0.1:4318/jobs/{args.job_id}/raster", timeout=60) as response:
    info = json.load(response)
assert info["sha256"] == sha256
assert info["previewDataUrl"].startswith("data:image/png;base64,")
png = base64.b64decode(info["previewDataUrl"].split(",", 1)[1], validate=True)
preview = Image.open(io.BytesIO(png)).convert("RGB")
assert preview.size == (info["previewWidth"], info["previewHeight"])
assert max(preview.size) <= 768

with rasterio.open(source) as dataset:
    pixels = dataset.read(1)
    assert [info["width"], info["height"], info["bandCount"]] == [dataset.width, dataset.height, dataset.count]
    assert info["dataType"].lower() == dataset.dtypes[0]
    assert info["crs"] == str(dataset.crs)
    assert np.allclose(info["bounds"], list(dataset.bounds))
    assert np.allclose(info["pixelSize"], list(dataset.res))
    assert info["nodata"] == dataset.nodata
    values, counts = np.unique(pixels, return_counts=True)
    expected_counts = dict(zip(map(int, values), map(int, counts)))
    returned_counts = {item["value"]: item["count"] for item in info["classes"] if item["count"]}
    assert returned_counts == expected_counts, "Classification counts differ from full source decode"
    palette = np.zeros((256, 3), dtype=np.uint8)
    for item in info["classes"]:
        color = item["color"].lstrip("#")
        palette[item["value"]] = [int(color[i:i+2], 16) for i in (0, 2, 4)]
    rows = np.arange(info["previewHeight"], dtype=np.int64) * dataset.height // info["previewHeight"]
    columns = np.arange(info["previewWidth"], dtype=np.int64) * dataset.width // info["previewWidth"]
    expected_preview = palette[pixels[np.ix_(rows, columns)]]
    assert np.array_equal(np.array(preview), expected_preview), "Preview does not match nearest source pixels"

report = {
    "jobId": str(args.job_id), "itemId": job["itemId"], "source": job["href"],
    **{key: value for key, value in info.items() if key != "previewDataUrl"},
    "independentChecks": {
        "decoder": "Rasterio / GDAL", "checksum": "passed", "metadata": "passed",
        "fullResolutionClassCounts": "passed", "nearestPreviewEveryPixel": "passed",
        "decodedPixels": int(pixels.size), "previewPixels": preview.width * preview.height,
    },
    "scope": "A real downloaded Sentinel-2 SCL sample; does not establish classification accuracy or general raster support",
}
args.report.parent.mkdir(parents=True, exist_ok=True)
args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
if args.preview:
    args.preview.parent.mkdir(parents=True, exist_ok=True)
    args.preview.write_bytes(png)
print(json.dumps(report, indent=2))
