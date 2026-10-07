"""Native actual-profile test after its immediate launch process has exited.

Public lookup/cache requests only. No model, user conversation, credential-vault
write, installer, desktop automation or new download task.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-binary", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True, help="Explicit stopped app runtime")
    args = parser.parse_args()
    source = args.test_binary.resolve(strict=True)
    assert source.is_relative_to(ROOT / "target/debug/deps")
    runtime = args.runtime.resolve(strict=True)
    assert runtime.is_dir()
    base = ROOT / ".verification" / ("detached-profile-" + str(time.time_ns()))
    base.mkdir()
    native = base / "native-tests.exe"
    shutil.copy2(source, native)
    script = base / "detached-launcher.ps1"
    script.write_text("""$ErrorActionPreference='Stop'
$geodDetachedNative=Start-Process -FilePath $env:GEOD_DETACHED_TEST_BINARY -ArgumentList @('--exact','agent::place_model_tests::live_native_system_proxy_and_profile_storage','--ignored','--nocapture') -WorkingDirectory $env:GEOD_DETACHED_WORKSPACE -WindowStyle Hidden -RedirectStandardOutput $env:GEOD_DETACHED_STDOUT -RedirectStandardError $env:GEOD_DETACHED_STDERR -PassThru
@{nativePid=$geodDetachedNative.Id;parentPid=$PID} | ConvertTo-Json -Compress | Set-Content -LiteralPath $env:GEOD_DETACHED_PIDS -Encoding UTF8
""", encoding="utf-8")
    env = os.environ.copy()
    env.update(GEOD_AGENT_SYSTEM_QA=str(base), GEOD_AGENT_SYSTEM_CORE=str(runtime),
               GEOD_AGENT_SYSTEM_START_GATE="1", GEOD_DETACHED_TEST_BINARY=str(native),
               GEOD_DETACHED_WORKSPACE=str(ROOT), GEOD_DETACHED_STDOUT=str(base / "native.stdout.log"),
               GEOD_DETACHED_STDERR=str(base / "native.stderr.log"), GEOD_DETACHED_PIDS=str(base / "pids.json"))
    launcher = subprocess.Popen(["powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(script)],
                                env=env, cwd=ROOT, creationflags=subprocess.CREATE_NO_WINDOW,
                                stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    code = launcher.wait(timeout=15)
    assert code == 0, "detached test launcher failed"
    pids = json.loads((base / "pids.json").read_text(encoding="utf-8-sig"))
    assert pids["parentPid"] == launcher.pid
    receipt = {"schema": "geod-detached-profile-launch/v1", "startedAt": datetime.now(timezone.utc).isoformat(),
               "parentExitedBeforeAnyNativeQuery": True, "parentExitCode": code, **pids,
               "nativeTestsSha256": hashlib.sha256(native.read_bytes()).hexdigest(),
               "modelRequests": 0, "userDesktopControlled": False, "published": False}
    (base / "start-gate").write_text("Parent has exited. Begin native queries.\n", encoding="utf-8")
    print(json.dumps({"directory": str(base), "phase": "parent exited; actual-profile native test"}), flush=True)
    deadline = time.monotonic() + 90
    stdout = base / "native.stdout.log"
    while time.monotonic() < deadline:
        text = stdout.read_text(encoding="utf-8", errors="replace") if stdout.exists() else ""
        if "test result:" in text:
            break
        time.sleep(0.2)
    else:
        raise RuntimeError("Detached native test exceeded deadline; inspect owned process and logs")
    assert "test result: ok. 1 passed; 0 failed" in text, "detached actual-profile native test failed"
    probe = json.loads((base / "native-system-probe.json").read_text(encoding="utf-8"))
    assert all("Ok" in row["result"] for row in probe["checks"])
    assert probe["afterReopen"][0]["result"]["Ok"]["cached"] is True
    assert probe["afterReopen"][1]["result"]["Ok"]["provenance"]["cached"] is True
    receipt.update(status="passed", finishedAt=datetime.now(timezone.utc).isoformat(),
                   nativeReceipt="native-system-probe.json", queries=len(probe["checks"]), cacheAfterReopen=True)
    (base / "launch.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    (ROOT / ".verification/detached-profile-latest.json").write_text(json.dumps({"directory": str(base)}, indent=2), encoding="utf-8")
    print(json.dumps({"status": "passed", "directory": str(base), "parentExitedBeforeQueries": True}), flush=True)


if __name__ == "__main__":
    main()
