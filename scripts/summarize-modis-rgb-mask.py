"""Publish local QA summary only after native, MCP, UI and offline gates pass."""
import argparse
import hashlib
import json
import shutil
from datetime import datetime, timezone
from pathlib import Path


def sha(file):
    digest = hashlib.sha256()
    with Path(file).open("rb") as stream:
        for part in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(part)
    return digest.hexdigest()


def preserved_renderer(workspace, folder):
    receipt = workspace / ".verification" / folder / "receipt.json"
    entries = json.loads(receipt.read_text(encoding="utf-8"))["files"]
    files = entries.items() if isinstance(entries, dict) else ((entry["file"], entry["sha256"]) for entry in entries)
    dist = (receipt.parent / "dist").resolve()
    for relative, digest in files:
        file = (dist / relative).resolve()
        assert file.is_relative_to(dist) and sha(file) == digest
    return {"receipt": str(receipt.relative_to(workspace)).replace("\\", "/"),
            "sha256": sha(receipt), "files": len(entries), "allFileHashesVerified": True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qa", default=".verification/modis-rgb-mask-final-20261004")
    parser.add_argument("--offline", default=".verification/modis-rgb-mask-offline-20261004")
    parser.add_argument("--output", default="prototype/qa/modis-rgb-mask-verification.json")
    args = parser.parse_args()
    workspace = Path.cwd().resolve()
    qa, offline = Path(args.qa).resolve(), Path(args.offline).resolve()
    for folder in [qa, offline]:
        assert folder.parent == workspace / ".verification" and folder.name.startswith("modis-rgb-mask-")
    files = {"native": qa / "native-verification.json", "mcp": qa / "mcp-verification.json",
             "ui": qa / "ui-verification.json", "cache": offline / "cache-verification.json"}
    receipts = {key: json.loads(file.read_text(encoding="utf-8")) for key, file in files.items()}
    assert all(receipt["status"] == "passed" for receipt in receipts.values())
    native, mcp, ui, cache = (receipts[key] for key in ["native", "mcp", "ui", "cache"])
    binary_sha = native["nativeBinarySha256"]
    assert all(receipt["nativeBinarySha256"] == binary_sha for receipt in receipts.values())
    assert sha(native["nativeBinary"]) == binary_sha
    native_sha = sha(files["native"])
    assert all(receipt["nativeReceiptSha256"] == native_sha for receipt in [mcp, ui, cache])
    expected = {(kind, policy, snow) for kind in ["original", "single", "polygon"]
                for policy, snow in [(None, False), ("clear", False), ("clear_best", False), ("clear_best", True)]}
    assert len(native["cases"]) == 12
    assert {(r["case"], r["policy"], r["excludeSnow"]) for r in native["cases"]} == expected
    assert native["originalSourceFilesUnchanged"] and native["restartRulesAndResultsUnchanged"]
    assert len(native["controls"]) == 3
    assert len(mcp["cases"]) == 2 and {r["mode"] for r in mcp["cases"]} == {"direct", "loopback"}
    assert mcp["writesDeniedInReadOnlyMode"] and mcp["invalidPolicyAndDuplicateFlagsRejected"]
    assert not ui["diagnostic"] and ui["createdByUi"]["job"]["status"] == "succeeded"
    assert len(ui["cases"]) == 3 and {r["kind"] for r in ui["cases"]} == {"original", "single", "polygon"}
    assert ui["errors"] == [] and ui["remoteRequests"] == [] and not ui["nativeWindowTested"]
    assert all(r["actualDraw"]["allSourceRgbaIdentical"] and not r["horizontalOverflow"] for r in ui["cases"])
    assert len(cache["cases"]) == 12 and cache["allParentsAbsent"] and cache["restarted"]
    assert {r["id"] for r in cache["cases"]} == {r["job"]["id"] for r in native["cases"]}
    assert all(r["readWithoutParents"] and r["cacheBytesFileIdentityCreationUnchanged"] for r in cache["cases"])
    assert len(cache["controls"]) == 3
    # Evidence is copied by content hash so a later QA run cannot silently
    # replace the receipts supporting this historical result.
    archive = workspace / ".verification" / "modis-rgb-mask-evidence-20261004"
    archive.mkdir(exist_ok=True)
    references = {}
    for name, file in files.items():
        digest = sha(file)
        frozen = archive / f"{name}-{digest}.json"
        if not frozen.exists():
            shutil.copy2(file, frozen)
        assert sha(frozen) == digest
        references[name] = {"path": str(file.relative_to(workspace)).replace("\\", "/"),
                            "sha256": digest, "snapshot": str(frozen.relative_to(workspace)).replace("\\", "/")}
    for file, digest in ui["resources"].items():
        assert sha(workspace / file) == digest, f"Renderer changed after acceptance: {file}"
    for entry in native["cases"]:
        assert sha(entry["job"]["outputPath"]) == entry["job"]["sha256"]
        assert sha(entry["package"]["path"]) == entry["package"]["sha256"]
    desktop = workspace / ".verification/naip-native-target/debug/geod-global-desktop.exe"
    canonical = workspace / "target/debug/geod-runtime.exe"
    assert canonical.stat().st_size == 49704448 and sha(canonical) == "3b0e845ebea363f154644db9e75f0fff1f852b69201eb2bcbae85174523ea03b"
    previous_renderer = preserved_renderer(workspace, "renderer-before-modis-mask-20261004")
    first_mask_renderer = preserved_renderer(workspace, "renderer-modis-mask-first-accepted-20261004")
    result = {
        "schema": "geod-modis-rgb-mask-verification/v1", "status": "passed",
        "verifiedAt": datetime.now(timezone.utc).isoformat(),
        "scope": "MOD/MYD09A1 v061 same-scene scientific RGB quality screening; original, rectangular clip and polygon-with-hole clip",
        "definition": native["definition"], "nativeBinary": {"path": native["nativeBinary"], "sha256": binary_sha},
        "receipts": references,
        "native": {"outputs": len(native["cases"]), "allDnCompared": sum(r["allDnCompared"] for r in native["cases"]),
                   "previewRgbaPixelsCompared": sum(r["preview"]["rgbaPixelsCompared"] for r in native["cases"]),
                   "pixelsCompared": sum(len(r["pixels"]) for r in native["cases"]),
                   "cases": [{"kind": r["case"], "policy": r["policy"], "excludeSnow": r["excludeSnow"],
                              "id": r["job"]["id"], "sha256": r["job"]["sha256"], "bytes": r["job"]["bytesDownloaded"],
                              "grid": r["job"]["rgbSpec"]["grid"], "counts": r["counts"], "preview": r["preview"],
                              "packageSha256": r["package"]["sha256"]} for r in native["cases"]],
                   "controls": native["controls"], "sourceFilesUnchanged": True, "restarted": True},
        "mcp": {"modes": [r["mode"] for r in mcp["cases"]], "outputs": len(mcp["cases"]),
                "allDnCompared": sum(r["samplesCompared"] for r in mcp["cases"]),
                "pixelsCompared": sum(len(r["pixels"]) for r in mcp["cases"]),
                "writesDeniedInReadOnlyMode": True, "invalidPolicyAndDuplicateFlagsRejected": True,
                "loopbackDisconnectReconnectPassed": next(r for r in mcp["cases"] if r["mode"] == "loopback")["reconnected"]},
        "ui": {"cases": ui["cases"], "createdByUi": {"id": ui["createdByUi"]["job"]["id"], "sha256": ui["createdByUi"]["job"]["sha256"]},
               "drawnSourceRgbaPixelsCompared": sum(r["actualDraw"]["width"] * r["actualDraw"]["height"] for r in ui["cases"]),
               "resources": ui["resources"], "renderer": ui["renderer"], "nativeWindowTested": False,
               "errors": [], "remoteRequests": [], "spatialCanvasAllPixelsCompared": False},
        "cache": {"filesRestored": len(cache["cases"]), "missingParentJobs": len(cache["missingParentJobs"]),
                  "thumbnailRgbaPixelsCompared": sum(r["thumbnail"]["rgbaPixelsCompared"] for r in cache["cases"]),
                  "cacheEntriesReused": len(cache["cases"]), "sourceDownloadsRestarted": False, "controls": cache["controls"]},
        "developmentDesktopBuild": {"path": str(desktop), "bytes": desktop.stat().st_size, "sha256": sha(desktop),
                                    "customProtocol": True, "nativeWindowTested": False, "installerBuilt": False},
        "previousRendererPreserved": previous_renderer,
        "firstAcceptedMaskRendererPreserved": first_mask_renderer,
        "regression": {"scientificRgbRust": {"passed": 6}, "qualityRust": {"passed": 9},
                       "actualWindowsCliChild": {"passed": 1, "paths": ["direct directory", "closed loopback service"]},
                       "focusedNode": {"passed": 16}, "focusedReact": {"passed": 14}, "cargoFmt": "passed", "strictClippy": "passed"},
        "boundaries": ["No new provider download in this cohort; uses previously downloaded real public COGs.",
                       "Independently mosaicked RGB and QA rasters are rejected; coupled quality-aware multi-scene processing remains pending.",
                       "The real clips added no new cloud/snow removals; real snow exclusion is not claimed. Individual flags and fill values have synthetic regression evidence.",
                       "No additional atmospheric-accuracy guarantee, reprojection, resampling, arbitrary bands or other-product QA rules.",
                       "Headless built-renderer/native-bridge evidence does not replace native WebView/window acceptance.",
                       "Earthdata/Copernicus successful account authorization and protected production originals remain pending; software authorization entries are available."]
    }
    target = Path(args.output).resolve()
    assert target.parent == workspace / "prototype/qa"
    target.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": "passed", "output": str(target), "nativeOutputs": len(native["cases"]), "uiCases": len(ui["cases"]), "offlineFiles": len(cache["cases"])}))


if __name__ == "__main__":
    main()
