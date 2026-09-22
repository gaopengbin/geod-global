"""Manual acceptance helper, separate from the app's signature-only validator.

Requires rasterio (QA dependency, not a bundled application dependency).
Reads a completed local Job and checks the file against its transfer record.
"""
import argparse
import hashlib
import json
from pathlib import Path

import rasterio

parser = argparse.ArgumentParser()
parser.add_argument("store", type=Path, help="Runtime data directory")
parser.add_argument("job_id")
parser.add_argument("--report", type=Path)
args = parser.parse_args()
job = json.loads((args.store / "jobs.json").read_text(encoding="utf-8"))[args.job_id]
assert job["status"] == "succeeded", "Job is not complete"
file = Path(job["outputPath"])
assert file.parent.samefile(args.store / "assets"), "File is outside the store"
digest = hashlib.sha256(file.read_bytes()).hexdigest()
assert digest == job["sha256"], "SHA-256 differs from transfer record"
assert file.stat().st_size == job["bytesDownloaded"], "Byte count differs from transfer record"
with rasterio.open(file) as dataset:
    # Decode all blocks, not only the header. This acceptance check does not
    # prove classification accuracy or validate all future downloaded datasets.
    minimum, maximum, pixels = None, None, 0
    for _, window in dataset.block_windows(1):
        values = dataset.read(1, window=window)
        lo, hi = int(values.min()), int(values.max())
        minimum = lo if minimum is None else min(minimum, lo)
        maximum = hi if maximum is None else max(maximum, hi)
        pixels += values.size
    report = {
        "itemId": job["itemId"], "assetKey": job["assetKey"],
        "bytes": file.stat().st_size, "sha256": digest,
        "driver": dataset.driver, "width": dataset.width, "height": dataset.height,
        "bands": dataset.count, "crs": str(dataset.crs), "resolution": list(dataset.res),
        "transform": list(dataset.transform), "dtype": dataset.dtypes[0],
        "nodata": dataset.nodata, "decodedBand1Pixels": pixels,
        "valueRange": [minimum, maximum], "source": job["href"],
        "scope": "Independent QA read of this downloaded artifact; not a built-in scientific validator",
    }
assert pixels == report["width"] * report["height"]
if args.report:
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report, indent=2))
