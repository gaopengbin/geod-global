"""Real stdio MCP acceptance over the isolated scientific-RGB QA workspace.

Both exclusive-directory and loopback-service modes use the real executable.
No provider calls, arbitrary paths or renderer mocks are used.
"""
import argparse
import contextlib
import hashlib
import importlib.util
import json
import subprocess
import time
import urllib.request
import zipfile
from pathlib import Path

import rasterio

module_spec = importlib.util.spec_from_file_location("geod_verify_mcp", Path(__file__).with_name("verify-mcp.py"))
module = importlib.util.module_from_spec(module_spec)
module_spec.loader.exec_module(module)
Client = module.Client


def sha(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


@contextlib.contextmanager
def client(exe, **options):
    connection = Client(exe, **options)
    try:
        yield connection
    finally:
        connection.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True)
    parser.add_argument("--exe", default="target/debug/geod-runtime.exe")
    parser.add_argument("--port", type=int, default=4600)
    args = parser.parse_args()
    root = Path(args.root).resolve()
    assert root.parent == Path(".verification").resolve() and root.name.startswith("scientific-rgb-")
    fixture = json.loads((root / "ui-fixture.json").read_text(encoding="utf-8"))
    exe = str(Path(args.exe).resolve())
    request = {"jobIds": fixture["sourceIds"]}
    report = {"schema": "geod-scientific-rgb-mcp/v1", "nativeBinarySha256": sha(exe), "cases": [], "qaOnly": True}
    required_read = {"geod_rgb_plan", "geod_rgb_inspect", "geod_rgb_pixel"}
    required_write = {"geod_rgb_run", "geod_rgb_package"}

    def discovery(c, write):
        tools = c.request("tools/list")["result"]["tools"]
        names = {t["name"] for t in tools}
        assert required_read <= names
        assert (required_write <= names) if write else not (required_write & names)
        if not write:
            for name in required_write:
                denied = c.request("tools/call", {"name": name, "arguments": {}})
                assert denied["error"]["code"] == -32602
        for tool in tools:
            if tool["name"] in required_read:
                assert tool["annotations"]["readOnlyHint"]
            if tool["name"] in required_write:
                assert not tool["annotations"]["readOnlyHint"]
        return sorted(required_read | (required_write if write else set()))

    def inspect(c, job):
        metadata = c.call("geod_rgb_inspect", {"id": job["id"]})
        assert metadata["artifact"] == {"jobId": job["id"], "sha256": job["sha256"]}
        assert metadata["previewOmitted"] and "previewDataUrl" not in metadata
        left, bottom, right, top = metadata["bounds"]
        pixel = c.call("geod_rgb_pixel", {"id": job["id"], "x": (left + right) / 2, "y": (bottom + top) / 2})
        assert pixel["artifact"] == metadata["artifact"]
        with rasterio.open(job["outputPath"]) as dataset:
            col, row = pixel["pixel"]
            assert dataset.read(window=rasterio.windows.Window(col, row, 1, 1))[:, 0, 0].tolist() == pixel["values"]
        assert sha(job["outputPath"]) == job["sha256"]
        return {"id": job["id"], "sha256": job["sha256"], "pixel": pixel["values"], "previewOmitted": True}

    def settled(c, job_id):
        deadline = time.monotonic() + 180
        while True:
            job = c.call("geod_job_status", {"id": job_id})
            if job["settled"] and job["status"] not in ("queued", "running"):
                assert job["status"] == "succeeded", job.get("error")
                return job
            assert time.monotonic() < deadline, "Scientific RGB did not settle"
            time.sleep(.4)

    def packaged(c, job):
        package = c.call("geod_rgb_package", {"id": job["id"]})
        assert sha(package["path"]) == package["sha256"]
        with zipfile.ZipFile(package["path"]) as archive:
            assert archive.testzip() is None and len(archive.namelist()) == 5
            metadata = json.loads(archive.read(f"{job['id']}.metadata.json"))
            assert metadata["output"]["sha256"] == job["sha256"]
        return {"sha256": package["sha256"], "bytes": package["bytes"], "files": package["files"]}

    with client(exe, data_dir=root) as c:
        tools = discovery(c, False)
        plan = c.call("geod_rgb_plan", {"request": request})
        assert [s["jobId"] for s in plan["spec"]["sources"]] == request["jobIds"]
        assert plan["requiredDiskBytes"] > plan["rawBytes"] > 0
        saved = inspect(c, fixture["derived"])
        bad = c.request("tools/call", {"name": "geod_rgb_plan", "arguments": {"request": {"jobIds": [request["jobIds"][1], request["jobIds"][0], request["jobIds"][2]]}}})
        assert bad["result"]["isError"]
        duplicate = c.request("tools/call", {"name": "geod_rgb_plan", "arguments": {"request": {"jobIds": [request["jobIds"][0]] * 3}}})
        assert duplicate["error"]["code"] == -32602
        report["cases"].append({"mode": "direct-read-only", "tools": tools, "savedFileWithoutParents": saved, "wrongOrderRejected": True, "duplicateIdsRejected": True, "writesDenied": True})
    with client(exe, data_dir=root, allow_write=True) as c:
        tools = discovery(c, True)
        submitted = c.call("geod_rgb_run", {"request": {**request, "name": "QA · direct MCP scientific RGB"}})
        assert submitted["poll"]["tool"] == "geod_job_status"
        direct = settled(c, submitted["jobId"])
        result = inspect(c, direct)
        package = packaged(c, direct)
        report["cases"].append({"mode": "direct-write", "tools": tools, "result": result, "package": package, "settled": True})
    with client(exe, data_dir=root) as c:
        assert inspect(c, direct)["sha256"] == direct["sha256"]
    report["directEofReopen"] = True

    base = f"http://127.0.0.1:{args.port}"
    process = subprocess.Popen([exe, "serve", "--data-dir", str(root), "--port", str(args.port)], stdout=(root / "mcp-runtime.stdout.log").open("ab"), stderr=(root / "mcp-runtime.stderr.log").open("ab"), creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    try:
        for _ in range(100):
            assert process.poll() is None, (root / "mcp-runtime.stderr.log").read_text(encoding="utf-8")
            try:
                req = urllib.request.Request(base + "/health", headers={"X-GeoD-Client": "geod-global"})
                with urllib.request.urlopen(req, timeout=3) as response:
                    assert Path(json.load(response)["storageRoot"]).samefile(root)
                break
            except OSError:
                time.sleep(.1)
        else:
            raise AssertionError("Isolated MCP service did not start")
        with client(exe, server=base) as c:
            discovery(c, False)
            c.call("geod_rgb_plan", {"request": request})
            inspect(c, fixture["derived"])
        with client(exe, server=base, allow_write=True) as c:
            tools = discovery(c, True)
            submitted = c.call("geod_rgb_run", {"request": {**request, "name": "QA · loopback MCP scientific RGB"}})
            assert submitted["poll"]["tool"] == "geod_job_status"
            assert submitted["job"]["status"] in ("queued", "running")
        # Disconnect before settlement: the loopback task service still owns the job.
        with client(exe, server=base) as c:
            server_job = settled(c, submitted["jobId"])
            result = inspect(c, server_job)
        with client(exe, server=base, allow_write=True) as c:
            package = packaged(c, server_job)
        report["cases"].append({"mode": "loopback-write-and-reconnect", "tools": tools, "result": result, "package": package, "disconnectBeforeSettlement": True, "settled": True})
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=20)
    report["status"] = "passed"
    report["protocolOnlyStdoutAndCleanEof"] = True
    (root / "mcp-verification.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": report["status"], "cases": len(report["cases"]), "directEofReopen": True, "loopbackReconnect": True}))


if __name__ == "__main__":
    main()
