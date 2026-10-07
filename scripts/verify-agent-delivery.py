"""Verify the actual delivery ZIP created by the headless Agent UI acceptance.

Reads the ZIP independently, compares its raw TIFF and provenance to the managed
result, and connects that result to the separate full-pixel science acceptance.
It does not call a model, create a package or operate the user's desktop.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import io
import json
from pathlib import Path
import re
import zipfile

from PIL import Image
import rasterio


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", required=True, type=Path)
    parser.add_argument("--science", required=True, type=Path)
    args = parser.parse_args()
    root = args.run.resolve(strict=True)
    ui = json.loads((root / "verification.json").read_text(encoding="utf-8"))
    science = json.loads((args.science / "independent-acceptance.json").read_text(encoding="utf-8"))
    assert ui["status"] == science["status"] == "passed"
    assert science["originalFilesUnchanged"] and science["rgb"]["exactSamples"] == science["rgb"]["width"] * science["rgb"]["height"] * 3
    packages = ui["nativeDeliveryPackages"]
    assert len(packages) == 4 and {value["locale"] for value in packages} == {"en", "zh-CN"}
    rejections = ui["nativeDeliveryRejections"]
    assert len(rejections) == 2 and {value["locale"] for value in rejections} == {"en", "zh-CN"}
    assert all(value["error"] == "The existing RGB delivery package changed; it will not be overwritten" for value in rejections)
    assert len({(value["jobId"], value["sha256"], value["bytes"]) for value in packages}) == 1
    package = packages[0]
    job = json.loads((root / "core/jobs.json").read_text(encoding="utf-8"))[package["jobId"]]
    assert job["kind"] == "raster_rgb" and job["status"] == "succeeded"
    report = {"schema": "geod-agent-delivery-acceptance/v1", "status": "pending",
              "checkedAt": datetime.now(timezone.utc).isoformat(), "jobId": job["id"],
              "modelCalls": 0, "usedUserDesktop": False, "newProviderRequests": 0,
              "reader": f"Python zipfile / Rasterio {rasterio.__version__} / GDAL {rasterio.__gdal_version__}"}
    try:
        path = Path(package["path"]).resolve(strict=True)
        assert path.parent.samefile(root / "core/exports") and path.name == package["filename"]
        data = path.read_bytes()
        assert len(data) == package["bytes"] and digest(data) == package["sha256"]
        names = {f"{job['id']}.tif", f"{job['id']}.metadata.json", "preview.png", "README.txt", "checksums.sha256"}
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            assert len(archive.infolist()) == len(names) and set(archive.namelist()) == names == set(package["files"])
            assert archive.testzip() is None
            checksums = {}
            for line in archive.read("checksums.sha256").decode("utf-8").splitlines():
                checksum, name = line.split("  ", 1)
                assert re.fullmatch(r"[0-9a-f]{64}", checksum) and name in names and name not in checksums
                checksums[name] = checksum
            assert set(checksums) == names - {"checksums.sha256"}
            for name, checksum in checksums.items():
                assert digest(archive.read(name)) == checksum
            tiff = archive.read(f"{job['id']}.tif")
            assert tiff == Path(job["outputPath"]).read_bytes() and digest(tiff) == job["sha256"]
            assert science["rgb"]["jobId"] == job["id"] and science["rgb"]["sha256"] == job["sha256"]
            metadata = archive.read(f"{job['id']}.metadata.json")
            assert metadata == Path(job["manifestPath"]).read_bytes()
            value = json.loads(metadata)
            def no_local_paths(value):
                if isinstance(value, str):
                    assert not re.match(r"^(?:[A-Za-z]:[\\/]|\\\\|file:)", value)
                elif isinstance(value, dict):
                    for child in value.values():
                        no_local_paths(child)
                elif isinstance(value, list):
                    for child in value:
                        no_local_paths(child)
            no_local_paths(value)
            with Image.open(io.BytesIO(archive.read("preview.png"))) as image:
                assert image.format == "PNG" and max(image.size) <= 768
                image.verify()
            with rasterio.MemoryFile(tiff) as file, file.open() as dataset:
                grid = job["rgbSpec"]["grid"]
                assert (dataset.width, dataset.height, dataset.count) == (grid["width"], grid["height"], 3)
                assert all(dtype == ("int16" if job["rgbSpec"]["profile"]["signed"] else "uint16") for dtype in dataset.dtypes)
                assert dataset.crs.to_string() == grid["crs"]
            readme = archive.read("README.txt").decode("utf-8")
            assert "display-only" in readme and "No reprojection or resampling" in readme
        report.update(status="passed", bytes=len(data), sha256=digest(data), files=sorted(names),
                      repeatedUiActions=4, rawTiffUnchanged=True, provenanceUnchanged=True,
                      linkedIndependentRgbPixelAcceptance=True, previewDisplayOnly=True,
                      noAbsolutePathsInProvenance=True, checksumsVerified=len(checksums),
                      changedZipRefusedInBothLocales=True)
    except Exception as error:
        report.update(status="failed", error=f"{type(error).__name__}: {error}")
        raise
    finally:
        (root / "independent-delivery.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(report))


if __name__ == "__main__":
    main()
