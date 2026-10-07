"""Small bounded readers shared by the actual-profile receipt audit."""
import json


def owned_json(root, path):
    actual = path.resolve(strict=True)
    assert actual.is_relative_to(root / ".verification")
    assert actual.stat().st_size < 8 * 1024 * 1024
    return actual, json.loads(actual.read_text(encoding="utf-8"))


def model_calls(case):
    rows, outputs = [], {}
    for path in (case / "sessions/codex/sessions").rglob("*.jsonl"):
        for line in path.read_text(encoding="utf-8").splitlines():
            row = json.loads(line)
            if row.get("type") != "response_item":
                continue
            item = row["payload"]
            if item.get("type") == "function_call":
                rows.append(item)
            elif item.get("type") == "function_call_output":
                outputs[item["call_id"]] = json.loads(item["output"])
    return [(row["name"], json.loads(row["arguments"]), outputs.get(row["call_id"], {})) for row in rows]
