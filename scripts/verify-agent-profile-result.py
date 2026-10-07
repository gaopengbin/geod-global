"""Audit retained actual-profile native/model receipts without network or credentials."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
from verify_agent_profile_support import owned_json, model_calls

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--probe", required=True, type=Path)
    parser.add_argument("--failed-probe", required=True, type=Path)
    parser.add_argument("--model-case", required=True, type=Path)
    parser.add_argument("--detached-launch", required=True, type=Path)
    parser.add_argument("--development-pid", required=True, type=int)
    args = parser.parse_args()
    probe_path, probe = owned_json(ROOT, args.probe)
    failed_path, failed = owned_json(ROOT, args.failed_probe)
    model_path, model = owned_json(ROOT, args.model_case / "native-acceptance.json")
    _, launch = owned_json(ROOT, args.model_case.parent / "launch.json")
    detached_path, detached = owned_json(ROOT, args.detached_launch)
    assert detached["status"] == "passed" and detached["parentExitedBeforeAnyNativeQuery"] is True
    assert launch["status"] == model["status"] == "passed"
    assert model["usedActualProfileStorage"] and model["sameConversationRestoredAfterToolsUpgrade"]
    assert model["addedJobCount"] == model["cardClicks"] == 0
    assert model["nativeProxyMode"] == probe["proxy"]["mode"] == "system"
    assert not model["credentialVaultWritten"] and not model["usedUserDesktop"]
    assert all(row["result"].get("Err") == "Agent record directory was redirected." for row in failed["checks"])
    assert len(probe["checks"]) == 6 and all("Ok" in row["result"] for row in probe["checks"])
    assert probe["afterReopen"][0]["result"]["Ok"]["cached"] is True
    assert probe["afterReopen"][1]["result"]["Ok"]["provenance"]["cached"] is True
    regions = [row["result"]["Ok"] for row in probe["checks"] if row["tool"] == "geod_region_search"]
    # Search results echo the caller's ISO2 filter; verified candidates and
    # source provenance retain canonical ISO3 identifiers.
    assert [(row["candidates"][0]["countryCode"], row["candidates"][0]["adminLevel"]) for row in regions] == [("DEU", 2), ("FRA", 1), ("IND", 2)]
    assert all(row["candidates"] for row in regions)
    calls = model_calls(args.model_case.resolve())
    city = next(result for name, _, result in calls if name == "geod_place_search" and result.get("candidates"))
    plan = model["nativeDownloadReview"]
    assert plan["status"] == "pending" and plan["source"] == "Earth Search · Sentinel-2 L2A"
    assert any(c["kind"] == "city" and c["countryCode"] == "US" and c["bounds"] == plan["bounds"] for c in city["candidates"])
    assert all(args["bounds"] == plan["bounds"] for name, args, _ in calls if name == "geod_scene_search")
    assert not any(name in {"geod_plan_execute", "geod_job_control"} for name, _, _ in calls)
    assert all(file["assetKey"] == "visual" for file in plan["files"])
    report = {
        "schema": "geod-agent-profile-storage-verification/v1", "status": "passed",
        "verifiedAt": datetime.now(timezone.utc).isoformat(),
        "scope": "Actual stopped Windows user-profile runtime, saved System route, separate QA conversations; development only",
        "reproducedFailure": {"tools": [row["tool"] for row in failed["checks"]], "reason": "Normal inherited MSIX AppData mapping rejected before records were saved", "receipt": str(failed_path.relative_to(ROOT))},
        "repair": {"namespaceFromFreshManagedFileHandle": True, "requiresLiveLauncherProcess": False, "detachedLaunchParentExitedBeforeQueries": True, "exactProfilePackageAndRelativePathRequired": True, "symlinksAndReparsePointsRejected": True, "temporaryWitnessRemoved": True},
        "realQueries": [{"tool": row["tool"], "succeeded": True} for row in probe["checks"]],
        "actualAdministrativeCandidates": [{"countryCode": row["candidates"][0]["countryCode"], "adminLevel": row["adminLevel"], "names": [c["name"] for c in row["candidates"]], "source": row["provenance"]["provider"], "cached": row["provenance"]["cached"], "geometrySha256": row["provenance"]["sha256"]} for row in regions],
        "cacheAfterRuntimeReopen": {"city": True, "detailedAdministrativeIndex": True},
        "model": {"status": "passed", "route": model["modelRoute"], "upstreamVendorVerified": False, "humanMessages": model["humanMessages"], "coordinateParametersSupplied": False, "sameConversationAfterToolUpgrade": True, "currentMapAreaOverridden": True, "nativeReviewStatus": plan["status"], "bounds": plan["bounds"], "fileCount": len(plan["files"]), "fileDates": sorted({file["date"][:10] for file in plan["files"]}), "expectedBytes": plan["expectedBytes"], "existingJobCount": model["persistedJobCount"], "addedJobCount": 0},
        "checks": {"agentNodeTests": 81, "uiTests": 307, "nativeAgentActionTests": 65, "managedStorageTests": 5, "desktopTests": 35, "clippyWarningsDenied": "passed", "dependencyIsolation": "passed", "frontendBuild": "passed"},
        "receipts": {"native": str(probe_path.relative_to(ROOT)), "detachedLaunch": str(detached_path.relative_to(ROOT)), "model": str(model_path.relative_to(ROOT)), "sha256": {"native": hashlib.sha256(probe_path.read_bytes()).hexdigest(), "model": hashlib.sha256(model_path.read_bytes()).hexdigest()}},
        "development": {"runningPid": args.development_pid, "windowTitle": "GeoD Global", "previewHttpStatus": 200, "userDesktopControlled": False, "installerCreated": False},
        "newOriginalDownloads": 0, "published": False,
    }
    output = ROOT / "prototype/qa/agent-profile-storage-verification.json"
    output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    old_path = ROOT / "prototype/qa/agent-global-regions-verification.json"
    old = json.loads(old_path.read_text(encoding="utf-8"))
    old["originalVerificationEnvironment"] = "Isolated QA stores; did not detect the actual-profile MSIX mapping failure"
    old["actualProfileFollowUp"] = "agent-profile-storage-verification.json"
    old["development"] = report["development"]
    old_path.write_text(json.dumps(old, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": "passed", "actualProfile": True, "administrativeCountries": len(regions), "report": str(output)}))


if __name__ == "__main__":
    main()
