"""Read real MODIS QC/State originals and processed outputs through stdio MCP.

Run after the native and independent verifiers, with this isolated store closed.
Direct mode and the loopback adapter both use persisted real files, offline.
"""
import argparse
import hashlib
import importlib.util
import json
import subprocess
import time
import urllib.request
from pathlib import Path
import rasterio

spec = importlib.util.spec_from_file_location("geod_verify_mcp", Path(__file__).with_name("verify-mcp.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--port", type=int, default=4603)
    args = parser.parse_args()
    root = args.root.resolve()
    assert root.parent == Path(".verification").resolve() and root.name.startswith("modis-quality-processing-")
    source = json.loads((root / "native-processing-verification.json").read_text(encoding="utf-8"))
    independent = json.loads((root / "independent-processing-verification.json").read_text(encoding="utf-8"))
    assert source["status"] == independent["status"] == "passed"
    binary = Path("target/debug/geod-runtime.exe").resolve()
    assert hashlib.sha256(binary.read_bytes()).hexdigest() == source["nativeBinarySha256"]
    report = {"schema": "geod-modis-quality-processing-mcp/v1", "nativeBinarySha256": source["nativeBinarySha256"],
              "readOnly": True, "cases": []}
    jobs = {job["id"]: job for job in source["originals"]}
    cases = [{"case": "original", "key": jobs[q["jobId"]]["assetKey"], "job": jobs[q["jobId"]],
              "metadata": q["metadata"]} for q in source["qualityOriginals"]]
    cases.extend(entry for entry in source["outputs"] if entry["key"] in ["modis_qc", "modis_state"])
    assert len(cases) == 12

    def inspect(client, mode):
        tools = client.request("tools/list")["result"]["tools"]
        for name in ["geod_raster_inspect", "geod_raster_pixel"]:
            tool = next(tool for tool in tools if tool["name"] == name)
            assert tool["annotations"]["readOnlyHint"] and "MODIS" in tool["description"]
        for entry in source["cases"]:
            project = client.call("geod_project_get", {"id": entry["project"]["id"]})
            assert project == entry["project"]
        for entry in cases:
            job, key = entry["job"], entry["key"]
            metadata = client.call("geod_raster_inspect", {"id": job["id"]})
            assert metadata["sha256"] == job["sha256"] and metadata["quality"] == entry["metadata"]["quality"]
            assert metadata["classes"] == entry["metadata"]["classes"]
            assert metadata["previewOmitted"] and "previewDataUrl" not in metadata
            if entry["case"] != "original":
                pixels = entry["pixels"]
                for expected in pixels:
                    x, y = expected["coordinate"]
                    assert client.call("geod_raster_pixel", {"id": job["id"], "x": x, "y": y}) == expected
                compared = len(pixels)
            else:
                path = Path(job["outputPath"].removeprefix("\\\\?\\")).resolve()
                assert path.parent == root / "assets"
                with rasterio.open(path) as raster:
                    for col, row in [(0, 0), (1200, 1200), (2399, 2399)]:
                        raw = int(raster.read(1, window=rasterio.windows.Window(col, row, 1, 1))[0, 0])
                        x, y = raster.transform * (col + .5, row + .5)
                        pixel = client.call("geod_raster_pixel", {"id": job["id"], "x": x, "y": y})
                        assert pixel["value"] == raw and pixel["pixel"] == [col, row] and pixel["isNoData"] == (raw == raster.nodata)
                        assert int(pixel["quality"]["hex"], 16) == raw and int(pixel["quality"]["binary"], 2) == raw
                        assert len(pixel["quality"]["binary"]) == (32 if key == "modis_qc" else 16)
                        fields = pixel["quality"]["fields"]
                        assert len(fields) == (0 if pixel["isNoData"] else 10 if key == "modis_qc" else 11)
                        for field in fields:
                            assert field["value"] == (raw >> field["startBit"]) & ((1 << (field["endBit"] - field["startBit"] + 1)) - 1)
                compared = 3
            report["cases"].append({"mode": mode, "case": entry["case"], "assetKey": key, "sha256": job["sha256"],
                                    "pixelsCompared": compared, "previewOmitted": True, "fullUnsignedFieldsRetained": True})

    client = module.Client(str(binary), data_dir=root)
    try:
        inspect(client, "direct read-only; rejected upstream proxy")
    finally:
        client.close()
    base = f"http://127.0.0.1:{args.port}"
    runtime = subprocess.Popen([str(binary), "serve", "--data-dir", str(root), "--port", str(args.port)],
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                               creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    client = None
    try:
        for i in range(100):
            try:
                with urllib.request.urlopen(base + "/health", timeout=2) as response:
                    assert response.status == 200
                break
            except Exception:
                assert runtime.poll() is None
                if i == 99:
                    raise
                time.sleep(.1)
        client = module.Client(str(binary), server=base)
        inspect(client, "loopback read-only; rejected upstream proxy")
        runtime.terminate()
        runtime.wait(timeout=10)
        failed = client.request("tools/call", {"name": "geod_raster_inspect", "arguments": {"id": source["outputs"][3]["job"]["id"]}})
        assert failed["result"]["isError"]
        report["disconnectedAdapterRejected"] = True
    finally:
        if client:
            client.close()
        if runtime.poll() is None:
            runtime.terminate()
            runtime.wait(timeout=10)
    report.update(status="passed", pixelsCompared=sum(entry["pixelsCompared"] for entry in report["cases"]))
    (root / "mcp-processing-verification.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({key: report[key] for key in ["status", "pixelsCompared", "disconnectedAdapterRejected"]}))


if __name__ == "__main__":
    main()
