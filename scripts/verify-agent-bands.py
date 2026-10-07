"""Run and independently verify complete Agent Landsat/MODIS RGB acquisitions.

The start command builds an owned Rust driver against this repository's runtime
and starts it detached with a fresh isolated core. Observation cannot kill or
restart the download. The verify command makes no provider/model requests and
compares native raw/calibrated pixel receipts with independent Rasterio reads.
"""
import argparse
import atexit
import ctypes
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import socket
import subprocess
import tomllib
from urllib.parse import urlparse
import winreg


REPO = Path(__file__).resolve().parent.parent


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write(path, value):
    path.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n", encoding="utf-8")


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def owned_process(record):
    """Only consider the recorded PID live when its executable identity matches."""
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.OpenProcess.argtypes = [ctypes.c_ulong, ctypes.c_int, ctypes.c_ulong]
    kernel.OpenProcess.restype = ctypes.c_void_p
    kernel.CloseHandle.argtypes = [ctypes.c_void_p]
    kernel.QueryFullProcessImageNameW.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_wchar_p, ctypes.POINTER(ctypes.c_ulong)]
    kernel.GetExitCodeProcess.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong)]
    handle = kernel.OpenProcess(0x1000, False, record["pid"])
    if not handle:
        error = ctypes.get_last_error()
        if error == 87:  # ERROR_INVALID_PARAMETER: PID no longer exists.
            return {"live": False, "handleMissing": True}
        return {"live": None, "inspectionError": str(ctypes.WinError(error))}
    try:
        size = ctypes.c_ulong(32768)
        buffer = ctypes.create_unicode_buffer(size.value)
        if not kernel.QueryFullProcessImageNameW(handle, 0, buffer, ctypes.byref(size)):
            raise ctypes.WinError(ctypes.get_last_error())
        same = os.path.normcase(buffer.value) == os.path.normcase(record["executable"])
        code = ctypes.c_ulong()
        if not kernel.GetExitCodeProcess(handle, ctypes.byref(code)):
            raise ctypes.WinError(ctypes.get_last_error())
        return {"live": same and code.value == 259, "executableMatches": same, "exitCode": code.value}
    finally:
        kernel.CloseHandle(handle)


def checked_root(value, exists):
    root = value.resolve()
    assert root.parent == (REPO / ".verification").resolve()
    assert root.name.startswith("agent-bands-20261005-") and root.exists() == exists
    return root


