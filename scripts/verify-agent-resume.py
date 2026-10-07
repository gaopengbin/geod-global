"""Resume one stopped, approved public Agent download in its isolated QA store.

This explicitly uses the normal native task retry, not a model tool or a second
download plan. The existing core lock prevents opening a still-owned runtime.
Keep this process alive while its worker runs; an observation deadline is not
an instruction to restart the task. No credentials or user desktop are used.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
from pathlib import Path
import time


SPEC = importlib.util.spec_from_file_location("mcp_qa", Path(__file__).with_name("verify-mcp.py"))
qa = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(qa)


def write_receipt(path, value):
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n", encoding="utf-8")
    temporary.replace(path)


def sha256(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", required=True, type=Path)
    parser.add_argument("--id", required=True)
    parser.add_argument("--executable", type=Path, default=Path("target/debug/geod-runtime.exe"))
    parser.add_argument("--server", help="Existing loopback runtime owning the isolated QA store")
    parser.add_argument("--receipt", default="resumed-agent-acceptance.json")
    args = parser.parse_args()
    workspace = Path(__file__).resolve().parent.parent
    root = args.run.resolve(strict=True)
    assert root.parent.samefile(workspace / ".verification"), "Only a direct isolated QA run is accepted"
    assert root.name.startswith("agent-public-"), "Use an existing public Agent acceptance run"
    core = root / "core"
    registry = core / "jobs.json"
    before = json.loads(registry.read_text(encoding="utf-8"))
    original = before[args.id]
    assert original["id"] == args.id and original["status"] in {"running", "queued", "interrupted", "failed"}
    assert original.get("agentApproval"), "The original job must have a native Agent approval"
    assert original["assetKey"] in {"vv", "vh", "hh", "hv", "aerial"}
    partial = core / "assets" / (args.id + ".part")
    assert partial.is_file() and partial.stat().st_size > 0
    assert partial.stat().st_size < original["totalBytes"]
    assert Path(args.receipt).name == args.receipt and args.receipt.endswith(".json")
    receipt_path = root / args.receipt
    assert not receipt_path.exists(), "Preserve previous resume evidence; inspect it before retrying"
    report = {
        "schema": "geod-agent-resumed-public-acceptance/v1",
        "status": "pending", "startedAt": datetime.now(timezone.utc).isoformat(),
        "jobId": args.id, "originalStatus": original["status"],
        "partialBytesBefore": partial.stat().st_size, "expectedBytes": original["totalBytes"],
        "binarySha256": sha256(args.executable), "modelCalls": 0,
        "usedUserDesktop": False, "newPlanCreated": False,
        "execution": "Explicit normal task retry of the same native Agent-approved job",
    }
    client = None
    try:
        if args.server:
            ownership = json.loads((root / "resume-server.json").read_text(encoding="utf-8"))
            assert args.server == ownership["baseUrl"]
            assert Path(ownership["ownedQaStore"]).samefile(core)
            client = qa.Client(args.executable, server=args.server, allow_write=True)
            report["runtimeOwnership"] = ownership
        else:
            client = qa.Client(args.executable, data_dir=core, allow_write=True)
        recovered = client.call("geod_job_status", {"id": args.id})
        assert recovered["status"] in {"interrupted", "failed"} and recovered["settled"]
        report["recoveredStatus"] = recovered["status"]
        write_receipt(receipt_path, report)
        client.call("geod_job_retry", {"id": args.id})
        print(json.dumps({"jobId": args.id, "nativeProcessId": client.process.pid,
                          "partialBytesBefore": report["partialBytesBefore"]}), flush=True)
        observed = time.monotonic()
        while True:
            job = client.call("geod_job_status", {"id": args.id})
            assert job["id"] == args.id
            if job["settled"] and job["status"] not in {"running", "queued"}:
                assert job["status"] == "succeeded", job.get("error", "Original transfer failed")
                break
            if time.monotonic() - observed >= 30:
                print(json.dumps({"jobId": args.id, "status": job["status"],
                                  "bytesDownloaded": job.get("bytesDownloaded")}), flush=True)
                observed = time.monotonic()
            time.sleep(2)
        raster = client.call("geod_raster_inspect", {"id": args.id})
        assert "previewDataUrl" not in raster
        client.close()
        client = None
        after = json.loads(registry.read_text(encoding="utf-8"))
        assert set(after) == set(before), "Retry must not create another task"
        result = after[args.id]
        assert result["agentApproval"] == original["agentApproval"]
        source = Path(result["outputPath"]).resolve(strict=True)
        assert source.parent.samefile(core / "assets") and source.name == args.id + ".tif"
        assert source.stat().st_size == result["bytesDownloaded"] == result["totalBytes"] == original["totalBytes"]
        checksum = sha256(source)
        assert checksum == result["sha256"] == raster["sha256"]
        assert result["attempts"] == original["attempts"] + 1
        report.update(status="passed", finishedAt=datetime.now(timezone.utc).isoformat(),
                      bytes=source.stat().st_size, sha256=checksum, raster=raster,
                      sameJob=True, sameApproval=True, noAdditionalJobs=True,
                      cleanObserverExit=True, externallyOwnedRuntime=bool(args.server),
                      attempts=result["attempts"])
    except Exception as error:
        report.update(status="failed", error=f"{type(error).__name__}: {error}")
        raise
    finally:
        if client is not None:
            client.close()
        write_receipt(receipt_path, report)
        print(json.dumps({key: report.get(key) for key in ["status", "jobId", "bytes", "sha256"]}), flush=True)


if __name__ == "__main__":
    main()
