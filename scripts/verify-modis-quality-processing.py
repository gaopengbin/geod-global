"""Independent GDAL/NumPy verification of actual five-layer project outputs."""
import argparse
import hashlib
import json
from pathlib import Path
from datetime import datetime, timezone
import numpy as np
import rasterio
from PIL import Image
from pyproj import Transformer
import shapely
from shapely.geometry import shape

parser = argparse.ArgumentParser()
parser.add_argument("directory", type=Path)
args = parser.parse_args()
root = args.directory.resolve()
assert root.parent == Path(".verification").resolve()
assert root.name.startswith("modis-quality-processing-")
native = json.loads((root / "native-processing-verification.json").read_text(encoding="utf-8"))
assert native["status"] == "passed"
originals = {job["id"]: job for job in native["originals"]}
projects = {entry["name"]: entry["project"] for entry in native["cases"]}
guide = "https://landweb.modaps.eosdis.nasa.gov/data/userguide/MOD09_User_Guide_V61.pdf"
palette = {
    "modis_qc": np.array([[37, 99, 235], [234, 179, 8], [239, 68, 68], [107, 114, 128]], dtype=np.uint8),
    "modis_state": np.array([[37, 99, 235], [226, 232, 240], [234, 179, 8], [107, 114, 128]], dtype=np.uint8),
}
# An explicit inverse of the source sphere, with no implicit ellipsoid/datum operation.
inverse = Transformer.from_pipeline("+proj=pipeline +step +inv +proj=sinu +R=6371007.181 +lon_0=0 +x_0=0 +y_0=0 +step +proj=unitconvert +xy_in=rad +xy_out=deg")
report = {"schema": "geod-modis-quality-processing-independent/v1", "checkedAt": datetime.now(timezone.utc).isoformat(),
          "decoder": f"Rasterio {rasterio.__version__} / GDAL {rasterio.__gdal_version__}", "cases": [], "originals": [], "status": "pending"}

def managed_file(job):
    value = job["outputPath"]
    if value.startswith("\\\\?\\"):
        value = value[4:]
    path = Path(value).resolve()
    assert path.parent == root / "assets", (path, root)
    assert path.name == job["id"] + ".tif"
    data = path.read_bytes()
    assert len(data) == job["bytesDownloaded"] == job["totalBytes"]
    assert hashlib.sha256(data).hexdigest() == job["sha256"]
    return path

def check_preview(key, data, fill, file):
    rendered = np.array(Image.open(root / file).convert("RGBA"))
    ph, pw = rendered.shape[:2]
    height, width = data.shape
    sx = np.arange(pw, dtype=np.uint64) * width // pw
    sy = np.arange(ph, dtype=np.uint64) * height // ph
    sampled = data[np.ix_(sy, sx)]
    expected = np.zeros(rendered.shape, dtype=np.uint8)
    valid = sampled != fill
    expected[valid, :3] = palette[key][(sampled[valid] & 3).astype(np.int64)]
    expected[valid, 3] = 255
    assert np.array_equal(rendered, expected), file
    return ph * pw

for job in originals.values():
    path = managed_file(job)
    with rasterio.open(path) as src:
        assert (src.width, src.height, src.count) == (2400, 2400, 1)
        assert src.tags()["AREA_OR_POINT"] == "Area"
        qa = job["assetKey"] in palette
        expected_type = "uint32" if job["assetKey"] == "modis_qc" else "uint16" if qa else "int16"
        assert src.dtypes == (expected_type,)
        fill = 4294967295 if job["assetKey"] == "modis_qc" else 65535 if qa else -28672
        assert src.nodata == fill
        assert abs(src.transform.a - 463.312716527778) < 1e-6
        assert abs(src.transform.e + 463.312716527778) < 1e-6
        receipt = {"itemId": job["itemId"], "key": job["assetKey"], "bytes": job["bytesDownloaded"],
                   "sha256": job["sha256"], "dtype": expected_type, "gridChecked": True}
        if qa:
            data = src.read(1)
            checked = next(q for q in native["qualityOriginals"] if q["jobId"] == job["id"])
            valid = data != fill
            counts = [int(np.count_nonzero(valid & ((data.astype(np.uint64) & 3) == i))) for i in range(4)]
            assert counts == [c["count"] for c in checked["metadata"]["classes"]]
            assert checked["metadata"]["quality"]["validSampleCount"] == int(valid.sum())
            receipt.update(originalPixelsCounted=data.size, counts=counts, noDataPixels=int((~valid).sum()),
                           minimum=int(data[valid].min()), maximum=int(data[valid].max()),
                           pngPixelsCompared=sum(check_preview(job["assetKey"], data, fill, checked[k]) for k in ["previewFile", "thumbnailFile"]))
        report["originals"].append(receipt)