def start(root):
    assert os.name == "nt"
    root = checked_root(root, False)
    with winreg.OpenKey(winreg.HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Internet Settings") as key:
        assert winreg.QueryValueEx(key, "ProxyEnable")[0] == 1
        proxy = "http://" + winreg.QueryValueEx(key, "ProxyServer")[0]
    parsed = urlparse(proxy)
    assert parsed.hostname in {"127.0.0.1", "localhost"} and parsed.port
    with socket.create_connection((parsed.hostname, parsed.port), timeout=3):
        pass
    root.mkdir()
    (root / "core").mkdir()
    driver = root / "driver"
    driver.mkdir()
    (driver / "acceptance.rs").write_bytes((REPO / "scripts/verify-agent-bands.rs").read_bytes())
    # All code/dependencies come from this checkout and its Cargo cache.
    manifest = """[package]
name = "geod-agent-band-acceptance"
version = "0.0.0"
edition = "2021"
[workspace]
resolver = "2"
[dependencies]
geod-runtime = { path = "../../../crates/geod-runtime" }
serde_json = { version = "1", features = ["float_roundtrip"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread", "time"] }
uuid = { version = "1", features = ["v4"] }
[[bin]]
name = "geod-agent-band-acceptance"
path = "acceptance.rs"
"""
    (driver / "Cargo.toml").write_text(manifest, encoding="utf-8")
    # Seed from the product lockfile rather than resolving newer cached transitives.
    # Cargo may prune unrelated desktop entries but retains the selected versions.
    (driver / "Cargo.lock").write_bytes((REPO / "Cargo.lock").read_bytes())
    build = subprocess.run(["cargo", "build", "--offline", "--manifest-path", str(driver / "Cargo.toml"),
                            "--target-dir", str(REPO / "target")], cwd=REPO,
                           creationflags=subprocess.CREATE_NO_WINDOW)
    assert build.returncode == 0, "Owned acceptance driver did not build; no transfer started."
    source = REPO / "target/debug/geod-agent-band-acceptance.exe"
    executable = root / "native-band-acceptance.exe"
    executable.write_bytes(source.read_bytes())
    with (root / "native-driver.log").open("wb") as log:
        process = subprocess.Popen([str(executable), str(root), proxy], cwd=REPO,
                                   stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
                                   creationflags=subprocess.CREATE_NO_WINDOW | subprocess.DETACHED_PROCESS)
    record = {"schema": "geod-agent-band-owned-process/v1", "pid": process.pid,
              "executable": str(executable), "binarySha256": digest(executable),
              "driverSourceSha256": digest(driver / "acceptance.rs"),
              "rootLockSha256": digest(REPO / "Cargo.lock"), "driverLockSha256": digest(driver / "Cargo.lock"),
              "startedAt": datetime.now(timezone.utc).isoformat(), "proxy": proxy,
              "detached": True, "usedUserDesktop": False}
    write(root / "owned-process.json", record)
    print(json.dumps({"started": True, "pid": process.pid, "run": str(root), "currentProcess": owned_process(record)}))


def status(root):
    root = checked_root(root, True)
    record = read(root / "owned-process.json")
    output = {"run": str(root), "process": owned_process(record)}
    if (root / "native-acceptance.json").exists():
        receipt = read(root / "native-acceptance.json")
        output.update(status=receipt["status"], stage=receipt["stage"], error=receipt.get("error"),
                      files=[{"provider": p["provider"], **{k: f.get(k) for k in
                             ["assetKey", "jobId", "status", "bytesDownloaded", "totalBytes", "settled"]}}
                             for p in receipt["providers"] for f in p["files"]])
    print(json.dumps(output, indent=2))


def wait_exit(root):
    """Observe one existing driver handle for at most 55 seconds; never stop it."""
    root = checked_root(root, True)
    record = read(root / "owned-process.json")
    assert owned_process(record)["live"] is True, "No current live owned handle to wait on."
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.OpenProcess.argtypes = [ctypes.c_ulong, ctypes.c_int, ctypes.c_ulong]
    kernel.OpenProcess.restype = ctypes.c_void_p
    kernel.CloseHandle.argtypes = [ctypes.c_void_p]
    kernel.WaitForSingleObject.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
    kernel.GetExitCodeProcess.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong)]
    kernel.QueryFullProcessImageNameW.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_wchar_p, ctypes.POINTER(ctypes.c_ulong)]
    handle = kernel.OpenProcess(0x101000, False, record["pid"])
    if not handle:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        size = ctypes.c_ulong(32768)
        buffer = ctypes.create_unicode_buffer(size.value)
        if not kernel.QueryFullProcessImageNameW(handle, 0, buffer, ctypes.byref(size)):
            raise ctypes.WinError(ctypes.get_last_error())
        assert os.path.normcase(buffer.value) == os.path.normcase(record["executable"]), "PID was reused; do not observe another process."
        assert digest(Path(record["executable"])) == record["binarySha256"]
        result = kernel.WaitForSingleObject(handle, 55000)
        if result == 0x102:
            print(json.dumps({"sameOwnedDriverStillRunning": True, "pid":record["pid"]}))
            return
        assert result == 0, f"Wait failed: {result}"
        code = ctypes.c_ulong()
        if not kernel.GetExitCodeProcess(handle, ctypes.byref(code)):
            raise ctypes.WinError(ctypes.get_last_error())
        receipt = {"schema":"geod-agent-band-observed-exit/v1", "pid":record["pid"],
                   "binarySha256":record["binarySha256"], "exitCode":code.value,
                   "observedAt":datetime.now(timezone.utc).isoformat(), "nativeHandleWait":True}
        write(root / "native-exit.json", receipt)
        print(json.dumps(receipt))
    finally:
        kernel.CloseHandle(handle)


