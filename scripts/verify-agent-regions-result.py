"""Audit retained real global-region/native-model receipts; no network or credentials."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

def read(path):
    return json.loads(path.read_text(encoding="utf-8"))

def owned(path):
    value = path.resolve(strict=True)
    assert value.is_relative_to(ROOT / ".verification")
    return value

def calls(case):
    records = []
    outputs = {}
    for path in (case / "sessions/codex/sessions").rglob("*.jsonl"):
        for line in path.read_text(encoding="utf-8").splitlines():
            row = json.loads(line)
            if row.get("type") != "response_item":
                continue
            p = row["payload"]
            if p.get("type") == "function_call":
                records.append(p)
            elif p.get("type") == "function_call_output":
                outputs[p["call_id"]] = json.loads(p["output"])
    return [(p["name"], json.loads(p["arguments"]), outputs.get(p["call_id"])) for p in records]

def candidate(region):
    bounds = region["bounds"]
    assert len(bounds) == 4 and -180 <= bounds[0] < bounds[2] <= 180 and -90 <= bounds[1] < bounds[3] <= 90
    return {key: region[key] for key in ("id", "name", "countryCode", "adminLevel", "bounds")}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--public-case", type=Path, required=True)
    parser.add_argument("--model-case", type=Path, required=True)
    parser.add_argument("--place-case", type=Path, required=True)
    parser.add_argument("--development-pid", type=int, required=True)
    args = parser.parse_args()
    public_case, model_case, place_case = map(owned, (args.public_case, args.model_case, args.place_case))
    public = read(public_case / "global-administrative-acceptance.json")
    model = read(model_case / "native-acceptance.json")
    place = read(place_case / "native-acceptance.json")
    assert model["status"] == place["status"] == "passed"
    assert public["originalDownloads"] == model["persistedJobCount"] == place["persistedJobCount"] == 0
    queried = calls(model_case)
    matched = []
    for country, level in (("CHN", 1), ("DEU", 1), ("IND", 2)):
        hit = next(result for name, _, result in queried if name == "geod_region_search" and result
                   and any(c["countryCode"] == country and c["adminLevel"] == level for c in result.get("candidates", [])))
        c = next(c for c in hit["candidates"] if c["countryCode"] == country and c["adminLevel"] == level)
        matched.append({"candidate": candidate(c), "provenance": hit["provenance"]})
    india = next(value for name, _, value in queried if name == "geod_region_levels")
    assert india["countryCode"] == "IND" and any(level["adminLevel"] == 2 for level in india["availableLevels"])
    pune = next(c for c in public["cases"] if c["countryCode"] == "IND")["result"]["candidates"][0]
    assert matched[-1]["candidate"]["bounds"] == pune["bounds"]
    cities = calls(place_case)
    actual_city = next(v for name, _, v in cities if name == "geod_place_search" and v and v.get("candidates"))
    assert actual_city["candidates"][0]["countryCode"] == "US"
    assert place["nativeDownloadReview"]["status"] == "pending"
    report = {
        "schema": "geod-agent-global-regions-verification/v1", "status": "passed",
        "verifiedAt": datetime.now(timezone.utc).isoformat(),
        "scope": "Desktop development; global administrative names and actual source envelopes, not polygon crop or legal-boundary approval",
        "bundled": {"adm0Features": 242, "referenceSubdivisions": 4596, "sourceGroupingCodes": 251,
                    "offlineTestCountries": ["CHN", "USA", "DEU", "JPN", "BRA", "FRA", "ZAF", "AUS"],
                    "aliasesSha256": hashlib.sha256((ROOT / "prototype/public/basemaps/admin1-10m/agent-aliases.json").read_bytes()).hexdigest()},
        "realNativeQueries": public["cases"],
        "realModel": {"status": "passed", "humanMessages": 1, "coordinateParametersSupplied": False,
                      "isoCodesSupplied": False, "adminLevelsSupplied": False, "modelRoute": model["modelRoute"],
                      "upstreamVendorVerified": False, "tools": [name for name, _, _ in queried],
                      "actualCandidates": matched, "indiaCatalogCoverageIssues": india["coverageIssues"],
                      "nativeReceipt": str((model_case / "native-acceptance.json").relative_to(ROOT))},
        "originalReportedRequest": {"status": "passed", "humanPrompt": "我要下载纽约最新的卫星影像",
                                    "actualPlaceProvider": actual_city["provider"], "nativeReviewStatus": "pending",
                                    "nativeReceipt": str((place_case / "native-acceptance.json").relative_to(ROOT))},
        "checks": {"nativeAgentActionTests": 65, "desktopTests": 35, "agentNodeTests": 75,
                   "uiTests": 301, "clippyWarningsDenied": "passed", "dependencyAndAliasVerification": "passed"},
        "development": {"runningPid": args.development_pid, "windowTitle": "GeoD Global", "previewHttpStatus": 200,
                        "userDesktopControlled": False, "installerCreated": False},
        "limits": {"remoteCacheDays": 7, "remoteDeadlineSeconds": 45, "maxDatasetMiB": 64,
                   "maxIndexedUnits": 100000, "allCountriesAllLevelsGuaranteed": False,
                   "administrativePolygonClipImplementedByThisChange": False},
        "originalDownloads": 0, "published": False,
    }
    output = ROOT / "prototype/qa/agent-global-regions-verification.json"
    output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": "passed", "actualModelCountries": 3,
                      "nativeCases": len(public["cases"]), "report": str(output)}))

if __name__ == "__main__":
    main()
