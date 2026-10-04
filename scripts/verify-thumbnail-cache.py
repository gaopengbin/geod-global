"""Verify cold/warm/restarted thumbnail requests using real, read-only originals.

Runs the product runtime in a new isolated store. Originals are hard linked on
the same drive, never changed or downloaded. Uses no desktop UI automation.
"""
import argparse
import base64
import copy
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time
from urllib.error import URLError
from urllib.request import urlopen


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write_json(path, data):
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def run(args):
    source = args.source_store.resolve(strict=True)
    destination = args.output.resolve()
    assert destination != source and not destination.is_relative_to(source)
    records = json.loads((source / "jobs.json").read_text(encoding="utf-8"))
    ids = set(args.jobs)
    for job_id in args.jobs:
        ids.update(pin["jobId"] for pin in (records[job_id].get("mosaic") or {}).get("sources", []))
    jobs = {job_id: copy.deepcopy(records[job_id]) for job_id in ids}
    destination.mkdir(parents=True, exist_ok=False)
    assets = destination / "assets"
    assets.mkdir()
    originals = {}
    for job in jobs.values():
        assert job["status"] == "succeeded"
        path = Path(job["outputPath"]).resolve(strict=True)
        assert path.parent.samefile(source / "assets")
        target = assets / f"{job['id']}.tif"
        os.link(path, target)
        if job["id"] in args.jobs:
            assert digest(path) == job["sha256"]
            originals[job["id"]] = path
        job["outputPath"] = str(target)
        if job.get("manifestPath"):
            target = assets / f"{job['id']}.metadata.json"
            os.link(Path(job["manifestPath"]), target)
            job["manifestPath"] = str(target)
    write_json(destination / "jobs.json", jobs)
    write_json(destination / "projects.json", json.loads((source / "projects.json").read_text(encoding="utf-8")))
    base = f"http://127.0.0.1:{args.port}"
    results = {job_id: {"jobId": job_id, "assetKey": jobs[job_id]["assetKey"], "kind": jobs[job_id]["kind"], "sourceSha256": jobs[job_id]["sha256"]} for job_id in args.jobs}
    first = {}
    for attempt in ["firstProcess", "restartedProcess"]:
        with (destination / f"{attempt}.out.log").open("wb") as out, (destination / f"{attempt}.err.log").open("wb") as err:
            process = subprocess.Popen([str(args.runtime.resolve(strict=True)), "serve", "--data-dir", str(destination), "--port", str(args.port)], stdout=out, stderr=err)
            try:
                deadline = time.monotonic() + 20
                while True:
                    assert process.poll() is None
                    try:
                        with urlopen(base + "/health", timeout=3) as response:
                            assert json.load(response)["status"] == "ok"
                            break
                    except URLError:
                        assert time.monotonic() < deadline
                        time.sleep(0.2)
                phases = ["cold", "warm"] if attempt == "firstProcess" else ["afterRestart"]
                for phase in phases:
                    for job_id in args.jobs:
                        started = time.monotonic()
                        with urlopen(f"{base}/jobs/{job_id}/thumbnail", timeout=90) as response:
                            preview = json.load(response)
                        seconds = time.monotonic() - started
                        assert preview["jobId"] == job_id and preview["sha256"] == jobs[job_id]["sha256"]
                        assert 0 < preview["width"] <= 160 and 0 < preview["height"] <= 160
                        png = base64.b64decode(preview["dataUrl"].removeprefix("data:image/png;base64,"), validate=True)
                        assert png.startswith(b"\x89PNG\r\n\x1a\n")
                        if phase == "cold":
                            first[job_id] = preview
                        else:
                            assert first[job_id] == preview
                        results[job_id][phase + "Seconds"] = round(seconds, 4)
                        results[job_id]["pngSha256"] = hashlib.sha256(png).hexdigest()
                        print(f"{phase}: {jobs[job_id]['assetKey']} / {jobs[job_id]['kind']} {seconds:.4f}s", flush=True)
            finally:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=10)
    cache = destination / "cache" / "thumbnails" / "v1"
    files = list(cache.glob("*.json"))
    assert len(files) == len(args.jobs)
    assert not list(cache.glob("*.part"))
    assert all(digest(path) == jobs[job_id]["sha256"] for job_id, path in originals.items())
    report = {"realLocalFiles": True, "desktopStoreNotModifiedByQa": True, "sourceFilesUnchanged": True, "noRedownload": True,
              "isolatedStore": str(destination), "cacheEntries": len(files),
              "cacheBytes": sum(path.stat().st_size for path in files),
              "samePngAfterRestart": True, "previews": list(results.values()),
              "runtimeBinarySha256": digest(args.runtime)}
    write_json(args.report, report)
    print(json.dumps(report, ensure_ascii=False, indent=2), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source_store", type=Path)
    parser.add_argument("jobs", nargs="+")
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--port", type=int, default=4321)
    run(parser.parse_args())