def verify(root):
    import rasterio
    from rasterio.windows import Window
    root = checked_root(root, True)
    assert owned_process(read(root / "owned-process.json"))["live"] is False, "Wait for a verified terminal/missing process; polling failure is not completion."
    native = read(root / "native-acceptance.json")
    assert native["status"] == "passed" and native["reopenedSameJobs"] is True
    assert not native["fixture"] and native["modelCalls"] == 0 and not native["usedUserDesktop"]
    assert {p["provider"] for p in native["providers"]} == {"planetary-landsat", "planetary-modis"}
    jobs = read(root / "core/jobs.json")
    assert len(jobs) == 10
    assert len({f["jobId"] for p in native["providers"] for f in p["files"]}) == 10
    registry = (root / "core/jobs.json").read_bytes()
    native_executable = REPO / "target/debug/geod-runtime.exe"
    adapter_spec = importlib.util.spec_from_file_location("agent_band_mcp_qa", REPO / "scripts/verify-mcp.py")
    adapter = importlib.util.module_from_spec(adapter_spec)
    adapter_spec.loader.exec_module(adapter)
    # Once the acquisition driver is verifiably gone, exercise the product's
    # current read-only product binary too. No live store is opened twice and no download
    # or model tool is exposed by this second, strictly local acceptance pass.
    client = adapter.Client(native_executable, data_dir=root / "core")
    atexit.register(client.close)
    report = {"schema": "geod-agent-bands-independent/v1", "status": "pending",
              "reader": f"Rasterio {rasterio.__version__} / GDAL {rasterio.__gdal_version__}",
              "newProviderRequests": 0, "modelCalls": 0, "usedUserDesktop": False, "files": []}
    report["productReadBinarySha256"] = digest(native_executable)
    report["nativeReadAdapter"] = "Read-only MCP with the product binary; same native raster worker, no model turn or transfer"
    for provider in native["providers"]:
        grids = []
        for file in provider["files"]:
            job = jobs[file["jobId"]]
            assert job["kind"] == "download" and job["status"] == "succeeded" and job["itemId"] == provider["itemId"]
            assert file["assetKey"] == job["assetKey"]
            assert file["settled"] and file["reconfirmedSameJob"] and not file["preflightOnly"]
            assert job["agentApproval"]["planHash"] == file["downloadPlan"]["planHash"]
            path = Path(job["outputPath"]).resolve(strict=True)
            assert path.parent.samefile(root / "core/assets")
            checksum = digest(path)
            assert checksum == file["sha256"] == job["sha256"] == file["raster"]["sha256"]
            assert path.stat().st_size == job["bytesDownloaded"] == job["totalBytes"] == file["downloadPlan"]["expectedBytes"]
            assert job["agentApproval"]["remote"]["bytes"] == path.stat().st_size
            assert job["agentApproval"]["remote"]["etag"].startswith('"')
            state = client.call("geod_job_status", {"id":job["id"]})
            assert state["status"] == "succeeded" and state["settled"] is True and state["sha256"] == checksum
            product_inspection = client.call("geod_raster_inspect", {"id":job["id"]})
            assert all(product_inspection[k] == file["raster"][k] for k in
                       ["width","height","bandCount","dataType","crs","bounds","pixelSize","nodata","sha256"])
            result = {"provider": provider["provider"], "itemId": provider["itemId"], "assetKey": file["assetKey"],
                      "jobId": job["id"], "sha256": checksum, "bytes": path.stat().st_size, "samples": []}
            with rasterio.open(path) as dataset:
                inspection = file["raster"]
                assert dataset.count == 1 and (dataset.width, dataset.height) == (inspection["width"], inspection["height"])
                # Custom MODIS sinusoidal CRS has no EPSG authority; compare its
                # native projection parameters rather than fabricating an EPSG.
                if provider["provider"] == "planetary-landsat":
                    assert dataset.crs.to_string() == inspection["crs"]
                else:
                    projection = dataset.crs.to_dict()
                    assert projection["proj"] == "sinu" and math.isclose(projection.get("R", projection.get("a")), 6371007.181)
                assert all(math.isclose(a,b,abs_tol=1e-7) for a,b in zip(dataset.bounds,inspection["bounds"]))
                assert all(math.isclose(a,b,abs_tol=1e-7) for a,b in zip(dataset.res,inspection["pixelSize"]))
                assert dataset.nodata == inspection["nodata"]
                expected_type = ({"red":"uint16","green":"uint16","blue":"uint16","qa_pixel":"uint16","qa_radsat":"uint16"}
                                 if provider["provider"] == "planetary-landsat" else
                                 {"red":"int16","green":"int16","blue":"int16","modis_qc":"uint32","modis_state":"uint16"})
                assert dataset.dtypes == (expected_type[file["assetKey"]],)
                assert inspection["dataType"].lower() == expected_type[file["assetKey"]]
                if file["assetKey"] in {"red", "green", "blue"}:
                    calibration = inspection["reflectance"]
                    assert math.isclose(calibration["scale"],0.0000275 if provider["provider"] == "planetary-landsat" else 0.0001)
                    assert calibration["offset"] == (-0.2 if provider["provider"] == "planetary-landsat" else 0.0)
                    assert calibration["band"] == file["assetKey"]
                    result["calibration"] = {k:calibration[k] for k in ["product","band","scale","offset","pixelInterpretation"]}
                grids.append((dataset.width,dataset.height,dataset.crs.to_wkt(),tuple(dataset.transform)))
                for pixel in file["pixels"]:
                    column, row = pixel["pixel"]
                    raw = int(dataset.read(1,window=Window(column,row,1,1))[0,0])
                    x,y = dataset.xy(row,column)
                    assert pixel["value"] == raw and pixel["sha256"] == checksum
                    product_pixel = client.call("geod_raster_pixel", {"id":job["id"], "x":x,"y":y})
                    assert product_pixel["pixel"] == [column,row] and product_pixel["value"] == raw
                    assert product_pixel["sha256"] == checksum and product_pixel["isNoData"] == pixel["isNoData"]
                    assert product_pixel.get("reflectance") == pixel.get("reflectance")
                    assert all(math.isclose(a,b,abs_tol=1e-7) for a,b in zip(pixel["center"],[x,y]))
                    # QA_PIXEL expresses fill in bit zero even when the TIFF
                    # has no GDAL NoData tag. Do not confuse that science flag
                    # with file-level NoData, or treat clear QA_RADSAT zero as fill.
                    nodata = bool(raw & 1) if file["assetKey"] == "qa_pixel" else (
                        False if file["assetKey"] == "qa_radsat" else
                        dataset.nodata is not None and raw == dataset.nodata)
                    assert pixel["isNoData"] == nodata
                    if file["assetKey"] not in {"red","green","blue"}:
                        assert product_pixel["quality"] == pixel["quality"]
                        for field in pixel["quality"]["fields"]:
                            start,end = field["startBit"],field["endBit"]
                            assert 0 <= start <= end < (32 if file["assetKey"] == "modis_qc" else 16)
                            assert field["value"] == ((raw >> start) & ((1 << (end-start+1))-1))
                    if file["assetKey"] in {"red","green","blue"}:
                        expected = raw * (0.0000275 if provider["provider"] == "planetary-landsat" else 0.0001)
                        expected += -0.2 if provider["provider"] == "planetary-landsat" else 0
                        if nodata:
                            assert "reflectance" not in pixel
                        else:
                            assert math.isclose(pixel["reflectance"],expected,abs_tol=1e-12)
                    result["samples"].append({"column":column,"row":row,"raw":raw,"isNoData":nodata})
                assert len(result["samples"]) == 7
                if file["assetKey"] in {"red", "green", "blue"}:
                    assert any(not sample["isNoData"] for sample in result["samples"]), "Calibration acceptance needs actual non-fill pixels."
                result.update(width=dataset.width,height=dataset.height,dataType=dataset.dtypes[0],
                              crs=dataset.crs.to_wkt(),pixelSize=list(dataset.res),nodata=dataset.nodata)
            assert digest(path) == checksum
            report["files"].append(result)
        assert len(set(grids)) == 1, "RGB and QA must share one complete observation grid."
    atexit.unregister(client.close)
    client.close()
    assert (root / "core/jobs.json").read_bytes() == registry
    root_packages = {(p["name"],p["version"]) for p in tomllib.loads((REPO / "Cargo.lock").read_text(encoding="utf-8"))["package"]}
    driver_packages = {(p["name"],p["version"]) for p in tomllib.loads((root / "driver/Cargo.lock").read_text(encoding="utf-8"))["package"]}
    report["driverBinarySha256"] = digest(root / "native-band-acceptance.exe")
    report["driverLockSha256"] = digest(root / "driver/Cargo.lock")
    report["driverPackagesAbsentFromCurrentProductLock"] = sorted([list(p) for p in driver_packages-root_packages if p[0] != "geod-agent-band-acceptance"])
    if (root / "native-exit.json").exists():
        exit_receipt = read(root / "native-exit.json")
        assert exit_receipt["binarySha256"] == report["driverBinarySha256"] and exit_receipt["exitCode"] == 0
        report["observedCleanNativeExit"] = True
    report.update(status="passed", originalFilesUnchanged=True, nativeRecordsUnchanged=True,
                  completeOriginals=10, originalRgbBands=6, matchedQaFiles=4,
                  exactRawPixels=sum(len(f["samples"]) for f in report["files"]), nativeAcquisitionWorkflow=True)
    report["currentProductNativeReadsMatched"] = True
    write(root / "independent-originals.json", report)
    print(json.dumps({k:report[k] for k in ["status","completeOriginals","originalRgbBands","matchedQaFiles","exactRawPixels"]}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode",choices=["start","status","wait_exit","verify"])
    parser.add_argument("--run",type=Path,required=True)
    args = parser.parse_args()
    globals()[args.mode](args.run)
