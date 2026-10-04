"""Save a Landsat project-processing summary only from mutually bound passed evidence."""
import argparse
import base64
import hashlib
import json
import shutil
from pathlib import Path


DEFINITION = "https://www.usgs.gov/landsat-missions/landsat-collection-2-quality-assessment-bands"
COVERAGE = "Coverage follows the independent internal mask, derived from matching QA_PIXEL bit 0 and the project geometry"
POLICY = "newest scene with QA_PIXEL bit 0 unset wins; complete UInt16 flags retained; filled newer scenes do not erase covered older samples; independent internal mask; no quality ranking or bit merging"
KEYS = {"qa_pixel", "qa_radsat"}
CASES = {"single", "mosaic", "polygon", "large"}
CANONICAL_RUNTIME = "3b0e845ebea363f154644db9e75f0fff1f852b69201eb2bcbae85174523ea03b"


def sha(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def load(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def png_sha(url):
    assert url.startswith("data:image/png;base64,")
    return hashlib.sha256(base64.b64decode(url.split(",", 1)[1], validate=True)).hexdigest()


def managed(root, filename):
    prefix = chr(92) * 2 + "?" + chr(92)
    path = Path(filename[len(prefix):] if filename.startswith(prefix) else filename)
    path = (path if path.is_absolute() else root / path).resolve()
    assert path.is_relative_to(root)
    return path


def preserve(path, directory, label):
    digest = sha(path)
    target = directory / f"{label}-{digest}{path.suffix}"
    if not target.exists():
        shutil.copy2(path, target)
    assert sha(target) == digest
    return {"snapshot": str(target), "sha256": digest}


def verify_renderer(path, current_dist=None):
    receipt = load(path / "receipt.json")
    files = receipt.get("files") or {entry["path"]: entry["sha256"] for entry in receipt["distFiles"]}
    assert len(files) >= 661
    for filename, digest in files.items():
        file = path / "dist" / filename
        assert file.resolve().is_relative_to((path / "dist").resolve()) and sha(file) == digest
        if current_dist:
            assert (current_dist / filename).is_file() and sha(current_dist / filename) == digest
    desktop = receipt["desktop"]
    assert sha(desktop["path"]) == desktop["sha256"]
    assert Path(desktop["path"]).stat().st_size == desktop["bytes"]
    return receipt, len(files)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qa", required=True)
    parser.add_argument("--cache", required=True)
    parser.add_argument("--snapshot", required=True)
    parser.add_argument("--output", default="prototype/qa/landsat-quality-processing-verification.json")
    args = parser.parse_args()
    workspace = Path.cwd().resolve()
    qa, cache, snapshot = (Path(value).resolve() for value in [args.qa, args.cache, args.snapshot])
    assert all(path.parent == workspace / ".verification" for path in [qa, cache, snapshot])
    assert qa.name.startswith("landsat-quality-processing-") and cache.name.startswith("landsat-quality-processing-cache-")
    assert snapshot.name.startswith("renderer-landsat-quality-processing-")
    paths = {name: qa / f"{name}-processing-verification.json" for name in ["native", "independent", "mcp", "ui"]}
    paths["cache"] = cache / "cache-verification.json"
    native, oracle, mcp, ui, cached = (load(paths[name]) for name in ["native", "independent", "mcp", "ui", "cache"])
    assert all(record["status"] == "passed" for record in [native, oracle, mcp, ui, cached])
    native_sha, binary_sha = sha(paths["native"]), native["nativeBinarySha256"]
    assert all(record["nativeReceiptSha256"] == native_sha and record["nativeBinarySha256"] == binary_sha
               for record in [oracle, mcp, ui, cached])
    assert ui["independentReceiptSha256"] == sha(paths["independent"])
    assert sha(native["nativeBinary"]) == binary_sha and native["rgbScreeningApplied"] is False
    assert not native["downloadFailures"]
    original_jobs = {job["id"]: job for job in native["originals"]}
    assert len(original_jobs) == 6
    source_proofs = {source["id"]: source for source in oracle["originals"]}
    assert set(source_proofs) == set(original_jobs)
    reuse = native["reusedOriginals"]
    assert len(reuse["originals"]) == 2
    previous = load(workspace / "prototype/qa/landsat-quality-verification.json")
    assert previous["status"] == "passed" and previous["evidence"]["native"]["sha256"] == reuse["sourceReceiptSha256"]
    reused_ids = {job["id"] for job in reuse["originals"]}
    for job in original_jobs.values():
        proof = source_proofs[job["id"]]
        file = managed(qa, job["outputPath"])
        assert job["kind"] == "download" and job["status"] == "succeeded" and job["error"] is None
        assert sha(file) == job["sha256"] == proof["sha256"]
        assert file.stat().st_size == job["bytesDownloaded"] == job["totalBytes"] == proof["bytes"]
        assert job["itemId"] == proof["itemId"] and job["assetKey"] == proof["key"] in KEYS
        assert "?" not in job["href"] and "#" not in job["href"] and proof["pixelInterpretation"] == "PixelIsPoint"
        if job["id"] in reused_ids:
            old = next(source for source in previous["originals"] if source["id"] == job["id"])
            assert old["sha256"] == job["sha256"] and old["bytes"] == proof["bytes"]
        else:
            assert job["transfer"] == {"mode": "fresh", "resumedBytes": 0}
    assert {job["itemId"].split("_")[0] for job in original_jobs.values()} == {"LC08", "LC09"}
    assert {case["name"] for case in native["cases"]} == CASES
    assert len(native["outputs"]) == len(oracle["cases"]) == 8
    assert {(entry["case"], entry["key"]) for entry in native["outputs"]} == {(case, key) for case in CASES for key in KEYS}
    outputs = []
    for entry in native["outputs"]:
        key, job, metadata = entry["key"], entry["job"], entry["metadata"]
        check = next(case for case in oracle["cases"] if case["jobId"] == job["id"])
        project = next(case["project"] for case in native["cases"] if case["name"] == entry["case"])
        spec, plan, quality = job["mosaic"], job["mosaicOutput"], metadata["quality"]
        assert job["kind"] == "raster_mosaic" and job["status"] == "succeeded" and job["settled"] and job["error"] is None
        assert job["assetKey"] == key == check["key"] and spec["projectId"] == project["id"]
        file = managed(qa, job["outputPath"])
        assert sha(file) == job["sha256"] == metadata["sha256"] == check["outputSha256"]
        assert file.stat().st_size == job["bytesDownloaded"] == job["totalBytes"]
        assert metadata["dataType"] == "UInt16" and metadata["bandCount"] == plan["bandCount"] == 1
        assert metadata["pixelSize"] == plan["pixelSize"] == [30, 30] and metadata["nodata"] is None
        assert plan["overlapPolicy"] == POLICY
        assert plan["landsatQuality"] == {"schemaVersion": "geod-landsat-quality-mosaic/v1", "product": "landsat-c2-l2", "band": key,
                                         "bits": 16, "definition": DEFINITION, "coverage": COVERAGE}
        assert len(spec["sources"]) == plan["sourceCount"] == len(project["scenes"]) == (1 if entry["case"] == "single" else 3)
        assert len(spec.get("coverageSources", [])) == (plan["sourceCount"] if key == "qa_radsat" else 0)
        dates = []
        for index, pin in enumerate(spec["sources"]):
            source = original_jobs[pin["jobId"]]
            assert source["assetKey"] == key and source["sha256"] == pin["sha256"]
            dates.append(source["itemId"].split("_")[3])
            if key == "qa_radsat":
                paired = spec["coverageSources"][index]
                mask_source = original_jobs[paired["jobId"]]
                assert mask_source["assetKey"] == "qa_pixel" and mask_source["sha256"] == paired["sha256"]
                assert mask_source["itemId"] == source["itemId"] and mask_source["href"].rsplit("/", 1)[0] == source["href"].rsplit("/", 1)[0]
        assert dates == sorted(dates)
        count = metadata["width"] * metadata["height"]
        assert count == quality["sampleCount"] == check["samplesCompared"] == check["coverageBitsCompared"]
        assert quality["validSampleCount"] == plan["coveredPixels"] == check["coveredPixels"]
        assert plan["maskedPixels"] == check["maskedPixels"]
        assert quality["pixelInterpretation"] == "PixelIsArea" and quality["definition"] == DEFINITION and quality["countsFullResolution"]
        flags = quality["flags"]
        assert flags["sourceNoData"] is None and flags["coverage"] == COVERAGE
        assert flags["coverageMask"] == {"kind": "internal-1bit", "coveredPixels": check["coveredPixels"], "uncoveredPixels": check["uncoveredPixels"]}
        assert flags["fields"] == check["flags"] and all(sum(field["counts"]) == count for field in flags["fields"])
        assert [group["count"] for group in metadata["classes"]] == check["classCounts"] and sum(check["classCounts"]) == check["coveredPixels"]
        if key == "qa_pixel":
            assert flags["fields"][0]["counts"][0] == check["coveredPixels"]
        for name, url in [("preview", metadata["previewDataUrl"]), ("thumbnail", entry["thumbnail"]["dataUrl"])]:
            assert png_sha(url) == check[name]["pngSha256"] == sha(managed(qa, entry[f"{name}File"]))
        assert len(entry["pixels"]) == check["rawSamplesCompared"] == 9
        for pixel in entry["pixels"]:
            raw = pixel["value"]
            assert isinstance(raw, int) and 0 <= raw <= 65535 and pixel["sha256"] == job["sha256"]
            assert pixel["isNoData"] == (not pixel["quality"]["covered"])
            assert pixel["quality"]["binary"] == f"{raw:016b}" and pixel["quality"]["hex"] == f"0x{raw:04X}"
            for field in pixel["quality"]["fields"]:
                assert field["value"] == (raw >> field["startBit"]) & ((1 << (field["endBit"] - field["startBit"] + 1)) - 1)
        outputs.append({"case": entry["case"], "key": key, "id": job["id"], "bytes": file.stat().st_size, "sha256": job["sha256"],
                        "sourcePins": spec, "plan": plan, "quality": quality, "independent": check})
    assert sum(case["samplesCompared"] for case in oracle["cases"]) == 64_025_460
    assert any(case["samplesCompared"] > 8_000_000 for case in oracle["cases"])
    assert any(case["fallbackEvents"] > 0 for case in oracle["cases"])
    assert any(case["key"] == "qa_radsat" and case["coveredZeroValues"] > 0 and case["uncoveredZeroValues"] > 0 for case in oracle["cases"])
    control = native["missingCoverageControl"]
    assert control["status"] == 400 and control["countBefore"] == control["countAfter"]
    assert "matching QA_PIXEL" in control["message"]["error"]
    assert mcp["readOnly"] and mcp["disconnectedAdapterRejected"] and len(mcp["cases"]) == 16
    assert {case["mode"] for case in mcp["cases"]} == {"direct; unreachable upstream proxy", "loopback adapter; unreachable upstream proxy"}
    for case in mcp["cases"]:
        result = next(output for output in outputs if output["id"] == case["jobId"])
        assert result["sha256"] == case["sha256"] and case["previewOmitted"]
        assert case["coverageMask"] == result["quality"]["flags"]["coverageMask"] and case["rawSamplesCompared"] >= 9
    assert ui["nativeWindowTested"] is False and not ui["errors"] and not ui["remoteRequests"]
    assert {(case["case"], case["key"], case["width"], case["locale"], case["theme"]) for case in ui["cases"]} == {
        ("single", "qa_pixel", 1440, "en", "light"), ("polygon", "qa_radsat", 1024, "zh-CN", "dark"), ("large", "qa_pixel", 900, "zh-CN", "dark")}
    assert ui["projectThumbnailsLoaded"] > 0
    button = ui["projectButton"]
    reference = next(result for result in outputs if result["id"] == button["independentReferenceJobId"])
    assert button["identicalVerifiedTiff"] and button["outputSha256"] == reference["sha256"]
    generated = load(qa / "jobs.json")[button["jobId"]]
    assert generated["status"] == "succeeded" and generated["mosaic"] == reference["sourcePins"] and generated["mosaicOutput"] == reference["plan"]
    assert sha(managed(qa, generated["outputPath"])) == reference["sha256"]
    for case in ui["cases"]:
        result = next(output for output in outputs if output["case"] == case["case"] and output["key"] == case["key"])
        assert result["sha256"] == case["outputSha256"] and case["rgbaSourcePixelsCompared"] == result["independent"]["preview"]["rgbaPixelsCompared"]
        assert case["mapPaintedPixels"] > 0 and case["decodedFields"] == len(result["quality"]["flags"]["fields"])
        assert case["flagCountBinsChecked"] == sum(n > 0 for field in result["quality"]["flags"]["fields"] for n in field["counts"])
        assert case["sourceDefinition"] == DEFINITION
    assert cached["allParentJobsAndFilesAbsent"] and set(cached["missingParentJobs"]) == set(original_jobs)
    assert cached["restarted"] and cached["originalAcceptedStoreUnchanged"] and len(cached["cases"]) == 8
    for case in cached["cases"]:
        result = next(output for output in outputs if output["id"] == case["id"])
        assert case["sha256"] == result["sha256"] and case["readWithoutParents"] and case["cacheBytesFileIdentityCreationUnchanged"]
        assert sha(cache / "assets" / f"{case['id']}.tif") == result["sha256"]
    assert all(not (cache / "assets" / f"{parent}.tif").exists() for parent in original_jobs)
    assert all(failure["status"] == 400 for failure in cached["controls"][0]["failures"].values())
    assert cached["controls"][1]["passed"]
    frozen, files = verify_renderer(snapshot, workspace / "prototype/dist")
    assert files == 662 and frozen["uiReceiptSha256"] == sha(paths["ui"])
    for filename, digest in ui["resources"].items():
        assert Path(filename).parts[:2] == ("prototype", "dist") and sha(workspace / filename) == digest
    prior = []
    for name in ["renderer-landsat-quality-20261004", "renderer-modis-coupled-20261004"]:
        path = workspace / ".verification" / name
        _, count = verify_renderer(path)
        prior.append({"snapshot": str(path), "files": count, "receiptSha256": sha(path / "receipt.json")})
    assert sha(workspace / "target/debug/geod-runtime.exe") == CANONICAL_RUNTIME
    evidence = qa / "accepted-evidence"
    evidence.mkdir(exist_ok=True)
    proof = {name: preserve(path, evidence, name) for name, path in paths.items()}
    images = evidence / f"ui-{proof['ui']['sha256']}"
    images.mkdir(exist_ok=True)
    screenshots = {file.name: preserve(file, images, file.stem) for file in sorted((qa / "ui").glob("*.png")) if file.name != "failure.png"}
    assert len(screenshots) == 7
    result = {"schema": "geod-landsat-quality-processing-verification/v1", "status": "passed", "checkedAt": native["checkedAt"],
              "scope": "Real Landsat 8/9 QA_PIXEL / QA_RADSAT single and multi-scene project clipping; complete UInt16 values and independent coverage masks",
              "definition": DEFINITION, "nativeBinary": {"path": native["nativeBinary"], "sha256": binary_sha, "bytes": Path(native["nativeBinary"]).stat().st_size},
              "evidence": proof, "catalog": native["catalog"], "originals": list(source_proofs.values()),
              "originalAcquisition": {"files": 6, "newNativeDownloads": 4, "reusedVerifiedOriginals": 2, "reuse": reuse},
              "outputs": outputs, "native": {"outputs": 8, "samplesCompared": 64_025_460, "coverageBitsCompared": 64_025_460,
                                                "largeOutputPixels": max(case["samplesCompared"] for case in oracle["cases"]),
                                                "independentDecoder": oracle["independentDecoder"], "missingCoverageControl": control},
              "mcp": {"cases": mcp["cases"], "rawSamplesCompared": sum(case["rawSamplesCompared"] for case in mcp["cases"]), "disconnectedAdapterRejected": True},
              "ui": {"cases": ui["cases"], "projectButton": button, "projectThumbnailsLoaded": ui["projectThumbnailsLoaded"], "screenshots": screenshots,
                     "resources": ui["resources"], "sourceRgbaPixelsCompared": sum(case["rgbaSourcePixelsCompared"] for case in ui["cases"]), "nativeWindowTested": False},
              "cache": {key: cached[key] for key in ["allParentJobsAndFilesAbsent", "network", "cases", "controls", "restarted", "originalAcceptedStoreUnchanged"]},
              "acceptedRenderer": {"snapshot": str(snapshot), "files": files, "receiptSha256": sha(snapshot / "receipt.json")},
              "developmentDesktopBuild": {**frozen["desktop"], "customProtocol": True, "nativeWindowTested": False, "installerBuilt": False},
              "priorRenderersPreserved": prior, "rgbScreeningApplied": False,
              "boundaries": [
                  "Four new complete native downloads plus two previously downloaded originals; subsequent processing retries reuse checksum-verified originals.",
                  "QA_RADSAT processing requires matching QA_PIXEL from the same scene, processing directory and aligned grid; zero remains a valid raw value.",
                  "This operation retains whole QA samples with a coverage mask; it does not rank cloud quality, merge bits, average values or screen RGB.",
                  "Real snow, terrain-occlusion and unused positive values are not a complete positive matrix; synthetic regressions cover retained high bits and fill fallbacks.",
                  "Only aligned 30 metre Landsat 8/9 C2 L2 project grids; general reprojection, other QA layers and generic mosaic delivery ZIPs remain pending.",
                  "Headless built renderer and exact desktop CSP with a real native bridge; native WebView/window and installer acceptance remain separate.",
                  "NASA/Earthdata and Copernicus software authorization entries are available; successful protected production downloads await user accounts."]}
    target = Path(args.output).resolve()
    assert target.parent == workspace / "prototype/qa"
    target.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": "passed", "outputs": 8, "samplesCompared": 64_025_460, "mcp": 16, "ui": 3, "cache": 8, "output": str(target)}))


if __name__ == "__main__":
    main()
