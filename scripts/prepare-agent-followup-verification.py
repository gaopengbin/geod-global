"""Copy only the selected failing conversation and metadata into isolated QA.

No user store writes, credentials, original imagery or download tasks. The
native acceptance reads the existing credential vault without exporting keys.
"""
import json
import os
from pathlib import Path
import shutil
import time
import sys

root = Path(__file__).resolve().parents[1]
profile = Path(os.environ["LOCALAPPDATA"]) / "xyz.laogao.geod.global"
history = json.loads((profile / "agent/sessions.json").read_text(encoding="utf-8"))
session = next(s for s in history["sessions"] if s["id"] == history["selectedId"])
assert session["status"] not in ("running", "starting", "stopping")
pending_boundary = '--pending-boundary' in sys.argv[1:]
if pending_boundary:
    assert any(e.get('decision', {}).get('status') == 'pending'
               and len(e['decision']['questions']) == 1
               and e['decision']['questions'][0]['id'] == 'crop_area' for e in session['entries'])
    assert session.get('executionMode', 'confirm-each') == 'confirm-each'
else:
    assert session["error"] in (
    "The model returned an empty response. Please retry.",
    "The model reached its output limit before completing the response. Please retry.",
    )
target = root / ".verification" / f"agent-crop-followup-{time.time_ns()}"
(target / "agent/codex/sessions").mkdir(parents=True)
(target / "core").mkdir()
(target / "agent/sessions.json").write_text(json.dumps({
    "version": 1, "selectedId": session["id"], "sessions": [session],
}, ensure_ascii=False), encoding="utf-8")
shutil.copy2(profile / "agent/registry.json", target / "agent/registry.json")
files = list((profile / "agent/codex/sessions").rglob(f"*{session['threadId']}*.jsonl"))
assert len(files) == 1
relative = files[0].relative_to(profile / "agent/codex/sessions")
(target / "agent/codex/sessions" / relative.parent).mkdir(parents=True)
shutil.copy2(files[0], target / "agent/codex/sessions" / relative)
for directory in ("agent-plans", "agent-searches", "agent-places", "agent-boundaries"):
    source = profile / "runtime" / directory
    (target / "core" / directory).mkdir()
    for file in source.glob("*.json"):
        data = json.loads(file.read_text(encoding="utf-8"))
        if directory == "agent-places" or data.get("sessionId") == session["id"]:
            shutil.copy2(file, target / "core" / directory / file.name)
metadata = {"directory": str(target), "sessionId": session["id"],
            "userStoreModified": False, "originalFilesCopied": False,
            "scenario": "pending source-boundary fallback" if pending_boundary else "empty crop followup"}
(target / "preparation.json").write_text(json.dumps(metadata, indent=2), encoding="utf-8")
(root / ".verification/agent-crop-followup-latest.json").write_text(json.dumps(metadata), encoding="utf-8")
print(json.dumps(metadata))
