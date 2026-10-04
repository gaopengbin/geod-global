"""Publish only mutually bound, fully passed original Landsat quality receipts."""
import argparse
import base64
import hashlib
import json
import shutil
from pathlib import Path


KEYS = {"qa_pixel", "qa_radsat"}
ASSETS = {"red", "green", "blue", *KEYS}
DEFINITION = "https://www.usgs.gov/landsat-missions/landsat-collection-2-quality-assessment-bands"


def sha(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def png_sha(data_url):
    assert data_url.startswith("data:image/png;base64,")
    return hashlib.sha256(base64.b64decode(data_url.split(",", 1)[1], validate=True)).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qa", required=True)
    parser.add_argument("--snapshot", required=True)
    parser.add_argument("--output", default="prototype/qa/landsat-quality-verification.json")
    args = parser.parse_args()
    workspace = Path.cwd().resolve()
    qa, snapshot = Path(args.qa).resolve(), Path(args.snapshot).resolve()
    assert qa.parent == workspace / ".verification" and qa.name.startswith("landsat-quality-")
    assert snapshot.parent == workspace / ".verification" and snapshot.name.startswith("renderer-landsat-quality-")
    paths = {name: qa / filename for name, filename in {
        "native": "native-verification.json", "independent": "independent-verification.json",
        "mcp": "mcp-verification.json", "ui": "ui-verification.json"}.items()}
    receipts = {name: json.loads(path.read_text(encoding="utf-8")) for name, path in paths.items()}
    native, independent, mcp, ui = (receipts[name] for name in ["native", "independent", "mcp", "ui"])
    assert all(receipt["status"] == "passed" for receipt in receipts.values())
    native_hash = sha(paths["native"])
    assert all(receipt["nativeReceiptSha256"] == native_hash for receipt in [independent, mcp, ui])
    assert ui["independentReceiptSha256"] == sha(paths["independent"])
    assert sha(native["nativeBinary"]) == native["nativeBinarySha256"]
    assert all(receipt["nativeBinarySha256"] == native["nativeBinarySha256"] for receipt in [mcp, ui])
    assert len(native["cases"]) == len(independent["cases"]) == 2
    assert {case["key"] for case in native["cases"]} == KEYS
    assert {case["key"] for case in independent["cases"]} == KEYS
    originals = []
    for case in native["cases"]:
        job, metadata = case["job"], case["metadata"]
        quality = metadata["quality"]
        oracle = next(entry for entry in independent["cases"] if entry["key"] == case["key"])
        assert job["kind"] == "download" and job["status"] == "succeeded" and job["error"] is None
        assert job["assetKey"] == case["key"] and job["sha256"] == oracle["sourceSha256"] == metadata["sha256"]
        assert sha(job["outputPath"]) == job["sha256"]
        assert Path(job["outputPath"]).stat().st_size == job["bytesDownloaded"] == job["totalBytes"] == oracle["bytes"]
        assert "?" not in job["href"] and "#" not in job["href"]
        assert metadata["dataType"] == "UInt16" and metadata["bandCount"] == 1 and metadata["nodata"] is None
        assert metadata["pixelSize"] == [30, 30] and quality["pixelInterpretation"] in {"PixelIsPoint", "PixelIsArea"}
        assert quality["product"] == "landsat-c2-l2" and quality["band"] == case["key"]
        assert quality["definition"] == DEFINITION and quality["countsFullResolution"]
        assert quality["sampleCount"] == metadata["width"] * metadata["height"] == oracle["samplesCounted"]
        assert quality["validSampleCount"] == oracle["validSamples"]
        assert [entry["count"] for entry in metadata["classes"]] == oracle["classCounts"]
        assert sum(oracle["classCounts"]) == quality["validSampleCount"]
        fields = quality["flags"]["fields"]
        assert [{"name": entry["name"], "counts": entry["counts"]} for entry in fields] == oracle["flags"]
        assert all(sum(entry["counts"]) == quality["sampleCount"] for entry in fields)
        assert all(len(entry["counts"]) == 2 ** (entry["endBit"] - entry["startBit"] + 1) for entry in fields)
        if case["key"] == "qa_pixel":
            assert len(fields) == 12 and fields[0]["name"] == "Fill"
            assert fields[0]["counts"][0] == quality["validSampleCount"]
            assert quality["flags"]["sourceNoData"] in (None, 1)
        else:
            assert len(fields) == 13 and quality["validSampleCount"] == quality["sampleCount"]
            assert quality["flags"]["sourceNoData"] in (None, 0)
        assert png_sha(metadata["previewDataUrl"]) == oracle["preview"]["pngSha256"]
        assert png_sha(case["thumbnail"]["dataUrl"]) == oracle["thumbnail"]["pngSha256"]
        for name in ["preview", "thumbnail"]:
            assert sha(qa / f"{case['key']}-{name}.png") == oracle[name]["pngSha256"]
        assert len(case["pixels"]) == oracle["rawSamplesCompared"] and oracle["rawSamplesCompared"] > 0
        for pixel in case["pixels"]:
            value = pixel["value"]
            assert isinstance(value, int) and 0 <= value <= 65535
            assert pixel["sha256"] == job["sha256"] and len(pixel["quality"]["fields"]) == len(fields)
            assert pixel["quality"]["binary"] == f"{value:016b}" and pixel["quality"]["hex"] == f"0x{value:04X}"
            assert pixel["isNoData"] == bool(value & 1 if case["key"] == "qa_pixel" else False)
            for field in pixel["quality"]["fields"]:
                assert field["value"] == (value >> field["startBit"]) & ((1 << (field["endBit"] - field["startBit"] + 1)) - 1)
        originals.append({"key": case["key"], "id": job["id"], "itemId": job["itemId"],
            "href": job["href"], "bytes": oracle["bytes"], "sha256": job["sha256"],
            "grid": {name: metadata[name] for name in ["width", "height", "dataType", "bandCount", "crs", "bounds", "pixelSize", "nodata"]},
            "quality": quality, "displayPriorityClasses": metadata["classes"],
            "independent": oracle, "rawSamples": [{name: pixel[name] for name in ["coordinate", "pixel", "center", "value", "isNoData"]} for pixel in case["pixels"]]})
    scene = native["project"]["scenes"][0]
    assert set(scene["assets"]) == ASSETS and set(native["catalog"]["keys"]) == ASSETS
    assert native["legacyUpgrade"]["unchangedRgb"] and set(native["legacyUpgrade"]["assets"]) == ASSETS
    assert all(native["legacyUpgrade"]["assets"][key] == scene["assets"][key] for key in ASSETS)
    assert all("rasterBand" not in scene["assets"][key] for key in KEYS)
    assert native["restart"]["unreachableProxy"] == "127.0.0.1:9" and native["restart"]["originalsLocal"]
    assert native["restart"]["metadataAndPixelsUnchanged"] and native["restart"]["cacheFilesUnchanged"] == 2
    assert native["changedSourceRejected"] and native["corruptCacheRebuilt"]
    assert mcp["readOnly"] and mcp["disconnectedAdapterRejected"] and len(mcp["cases"]) == 4
    expected_modes = {"direct read-only; rejected upstream proxy", "loopback read-only; rejected upstream proxy"}
    assert {(case["mode"], case["assetKey"]) for case in mcp["cases"]} == {(mode, key) for mode in expected_modes for key in KEYS}
    for case in mcp["cases"]:
        source = next(entry for entry in originals if entry["key"] == case["assetKey"])
        assert case["sha256"] == source["sha256"] and case["pixelsCompared"] == len(source["rawSamples"])
        assert case["previewOmitted"] and case["allBitFieldsRetained"]
    assert ui["nativeWindowTested"] is False and not ui["errors"] and not ui["remoteRequests"]
    assert len(ui["cases"]) == 3
    assert {(case["key"], case["width"], case["locale"], case["theme"]) for case in ui["cases"]} == {
        ("qa_pixel", 1440, "en", "light"), ("qa_radsat", 1024, "zh-CN", "dark"), ("qa_pixel", 900, "zh-CN", "dark")}
    for case in ui["cases"]:
        source = next(entry for entry in originals if entry["key"] == case["key"])
        assert case["rgbaSourcePixelsCompared"] == source["independent"]["preview"]["rgbaPixelsCompared"]
        assert case["mapPaintedPixels"] > 0 and case["decodedFields"] == len(source["quality"]["flags"]["fields"])
        assert case["flagCountBinsChecked"] == sum(count > 0 for field in source["quality"]["flags"]["fields"] for count in field["counts"])
        assert case["sourceDefinition"] == DEFINITION
    frozen = json.loads((snapshot / "receipt.json").read_text(encoding="utf-8"))
    assert len(frozen["files"]) >= 662 and frozen["uiReceiptSha256"] == sha(paths["ui"])
    for filename, digest in frozen["files"].items():
        assert sha(snapshot / "dist" / filename) == digest == sha(workspace / "prototype/dist" / filename)
    for filename, digest in ui["resources"].items():
        assert sha(workspace / filename) == digest
    desktop = frozen["desktop"]
    assert sha(desktop["path"]) == desktop["sha256"] and Path(desktop["path"]).stat().st_size == desktop["bytes"]
    prior = workspace / ".verification/renderer-modis-coupled-20261004"
    prior_receipt = json.loads((prior / "receipt.json").read_text(encoding="utf-8"))
    assert len(prior_receipt["files"]) == 661
    for filename, digest in prior_receipt["files"].items():
        assert sha(prior / "dist" / filename) == digest
    assert sha(prior_receipt["desktop"]["path"]) == prior_receipt["desktop"]["sha256"]
    assert sha(workspace / "target/debug/geod-runtime.exe") == "3b0e845ebea363f154644db9e75f0fff1f852b69201eb2bcbae85174523ea03b"
    evidence = qa / "accepted-evidence"
    evidence.mkdir(exist_ok=True)
    proof = {}
    for name, path in paths.items():
        digest = sha(path)
        target = evidence / f"{name}-{digest}.json"
        if not target.exists():
            shutil.copy2(path, target)
        assert sha(target) == digest
        proof[name] = {"snapshot": str(target), "sha256": digest}
    screenshots = {}
    images = evidence / f"ui-{proof['ui']['sha256']}"
    images.mkdir(exist_ok=True)
    for image in sorted((qa / "ui").glob("*.png")):
        target = images / image.name
        if not target.exists():
            shutil.copy2(image, target)
        assert sha(target) == sha(image)
        screenshots[image.name] = {"path": str(target), "sha256": sha(target)}
    assert len(screenshots) == 7
    result = {"schema": "geod-landsat-quality-verification/v1", "status": "passed", "checkedAt": native["checkedAt"],
        "scope": "Original Landsat 8/9 C2 L2 QA_PIXEL / QA_RADSAT viewing; real Landsat 9 source acceptance",
        "definition": DEFINITION, "nativeBinary": {"path": native["nativeBinary"], "sha256": native["nativeBinarySha256"], "bytes": Path(native["nativeBinary"]).stat().st_size},
        "evidence": proof, "catalog": native["catalog"], "originalAcquisition": native["originalAcquisition"],
        "resumedFailedReceiptSha256": native.get("resumedFailedReceiptSha256"), "originals": originals,
        "native": {"files": 2, "bytes": sum(source["bytes"] for source in originals),
            "fullResolutionSamplesCompared": sum(source["quality"]["sampleCount"] for source in originals),
            "previewRgbaPixelsCompared": sum(source["independent"]["preview"]["rgbaPixelsCompared"] for source in originals),
            "thumbnailRgbaPixelsCompared": sum(source["independent"]["thumbnail"]["rgbaPixelsCompared"] for source in originals),
            "rawSamplesCompared": sum(len(source["rawSamples"]) for source in originals),
            "independentDecoder": independent["independentDecoder"], "legacyRgbPreserved": True},
        "mcp": {"cases": mcp["cases"], "rawSamplesCompared": sum(case["pixelsCompared"] for case in mcp["cases"]), "disconnectedAdapterRejected": True},
        "ui": {"cases": ui["cases"], "renderer": ui["renderer"], "resources": ui["resources"], "screenshots": screenshots,
            "sourceRgbaPixelsCompared": sum(case["rgbaSourcePixelsCompared"] for case in ui["cases"]),
            "flagCountBinsChecked": sum(case["flagCountBinsChecked"] for case in ui["cases"]), "nativeWindowTested": False},
        "cache": {**native["restart"], "identityProof": "content, file identity and creation time; LRU modification time may advance",
            "changedOriginalRejected": True, "corruptEntryRebuilt": True},
        "acceptedRenderer": {"snapshot": str(snapshot), "receiptSha256": sha(snapshot / "receipt.json"), "files": len(frozen["files"])},
        "developmentDesktopBuild": {**desktop, "customProtocol": True, "nativeWindowTested": False, "installerBuilt": False},
        "priorModisRendererPreserved": {"snapshot": str(prior), "receiptSha256": sha(prior / "receipt.json"), "files": 661},
        "boundaries": [
            "The two original files were downloaded through the native worker in this private directory, then reused while repairing verification and Point-grid handling; failed receipts remain diagnostics.",
            "The real product is Landsat 9; Landsat 8 shares the reviewed 8/9 schema but has no separate real-file acceptance in this cohort.",
            "QA_PIXEL fill follows bit 0. QA_RADSAT zero is retained and cannot establish image coverage by itself.",
            "Display priority classes are mutually exclusive previews, not all individual flag counts or cloud percentages.",
            "Real snow, terrain-occlusion and reserved/unused positive samples were absent; high-bit and reserved-code regressions use explicitly synthetic inputs.",
            "No Landsat quality clipping, RGB quality screening, aerosol/thermal layers or general reprojection was added.",
            "Headless production renderer, native bridge and CSP do not replace native WebView/window acceptance.",
            "Earthdata/Copernicus software authorization entries are available; successful protected production downloads remain pending accounts."]}
    target = Path(args.output).resolve()
    assert target.parent == workspace / "prototype/qa"
    target.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": "passed", "originals": 2, "mcp": 4, "ui": 3, "output": str(target)}))


if __name__ == "__main__":
    main()
