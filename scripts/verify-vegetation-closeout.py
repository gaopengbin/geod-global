"""Bind actual MOD13Q1/MYD13Q1 receipts, files, renderer and desktop build.

This gate does not substitute for GDAL, real downloads or a native-window test.
Its negative controls mutate copies of receipts only, never accepted evidence.
"""
from __future__ import annotations

import copy
import hashlib
import json
import shutil
import sys
from datetime import datetime, timezone
from pathlib import Path


WORKSPACE = Path.cwd().resolve()
CANONICAL_SHA = "3b0e845ebea363f154644db9e75f0fff1f852b69201eb2bcbae85174523ea03b"


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        value = hashlib.sha256()
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
        return value.hexdigest()


def local(value: str) -> Path:
    return Path(value.removeprefix("\\\\?\\")).resolve()


def manifest(root: Path) -> list[dict]:
    return sorted(
        ({"path": path.relative_to(root).as_posix(), "sha256": digest(path)}
         for path in root.rglob("*") if path.is_file()),
        key=lambda entry: entry["path"],
    )


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def validate(bundle: dict) -> None:
    source, core, temporal, mcp, temporal_mcp, cache, ui, explore = (
        bundle[key] for key in ["source", "native", "temporal", "mcp", "temporalMcp", "cache", "ui", "explore"]
    )
    require(all(value["status"] == "passed" for value in bundle.values()), "All stages must pass")
    runtime_sha = core["nativeBinarySha256"]
    require(all(bundle[key]["nativeBinarySha256"] == runtime_sha for key in bundle if key != "source"), "Current native hash differs")
    require(len(source["cases"]) == len(core["originals"]) == 6 and not temporal["originals"], "Six real downloads; temporal evidence reuses them")
    downloaded = {entry["job"]["id"]: entry["job"] for entry in source["cases"]}
    originals = {entry["job"]["id"]: entry["job"] for entry in core["originals"]}
    require(downloaded.keys() == originals.keys(), "Source IDs differ")
    for job_id, job in originals.items():
        require(all(downloaded[job_id][key] == job[key] for key in ["itemId", "assetKey", "sha256", "bytesDownloaded", "href"]), "Original identity or SHA changed")
        # The persisted store omits the transient API `settled` flag. The live
        # download receipt must retain it; full file hashes are rechecked below.
        require(job["status"] == "succeeded" and downloaded[job_id]["settled"] and job["assetKey"] in ["ndvi", "evi"], "Original not settled")
    entries = core["originals"] + core["outputs"] + temporal["outputs"]
    outputs = core["outputs"] + temporal["outputs"]
    require(len(outputs) == 8, "Eight independently verified outputs required")
    require({entry["case"] for entry in outputs} == {"single", "mosaic", "polygon", "temporal"}, "Processing case missing")
    for entry in entries:
        job, metadata = entry["job"], entry["metadata"]
        vegetation = metadata["vegetation"]
        require(metadata["dataType"] == "Int16" and metadata["nodata"] == -3000 and metadata["sha256"] == job["sha256"], "Signed index metadata differs")
        require(vegetation["product"] == "modis-13q1-v061" and vegetation["index"] == job["assetKey"]
                and vegetation["scale"] == 0.0001 and vegetation["offset"] == 0
                and vegetation["validRange"] == [-2000, 10000] and vegetation["palette"] == "modis-vi-v1"
                and vegetation["pixelInterpretation"] == "PixelIsArea", "Vegetation interpretation differs")
        require(not metadata.get("reflectance") and not metadata.get("quality"), "Foreign raster interpretation")
        require(entry["thumbnail"]["sha256"] == job["sha256"], "Thumbnail source differs")
        if entry in outputs:
            require(entry["dnPixelsCompared"] == metadata["width"] * metadata["height"], "Incomplete output comparison")
            require(all(pin["jobId"] in originals and originals[pin["jobId"]]["sha256"] == pin["sha256"]
                        and originals[pin["jobId"]]["assetKey"] == job["assetKey"] for pin in job["mosaic"]["sources"]), "Output source pin differs")
    require(core["dnPixelsRead"] == sum(entry["dnPixelsRead"] for entry in core["originals"]) == 138240000, "Original full-array comparison missing")
    for receipt, count in [(core, 12), (temporal, 2)]:
        require(receipt["outputDnPixelsCompared"] == sum(entry["dnPixelsCompared"] for entry in receipt["outputs"]), "Output count differs")
        require(set(receipt["negativeControls"]) == {"nodata", "date", "crs", "channel"}, "Binding refusal controls missing")
        offline = receipt["offlineRestart"]
        require(offline["upstreamBlocked"] and offline["cacheFilesNotRegenerated"] and offline["rastersRestored"] == offline["persistentThumbnailsReused"] == count, "Offline source/derived cache acceptance missing")
    for entry in temporal["outputs"]:
        older = {key: value for key, value in entry["winnerCounts"].items() if key.startswith("MYD13Q1")}
        require(sum(older.values()) == (71 if entry["job"]["assetKey"] == "ndvi" else 104), "Actual older valid fallback not proved")
    for receipt, expected in [(mcp, 152), (temporal_mcp, 32)]:
        require(receipt["pixelsCompared"] == sum(case["pixelsCompared"] for case in receipt["cases"]) == expected, "MCP point comparison missing")
        require({case["mode"] for case in receipt["cases"]} == {"direct", "loopback adapter"}, "Both MCP transports required")
        require(receipt["readOnly"] and receipt["upstreamBlocked"] and receipt["disconnectedAdapterRejected"]
                and receipt["readOnlyWriteDeniedInBothModes"] and receipt["protocolOnlyStdoutAndCleanExit"], "MCP controls missing")
    require(cache["originalsAbsentDuringColdAndRestartReads"] == 6 and cache["derivedFilesRestored"] == 8
            and cache["diskEntriesReusedWithoutRegeneration"] == 8 and cache["pixelsCompared"] == 96
            and cache["corruptedPngHashRegeneratedIdentically"] and cache["changedOriginalAndDerivedRejectCachedPreview"]
            and cache["restoredCopiesReturnIdenticalPreviews"] and cache["upstreamBlocked"], "Parent-free persistent cache evidence missing")
    require(not ui["nativeWindowTested"] and not ui["usedUserDesktop"] and ui["upstreamBlocked"], "UI evidence boundary changed")
    require(not ui["errors"] and not ui["remoteRequests"] and len(ui["cases"]) == len(ui["createdByUi"]) == 4, "Four actual UI cases missing")
    for case in ui["cases"]:
        require(case["paintedRaster"] and case["indexMetadataVisible"] and case["thumbnailsFillUniformCards"]
                and case["pixelInteraction"] == "actual interior map click", "Actual UI interaction missing")
        require(case["independentPixel"]["pixel"] == case["gdalPixel"]["pixel"]
                and case["independentPixel"]["value"] == case["gdalPixel"]["value"], "GDAL map point differs")
    by_case = {(entry["case"], entry["job"]["assetKey"]): entry["job"] for entry in outputs}
    for actual in ui["createdByUi"]:
        require(actual["sha256"] == by_case[(actual["case"], actual["key"])]["sha256"]
                and all(actual[key] for key in ["sourcePinsIdentical", "outputBytesIdentical", "planIdentical", "displayIdentical"]), "Actual UI processing differs")
    require(explore["providerCount"] == 15 and explore["createdOriginalDownloads"] == 0 and explore["projectRestored"]
            and explore["bothDownloadChoicesVisible"] and explore["returnToSameProject"] and explore["noOpticalCloudFilter"]
            and not explore["nativeWindowTested"] and not explore["usedUserDesktop"] and not explore["errors"]
            and not explore["rejectedRequests"] and len(explore["liveCatalogRequests"]) == 1, "Live discovery evidence differs")


