"""Compare live preview pixels with already downloaded, hash-pinned originals.

Usage: python scripts/verify-vegetation-preview-pixels.py SOURCE_RECEIPT OUTPUT_DIR
Requires Rasterio, NumPy, Pillow and Matplotlib; no original download is created.
"""
import hashlib
import io
import json
import sys
import urllib.parse
import urllib.request
from pathlib import Path

import matplotlib
import numpy as np
import rasterio
from PIL import Image
from rasterio.warp import transform

receipt = json.loads(Path(sys.argv[1]).read_bytes())
output = Path(sys.argv[2])
output.mkdir(parents=True, exist_ok=True)
item = "MOD13Q1.A2025177.h08v05.061.2025195142416"
palette = matplotlib.colormaps["RdYlGn"](np.arange(256), bytes=True)
report = {"schema": "geod-vegetation-online-preview-pixels/v1", "status": "pending", "cases": [], "originalDownloadsCreated": 0}
for index in ("ndvi", "evi"):
    job = next(case["job"] for case in receipt["cases"] if case["job"]["itemId"] == item and case["job"]["assetKey"] == index)
    original = Path(job["outputPath"])
    assert hashlib.sha256(original.read_bytes()).hexdigest() == job["sha256"]
    query = {"collection": "modis-13Q1-061", "item": item, "assets": f"250m_16_days_{index.upper()}",
             "rescale": "-2000,10000", "colormap_name": "rdylgn", "nodata": "-3000", "unscale": "false",
             "resampling": "nearest", "reproject": "nearest", "return_mask": "true"}
    url = "https://planetarycomputer.microsoft.com/api/data/v1/item/tiles/WebMercatorQuad/9/81/197.png?" + urllib.parse.urlencode(query)
    with urllib.request.urlopen(url, timeout=45) as response:
        assert response.status == 200 and response.headers["content-type"].startswith("image/png")
        content = response.read()
    (output / (index + ".png")).write_bytes(content)
    image = np.array(Image.open(io.BytesIO(content)).convert("RGBA"))
    radius = 20037508.342789244
    span = radius * 2 / 512
    x0, y0 = -radius + 81 * span, radius - 197 * span
    samples = []
    with rasterio.open(original) as dataset:
        assert dataset.dtypes == ("int16",) and dataset.nodata == -3000
        for row in range(8, 256, 24):
            for column in range(8, 256, 24):
                mx, my = x0 + (column + .5) * span / 256, y0 - (row + .5) * span / 256
                sx, sy = transform("EPSG:3857", dataset.crs, [mx], [my])
                dn = int(next(dataset.sample([(sx[0], sy[0])]))[0])
                actual = image[row, column]
                if dn == -3000:
                    assert actual[3] == 0, (index, row, column, dn, actual.tolist())
                else:
                    display = int(np.uint8(np.clip((dn + 2000) / 12000, 0, 1) * 255))
                    expected = palette[display]
                    assert max(abs(actual.astype(int) - expected.astype(int))) <= 1, (index, row, column, dn, actual.tolist(), expected.tolist())
                samples.append({"tilePixel": [column, row], "mercator": [mx, my], "originalDN": dn, "indexValue": None if dn == -3000 else dn * .0001, "rgba": actual.tolist()})
    report["cases"].append({"index": index, "itemId": item, "sourceSha256": job["sha256"], "url": url,
                            "pngSha256": hashlib.sha256(content).hexdigest(), "bytes": len(content),
                            "samples": samples, "verifiedSampleCount": len(samples)})
report["status"] = "passed"
(output / "verification.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
print(json.dumps({"status": report["status"], "indexes": len(report["cases"]), "verifiedSamples": sum(case["verifiedSampleCount"] for case in report["cases"])}))
