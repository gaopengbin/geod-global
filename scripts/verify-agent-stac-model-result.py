"""Independently decode the actual original from the live custom-source Agent test.

The full GDAL read proves decode/metadata coverage; only the explicitly recorded
native pixel is compared. It is not an all-pixel native-equivalence claim.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path

import numpy as np
import rasterio


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", required=True, type=Path)
    args = parser.parse_args()
    base = args.directory.resolve(strict=True)
    root = Path(__file__).resolve().parent.parent
    assert base.is_relative_to(root / ".verification")
    launch = json.loads((base / "launch.json").read_text(encoding="utf-8"))
    acceptance = json.loads((base / "case/native-acceptance.json").read_text(encoding="utf-8"))
    assert launch["status"] == acceptance["status"] == "passed"
    assert acceptance["persistedJobCount"] == 1
    jobs = json.loads((base / "case/core/jobs.json").read_text(encoding="utf-8"))
    assert len(jobs) == 1
    job = jobs[acceptance["jobId"]]
    assert job["status"] == "succeeded" and job["stacSource"]
    path = Path(job["outputPath"]).resolve(strict=True)
    # Native canonical Windows paths may include the extended-length prefix.
    assert path.parent.samefile(base / "case/core/assets")
    assert path.name == job["id"] + ".tif"
    assert path.stat().st_size == acceptance["bytes"] == job["bytesDownloaded"]
    sha = hashlib.sha256(path.read_bytes()).hexdigest()
    assert sha == job["sha256"] == acceptance["sha256"]
    inspection, pixel = acceptance["inspection"], acceptance["pixel"]
    with rasterio.open(path) as dataset:
        raw = dataset.read()
        assert (dataset.width, dataset.height) == (inspection["width"], inspection["height"])
        assert dataset.count == len(inspection["bands"])
        assert dataset.crs.to_string() == inspection["crs"]
        # GeoD uses [a,b,c,d,e,f] affine order, not GDAL's origin-first tuple.
        assert np.allclose(tuple(dataset.transform)[:6], inspection["transform"], atol=1e-10, rtol=0)
        assert np.allclose(tuple(dataset.bounds), inspection["bounds"], atol=1e-8, rtol=0)
        values = raw[:, pixel["row"], pixel["column"]].tolist()
        assert values == pixel["values"]
        assert [dataset.nodatavals[index] is not None and value == dataset.nodatavals[index]
                for index, value in enumerate(values)] == pixel["noData"]
        result = {"schema": "geod-agent-custom-model-independent-read/v1", "status": "passed",
                  "checkedAt": datetime.now(timezone.utc).isoformat(),
                  "reader": {"rasterio": rasterio.__version__, "gdal": rasterio.__gdal_version__},
                  "jobId": job["id"], "sha256": sha, "bytes": path.stat().st_size,
                  "width": dataset.width, "height": dataset.height, "bands": dataset.count,
                  "sampleTypes": dataset.dtypes, "crs": dataset.crs.to_string(),
                  "independentDecodedSamples": int(raw.size), "nativePixelPointsCompared": 1,
                  "pixel": {"column": pixel["column"], "row": pixel["row"], "values": values},
                  "modelCalledByThisReader": False, "newSourceRequests": 0, "usedUserDesktop": False}
    target = base / "independent-read.json"
    assert not target.exists(), "Never overwrite an independent evidence record"
    target.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(result, ensure_ascii=False))


if __name__ == "__main__":
    main()
