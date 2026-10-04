"""Re-run a saved mosaic in an isolated store and compare every pixel with GDAL.

Reuses verified local originals via hard links on the same drive. Never edits
the desktop store or downloads source data. Rasterio/NumPy are QA dependencies.
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
from rasterio.windows import Window


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write_json(path, data):
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def run(args):
    root = args.source_store.resolve(strict=True)
    jobs = json.loads((root / "jobs.json").read_text(encoding="utf-8"))
    projects = json.loads((root / "projects.json").read_text(encoding="utf-8"))
    original_job = copy.deepcopy(jobs[args.job])
    assert original_job["kind"] == "raster_mosaic" and original_job["status"] == "failed"
    project = copy.deepcopy(projects[original_job["mosaic"]["projectId"]])
    assert project.get("geometry") is None, "This oracle validates the saved rectangular project"
    selected = [copy.deepcopy(jobs[source["jobId"]]) for source in original_job["mosaic"]["sources"]]
    destination = args.output.resolve()
    assert destination != root and not destination.is_relative_to(root)
    destination.mkdir(parents=True, exist_ok=False)
    assets = destination / "assets"
    assets.mkdir()
    originals = []
    for job, pinned in zip(selected, original_job["mosaic"]["sources"]):
        assert job["status"] == "succeeded" and job["kind"] == "download"
        assert job["assetKey"] == original_job["assetKey"] and job["sha256"] == pinned["sha256"]
        path = Path(job["outputPath"]).resolve(strict=True)
        assert path.parent.samefile(root / "assets")
        assert digest(path) == job["sha256"]
        originals.append(path)
        target = assets / f"{job['id']}.tif"
        os.link(path, target)
        job["outputPath"] = str(target)
    write_json(destination / "jobs.json", {job["id"]: job for job in [*selected, original_job]})
    write_json(destination / "projects.json", {project["id"]: project})
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
            ready = time.monotonic() + 20
            while True:
                assert process.poll() is None, "QA service exited before becoming ready"
                try:
                    assert api("/health")["status"] == "ok"
                    break
                except URLError:
                    assert time.monotonic() < ready
                    time.sleep(0.2)
            started = time.monotonic()
            job = api(f"/jobs/{args.job}/retry", {})
            deadline = started + 1200
            last_progress = None
            while job["status"] in {"queued", "running"}:
                assert time.monotonic() < deadline, "Mosaic did not settle within 20 minutes"
                progress = (job.get("bytesDownloaded"), job.get("validation"))
                if progress != last_progress:
                    print(f"{progress[0]}/{job.get('totalBytes')} {progress[1]}", flush=True)
                    last_progress = progress
                time.sleep(1)
                job = api(f"/jobs/{args.job}")
            elapsed = time.monotonic() - started
            assert job["status"] == "succeeded", job.get("error")
            output = Path(job["outputPath"])
            assert output.resolve().parent.samefile(assets) and digest(output) == job["sha256"]
            contributors = [0] * len(selected)
            covered = 0
            with rasterio.open(output) as result:
                rasters = [rasterio.open(path) for path in originals]
                try:
                    assert result.crs == rasters[0].crs
                    assert result.count == (3 if job["assetKey"] == "visual" else 1)
                    assert result.res == rasters[0].res and result.nodata == 0
                    assert result.dtypes == ("uint8",) * result.count
                    assert result.width * result.height > 8_000_000
                    for y in range(0, result.height, 128):
                        rows = min(128, result.height - y)
                        window = Window(0, y, result.width, rows)
                        actual = result.read(window=window)
                        expected = np.zeros_like(actual)
                        owner = np.full((rows, result.width), -1, dtype="int8")
                        left = result.bounds.left
                        top = result.bounds.top - y * result.res[1]
                        for index, raster in enumerate(rasters):
                            assert raster.crs == result.crs and raster.res == result.res
                            col, row = (~raster.transform) * (left, top)
                            assert abs(col - round(col)) < 1e-7 and abs(row - round(row)) < 1e-7
                            pixels = raster.read(window=Window(round(col), round(row), result.width, rows), boundless=True, fill_value=0)
                            valid = np.any(pixels != 0, axis=0)
                            expected[:, valid] = pixels[:, valid]
                            owner[valid] = index
                        np.testing.assert_array_equal(actual, expected)
                        covered += int(np.count_nonzero(owner >= 0))
                        for index in range(len(selected)):
                            contributors[index] += int(np.count_nonzero(owner == index))
                    metadata = json.loads(Path(job["manifestPath"]).read_text(encoding="utf-8"))
                    assert [source["sha256"] for source in metadata["sources"]] == [source["sha256"] for source in selected]
                    assert metadata["plan"]["coveredPixels"] == covered
                    assert metadata["plan"]["maskedPixels"] == 0
                    assert metadata["output"]["sha256"] == job["sha256"]
                    report = {
                        "independentDecoder": "Rasterio / GDAL", "originalProjectId": project["id"],
                        "originalProjectBounds": project["bounds"], "isolatedStore": str(destination),
                        "retryJobId": job["id"], "sourceCount": len(selected), "noRedownload": True,
                        "sourceFiles": [{"jobId": source["id"], "itemId": source["itemId"], "sha256": source["sha256"], "contributingPixels": count}
                                        for source, count in zip(selected, contributors)],
                        "width": result.width, "height": result.height, "bands": result.count,
                        "verifiedSamples": result.width * result.height * result.count,
                        "crs": str(result.crs), "pixelSize": list(result.res), "bounds": list(result.bounds),
                        "coveredPixels": covered, "allSamplesExact": True, "outputBytes": output.stat().st_size,
                        "outputSha256": job["sha256"], "processingSeconds": round(elapsed, 2),
                        "outputPath": str(output), "runtimeBinarySha256": digest(args.runtime),
                    }
                finally:
                    for raster in rasters:
                        raster.close()
            assert all(digest(path) == source["sha256"] for path, source in zip(originals, selected))
            report["sourcesUnchanged"] = True
            write_json(destination / "independent-verification.json", report)
            write_json(args.report, report)
            print(json.dumps(report, ensure_ascii=False, indent=2), flush=True)
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
    parser.add_argument("job")
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--port", type=int, default=4320)
    run(parser.parse_args())
