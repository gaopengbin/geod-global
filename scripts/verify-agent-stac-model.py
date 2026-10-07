"""Opt-in Windows live-model checks in fresh isolated STAC or vector stores.

Uses the existing downstream LAOGAO_API_KEY from the process or user environment,
an encrypted local SSH tunnel, and the repository-owned native test executable.
No installer, user desktop, credential-vault write or public release is involved.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import time
import urllib.request
import winreg


def listening(port):
    try:
        with socket.create_connection(("127.0.0.1", port), timeout=0.4):
            return True
    except OSError:
        return False


def downstream_key():
    value = os.environ.get("LAOGAO_API_KEY")
    if not value:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, "Environment") as environment:
            value = winreg.QueryValueEx(environment, "LAOGAO_API_KEY")[0]
    if not isinstance(value, str) or not value.strip():
        raise RuntimeError("Existing project downstream LAOGAO_API_KEY is required")
    return value


def request(path, key=None):
    headers = {"Authorization": "Bearer " + key} if key else {}
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    with opener.open(urllib.request.Request("http://127.0.0.1:19094" + path, headers=headers), timeout=30) as response:
        return json.load(response)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-binary", required=True, type=Path)
    parser.add_argument("--model", default="deepseek-v4-flash")
    parser.add_argument("--scenario", choices=("stac", "vector", "vector-plan", "wcs", "place", "place-restore", "regions"), default="stac")
    parser.add_argument("--vector-source", type=Path, help="Explicit public QA source contract, only for vector-plan; never a product default")
    parser.add_argument("--wcs-source", type=Path, help="Explicit public WCS QA service/coverage/area; required for wcs, never a product default")
    parser.add_argument("--source-direct", action="store_true", help="Use direct provider access in this isolated QA store; global proxy is unchanged")
    parser.add_argument("--source-system", action="store_true", help="Retain the native saved/default System provider route")
    parser.add_argument("--place-core", type=Path, help="Explicit stopped app runtime for place review acceptance; separate QA conversations, no job submission or credential changes")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    assert not (args.source_system and args.source_direct)
    if args.place_core:
        assert args.scenario.startswith("place") and args.source_system
        assert args.place_core.resolve().is_dir()
    source_case = None
    if args.scenario == "wcs":
        assert args.wcs_source and not args.vector_source
        source_case = json.loads(args.wcs_source.read_text(encoding="utf-8"))
        assert isinstance(source_case, dict) and set(source_case) == {"name", "url", "coverageId", "bounds"}
    else:
        assert not args.wcs_source
    if args.vector_source:
        assert args.scenario == "vector-plan"
        source_case = json.loads(args.vector_source.read_text(encoding="utf-8"))
        assert isinstance(source_case, dict) and set(source_case) == {"name","url","protocol","collectionId","bounds","pageSize","responseFormat"}
        assert source_case["protocol"] in {"OGC","ArcGIS","WFS2","Overpass"}
    source = args.test_binary.resolve(strict=True)
    assert source.is_relative_to(root / "target/debug/deps")
    base = root / ".verification" / ("agent-" + args.scenario + "-model-" + str(time.time_ns()))
    base.mkdir()
    owned = None
    process = None
    secret = downstream_key()
    receipt = {"schema": "geod-agent-" + args.scenario + "-model-launch/v1", "status": "pending", "scenario":args.scenario,
               "startedAt": datetime.now(timezone.utc).isoformat(), "modelRoute": args.model,
               "credentialInFilesOrArgs": False, "usedUserDesktop": False, "published": False}
    if source_case:
        receipt["sourceCase"] = source_case
    try:
        if not listening(19094):
            identity = Path.home() / ".ssh/laogao_tencent_ed25519"
            assert identity.is_file(), "Existing pinned SSH identity is required"
            owned = subprocess.Popen([
                "ssh.exe", "-N", "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=yes",
                "-o", "ConnectTimeout=15", "-o", "ExitOnForwardFailure=yes",
                "-o", "ServerAliveInterval=30", "-o", "ServerAliveCountMax=3",
                "-i", str(identity), "-L", "127.0.0.1:19094:127.0.0.1:9094",
                "ubuntu@62.234.147.130"], creationflags=subprocess.CREATE_NO_WINDOW,
                stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            receipt["ownedTunnelPid"] = owned.pid
            for _ in range(60):
                if listening(19094):
                    break
                if owned.poll() is not None:
                    raise RuntimeError("Owned SSH tunnel did not start")
                time.sleep(0.25)
            assert listening(19094), "Owned encrypted SSH tunnel is not listening"
        status = request("/api/status")
        pricing = request("/api/pricing")
        models = request("/v1/models", secret)
        assert any(item["id"] == args.model for item in models["data"]), "Requested route is absent from live model inventory"
        rows = [item for item in pricing.get("data", []) if item.get("model_name") == args.model]
        assert rows, "Requested route has no live consumer pricing entry"
        receipt["relay"] = {"publicStatusSuccess": status.get("success"), "authenticatedModelCount": len(models["data"]),
                            "requestedModelPresent": True, "pricing": [{key: value for key, value in row.items() if key in
                                {"model_name", "model_ratio", "completion_ratio", "model_price", "quota_type", "unit_price"}} for row in rows]}
        executable = base / "native-tests.exe"
        shutil.copy2(source, executable)
        receipt["nativeTestsSha256"] = hashlib.sha256(executable.read_bytes()).hexdigest()
        environment = os.environ.copy()
        environment.update(GEOD_AGENT_TEST_KEY=secret, GEOD_AGENT_TEST_BASE_URL="http://127.0.0.1:19094/v1",
                           GEOD_AGENT_TEST_MODEL=args.model)
        environment["GEOD_AGENT_" + ("REGIONS" if args.scenario == "regions" else "PLACE" if args.scenario.startswith("place") else "WCS" if args.scenario == "wcs" else "STAC" if args.scenario == "stac" else "VECTOR") + "_MODEL_QA"] = str(base / "case")
        if args.place_core:
            environment["GEOD_AGENT_PLACE_MODEL_CORE"] = str(args.place_core.resolve())
            receipt["usedActualProfileStorage"] = True
        if source_case:
            environment["GEOD_AGENT_" + ("WCS" if args.scenario == "wcs" else "VECTOR") + "_SOURCE_QA"] = json.dumps(source_case)
        if args.source_system:
            environment.pop("GEOD_AGENT_TEST_DIRECT", None)
            environment.pop("GEOD_AGENT_TEST_PROXY", None)
        elif args.source_direct:
            environment["GEOD_AGENT_TEST_DIRECT"] = "1"
            environment.pop("GEOD_AGENT_TEST_PROXY", None)
        elif "GEOD_AGENT_TEST_PROXY" not in environment and listening(7890):
            environment["GEOD_AGENT_TEST_PROXY"] = "http://127.0.0.1:7890"
        receipt["sourceProxyConfigured"] = bool(environment.get("GEOD_AGENT_TEST_PROXY"))
        receipt["isolatedSourceRoute"] = "saved-system" if args.source_system else "direct" if args.source_direct else "configured-proxy"
        print(json.dumps({"directory": str(base), "phase": "live model and native source workflow",
                          "modelRoute": args.model, "tunnelOwned": owned is not None}), flush=True)
        test_name = {
            "regions": "agent::region_model_tests::live_global_administrative_dialogue_without_coordinates",
            "place": "agent::place_model_tests::live_named_city_latest_download_review_without_parameters",
            "place-restore": "agent::place_model_tests::live_tool_upgrade_continues_same_conversation",
            "stac": "agent::stac_model_tests::live_custom_source_model_project_download_and_resume",
            "vector": "agent::vector_model_tests::live_vector_reads_and_resume",
            "vector-plan": "agent::vector_model_tests::live_vector_plan_and_resume",
            "wcs": "agent::wcs_model_tests::live_wcs_model_project_download_and_resume",
        }[args.scenario]
        process = subprocess.Popen([str(executable), "--exact", test_name, "--ignored", "--nocapture"],
            cwd=root, env=environment, creationflags=subprocess.CREATE_NO_WINDOW,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        receipt["nativeProcessPid"] = process.pid
        print(json.dumps({"ownedNativeProcess": process.pid}), flush=True)
        # Redact before either logging or printing; the key is never an argument.
        with (base / "native-test.log").open("w", encoding="utf-8") as log:
            for line in iter(process.stdout.readline, b""):
                text = line.decode("utf-8", errors="replace").replace(secret, "[redacted]")
                log.write(text)
                log.flush()
                sys.stdout.write(text)
                sys.stdout.flush()
        code = process.wait()
        receipt["nativeExit"] = code
        if code:
            raise RuntimeError("Live custom-source Agent acceptance failed; inspect the retained sanitized log")
        native = json.loads((base / "case/native-acceptance.json").read_text(encoding="utf-8"))
        assert native["status"] == "passed"
        if args.place_core:
            assert native["usedActualProfileStorage"] is True and native["addedJobCount"] == 0
            assert native["nativeProxyMode"] == "system"
        else:
            assert native["persistedJobCount"] == (1 if args.scenario in {"stac", "wcs"} else 0)
        if args.scenario.startswith("vector"):
            assert native["persistedVectorCount"] == 1 and native["modelPerformedExtraction"] is False
        if args.scenario == "vector-plan":
            assert native["modelPreparedExtractionPlan"] and native["nativeConfirmationPerformedExtraction"]
            assert native["humanRevisionRequiredSeparateConfirmation"] and native["harnessAcquiredPublicVector"] is False
        receipt["status"] = "passed"
        receipt["nativeAcceptance"] = "case/native-acceptance.json"
        (root / (".verification/agent-" + args.scenario + "-model-latest.json")).write_text(json.dumps({"directory": str(base)}, indent=2), encoding="utf-8")
    except Exception as error:
        receipt.update(status="failed", error=str(error).replace(secret, "[redacted]"))
        raise
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            process.wait(timeout=20)
            receipt["ownedNativeProcessStoppedAfterFailure"] = True
        if owned is not None:
            owned.terminate()
            owned.wait(timeout=20)
            receipt["ownedTunnelStopped"] = True
        receipt["finishedAt"] = datetime.now(timezone.utc).isoformat()
        (base / "launch.json").write_text(json.dumps(receipt, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps({"directory": str(base), "status": receipt["status"]}), flush=True)


if __name__ == "__main__":
    main()
