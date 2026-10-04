"""Verify actual MOD13Q1/MYD13Q1 files through both read-only MCP transports.

Run against a closed private store accepted by verify-vegetation.py. The
upstream proxy remains blocked; no external account or user window is used.
"""
import argparse
import copy
import hashlib
import importlib.util
import json
import subprocess
import time
import urllib.request
from pathlib import Path

spec = importlib.util.spec_from_file_location("geod_verify_mcp", Path(__file__).with_name("verify-mcp.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--port", type=int, default=4633)
    args = parser.parse_args()
    root = args.root.resolve()
    science = root.name.startswith("modis-science-")
    quality = root.name.startswith("modis-vi-quality-")
    assert root.parent == Path(".verification").resolve() and (root.name.startswith("modis-vegetation-") or science or quality)
    accepted = json.loads((root / "verification.json").read_text(encoding="utf-8"))
    assert accepted["status"] == "passed"
    binary = root / f"runtime-{accepted['nativeBinarySha256'][:16]}.exe"
    assert hashlib.sha256(binary.read_bytes()).hexdigest() == accepted["nativeBinarySha256"]
    assert json.loads((root / "proxy-settings.json").read_text())["url"] == "http://127.0.0.1:9"
    cases = accepted["outputs"] if quality else accepted["originals"] + accepted["outputs"]
    if quality:
        assert len(cases) == 20 and accepted["pairedSelectionsIdentical"]
    elif science:
        assert len(cases) == 70 and len(accepted["originals"]) == 30
    elif accepted.get("originalReference"):
        assert len(cases) == 2 and [entry["name"] for entry in accepted["cases"]] == ["temporal"]
    else:
        assert len(cases) == 12
    report = {"schema": "geod-modis-vi-quality-mcp/v1" if quality else "geod-modis-science-mcp/v1" if science else "geod-modis-vegetation-mcp/v1", "nativeBinarySha256": accepted["nativeBinarySha256"],
              "nativeReceiptSha256": hashlib.sha256((root / "verification.json").read_bytes()).hexdigest(),
              "readOnly": True, "upstreamBlocked": True, "cases": []}

    def inspect(client, mode):
        tools = client.request("tools/list")["result"]["tools"]
        names = {tool["name"] for tool in tools}
        assert "geod_recipe_run" not in names and "geod_download" not in names
        denied = client.request("tools/call", {"name": "geod_recipe_run", "arguments": {}})
        assert denied["error"]["code"] == -32602
        for name in ["geod_raster_inspect", "geod_raster_pixel"]:
            tool = next(tool for tool in tools if tool["name"] == name)
            assert tool["annotations"]["readOnlyHint"]
            assert ("countsFullResolution=false" if science and name == "geod_raster_inspect" else "signed Int8 reliability" if science else "NDVI" if name == "geod_raster_inspect" else "indexValue") in tool["description"]
        for entry in accepted["cases"]:
            assert client.call("geod_project_get", {"id": entry["project"]["id"]}) == entry["project"]
        for entry in cases:
            job = entry["job"]
            expected = copy.deepcopy(entry["metadata"])
            del expected["previewDataUrl"]
            expected["previewOmitted"] = True
            assert client.call("geod_raster_inspect", {"id": job["id"]}) == expected
            assert "reflectance" not in expected
            if science:
                assert expected["science"]["band"] == job["assetKey"] and not expected["science"]["countsFullResolution"]
            else:
                assert expected["vegetation"]["index"] == job["assetKey"]
            if quality:
                assert expected["vegetation"]["qualitySelection"] == job["mosaicOutput"]["viQuality"]
                assert expected["vegetation"]["qualitySelection"]["countsFullResolution"]
            for pixel in entry["pixels"]:
                x, y = pixel["coordinate"]
                assert client.call("geod_raster_pixel", {"id": job["id"], "x": x, "y": y}) == pixel
            report["cases"].append({"mode": mode, "case": entry.get("case", "original"), "assetKey": job["assetKey"],
                                    "jobId": job["id"], "sha256": job["sha256"], "pixelsCompared": len(entry["pixels"]),
                                    ("originalScienceDnUnitsDatesAndFlagsRetained" if science else "signedDnAndScaledIndexRetained"): True, "noDataIsSeparate": True, "previewOmitted": True})
            print(json.dumps({"mode": mode, "id": job["id"], "key": job["assetKey"]}), flush=True)

    client = module.Client(str(binary), data_dir=root)
    try:
        inspect(client, "direct")
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
        inspect(client, "loopback adapter")
        runtime.terminate()
        runtime.wait(timeout=15)
        failure = client.request("tools/call", {"name": "geod_raster_inspect", "arguments": {"id": cases[-1]["job"]["id"]}})
        assert failure["result"]["isError"]
        report["disconnectedAdapterRejected"] = True
    finally:
        if client:
            client.close()
        if runtime.poll() is None:
            runtime.terminate()
            runtime.wait(timeout=15)
    report.update(status="passed", pixelsCompared=sum(entry["pixelsCompared"] for entry in report["cases"]),
                  readOnlyWriteDeniedInBothModes=True, protocolOnlyStdoutAndCleanExit=True)
    (root / "mcp-verification.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({key: report[key] for key in ["status", "pixelsCompared", "disconnectedAdapterRejected"]}))


if __name__ == "__main__":
    main()
