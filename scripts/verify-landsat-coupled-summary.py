"""Reject partial or mismatched evidence using in-memory copies of real receipts."""
import argparse
import copy
import importlib.util
import json
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "landsat_summary", Path(__file__).with_name("summarize-landsat-coupled.py")
)
summary = importlib.util.module_from_spec(spec)
spec.loader.exec_module(summary)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True)
    parser.add_argument("--offline", required=True)
    args = parser.parse_args()
    root, offline = (Path(p).resolve() for p in [args.root, args.offline])
    for folder in [root, offline]:
        assert folder.parent == Path(".verification").resolve()
        assert folder.name.startswith("landsat-coupled-")
    paths = [root / f"{kind}-verification.json" for kind in ["native", "mcp", "ui"]]
    paths.append(offline / "cache-verification.json")
    receipts = [json.loads(p.read_text(encoding="utf-8")) for p in paths]
    native_hash = summary.sha(paths[0])
    summary.validate_receipts(*receipts, native_hash)
    mutations = [
        ("failed-interface", lambda r: r[2].update(status="failed")),
        ("wrong-runtime", lambda r: r[1].update(nativeBinarySha256="0" * 64)),
        ("wrong-native-receipt", lambda r: r[3].update(nativeReceiptSha256="0" * 64)),
        ("incomplete-real-matrix", lambda r: r[0]["cases"].pop()),
        ("missing-fallback-proof", lambda r: next(c for c in r[0]["cases"] if c["case"] == "fallback")["counts"]["coupled"].update(fallbackPixels=0)),
        ("missing-original-scene-links", lambda r: r[2]["cases"][0]["sourceDetails"].update(originalLinks=0)),
        ("wrong-drawn-values", lambda r: r[2]["cases"][0]["actualDraw"].update(allSourceRgbaIdentical=False)),
        ("wrong-thumbnail", lambda r: r[2]["cases"][0]["libraryThumbnail"].update(pngSha256="0" * 64)),
        ("parents-still-present", lambda r: r[3].update(allParentsAbsent=False)),
        ("cache-recreated-on-restart", lambda r: r[3]["cases"][0].update(cacheBytesFileIdentityCreationUnchanged=False)),
        ("loopback-not-reconnected", lambda r: next(c for c in r[1]["cases"] if c["mode"] == "loopback").update(reconnected=False)),
    ]
    controls = []
    for name, mutate in mutations:
        changed = copy.deepcopy(receipts)
        mutate(changed)
        try:
            summary.validate_receipts(*changed, native_hash)
        except AssertionError:
            controls.append({"case": name, "rejected": True})
        else:
            raise AssertionError(f"Summary accepted invalid evidence: {name}")
    report = {
        "schema": "geod-landsat-coupled-summary-controls/v1",
        "status": "passed",
        "scope": "In-memory receipt mutations only; accepted artifacts are unchanged",
        "receipts": {p.name: summary.sha(p) for p in paths},
        "controls": controls,
    }
    output = root / "summary-controls-verification.json"
    assert not output.exists(), "Keep prior gate evidence; use a fresh receipt cohort"
    output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    assert all(summary.sha(p) == report["receipts"][p.name] for p in paths)
    print(json.dumps({"status": "passed", "rejectedControls": len(controls)}))


if __name__ == "__main__":
    main()