def main() -> None:
    core_root, temporal_root, cache_root, ui_root, explore_root, root = [Path(value).resolve() for value in sys.argv[1:7]]
    require(root.parent == WORKSPACE / ".verification" and root.name.startswith("modis-vegetation-"), "Private output outside verification root")
    require(not root.exists(), "Preserve prior cohort")
    source_path = WORKSPACE / ".verification/modis-vegetation-native-20261004/source-verification.json"
    paths = dict(source=source_path, native=core_root/"verification.json", temporal=temporal_root/"verification.json",
                 mcp=core_root/"mcp-verification.json", temporalMcp=temporal_root/"mcp-verification.json",
                 cache=cache_root/"cache-verification.json", ui=ui_root/"ui/verification.json", explore=explore_root/"verification.json")
    bundle = {key: json.loads(value.read_text(encoding="utf-8")) for key, value in paths.items()}
    validate(bundle)
    hashes = {key: digest(value) for key, value in paths.items()}
    core, temporal, ui, explore = (bundle[key] for key in ["native", "temporal", "ui", "explore"])
    require(core["sourceReceiptSha256"] == temporal["sourceReceiptSha256"] == hashes["source"], "Downloaded source receipt changed")
    require(temporal["originalReference"]["sha256"] == hashes["native"] and temporal["originalReference"]["filesReusedUnchanged"] == 6, "Temporal source reuse not bound")
    require(bundle["mcp"]["nativeReceiptSha256"] == hashes["native"] and bundle["temporalMcp"]["nativeReceiptSha256"] == hashes["temporal"], "MCP receipt not bound")
    for key in ["ui", "cache"]:
        require([row["sha256"] for row in bundle[key]["sourceReceipts"]] == [hashes["native"], hashes["temporal"]], "UI/cache source receipts not bound")
    require(explore["rendererReceipt"]["sha256"] == hashes["ui"], "Live UI uses another renderer")
    require(digest(WORKSPACE / "target/debug/geod-runtime.exe") == CANONICAL_SHA, "User runtime changed")
    native = core_root / ("runtime-" + core["nativeBinarySha256"][:16] + ".exe")
    require(digest(native) == core["nativeBinarySha256"], "Accepted native binary changed")
    entries = core["originals"] + core["outputs"] + temporal["outputs"]
    for entry in entries:
        job = entry["job"]
        require(digest(local(job["outputPath"])) == job["sha256"], "Accepted source/output file changed")
        if job.get("manifestPath"):
            require(local(job["manifestPath"]).is_file(), "Output provenance missing")
    for row in explore["liveCatalogRequests"] + explore["providerPreviews"] + explore["previewFailures"]:
        require(digest(local(row["path"])) == row["sha256"], "Live catalogue/preview capture changed")
    accepted_files = sorted(ui["rendererFiles"], key=lambda entry: entry["path"])
    require(manifest(ui_root/"renderer") == manifest(WORKSPACE/"prototype/dist") == accepted_files, "Renderer changed after UI acceptance")
    desktop = WORKSPACE / ".verification/naip-native-target/debug/geod-global-desktop.exe"
    require(desktop.is_file(), "Desktop development build missing")
    snapshot = WORKSPACE / ".verification/renderer-modis-vegetation-accepted-20261004"
    root.mkdir(); (root/"evidence").mkdir()
    desktop_sha = digest(desktop); desktop_copy = snapshot/("desktop-"+desktop_sha[:16]+".exe")
    if snapshot.exists():
        require(desktop_copy.is_file() and digest(desktop_copy) == desktop_sha
                and manifest(snapshot/"renderer") == accepted_files, "Existing accepted snapshot differs; preserve it")
    else:
        shutil.copytree(ui_root/"renderer", snapshot/"renderer");shutil.copy2(desktop,desktop_copy)
    require(digest(desktop_copy) == desktop_sha and manifest(snapshot/"renderer") == accepted_files, "Frozen desktop/renderer copy differs")
    evidence = {}
    for key, value in paths.items():
        saved = root/"evidence"/(key+"-"+hashes[key]+".json");shutil.copy2(value,saved)
        require(digest(saved) == hashes[key], "Frozen receipt copy differs")
        evidence[key] = {"snapshot":str(saved),"sha256":hashes[key]}
    controls = []
    mutations = [
        ("failed-stage", lambda value: value["source"].update(status="failed")),
        ("changed-original-sha", lambda value: value["native"]["originals"][0]["job"].update(sha256="0"*64)),
        ("stale-native", lambda value: value["temporal"].update(nativeBinarySha256="0"*64)),
        ("wrong-index-scale", lambda value: value["native"]["outputs"][0]["metadata"]["vegetation"].update(scale=1)),
        ("partial-output-comparison", lambda value: value["native"]["outputs"][0].update(dnPixelsCompared=0)),
        ("missing-real-fallback", lambda value: value["temporal"]["outputs"][0]["winnerCounts"].clear()),
        ("mcp-count-inflation", lambda value: value["mcp"].update(pixelsCompared=153)),
        ("parents-present", lambda value: value["cache"].update(originalsAbsentDuringColdAndRestartReads=0)),
        ("false-native-window-claim", lambda value: value["ui"].update(nativeWindowTested=True)),
        ("unmatched-ui-processing", lambda value: value["ui"]["createdByUi"][0].update(sha256="0"*64)),
        ("discovery-counted-as-download", lambda value: value["explore"].update(createdOriginalDownloads=6)),
    ]
    for name, change in mutations:
        amended = copy.deepcopy(bundle);change(amended)
        try:
            validate(amended)
        except (ValueError,KeyError,TypeError):
            controls.append({"name":name,"rejected":True})
        else:
            raise ValueError("Summary counterexample accepted: "+name)
    gate = {"schema":"geod-modis-vegetation-closeout/v1","status":"passed","controls":controls,"originalsAndOutputsUnchanged":14,
            "canonicalRuntimeUnchanged":True,"rendererFiles":len(accepted_files),"scope":"Receipt binding and file hashes only; no new download, GDAL result or native-window acceptance"}
    gate_path = root/"gate.json";gate_path.write_text(json.dumps(gate,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
    evidence["gate"]={"snapshot":str(gate_path),"sha256":digest(gate_path)}
    outputs = core["outputs"]+temporal["outputs"]
    summary = {
        "schema":"geod-modis-vegetation-verification/v1","status":"passed","checkedAt":datetime.now(timezone.utc).isoformat(),
        "scope":"Only public Planetary Computer MOD13Q1/MYD13Q1 v061 NDVI and EVI converted COGs; two of twelve science layers",
        "definition":"https://lpdaac.usgs.gov/documents/621/MOD13_User_Guide_V61.pdf","evidence":evidence,
        "nativeBinary":{"path":str(native),"sha256":core["nativeBinarySha256"]},
        "sources":{"actualOriginalDownloads":6,"totalBytes":sum(row["job"]["bytesDownloaded"] for row in core["originals"]),
                   "initialDownloadBinarySha256":bundle["source"]["nativeBinarySha256"],"reinspectedWithCurrentBinary":True,
                   "originals":[{key:row["job"][key] for key in ["id","itemId","assetKey","href","sha256","bytesDownloaded"]} for row in core["originals"]]},
        "native":{"originalDnRead":core["dnPixelsRead"],"outputDnCompared":sum(row["dnPixelsCompared"] for row in outputs),
                  "previewAndThumbnailRgbaPixelsCompared":core["pngPixelsCompared"]+temporal["pngPixelsCompared"],
                  "independentRawPointsCompared":sum(len(row["pixels"]) for row in entries),
                  "cases":["single","mosaic","polygon-with-hole","temporal-fallback"],
                  "outputs":[{"id":row["job"]["id"],"case":row["case"],"assetKey":row["job"]["assetKey"],"sha256":row["job"]["sha256"],
                              "width":row["metadata"]["width"],"height":row["metadata"]["height"],"allDnCompared":row["dnPixelsCompared"],
                              "winnerCounts":row["winnerCounts"],"maskedPixels":row["maskedPixels"]} for row in outputs],
                  "bindingRefusalControls":["nodata","date","crs","channel"],"offlineCacheEntriesReused":14},
        "mcp":{"modes":["direct","adapter"],"rawPointsCompared":bundle["mcp"]["pixelsCompared"]+bundle["temporalMcp"]["pixelsCompared"],
               "readOnlyWritesDenied":True,"disconnectedAdapterRejected":True,"protocolOnlyStdoutAndCleanExit":True},
        "cache":{key:bundle["cache"][key] for key in ["originalsAbsentDuringColdAndRestartReads","derivedFilesRestored","diskEntriesReusedWithoutRegeneration",
                 "pixelsCompared","corruptedPngHashRegeneratedIdentically","changedOriginalAndDerivedRejectCachedPreview","restoredCopiesReturnIdenticalPreviews"]},
        "ui":{"cases":[{key:case[key] for key in ["case","key","width","locale","theme","paintedRaster","pixelInteraction","indexMetadataVisible","thumbnailsFillUniformCards","cardCount","cardHeight"]} for case in ui["cases"]],
              "actualNativeProcessingButtons":len(ui["createdByUi"]),"liveCatalogItems":explore["liveCatalogRequests"][0]["features"],
              "providerPreviews":len(explore["providerPreviews"]),"unavailableProviderPreviews":len(explore["previewFailures"]),
              "sameProjectRestoredAndReturned":True,"providerCount":15,"nativeWindowTested":False,"usedUserDesktop":False},
        "rendererSnapshot":{"root":str(snapshot),"files":accepted_files,"desktop":{"path":str(desktop_copy),"bytes":desktop_copy.stat().st_size,"sha256":desktop_sha}},
        "summaryControls":gate,
        "boundaries":[
            "Converted PC COGs are not the complete original NASA HDF. Ten other science layers, QA and per-pixel composite dates remain unimplemented.",
            "NDVI and EVI are processed independently; this is not coupled same-observation selection or cloud-free imagery.",
            "Nominal resolution is 250 metres; verified sinusoidal PixelIsArea spacing is approximately 231.656358264 metres. No reprojection or resampling.",
            "Signed DN, scale 0.0001 and fill -3000 are retained; palette clipping affects display only and NoData has no indexValue.",
            "Live catalogue and previews are discovery evidence only. Six source downloads were completed separately and reused without inflating counts.",
            "Production renderer with exact desktop CSP and a native HTTP bridge is not an installed or native WebView/window acceptance.",
            "NASA/Copernicus account entries exist, but successful protected-file authorization awaits accounts. No installer or publication was performed.",
        ],
    }
    destination=WORKSPACE/"prototype/qa/modis-vegetation-verification.json"
    if destination.exists():
        previous=json.loads(destination.read_text(encoding="utf-8"))
        require(previous["status"] == "passed" and previous["nativeBinary"] == summary["nativeBinary"]
                and previous["rendererSnapshot"] == summary["rendererSnapshot"], "Do not replace a different accepted build")
        previous_sha=digest(destination);saved=root/"evidence"/("previous-public-"+previous_sha+".json");shutil.copy2(destination,saved)
        summary["previousPublicReceipt"]={"snapshot":str(saved),"sha256":previous_sha}
    destination.write_text(json.dumps(summary,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
    print(json.dumps({"status":summary["status"],"actualDownloads":6,"downloadBytes":summary["sources"]["totalBytes"],
                      "outputDnCompared":summary["native"]["outputDnCompared"],"previewRgbaPixelsCompared":summary["native"]["previewAndThumbnailRgbaPixelsCompared"],
                      "independentRawPoints":summary["native"]["independentRawPointsCompared"],"mcpRawPoints":summary["mcp"]["rawPointsCompared"],
                      "desktop":summary["rendererSnapshot"]["desktop"],"controls":len(controls),"rendererFiles":len(accepted_files)},ensure_ascii=False))


if __name__ == "__main__":
    main()
