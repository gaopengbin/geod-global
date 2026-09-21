"""Check proposed design fixtures; this is not GeoD runtime or a data validator.

Uses an existing jsonschema installation. No network, file-content checks or
product data access. Only writes verification-result.json next to this script.
"""
from __future__ import annotations

import copy
import importlib.metadata
import json
import platform
from datetime import datetime, timezone
from pathlib import Path

from jsonschema import Draft202012Validator, FormatChecker

ROOT = Path(__file__).resolve().parent
SCHEMA = json.loads((ROOT / "draft-contract.schema.json").read_text(encoding="utf-8"))
Draft202012Validator.check_schema(SCHEMA)
VALIDATOR = Draft202012Validator(SCHEMA, format_checker=FormatChecker())
EXAMPLES = json.loads((ROOT / "examples.json").read_text(encoding="utf-8"))
BASE = EXAMPLES["records"]
TERMINAL = {"Succeeded", "Partial", "Failed", "Cancelled"}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def date(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def expected_outcome(job: dict) -> str:
    outputs = job["outputs"]
    valid = [output for output in outputs if output["status"] == "valid"]
    if not valid:
        return "Failed"
    if len(valid) == len(outputs):
        return "Succeeded"
    if job["outputPolicy"] == "all_required" and any(
        output["required"] and output["status"] != "valid" for output in outputs
    ):
        return "Failed"
    return "Partial"


def validate(records: list[dict]) -> None:
    for record in records:
        VALIDATOR.validate(record)
    ids = [record["id"] for record in records]
    require(len(ids) == len(set(ids)), "record IDs must be unique")
    by_id = {record["id"]: record for record in records}
    for record in records:
        kind = record["kind"]
        if kind == "Area":
            geometry = record["geometry"]
            polygons = [geometry["coordinates"]] if geometry["type"] == "Polygon" else geometry["coordinates"]
            for polygon in polygons:
                for ring in polygon:
                    require(ring[0] == ring[-1], "polygon ring must be closed")
                    require(len({tuple(position) for position in ring[:-1]}) >= 3, "ring requires three distinct vertices")
        elif kind == "Recipe":
            require(by_id.get(record["areaId"], {}).get("kind") == "Area", "recipe area reference is missing")
            for field in ("inputs", "outputs"):
                item_ids = [item["id"] for item in record[field]]
                require(len(item_ids) == len(set(item_ids)), f"duplicate recipe {field} IDs")
        elif kind == "Job":
            recipe = by_id.get(record["recipeId"], {})
            require(recipe.get("kind") == "Recipe", "job recipe reference is missing")
            require(record["outputPolicy"] == recipe["outputPolicy"], "job output policy differs from frozen recipe")
            expected = {output["id"]: output for output in recipe["outputs"]}
            actual = {output["id"]: output for output in record["outputs"]}
            require(len(actual) == len(record["outputs"]), "duplicate job output IDs")
            require(set(expected) == set(actual), "job must account for every declared output")
            for output_id, output in actual.items():
                require(output["required"] == expected[output_id]["required"], "job cannot downgrade required output")
                if output["status"] == "valid":
                    artifact = by_id.get(output["artifactId"], {})
                    require(artifact.get("kind") == "Artifact", "valid output requires an artifact")
                    require(artifact["jobId"] == record["id"] and artifact["outputId"] == output_id, "artifact must belong to job output")
                    require(date(artifact["committedAt"]) <= date(record["updatedAt"]), "job cannot finish before artifact commit")
            artifact_ids = [output["artifactId"] for output in actual.values() if output["status"] == "valid"]
            require(len(artifact_ids) == len(set(artifact_ids)), "one artifact cannot satisfy distinct outputs")
            require(set(artifact_ids) == set(record["artifactIds"]), "job artifact index differs from valid outputs")
            require(date(record["updatedAt"]) >= date(record["createdAt"]), "job timestamps are reversed")
            if record["state"] in TERMINAL and record["state"] != "Cancelled":
                require(record["state"] == expected_outcome(record), "terminal state misrepresents declared outputs")
            if "retryOf" in record:
                previous = by_id.get(record["retryOf"], {})
                require(previous.get("kind") == "Job" and previous["state"] in TERMINAL, "retry requires an existing terminal job")
                require(record["id"] != previous["id"] and record["attempt"] == previous["attempt"] + 1, "retry must be a new incremented attempt")
        elif kind == "Artifact":
            job = by_id.get(record["jobId"], {})
            require(job.get("kind") == "Job", "artifact job reference is missing")
            require(any(output["id"] == record["outputId"] and output.get("artifactId") == record["id"] for output in job["outputs"]), "orphan artifact")
            recipe = by_id[job["recipeId"]]
            input_ids = {item["id"] for item in recipe["inputs"]}
            require(set(record["provenance"]["inputIds"]).issubset(input_ids), "artifact provenance contains unknown input")
            paths = [item["relativePath"] for item in record["files"]]
            require(len(paths) == len(set(paths)), "duplicate artifact paths")
            require(date(record["validation"]["checkedAt"]) <= date(record["committedAt"]), "artifact commit predates validation")


def records_by_kind(records: list[dict]) -> dict:
    return {record["kind"]: record for record in records}


def scenario(name: str, mutation=None, should_pass: bool = True) -> dict:
    records = copy.deepcopy(BASE)
    if mutation:
        mutation(records, records_by_kind(records))
    try:
        validate(records)
        require(should_pass, f"negative case was accepted: {name}")
    except Exception as error:
        if should_pass or str(error).startswith("negative case was accepted"):
            raise
        return {"name": name, "expected": "reject", "result": "passed", "reason": str(error).splitlines()[0]}
    return {"name": name, "expected": "accept", "result": "passed"}


def required_failure(records, groups):
    groups["Recipe"]["outputs"][1]["required"] = True
    groups["Job"]["outputs"][1]["required"] = True
    groups["Job"]["state"] = "Failed"


def best_effort(records, groups):
    required_failure(records, groups)
    groups["Recipe"]["outputPolicy"] = groups["Job"]["outputPolicy"] = "best_effort"
    groups["Job"]["state"] = "Partial"


def all_success(records, groups):
    artifact = copy.deepcopy(groups["Artifact"])
    artifact["id"] = "artifact-report"
    artifact["outputId"] = "output-report"
    artifact["files"][0]["relativePath"] = "fixture/report.json"
    records.append(artifact)
    groups["Job"]["state"] = "Succeeded"
    groups["Job"]["outputs"][1] = {"id": "output-report", "required": False, "status": "valid", "artifactId": artifact["id"]}
    groups["Job"]["artifactIds"].append(artifact["id"])


def none_valid(records, groups):
    records.remove(groups["Artifact"])
    groups["Job"]["state"] = "Failed"
    groups["Job"]["artifactIds"] = []
    groups["Job"]["outputs"][0] = {"id": "output-imagery", "required": True, "status": "failed", "error": {"code": "VALIDATION_FAILED", "message": "Fixture failure", "retryable": False}}


def false_partial(records, groups):
    required_failure(records, groups)
    groups["Job"]["state"] = "Partial"


results = [
    scenario("optional output failure produces Partial"),
    scenario("all declared outputs valid produces Succeeded", all_success),
    scenario("required failure retains good artifact but produces Failed", required_failure),
    scenario("explicit best effort permits independent partial delivery", best_effort),
    scenario("zero valid outputs produces Failed", none_valid),
    scenario("cancelled job may retain committed artifact", lambda r, g: g["Job"].update(state="Cancelled")),
    scenario("future schema version rejected", lambda r, g: g["Area"].update(schemaVersion="1.0.0"), False),
    scenario("undeclared secret field rejected", lambda r, g: g["Recipe"].update(apiKey="invented-secret"), False),
    scenario("incomplete result cannot claim success", lambda r, g: g["Job"].update(state="Succeeded"), False),
    scenario("required failure cannot silently become Partial", false_partial, False),
    scenario("missing output cannot be removed from accounting", lambda r, g: g["Job"]["outputs"].pop(), False),
    scenario("required intent cannot be downgraded", lambda r, g: g["Job"]["outputs"][0].update(required=False), False),
    scenario("valid output needs an existing artifact", lambda r, g: r.remove(g["Artifact"]), False),
    scenario("output path traversal rejected", lambda r, g: g["Artifact"]["files"][0].update(relativePath="../outside.tif"), False),
    scenario("invalid latitude rejected", lambda r, g: g["Area"]["geometry"]["coordinates"][0][1].__setitem__(1, 95), False),
    scenario("unclosed polygon rejected", lambda r, g: g["Area"]["geometry"]["coordinates"][0][-1].__setitem__(0, 13.01), False),
    scenario("job clock reversal rejected", lambda r, g: g["Job"].update(updatedAt="2026-09-20T00:00:00Z"), False),
    scenario("artifact cannot commit before validation", lambda r, g: g["Artifact"].update(committedAt="2026-09-21T00:00:30Z"), False),
    scenario("duplicate output IDs rejected", lambda r, g: g["Recipe"]["outputs"][1].update(id="output-imagery"), False),
    scenario("provenance requires declared input", lambda r, g: g["Artifact"]["provenance"].update(inputIds=["unknown-input"]), False),
]
report = {
    "scope": "proposed contract fixtures only; no GeoD runtime, real artifacts or spatial topology verification",
    "checkedAt": datetime.now(timezone.utc).isoformat(),
    "schemaVersion": "0.1.0-proposed.1",
    "pythonVersion": platform.python_version(),
    "jsonschemaVersion": importlib.metadata.version("jsonschema"),
    "schemaCheck": "passed", "baseRecordCount": len(BASE),
    "acceptedCases": sum(result["expected"] == "accept" for result in results),
    "rejectedCases": sum(result["expected"] == "reject" for result in results),
    "results": results,
}
(ROOT / "verification-result.json").write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
print(json.dumps({key: report[key] for key in ("schemaCheck", "baseRecordCount", "acceptedCases", "rejectedCases", "scope")}))
