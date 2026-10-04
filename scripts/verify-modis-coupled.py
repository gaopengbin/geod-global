"""Independently verify coherent multi-scene MODIS RGB using real public files.

Creates a fresh private workspace, disables provider traffic, and compares every
output DN, preview pixel, retained-scene count, raw sample and delivery package.
"""
import argparse, atexit, copy, hashlib, importlib.util, json, shutil, subprocess, time, urllib.request, zipfile
from datetime import datetime, timezone
from pathlib import Path
import numpy as np
import rasterio
import shapely
from pyproj import Transformer
from shapely.geometry import shape

helper = importlib.util.spec_from_file_location("mask_acceptance", Path(__file__).with_name("verify-modis-rgb-mask.py"))
mask_acceptance = importlib.util.module_from_spec(helper)
helper.loader.exec_module(mask_acceptance)
sha, dump, read_layers, expected_rgb, preview_check = (getattr(mask_acceptance, n) for n in
    ["sha", "dump", "read_layers", "expected_rgb", "preview_check"])
KEYS, FILL = mask_acceptance.KEYS, mask_acceptance.FILL
SELECTION = "newest complete qualified RGB scene wins; composite start then item ID break ties"


def coherent_oracle(spec, jobs):
    grid, selection = spec["grid"], spec["qualityMask"]["coupled"]
    height, width = grid["height"], grid["width"]
    expected = np.full((3, height, width), FILL, dtype=np.int16)
    winner = np.full((height, width), -1, dtype=np.int16)
    baseline = winner.copy()
    covered = np.ones((height, width), dtype=bool)
    geometry = selection["geometry"]
    if geometry:
        rows, cols = np.indices((height, width))
        xs = grid["bounds"][0] + (cols + .5) * grid["pixelSize"][0]
        ys = grid["bounds"][3] - (rows + .5) * grid["pixelSize"][1]
        inverse = Transformer.from_crs("+proj=sinu +R=6371007.181 +units=m +no_defs", "EPSG:4326", always_xy=True)
        lon, lat = inverse.transform(xs, ys)
        covered = shapely.contains_xy(shape(geometry), lon, lat)
    for index, scene in enumerate(selection["scenes"]):
        layers = [jobs[p["jobId"]] for p in scene["sources"]]
        assert [j["assetKey"] for j in layers] == KEYS
        assert len({j["itemId"] for j in layers}) == 1
        for job, pin in zip(layers, scene["sources"]):
            assert pin["sha256"] == job["sha256"] and pin["href"] == job["href"]
            assert pin["itemId"] == job["itemId"] and pin["bytes"] == job["bytesDownloaded"]
        with rasterio.open(layers[0]["outputPath"]) as ds:
            assert ds.width == ds.height == 2400 and ds.dtypes == ("int16",)
            offsets = [(ds.transform.c - grid["bounds"][0]) / grid["pixelSize"][0],
                       (grid["bounds"][3] - ds.transform.f) / grid["pixelSize"][1]]
            assert max(abs(v - round(v)) for v in offsets) < 1e-6
            dx, dy = map(round, offsets)
            x0, x1 = max(0, dx), min(width, dx + ds.width)
            y0, y1 = max(0, dy), min(height, dy + ds.height)
        if x0 >= x1 or y0 >= y1:
            continue
        window = rasterio.windows.Window(x0-dx, y0-dy, x1-x0, y1-y0)
        arrays = []
        for layer in layers:
            assert sha(layer["outputPath"]) == layer["sha256"]
            with rasterio.open(layer["outputPath"]) as ds:
                arrays.append(ds.read(1, window=window))
        complete = np.all(np.stack(arrays[:3]) != FILL, axis=0) & covered[y0:y1, x0:x1]
        qualified, _ = expected_rgb(arrays, spec["qualityMask"])
        accepted = np.all(qualified != FILL, axis=0) & complete
        baseline[y0:y1, x0:x1][complete] = index
        winner[y0:y1, x0:x1][accepted] = index
        expected[:, y0:y1, x0:x1][:, accepted] = qualified[:, accepted]
    valid = winner >= 0
    counts = {"examinedPixels": width*height, "rejectedPixels": int((~valid).sum()),
              "inputCommonValidPixels": int((baseline >= 0).sum()),
              "removedValidPixels": int(((baseline >= 0) & ~valid).sum()),
              "coupled": {"sceneValidPixels": [int((winner == i).sum()) for i in range(len(selection["scenes"]))],
                          "fallbackPixels": int((valid & (winner != baseline)).sum())}}
    return expected, counts, winner, baseline, covered


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True)
    parser.add_argument("--source", default=".verification/modis-quality-processing-20261004")
    parser.add_argument("--exe", required=True)
    parser.add_argument("--port", type=int, default=4631)
    args = parser.parse_args()
    root, source, original_exe = map(lambda p: Path(p).resolve(), [args.root, args.source, args.exe])
    assert root.parent == Path(".verification").resolve() and root.name.startswith("modis-coupled-") and not root.exists()
    assert source.parent == root.parent
    source_receipt = source/"native-processing-verification.json"
    old = json.loads(source_receipt.read_text(encoding="utf-8")); assert old["status"] == "passed"
    old_jobs = json.loads((source/"jobs.json").read_text(encoding="utf-8"))
    projects = json.loads((source/"projects.json").read_text(encoding="utf-8"))
    selected = {j["id"]: copy.deepcopy(j) for j in old["originals"]}
    selected.update({row["job"]["id"]: copy.deepcopy(old_jobs[row["job"]["id"]]) for row in old["outputs"]})
    root.mkdir(); (root/"assets").mkdir()
    binary_sha = sha(original_exe); exe = root/f"runtime-{binary_sha[:16]}.exe"
    shutil.copy2(original_exe, exe); assert sha(exe) == binary_sha
    before = []
    for job in selected.values():
        src = Path(job["outputPath"]); assert sha(src) == job["sha256"]
        before.append({"path": str(src), "sha256": job["sha256"], "mtimeNs": src.stat().st_mtime_ns})
        target = root/"assets"/f"{job['id']}.tif"; shutil.copy2(src, target); job["outputPath"] = str(target)
        if job.get("manifestPath"):
            metadata = root/"assets"/f"{job['id']}.metadata.json"; shutil.copy2(job["manifestPath"], metadata); job["manifestPath"] = str(metadata)
    dump(root/"jobs.json", selected); dump(root/"projects.json", projects)
    dump(root/"proxy-settings.json", {"mode": "custom", "url": "http://127.0.0.1:9"})
    dump(root/"input-snapshots.json", {"jobs": selected, "projects": projects, "sourceFiles": before, "receiptSha256": sha(source_receipt)})
    report = {"schema": "geod-modis-coupled-native/v1", "status": "running", "qaOnly": True,
              "checkedAt": datetime.now(timezone.utc).isoformat(), "nativeBinary": str(exe), "nativeBinarySha256": binary_sha,
              "sourceReceiptSha256": sha(source_receipt), "definition": mask_acceptance.GUIDE,
              "independentReader": f"Rasterio {rasterio.__version__} / GDAL {rasterio.__gdal_version__}, NumPy, Shapely",
              "network": "provider traffic disabled via loopback:9", "inputs": {}, "cases": [], "controls": [], "regressions": []}
    dump(root/"native-verification.json", report)
    process = None; base = f"http://127.0.0.1:{args.port}"
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    def stop():
        nonlocal process
        if process and process.poll() is None:
            process.terminate(); process.wait(timeout=20)
        process = None
    atexit.register(stop)
    def api(route, body=None):
        req = urllib.request.Request(base+route, data=json.dumps(body).encode() if body is not None else None,
                                     headers={"Content-Type": "application/json", "X-GeoD-Client": "geod-global"})
        with opener.open(req, timeout=120) as res: return json.load(res)
    def start():
        nonlocal process
        process = subprocess.Popen([str(exe), "serve", "--data-dir", str(root), "--port", str(args.port)],
            stdout=(root/"runtime.stdout.log").open("ab"), stderr=(root/"runtime.stderr.log").open("ab"),
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        for _ in range(100):
            assert process.poll() is None, (root/"runtime.stderr.log").read_text(encoding="utf-8")
            try:
                assert Path(api("/health")["storageRoot"]).samefile(root); return
            except OSError: time.sleep(.1)
        raise AssertionError("Private runtime did not start")
    def wait(job):
        deadline = time.monotonic()+300
        while time.monotonic() < deadline:
            job = api(f"/jobs/{job['id']}"); assert job["status"] not in ["failed", "cancelled", "interrupted"], job
            if job["status"] == "succeeded" and job["settled"]: return job
            time.sleep(.2)
        raise AssertionError("Private task did not settle")
    def cli(*arguments, success=True):
        result = subprocess.run([str(exe), "scientific-rgb", *arguments, "--server", base], capture_output=True,
                                text=True, encoding="utf-8", timeout=300, creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        if success:
            assert result.returncode == 0, result.stderr
            return json.loads(result.stdout)
        assert result.returncode != 0
        return result.stderr.strip()
    try:
        start()
        for label in ["mosaic", "polygon", "single"]:
            layers = [selected[next(r["job"]["id"] for r in old["outputs"] if r["case"] == label and r["key"] == k)] for k in KEYS]
            report["inputs"][label] = {"project": next(c["project"] for c in old["cases"] if c["name"] == label), "jobs": layers}
        # Select an observed cloud fallback region from actual old/new original files.
        project_template = report["inputs"]["mosaic"]["project"]
        h08 = [s for s in project_template["scenes"] if ".h08v05." in s["itemId"]]
        items = sorted(s["itemId"] for s in h08)
        original_groups = [[next(j for j in selected.values() if j["kind"] == "download" and j["itemId"] == item and j["assetKey"] == k) for k in KEYS] for item in items]
        original_arrays = [read_layers(layers) for layers in original_groups]
        screened = [expected_rgb(a, {"policy": "clear", "excludeSnow": False})[0] for a in original_arrays]
        fallback = np.all(screened[0] != FILL, axis=0) & np.all(np.stack(original_arrays[1][:3]) != FILL, axis=0) & ~np.all(screened[1] != FILL, axis=0)
        positions = np.argwhere(fallback); assert len(positions) > 0
        row, col = map(int, positions[len(positions)//2])
        with rasterio.open(original_groups[0][0]["outputPath"]) as ds:
            inverse = Transformer.from_crs(ds.crs, "EPSG:4326", always_xy=True)
            xs = [ds.transform.c+(col+d)*ds.transform.a for d in [-20, 20]]
            ys = [ds.transform.f+(row+d)*ds.transform.e for d in [-10, 10]]
            lon, lat = inverse.transform([xs[0], xs[0], xs[1], xs[1]], [ys[0], ys[1], ys[0], ys[1]])
            bounds = [min(lon), min(lat), max(lon), max(lat)]
        report["fallbackRegion"] = {"centerOriginalPixel": [col, row], "wholeTileObservedFallbackPixels": int(fallback.sum()), "bounds": bounds}
        west, south, east, north = bounds; cx, cy = (west+east)/2, (south+north)/2
        outer = [[west,south],[east,south],[east,north],[west,north],[west,south]]
        hw, hh = (east-west)/10, (north-south)/10
        hole = [[cx-hw,cy-hh],[cx-hw,cy+hh],[cx+hw,cy+hh],[cx+hw,cy-hh],[cx-hw,cy-hh]]
        for label, geometry in [("fallback", None), ("fallback-polygon", {"type": "Polygon", "coordinates": [outer, hole]})]:
            draft = {"name": "QA · coupled MODIS · "+label, "bounds": bounds, "scenes": h08}
            if geometry: draft["geometry"] = geometry
            project = api("/projects", draft)
            layers = [wait(api(f"/projects/{project['id']}/mosaics", {"assetKey": k})) for k in KEYS]
            selected.update({j["id"]: j for j in layers})
            report["inputs"][label] = {"project": project, "jobs": layers}
            dump(root/"native-verification.json", report)
            print(json.dumps({"input": label, "projectId": project["id"], "layers": len(layers)}), flush=True)
        for label in ["fallback", "fallback-polygon", "mosaic", "polygon"]:
            layers, project = report["inputs"][label]["jobs"], report["inputs"][label]["project"]
            for policy, snow in [("clear", False), ("clear_best", False), ("clear_best", True)]:
                request = {"jobIds": [j["id"] for j in layers[:3]], "projectId": project["id"],
                    "name": f"QA · coupled {label} · {policy} · snow {snow}",
                    "qualityMask": {"qcJobId": layers[3]["id"], "stateJobId": layers[4]["id"], "policy": policy, "excludeSnow": snow}}
                slug = f"{label}-{policy}-{snow}"; request_file = root/f"{slug}-request.json"; dump(request_file, request)
                plan = cli("plan", "--request", str(request_file)); spec = plan["spec"]
                mask = spec["qualityMask"]; selection = mask["coupled"]
                assert mask["schemaVersion"] == "geod-modis-rgb-mask/v2" and selection["selection"] == SELECTION
                assert selection["geometry"] == project.get("geometry")
                assert len(selection["scenes"]) == len(project["scenes"])
                priorities = [(s["sources"][0]["itemId"].split('.')[1], s["sources"][0]["itemId"]) for s in selection["scenes"]]
                assert priorities == sorted(priorities) and len(set(priorities)) == len(priorities)
                expected, counts, winner, baseline, coverage = coherent_oracle(spec, selected)
                if label.startswith("fallback"): assert counts["coupled"]["fallbackPixels"] > 0
                job = cli("run", "--request", str(request_file)); assert job["status"] == "succeeded"
                assert job["rgbSpec"] == spec and sha(job["outputPath"]) == job["sha256"]
                with rasterio.open(layers[0]["outputPath"]) as parent, rasterio.open(job["outputPath"]) as out:
                    # Original COGs and project mosaics name the same spherical
                    # datum differently. Require every actual CRS parameter.
                    assert out.crs.to_dict() == parent.crs.to_dict() == {"proj":"sinu", "lon_0":0, "x_0":0, "y_0":0, "R":6371007.181, "units":"m", "no_defs":True}
                    assert out.transform == parent.transform and out.bounds == parent.bounds
                    assert out.count == 3 and out.dtypes == ("int16",)*3 and out.nodata == FILL
                    assert out.scales == (.0001,)*3 and out.offsets == (0.,)*3
                    actual = out.read(); assert np.array_equal(actual, expected), (label, policy, int(np.count_nonzero(actual != expected)))
                output = job["rgbOutput"]; valid = np.all(expected != FILL, axis=0)
                assert output["qualityMask"] == counts and output["channelValidPixels"] == [int(valid.sum())]*3
                assert output["commonValidPixels"] == int(valid.sum())
                assert output["samplesSha256"] == [hashlib.sha256(a.astype('<i2').tobytes()).hexdigest() for a in expected]
                metadata = api(f"/jobs/{job['id']}/rgb"); assert metadata["artifact"]["sha256"] == job["sha256"]
                preview = preview_check(metadata, expected, root/f"{slug}-preview.png")
                samples = []
                categories = {"fallback": valid & (winner != baseline), "newest": valid & (winner == baseline), "nodata": ~valid, "polygon-hole-or-outside": ~coverage}
                for category, choices in categories.items():
                    points = np.argwhere(choices)
                    if not len(points): continue
                    y, x = map(int, points[len(points)//2])
                    xy = [metadata["bounds"][0]+(x+.5)*metadata["pixelSize"][0], metadata["bounds"][3]-(y+.5)*metadata["pixelSize"][1]]
                    pixel = cli("pixel", "--id", job["id"], "--x", str(xy[0]), "--y", str(xy[1]))
                    assert pixel["values"] == expected[:,y,x].tolist()
                    assert pixel["reflectances"] == [None if v == FILL else int(v)*.0001 for v in expected[:,y,x]]
                    samples.append({"category": category, "winnerSceneIndex": int(winner[y,x]), "baselineSceneIndex": int(baseline[y,x]), "sample": pixel})
                package = cli("package", "--id", job["id"]); assert sha(package["path"]) == package["sha256"]
                with zipfile.ZipFile(package["path"]) as z:
                    assert z.testzip() is None and len(z.namelist()) == 5
                    assert hashlib.sha256(z.read(f"{job['id']}.tif")).hexdigest() == job["sha256"]
                    manifest = json.loads(z.read(f"{job['id']}.metadata.json"))
                    assert manifest["spec"] == spec and manifest["output"]["samples"] == output
                    note = z.read("README.txt").decode(); assert "Every retained RGB triplet" in note and "per-scene" in note
                case = {"case": label, "policy": policy, "excludeSnow": snow, "request": request, "plan": plan, "job": job,
                        "allDnCompared": expected.size, "counts": counts, "preview": preview, "pixels": samples, "package": package,
                        "outsideOrHolePixels": int((~coverage).sum())}
                report["cases"].append(case); dump(root/"native-verification.json", report)
                print(json.dumps({"case": label, "policy": policy, "snow": snow, "samples": expected.size, "fallback": counts["coupled"]["fallbackPixels"]}), flush=True)
        # Refactoring the shared encoder must retain same-scene and unmasked behavior.
        for policy in [None, "clear_best"]:
            layers = report["inputs"]["single"]["jobs"]
            request = {"jobIds": [j["id"] for j in layers[:3]], "name": "QA · legacy single · "+str(policy)}
            if policy: request["qualityMask"] = {"qcJobId": layers[3]["id"], "stateJobId": layers[4]["id"], "policy": policy, "excludeSnow": False}
            file = root/f"legacy-{policy}-request.json"; dump(file, request)
            job = cli("run", "--request", str(file))
            expected, counts = expected_rgb(read_layers(layers), request.get("qualityMask"))
            with rasterio.open(job["outputPath"]) as ds: assert np.array_equal(ds.read(), expected)
            assert job["rgbOutput"].get("qualityMask") == counts
            if policy: assert job["rgbSpec"]["qualityMask"]["schemaVersion"] == "geod-modis-rgb-mask/v1"
            report["regressions"].append({"policy": policy, "job": job, "allDnCompared": expected.size})
        good = report["cases"][0]["request"]
        count_before = len(api("/jobs"))
        for label, change in [("invalid policy", {"policy": "unknown"}), ("different area QA", {"qcJobId": report["inputs"]["mosaic"]["jobs"][3]["id"]})]:
            wrong = copy.deepcopy(good); wrong["qualityMask"].update(change)
            file = root/f"control-{label.replace(' ', '-')}.json"; dump(file, wrong)
            message = cli("plan", "--request", str(file), success=False)
            assert len(api("/jobs")) == count_before
            report["controls"].append({"name": label, "message": message, "queued": False})
        stop()
        stored = json.loads((root/"jobs.json").read_text(encoding="utf-8"))
        original_id = report["cases"][0]["job"]["rgbSpec"]["qualityMask"]["coupled"]["scenes"][0]["sources"][3]["jobId"]
        missing = copy.deepcopy(stored); missing.pop(original_id); dump(root/"jobs.json", missing); start()
        file = root/"missing-original-request.json"; dump(file, good)
        message = cli("plan", "--request", str(file), success=False); assert "original" in message.lower()
        assert len(api("/jobs")) == len(missing)
        report["controls"].append({"name": "missing original QA", "message": message, "queued": False})
        stop(); dump(root/"jobs.json", stored); start()
        for case in report["cases"]:
            readback = api(f"/jobs/{case['job']['id']}/rgb")
            assert readback["artifact"]["sha256"] == case["job"]["sha256"]
        for entry in before:
            file = Path(entry["path"]); assert sha(file) == entry["sha256"] and file.stat().st_mtime_ns == entry["mtimeNs"]
        report["sourceFilesUnchanged"] = len(before); report["restartInspections"] = len(report["cases"])
        report["status"] = "passed"; report["finishedAt"] = datetime.now(timezone.utc).isoformat()
        dump(root/"native-verification.json", report)
        dump(root/"ui-fixture.json", {"nativeBinary": str(exe), "cases": {label:report["inputs"][label] for label in ["fallback", "fallback-polygon", "polygon"]}})
        print(json.dumps({"status": "passed", "cases": len(report["cases"]), "dnSamples": sum(c["allDnCompared"] for c in report["cases"]),
            "fallbackPixels": sum(c["counts"]["coupled"]["fallbackPixels"] for c in report["cases"])}), flush=True)
    except Exception as error:
        report["status"] = "failed"; report["error"] = repr(error); dump(root/"native-verification.json", report)
        raise
    finally:
        stop()


if __name__ == "__main__": main()
