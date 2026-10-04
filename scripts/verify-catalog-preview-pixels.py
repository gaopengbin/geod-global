"""Compare display tiles with hash-pinned, previously accepted original COGs.

Usage: python scripts/verify-catalog-preview-pixels.py MANIFEST OUTPUT_DIR
The manifest contains the application's tile descriptors and original job
receipts. This reads existing originals and requests PNGs; it creates no jobs
or full original downloads. Requires the optional requirements-raster-qa.txt.
"""
import hashlib
import io
import json
import math
import sys
import urllib.request
from contextlib import ExitStack
from pathlib import Path

import numpy as np
import rasterio
from PIL import Image
from rasterio.warp import transform

manifest = json.loads(Path(sys.argv[1]).read_bytes())
output = Path(sys.argv[2])
output.mkdir(parents=True, exist_ok=True)
report = {"schema": "geod-catalog-preview-pixels/v1", "status": "pending", "cases": [], "originalDownloadsCreated": 0}
radius = 20037508.342789244
for case in manifest["cases"]:
    with ExitStack() as stack:
        sources = []
        hashes = []
        for job in case["jobs"]:
            original = Path(job["outputPath"])
            with original.open("rb") as handle:
                digest = hashlib.file_digest(handle, "sha256").hexdigest()
            assert digest == job["sha256"] and original.stat().st_size == job["totalBytes"]
            assert job["status"] == "succeeded" and job["itemId"] == case["itemId"]
            dataset = stack.enter_context(rasterio.open(original))
            expected_type, nodata = ("int16", -28672) if case["kind"] == "reflectance" else ("float32", -32768)
            assert dataset.count == 1 and dataset.dtypes == (expected_type,) and dataset.nodata == nodata
            sources.append(dataset)
            hashes.append(digest)
        assert len(sources) == (3 if case["kind"] == "reflectance" else 1)
        assert all(source.transform == sources[0].transform and source.crs == sources[0].crs for source in sources)
        if case.get("originalPixel"):
            column, row = case["originalPixel"]
            x, y = sources[0].xy(row, column)
            mx, my = transform(sources[0].crs, "EPSG:3857", [x], [y])
            centre = [mx[0], my[0]]
        else:
            mx, my = transform("EPSG:4326", "EPSG:3857", [case["centre"][0]], [case["centre"][1]])
            centre = [mx[0], my[0]]
        z = case["zoom"]
        span = radius * 2 / 2**z
        tx, ty = math.floor((centre[0] + radius) / span), math.floor((radius - centre[1]) / span)
        url = case["tileURL"].replace("{z}", str(z)).replace("{x}", str(tx)).replace("{y}", str(ty))
        with urllib.request.urlopen(url, timeout=45) as response:
            assert response.status == 200 and response.headers["content-type"].startswith("image/png")
            content = response.read()
        (output / (case["kind"] + ".png")).write_bytes(content)
        image = np.array(Image.open(io.BytesIO(content)).convert("RGBA"))
        assert image.shape == (256, 256, 4)
        x0, y0 = -radius + tx * span, radius - ty * span
        samples = []
        skipped = 0
        for row in range(8, 256, 24):
            for column in range(8, 256, 24):
                mx, my = x0 + (column + .5) * span / 256, y0 - (row + .5) * span / 256
                sx, sy = transform("EPSG:3857", sources[0].crs, [mx], [my])
                fc, fr = ~sources[0].transform * (sx[0], sy[0])
                # Avoid nearest-neighbour boundary ties and the server's GDAL
                # warp tolerance. Compare unambiguous interior source samples.
                if not (0.2 < fc % 1 < 0.8 and 0.2 < fr % 1 < 0.8):
                    skipped += 1
                    continue
                values = [float(next(source.sample([(sx[0], sy[0])]))[0]) for source in sources]
                actual = image[row, column]
                valid = all(value != nodata for value in values)
                if not valid:
                    assert actual[3] == 0, (case["kind"], row, column, values, actual.tolist())
                    expected = [0, 0, 0, 0]
                elif case["kind"] == "reflectance":
                    display = (np.clip(np.asarray(values) / 3000, 0, 1) * 255).astype(np.uint8)
                    rgb = np.floor((display.astype(np.float64) / 255) ** (1 / 2.2) * 255).astype(np.uint8)
                    expected = [*rgb.tolist(), 255]
                    assert max(abs(actual.astype(int) - expected)) <= 1, (row, column, values, actual.tolist(), expected)
                else:
                    db = 10 * math.log10(values[0]) if values[0] > 0 else -30
                    gray = int(np.clip((db + 30) / 30, 0, 1) * 255)
                    expected = [gray, gray, gray, 255]
                    assert max(abs(actual.astype(int) - expected)) <= 1, (row, column, values, actual.tolist(), expected)
                samples.append({"tilePixel": [column, row], "sourcePixel": [math.floor(fc), math.floor(fr)],
                                "originalValues": values, "rgba": actual.tolist(), "expected": expected, "noData": not valid})
        assert len(samples) >= 25 and sum(not sample["noData"] for sample in samples) >= 15
        report["cases"].append({"kind": case["kind"], "itemId": case["itemId"], "sourceSha256": hashes,
                                "url": url, "pngSha256": hashlib.sha256(content).hexdigest(), "bytes": len(content),
                                "samples": samples, "verifiedSampleCount": len(samples), "boundarySamplesSkipped": skipped,
                                "maxAllowedDisplayDifference": 1})
report["status"] = "passed"
(output / "verification.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
print(json.dumps({"status": report["status"], "cases": len(report["cases"]),
                  "verifiedSamples": sum(case["verifiedSampleCount"] for case in report["cases"])}))
