"""Offline persistence controls on private copies of accepted scientific RGB.

Only completed RGB TIFFs and manifests are copied; none of their five parent
files or jobs are present. No new provider download or native window is claimed.
"""
import argparse
import base64
import copy
import hashlib
import io
import json
import shutil
import subprocess
import time
import urllib.error
import urllib.request
import zipfile
from datetime import datetime, timezone
from pathlib import Path

import numpy as np
import rasterio
from PIL import Image


def sha(file):
    digest = hashlib.sha256()
    with Path(file).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def dump(file, data):
    Path(file).write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def check_thumbnail(data, rgb, nodata=-28672):
    width, height = data["width"], data["height"]
    rows = np.arange(height, dtype=np.int64) * rgb.shape[1] // height
    cols = np.arange(width, dtype=np.int64) * rgb.shape[2] // width
    sampled = rgb[:, rows[:, None], cols[None, :]].astype(np.int32)
    valid = np.all(sampled != nodata, axis=0)
    expected = np.zeros((height, width, 4), dtype=np.uint8)
    for channel in range(3):
        values = np.sort(sampled[channel][valid])
        if not len(values):
            continue
        low, high = values[[(len(values) - 1) * 2 // 100, (len(values) - 1) * 98 // 100]]
        expected[:, :, channel][valid] = 128 if low == high else np.floor(
            np.clip((sampled[channel][valid] - low) / (high - low), 0, 1) * 255 + .5
        ).astype(np.uint8)
    expected[:, :, 3][valid] = 255
    png = base64.b64decode(data["dataUrl"].split(",", 1)[1])
    assert np.array_equal(np.asarray(Image.open(io.BytesIO(png)).convert("RGBA")), expected)
    return {"width": width, "height": height, "rgbaPixelsCompared": width * height,
            "pngSha256": hashlib.sha256(png).hexdigest()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True)
    parser.add_argument("--root", required=True)
    parser.add_argument("--port", type=int, default=4627)
    args = parser.parse_args()
    source, root = Path(args.source).resolve(), Path(args.root).resolve()
    for folder in [source, root]:
        assert folder.parent == Path(".verification").resolve()
        assert folder.name.startswith(("modis-rgb-mask-", "modis-coupled-", "landsat-rgb-mask-", "landsat-coupled-"))
    assert source != root and not root.exists()
    native = json.loads((source / "native-verification.json").read_text(encoding="utf-8"))
    assert native["status"] == "passed"
    root.mkdir()
    (root / "assets").mkdir()
    exe = root / Path(native["nativeBinary"]).name
    assert sha(native["nativeBinary"]) == native["nativeBinarySha256"]
    shutil.copy2(native["nativeBinary"], exe)
    assert sha(exe) == native["nativeBinarySha256"]
    jobs, parents = {}, set()
    for entry in native["cases"]:
        job = copy.deepcopy(entry["job"])
        for pin in job["rgbSpec"]["sources"] + job["rgbSpec"].get("qualityMask", {}).get("sources", []):
            parents.add(pin["jobId"])
        for scene in job["rgbSpec"].get("qualityMask", {}).get("coupled", {}).get("scenes", []):
            parents.update(pin["jobId"] for pin in scene["sources"])
        target = root / "assets" / f"{job['id']}.tif"
        metadata = root / "assets" / f"{job['id']}.metadata.json"
        assert sha(job["outputPath"]) == job["sha256"]
        shutil.copy2(job["outputPath"], target)
        shutil.copy2(job["manifestPath"], metadata)
        job.update(outputPath=str(target), manifestPath=str(metadata))
        job.pop("settled", None)
        jobs[job["id"]] = job
    assert not parents.intersection(jobs)
    assert not any((root / "assets" / f"{parent}.tif").exists() for parent in parents)
    dump(root / "jobs.json", jobs)
    shutil.copy2(source / "projects.json", root / "projects.json")
    dump(root / "proxy-settings.json", {"mode": "custom", "url": "http://127.0.0.1:9"})
    base = f"http://127.0.0.1:{args.port}"
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    process = None
    report = {"schema": "geod-landsat-coupled-cache/v1" if source.name.startswith("landsat-coupled-") else "geod-landsat-rgb-mask-cache/v1" if source.name.startswith("landsat-rgb-mask-") else "geod-modis-coupled-cache/v1" if source.name.startswith("modis-coupled-") else "geod-modis-rgb-mask-cache/v1", "qaOnly": True,
              "checkedAt": datetime.now(timezone.utc).isoformat(), "status": "running",
              "nativeBinarySha256": native["nativeBinarySha256"], "nativeReceiptSha256": sha(source / "native-verification.json"),
              "scope": "Private copies of accepted real outputs, with all RGB and QA parents absent",
              "network": "unreachable upstream proxy", "missingParentJobs": sorted(parents), "cases": [], "controls": []}

    def stop():
        nonlocal process
        if process is not None and process.poll() is None:
            process.terminate()
            process.wait(timeout=20)
        process = None

    def api(route, body=None, rejected=False):
        request = urllib.request.Request(base + route, data=json.dumps(body).encode() if body is not None else None,
                                         headers={"Content-Type": "application/json", "X-GeoD-Client": "geod-global"})
        try:
            with opener.open(request, timeout=120) as response:
                data = json.load(response)
        except urllib.error.HTTPError as error:
            assert rejected
            return {"statusCode": error.code, "body": json.loads(error.read())}
        assert not rejected, "The changed input should have been rejected"
        return data

    def start():
        nonlocal process
        process = subprocess.Popen([str(exe), "serve", "--data-dir", str(root), "--port", str(args.port)],
                                   stdout=(root / "runtime.stdout.log").open("ab"), stderr=(root / "runtime.stderr.log").open("ab"),
                                   creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        for attempt in range(100):
            assert process.poll() is None, (root / "runtime.stderr.log").read_text(encoding="utf-8")
            try:
                assert Path(api("/health")["storageRoot"]).samefile(root)
                assert api("/proxy") == {"mode": "custom", "url": "http://127.0.0.1:9"}
                assert len(api("/jobs")) == len(jobs)
                return
            except OSError:
                time.sleep(.1)
        raise AssertionError("Offline service did not start")

    def cache_entries():
        result = {}
        for file in (root / "cache" / "thumbnails" / "v1").glob("*.json"):
            raw = file.read_bytes()
            entry = json.loads(raw)
            if entry["preview"]["jobId"] in jobs:
                stat = file.stat()
                result[entry["preview"]["jobId"]] = {"path": file, "bytes": raw, "inode": stat.st_ino, "createdNs": stat.st_ctime_ns}
        assert len(result) == len(jobs)
        return result

    try:
        start()
        thumbnails = {}
        for entry in native["cases"]:
            job = jobs[entry["job"]["id"]]
            assert api(f"/jobs/{job['id']}")["settled"]
            data = api(f"/jobs/{job['id']}/rgb")
            png = base64.b64decode(data["previewDataUrl"].split(",", 1)[1])
            assert hashlib.sha256(png).hexdigest() == entry["preview"]["pngSha256"]
            thumb = api(f"/jobs/{job['id']}/thumbnail")
            assert thumb["sha256"] == job["sha256"]
            with rasterio.open(job["outputPath"]) as ds:
                thumbnail = check_thumbnail(thumb, ds.read(), job["rgbSpec"]["profile"]["nodata"])
            thumbnails[job["id"]] = thumb
            pixels = []
            for expected in entry["pixels"]:
                expected = expected.get("sample", expected)
                x, y = expected["coordinate"]
                value = api(f"/jobs/{job['id']}/rgb/pixel?x={x}&y={y}")
                assert value == expected
                pixels.append(value)
            package = api(f"/jobs/{job['id']}/package", {})
            assert sha(package["path"]) == entry["package"]["sha256"] == package["sha256"]
            with zipfile.ZipFile(package["path"]) as archive:
                assert archive.testzip() is None and len(archive.namelist()) == 5
            report["cases"].append({"id": job["id"], "case": entry["case"], "policy": entry["policy"], "excludeSnow": entry["excludeSnow"],
                                    "sha256": job["sha256"], "previewIdentical": True, "thumbnail": thumbnail, "pixelsCompared": len(pixels),
                                    "packageSha256": package["sha256"], "readWithoutParents": True})
            print(json.dumps({"case": entry["case"], "policy": entry["policy"], "offlineRead": "passed"}), flush=True)
        cache = cache_entries()
        pinned = jobs[native["cases"][-1]["job"]["id"]]
        tiff = Path(pinned["outputPath"])
        original = tiff.read_bytes()
        try:
            with tiff.open("ab") as stream:
                stream.write(b"\0")
            failures = {route: api(f"/jobs/{pinned['id']}/{route}", {} if route == "package" else None, rejected=True)
                        for route in ["rgb", "thumbnail", "package"]}
            report["controls"].append({"name": "changed output refused with existing cache", "failures": failures})
        finally:
            tiff.write_bytes(original)
        assert sha(tiff) == pinned["sha256"]
        assert api(f"/jobs/{pinned['id']}/thumbnail") == thumbnails[pinned["id"]]
        damaged = cache[next(iter(jobs))]["path"]
        damaged.write_bytes(b"{invalid cache")
        assert api(f"/jobs/{next(iter(jobs))}/thumbnail") == thumbnails[next(iter(jobs))]
        report["controls"].append({"name": "corrupt cache rebuilt from unchanged TIFF", "passed": True})
        manifest = Path(pinned["manifestPath"])
        original_manifest = manifest.read_bytes()
        try:
            altered = json.loads(original_manifest)
            altered["spec"]["qualityMask"]["excludeSnow"] = False
            dump(manifest, altered)
            report["controls"].append({"name": "changed rule manifest refused for delivery", "failure": api(f"/jobs/{pinned['id']}/package", {}, rejected=True)})
        finally:
            manifest.write_bytes(original_manifest)
        before = cache_entries()
        stop()
        start()
        for row in report["cases"]:
            started = time.perf_counter()
            assert api(f"/jobs/{row['id']}/thumbnail") == thumbnails[row["id"]]
            cache_file = before[row["id"]]
            stat = cache_file["path"].stat()
            assert cache_file["path"].read_bytes() == cache_file["bytes"]
            assert stat.st_ino == cache_file["inode"] and stat.st_ctime_ns == cache_file["createdNs"]
            row.update(cacheBytesFileIdentityCreationUnchanged=True, warmRestartMilliseconds=(time.perf_counter() - started) * 1000)
            job = api(f"/jobs/{row['id']}")
            assert job["rgbSpec"] == jobs[row["id"]]["rgbSpec"] and job["rgbOutput"] == jobs[row["id"]]["rgbOutput"]
        report.update(status="passed", allParentsAbsent=True, restarted=True, originalAcceptedWorkspaceUnchanged=True)
        dump(root / "cache-verification.json", report)
        print(json.dumps({"status": "passed", "files": len(jobs), "thumbnailPixels": sum(r["thumbnail"]["rgbaPixelsCompared"] for r in report["cases"]), "cacheEntriesReused": len(jobs)}), flush=True)
    finally:
        stop()


if __name__ == "__main__":
    main()
