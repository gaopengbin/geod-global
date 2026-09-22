"""Independent local QA: exact source pixels, delivery ZIP and support report.

Rasterio/GDAL is used only by this verification script, never by the product.
Only accepts loopback HTTP and managed successful jobs in the supplied store.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
from urllib.error import HTTPError
from urllib.parse import urlencode, urlparse
from urllib.request import Request, urlopen
from uuid import UUID
import zipfile

import rasterio

parser = argparse.ArgumentParser()
parser.add_argument("store", type=Path)
parser.add_argument("source", type=UUID)
parser.add_argument("derived", type=UUID)
parser.add_argument("--server", default="http://127.0.0.1:4318")
parser.add_argument("--report", type=Path, required=True)
args = parser.parse_args()
server = urlparse(args.server)
assert server.scheme == "http" and server.hostname in {"127.0.0.1", "localhost", "::1"}
assert not server.username and not server.password and not server.query and not server.fragment
jobs = json.loads((args.store / "jobs.json").read_text(encoding="utf-8"))


def request(path, method="GET", origin=None):
    headers = {"X-GeoD-Client": "geod-global", "Content-Type": "application/json"}
    if origin:
        headers["Origin"] = origin
    with urlopen(Request(args.server.rstrip("/") + path, data=b"{}" if method == "POST" else None,
                         headers=headers, method=method), timeout=60) as response:
        return response.read(), dict(response.headers)


samples = []
for job_id in [str(args.source), str(args.derived)]:
    job = jobs[job_id]
    path = Path(job["outputPath"])
    assert job["status"] == "succeeded" and path.parent.samefile(args.store / "assets")
    checksum = hashlib.sha256(path.read_bytes()).hexdigest()
    with rasterio.open(path) as dataset:
        pixels = dataset.read(1)
        locations = {(0, 0), (dataset.height - 1, dataset.width - 1),
                     (dataset.height // 2, dataset.width // 2),
                     (dataset.height // 3, dataset.width // 3),
                     (0, dataset.width - 1), (dataset.height - 1, 0)}
        for row, col in sorted(locations):
            center = dataset.xy(row, col)
            # Noncentral coordinates must still read the same source sample.
            x, y = center[0] + 0.17 * dataset.res[0], center[1] - 0.13 * dataset.res[1]
            response = json.loads(request(f"/jobs/{job_id}/pixel?" + urlencode({"x": x, "y": y}))[0])
            assert response["jobId"] == job_id and response["sha256"] == checksum
            assert response["pixel"] == [col, row] and response["coordinate"] == [x, y]
            assert response["center"] == list(center) and response["crs"] == str(dataset.crs)
            assert response["value"] == int(pixels[row, col])
            assert response["isNoData"] == (dataset.nodata is not None and pixels[row, col] == dataset.nodata)
            samples.append({"jobId": job_id, "pixel": [col, row], "center": list(center), "value": response["value"]})
        for x, y in [(dataset.bounds.right, dataset.bounds.top), (dataset.bounds.left, dataset.bounds.bottom)]:
            try:
                request(f"/jobs/{job_id}/pixel?" + urlencode({"x": x, "y": y}))
                raise AssertionError("Exclusive grid edge was accepted")
            except HTTPError as error:
                assert error.code == 400

job_id = str(args.derived)
package = json.loads(request(f"/jobs/{job_id}/package", "POST")[0])
binary, headers = request(f"/jobs/{job_id}/package")
assert hashlib.sha256(binary).hexdigest() == package["sha256"]
assert len(binary) == package["bytes"]
assert Path(package["path"]).parent.samefile(args.store / "exports")
assert binary == Path(package["path"]).read_bytes()
assert headers["content-type"] == "application/zip"
assert "attachment;" in headers["content-disposition"]
assert json.loads(request(f"/jobs/{job_id}/package", "POST")[0]) == package
with zipfile.ZipFile(io.BytesIO(binary)) as archive:
    assert archive.testzip() is None
    assert archive.namelist() == package["files"]
    assert archive.read(f"{job_id}.tif") == Path(jobs[job_id]["outputPath"]).read_bytes()
    assert archive.read(f"{job_id}.metadata.json") == Path(jobs[job_id]["manifestPath"]).read_bytes()
    assert json.loads(archive.read("recipe.json")) == jobs[job_id]["recipe"]
    for line in archive.read("checksums.sha256").decode("utf-8").splitlines():
        checksum, name = line.split("  ", 1)
        assert hashlib.sha256(archive.read(name)).hexdigest() == checksum

diagnostics = json.loads(request("/diagnostics")[0])
assert set(diagnostics) == {"schemaVersion", "runtime", "version", "platform", "architecture", "capabilities", "limits", "jobCounts", "savedRecipeCount", "privacy"}
assert all(value is False for value in diagnostics["privacy"].values())
assert diagnostics["jobCounts"]["succeeded"] >= 2
report = {"independentDecoder": "Rasterio / GDAL", "verifiedSourceSamples": samples,
          "exclusiveGridEdgesRejected": 4, "package": package,
          "deliveryChecks": ["ZIP integrity", "exact TIFF and metadata bytes", "all entry hashes", "repeatable package", "HTTP attachment"],
          "diagnostics": diagnostics}
args.report.parent.mkdir(parents=True, exist_ok=True)
args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(json.dumps({"pixelSamples": len(samples), "packageBytes": package["bytes"],
                  "packageSha256": package["sha256"], "report": str(args.report)}, indent=2))