for entry in native["outputs"]:
    job, meta = entry["job"], entry["metadata"]
    project = projects[entry["case"]]
    key = job["assetKey"]
    path = managed_file(job)
    plan = job["mosaicOutput"]
    with rasterio.open(path) as dst:
        actual = dst.read(1)
        assert dst.count == 1 and dst.width == meta["width"] and dst.height == meta["height"]
        assert tuple(dst.bounds) == tuple(meta["bounds"])
        assert dst.nodata == meta["nodata"] and dst.tags()["AREA_OR_POINT"] == "Area"
        expected = np.full(actual.shape, dst.nodata, dtype=actual.dtype)
        covered = np.zeros(actual.shape, dtype=bool)
        winner = np.full(actual.shape, -1, dtype=np.int16)
        for source_index, pin in enumerate(job["mosaic"]["sources"]):
            original = originals[pin["jobId"]]
            assert pin["sha256"] == original["sha256"] and original["assetKey"] == key
            with rasterio.open(managed_file(original)) as src:
                # Both files name their custom spherical datum differently. Compare every
                # actual projection parameter, including radius, units and false origin.
                assert src.crs.to_dict() == dst.crs.to_dict() == {"proj": "sinu", "lon_0": 0, "x_0": 0, "y_0": 0, "R": 6371007.181, "units": "m", "no_defs": True}
                assert src.dtypes == dst.dtypes and src.nodata == dst.nodata
                assert abs(src.transform.a - dst.transform.a) < 1e-8 and abs(src.transform.e - dst.transform.e) < 1e-8
                x_offset = (src.transform.c - dst.transform.c) / dst.transform.a
                y_offset = (src.transform.f - dst.transform.f) / dst.transform.e
                assert abs(x_offset - round(x_offset)) < 1e-5 and abs(y_offset - round(y_offset)) < 1e-5
                x_offset, y_offset = round(x_offset), round(y_offset)
                x0, x1 = max(0, x_offset), min(dst.width, x_offset + src.width)
                y0, y1 = max(0, y_offset), min(dst.height, y_offset + src.height)
                if x0 >= x1 or y0 >= y1:
                    continue
                values = src.read(1, window=rasterio.windows.Window(x0 - x_offset, y0 - y_offset, x1 - x0, y1 - y0))
                valid = values != src.nodata
                expected[y0:y1, x0:x1][valid] = values[valid]
                covered[y0:y1, x0:x1][valid] = True
                winner[y0:y1, x0:x1][valid] = source_index
        rows, columns = np.indices(actual.shape)
        xs = dst.transform.c + (columns + 0.5) * dst.transform.a
        ys = dst.transform.f + (rows + 0.5) * dst.transform.e
        lon, lat = inverse.transform(xs, ys)
        masked = np.zeros(actual.shape, dtype=bool)
        if project.get("geometry"):
            inside = shapely.contains_xy(shape(project["geometry"]), lon, lat)
            masked = ~inside
            expected[masked] = dst.nodata
            covered[masked] = False
            winner[masked] = -1
        assert np.array_equal(actual, expected), (entry["case"], key, int(np.count_nonzero(actual != expected)))
        assert int(covered.sum()) == plan["coveredPixels"]
        assert int(masked.sum()) == plan["maskedPixels"]
        assert (plan["sourceCount"] == len(job["mosaic"]["sources"]))
        qa = key in palette
        if qa:
            p = plan["quality"]
            assert p["bits"] == (32 if key == "modis_qc" else 16)
            assert p["nodata"] == dst.nodata and p["definition"] == guide and p["band"] == key
            assert dst.scales == (1.0,) and dst.offsets == (0.0,)
            assert dst.tags(1)["QUALITY_LAYER"] == p["layer"] and int(dst.tags(1)["FLAG_BITS"]) == p["bits"]
            assert dst.tags()["PRODUCT"] == p["product"] and dst.tags()["DEFINITION"] == p["definition"]
            assert "calibration" not in plan and "reflectance" not in meta
            classes = [int(np.count_nonzero(covered & ((actual.astype(np.uint64) & 3) == value))) for value in range(4)]
            assert classes == [item["count"] for item in meta["classes"]]
            assert meta["quality"]["sampleCount"] == actual.size
            assert meta["quality"]["validSampleCount"] == int(covered.sum())
            png_pixels = sum(check_preview(key, actual, dst.nodata, file) for file in [entry["previewFile"], entry["thumbnailFile"]])
        else:
            assert dst.scales == (0.0001,) and dst.offsets == (0.0,)
            assert plan["calibration"]["product"] == "modis-09a1-v061"
            classes, png_pixels = None, 0
        for pixel in entry["pixels"]:
            col, row = pixel["pixel"]
            raw = int(actual[row, col])
            assert pixel["value"] == raw and pixel["isNoData"] == (raw == dst.nodata)
            if qa:
                fields = pixel["quality"]["fields"]
                assert not fields if raw == dst.nodata else len(fields) == (10 if key == "modis_qc" else 11)
                for field in fields:
                    assert field["value"] == (raw >> field["startBit"]) & ((1 << (field["endBit"] - field["startBit"] + 1)) - 1)
            elif raw != dst.nodata:
                assert abs(pixel["reflectance"] - raw * 0.0001) < 1e-12
        receipt = {"case": entry["case"], "key": key, "jobId": job["id"], "dtype": dst.dtypes[0], "sha256": job["sha256"],
                   "outputPixelsCompared": actual.size, "rawSamplesCompared": len(entry["pixels"]), "pngPixelsCompared": png_pixels,
                   "maskedPixels": int(masked.sum()), "coveredPixels": int(covered.sum()), "sourcePixelsByPin": [int((winner == i).sum()) for i in range(plan["sourceCount"])],
                   "sourcePeriodOrder": [originals[pin["jobId"]]["itemId"] for pin in job["mosaic"]["sources"]], "counts": classes}
        report["cases"].append(receipt)
        print(json.dumps(receipt), flush=True)

assert len(report["cases"]) == 15
assert all(case["maskedPixels"] > 0 for case in report["cases"] if case["case"] == "polygon")
assert all(sum(n > 0 for n in case["sourcePixelsByPin"]) >= 2 for case in report["cases"] if case["case"] == "mosaic")
report.update(status="passed", originalBytes=sum(job["bytesDownloaded"] for job in originals.values()),
              originalPixelsCounted=sum(c.get("originalPixelsCounted", 0) for c in report["originals"]),
              outputPixelsCompared=sum(c["outputPixelsCompared"] for c in report["cases"]),
              rawSamplesCompared=sum(c["rawSamplesCompared"] for c in report["cases"]),
              pngPixelsCompared=sum(c["pngPixelsCompared"] for c in report["cases"]) + sum(c.get("pngPixelsCompared",0) for c in report["originals"]))
(root / "independent-processing-verification.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(json.dumps({key: report[key] for key in ["status", "originalBytes", "outputPixelsCompared", "rawSamplesCompared", "pngPixelsCompared"]}), flush=True)
