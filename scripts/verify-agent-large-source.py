"""Independently check a completed Agent NAIP/radar original and native reads.

This reuses an isolated live-test file. It makes no new provider request, model
call or approval, and uses the read-only MCP adapter for the same raster worker
that Agent calls. It does not establish a new model turn or product entitlement.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import math
from pathlib import Path

import rasterio
from rasterio.windows import Window


SPEC = importlib.util.spec_from_file_location("mcp_qa", Path(__file__).with_name("verify-mcp.py"))
qa = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(qa)


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", required=True, type=Path)
    parser.add_argument("--executable", type=Path, default=Path("target/debug/geod-runtime.exe"))
    parser.add_argument("--receipt", default="public-agent-acceptance.json")
    parser.add_argument("--server", help="Existing owned loopback QA runtime")
    args = parser.parse_args()
    root = args.run.resolve(strict=True)
    core = root / "core"
    assert Path(args.receipt).name == args.receipt and args.receipt.endswith(".json")
    receipt = json.loads((root / args.receipt).read_text(encoding="utf-8"))
    registry_path = core / "jobs.json"
    original_registry = json.loads(registry_path.read_text(encoding="utf-8"))
    if isinstance(receipt, list):
        assert len(receipt) == 1 and receipt[0]["provider"] in {"planetary-naip", "planetary-radar"}
        row = receipt[0]
        assert not row.get("preflightOnly") and row.get("fixture") is False
    else:
        assert receipt["schema"] == "geod-agent-resumed-public-acceptance/v1" and receipt["status"] == "passed"
        assert receipt["sameJob"] and receipt["sameApproval"] and receipt["noAdditionalJobs"]
        resumed = original_registry[receipt["jobId"]]
        assert resumed.get("agentApproval") and resumed["assetKey"] in {"vv", "vh", "hh", "hv", "aerial"}
        row = dict(receipt, provider="planetary-naip" if resumed["assetKey"] == "aerial" else "planetary-radar")
    job_id = row["jobId"]
    job = original_registry[job_id]
    assert job["status"] == "succeeded"
    source = Path(job["outputPath"]).resolve(strict=True)
    assert source.parent.samefile(core / "assets") and source.name == f"{job_id}.tif"
    checksum = digest(source)
    assert source.stat().st_size == row["bytes"] == job["bytesDownloaded"] == job["totalBytes"]
    assert checksum == row["sha256"] == job["sha256"]
    report = {
        "schema": "geod-agent-large-source-acceptance/v1", "status": "pending",
        "checkedAt": datetime.now(timezone.utc).isoformat(), "provider": row["provider"],
        "jobId": job_id, "bytes": source.stat().st_size, "sha256": checksum,
        "binarySha256": digest(args.executable), "modelCalls": 0, "newProviderRequests": 0,
        "usedUserDesktop": False, "fixture": False,
        "nativeReadAdapter": "Read-only MCP using the same native raster worker as Agent; not a model turn",
        "rasterio": rasterio.__version__, "gdal": rasterio.__gdal_version__, "samples": [],
    }
    client = None
    try:
        if args.server:
            ownership = json.loads((root / "resume-server.json").read_text(encoding="utf-8"))
            assert args.server == ownership["baseUrl"] and Path(ownership["ownedQaStore"]).samefile(core)
            client = qa.Client(args.executable, server=args.server)
        else:
            client = qa.Client(args.executable, data_dir=core)
        fresh_job = client.call("geod_job_status", {"id": job_id})
        assert fresh_job["status"] == "succeeded" and fresh_job["settled"]
        native = client.call("geod_raster_inspect", {"id": job_id})
        assert native["sha256"] == checksum and "previewDataUrl" not in native
        with rasterio.open(source) as dataset:
            assert (dataset.width, dataset.height, dataset.count) == (native["width"], native["height"], native["bandCount"])
            assert dataset.crs.to_string() == native["crs"]
            assert all(math.isclose(a, b, abs_tol=1e-8) for a, b in zip(dataset.bounds, native["bounds"]))
            assert all(math.isclose(a, b, abs_tol=1e-8) for a, b in zip(dataset.res, native["pixelSize"]))
            assert dataset.nodata == native["nodata"] or (dataset.nodata is not None and math.isnan(dataset.nodata) and native["nodata"] is None)
            expected_type = "uint8" if row["provider"] == "planetary-naip" else "float32"
            assert all(dtype == expected_type for dtype in dataset.dtypes)
            assert native["dataType"] == ("UInt8" if expected_type == "uint8" else "Float32")
            report["raster"] = {"width": dataset.width, "height": dataset.height, "bands": dataset.count,
                                "crs": dataset.crs.to_string(), "type": expected_type, "pixelSize": list(dataset.res),
                                "bounds": list(dataset.bounds), "noData": native["nodata"]}
            if row["provider"] == "planetary-naip":
                assert dataset.count == 4 and native["aerial"]["bands"] == ["red", "green", "blue", "nir"]
            else:
                assert dataset.count == 1 and native["radar"]["product"] == "sentinel-1-iw-rtc"
            points = {(0, 0), (dataset.width - 1, 0), (0, dataset.height - 1),
                      (dataset.width - 1, dataset.height - 1), (dataset.width // 2, dataset.height // 2),
                      (dataset.width // 4, dataset.height // 4), (dataset.width * 3 // 4, dataset.height * 3 // 4)}
            for column, row_index in sorted(points):
                x, y = dataset.xy(row_index, column)
                decoded = dataset.read(window=Window(column, row_index, 1, 1))[:, 0, 0]
                actual = client.call("geod_raster_pixel", {"id": job_id, "x": x, "y": y})
                assert actual["pixel"] == [column, row_index] and actual["sha256"] == checksum
                assert actual["crs"] == dataset.crs.to_string()
                assert all(math.isclose(a, b, abs_tol=1e-8) for a, b in zip(actual["center"], [x, y]))
                if expected_type == "uint8":
                    values = [int(v) for v in decoded]
                    assert actual["values"] + [actual["nearInfrared"]] == values
                else:
                    value = float(decoded[0])
                    values = [None if not math.isfinite(value) else value]
                    assert actual["value"] == values[0]
                report["samples"].append({"column": column, "row": row_index, "center": [x, y], "values": values})
        client.close()
        client = None
        assert json.loads(registry_path.read_text(encoding="utf-8")) == original_registry
        assert digest(source) == checksum
        report.update(status="passed", sourceUnchanged=True, jobsUnchanged=True,
                      cleanObserverExit=True, externallyOwnedRuntime=bool(args.server))
    except Exception as error:
        report.update(status="failed", error=f"{type(error).__name__}: {error}")
        raise
    finally:
        if client is not None:
            client.close()
        (root / "independent-original.json").write_text(json.dumps(report, indent=2, allow_nan=False) + "\n", encoding="utf-8")
        print(json.dumps({key: report[key] for key in ["status", "provider", "jobId", "bytes", "sha256"]}))


if __name__ == "__main__":
    main()
