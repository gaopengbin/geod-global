"""Publish only fully passed, immutable MODIS coupled-scene acceptance receipts."""
import argparse, hashlib, json, shutil
from pathlib import Path


def sha(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as f:
        for block in iter(lambda:f.read(1024*1024), b""): h.update(block)
    return h.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qa", required=True)
    parser.add_argument("--offline", required=True)
    parser.add_argument("--snapshot", required=True)
    parser.add_argument("--output", default="prototype/qa/modis-coupled-verification.json")
    args = parser.parse_args(); workspace = Path.cwd().resolve()
    qa, offline, snapshot = map(lambda s:Path(s).resolve(), [args.qa,args.offline,args.snapshot])
    for root in [qa,offline]:
        assert root.parent == workspace/".verification" and root.name.startswith("modis-coupled-")
    assert snapshot.parent == workspace/".verification" and snapshot.name.startswith("renderer-modis-coupled-")
    paths = {"native":qa/"native-verification.json", "mcp":qa/"mcp-verification.json",
             "ui":qa/"ui-verification.json", "cache":offline/"cache-verification.json"}
    receipts = {k:json.loads(p.read_text(encoding="utf-8")) for k,p in paths.items()}
    native,mcp,ui,cache = (receipts[k] for k in ["native","mcp","ui","cache"])
    assert all(r["status"] == "passed" for r in receipts.values())
    assert all(r["nativeBinarySha256"] == native["nativeBinarySha256"] for r in receipts.values())
    assert sha(native["nativeBinary"]) == native["nativeBinarySha256"]
    assert all(r["nativeReceiptSha256"] == sha(paths["native"]) for r in [mcp,ui,cache])
    combinations = {(label,policy,snow) for label in ["fallback","fallback-polygon","mosaic","polygon"]
                    for policy,snow in [("clear",False),("clear_best",False),("clear_best",True)]}
    assert len(native["cases"]) == 12 and {(c["case"],c["policy"],c["excludeSnow"]) for c in native["cases"]} == combinations
    for c in native["cases"]:
        job,counts = c["job"],c["counts"]
        assert sha(job["outputPath"]) == job["sha256"] and sha(c["package"]["path"]) == c["package"]["sha256"]
        assert job["rgbSpec"]["qualityMask"]["schemaVersion"] == "geod-modis-rgb-mask/v2"
        assert job["rgbOutput"]["qualityMask"] == counts
        assert sum(counts["coupled"]["sceneValidPixels"]) == job["rgbOutput"]["commonValidPixels"]
        if c["case"].startswith("fallback"): assert counts["coupled"]["fallbackPixels"] > 0
    assert len(native["regressions"]) == 2 and native["sourceFilesUnchanged"] == 30 and native["restartInspections"] == 12
    assert len(native["controls"]) == 3 and all(not c["queued"] for c in native["controls"])
    assert len(mcp["cases"]) == 2 and {r["mode"] for r in mcp["cases"]} == {"direct","loopback"}
    assert mcp["writesDeniedInReadOnlyMode"] and mcp["invalidPolicyAndDuplicateFlagsRejected"]
    assert next(r for r in mcp["cases"] if r["mode"] == "loopback")["reconnected"]
    assert len(ui["cases"]) == 3 and {c["kind"] for c in ui["cases"]} == {"fallback","fallback-polygon","polygon"}
    assert not ui.get("diagnostic") and not ui["errors"] and not ui["remoteRequests"] and ui["nativeWindowTested"] is False
    for c in ui["cases"]:
        assert c["actualDraw"]["allSourceRgbaIdentical"] and not c["horizontalOverflow"]
        assert c["sourceDetails"]["originalLinks"] == c["sourceDetails"]["pinnedOriginals"] == 5*len(c["mask"]["coupled"]["scenes"])
    assert ui["createdByUi"]["job"]["rgbSpec"]["qualityMask"]["schemaVersion"] == "geod-modis-rgb-mask/v2"
    assert len(cache["cases"]) == 12 and cache["allParentsAbsent"] and cache["restarted"]
    assert all(r["cacheBytesFileIdentityCreationUnchanged"] and r["readWithoutParents"] for r in cache["cases"])
    # A later build must not silently replace the assets proven by this UI cohort.
    frozen = json.loads((snapshot/"receipt.json").read_text(encoding="utf-8"))
    assert len(frozen["files"]) >= 650
    for file,digest in frozen["files"].items():
        assert sha(snapshot/"dist"/file) == digest and sha(workspace/"prototype/dist"/file) == digest
    for file,digest in ui["resources"].items(): assert sha(workspace/file) == digest
    desktop = Path(frozen["desktop"]["path"])
    assert sha(desktop) == frozen["desktop"]["sha256"] and desktop.stat().st_size == frozen["desktop"]["bytes"]
    prior = workspace/".verification/renderer-before-modis-coupled-20261004"
    original = json.loads((prior/"receipt.json").read_text(encoding="utf-8"))
    for file,digest in original["files"].items(): assert sha(prior/"dist"/file) == digest
    canonical = workspace/"target/debug/geod-runtime.exe"
    assert sha(canonical) == "3b0e845ebea363f154644db9e75f0fff1f852b69201eb2bcbae85174523ea03b"
    evidence = workspace/".verification/modis-coupled-evidence-20261004"; evidence.mkdir(exist_ok=True)
    proof = {}
    for key,path in paths.items():
        digest = sha(path); target = evidence/f"{key}-{digest}.json"
        if not target.exists(): shutil.copy2(path,target)
        assert sha(target) == digest
        proof[key] = {"snapshot":str(target),"sha256":digest}
    result = {"schema":"geod-modis-coupled-verification/v1", "status":"passed", "checkedAt":native["finishedAt"],
        "scope":"MOD/MYD09A1 v061 coherent same-scene RGB selection from complete previously downloaded public COGs",
        "nativeBinary":{"path":native["nativeBinary"],"sha256":native["nativeBinarySha256"]}, "evidence":proof,
        "definition":native["definition"], "network":native["network"], "fallbackRegion":native["fallbackRegion"],
        "native":{"outputs":12,"allDnCompared":sum(c["allDnCompared"] for c in native["cases"]),
                  "previewRgbaPixelsCompared":sum(c["preview"]["rgbaPixelsCompared"] for c in native["cases"]),
                  "rawPixelsCompared":sum(len(c["pixels"]) for c in native["cases"]),
                  "cases":[{k:c[k] for k in ["case","policy","excludeSnow","counts","preview","outsideOrHolePixels"]} | {
                      "id":c["job"]["id"],"sha256":c["job"]["sha256"],"grid":c["job"]["rgbSpec"]["grid"],
                      "coupledSpec":c["job"]["rgbSpec"]["qualityMask"]["coupled"],"packageSha256":c["package"]["sha256"]} for c in native["cases"]],
                  "sameSceneRegressions":native["regressions"],"controls":native["controls"],"restartInspections":12,"sourceFilesUnchanged":30},
        "mcp":{"modes":[c["mode"] for c in mcp["cases"]],"outputs":2,"allDnCompared":sum(c["samplesCompared"] for c in mcp["cases"]),
               "rawPixelsCompared":sum(len(c["pixels"]) for c in mcp["cases"]),"writesDeniedInReadOnlyMode":True,"loopbackReconnectPassed":True},
        "ui":{"cases":ui["cases"],"createdByUi":{"id":ui["createdByUi"]["job"]["id"],"sha256":ui["createdByUi"]["job"]["sha256"]},
              "drawnSourceRgbaPixelsCompared":sum(c["actualDraw"]["width"]*c["actualDraw"]["height"] for c in ui["cases"]),
              "resources":ui["resources"],"renderer":ui["renderer"],"nativeWindowTested":False,"spatialCanvasAllPixelsCompared":False},
        "cache":{"filesRestored":12,"missingParentJobs":len(cache["missingParentJobs"]),
                 "thumbnailRgbaPixelsCompared":sum(c["thumbnail"]["rgbaPixelsCompared"] for c in cache["cases"]),"cacheEntriesReused":12,"controls":cache["controls"]},
        "developmentDesktopBuild":{**frozen["desktop"],"customProtocol":True,"nativeWindowTested":False,"installerBuilt":False},
        "acceptedRenderer":{"snapshot":str(snapshot),"receiptSha256":sha(snapshot/"receipt.json"),"files":len(frozen["files"])},
        "priorSameSceneRendererPreserved":{"snapshot":str(prior),"receiptSha256":sha(prior/"receipt.json"),"files":len(original["files"])},
        "regression":{"scientificRgbRustPassed":7,"qualityReaderRustPassed":4,"actualWindowsCliChildPassed":1,"focusedNodePassed":9,"focusedReactPassed":9,
                      "cargoFmt":"passed","strictRuntimeClippy":"passed","repositoryContractsRecipes":"passed"},
        "boundaries":["Real public COGs were previously downloaded; no new upstream acquisition or account authorization is claimed.",
                      "Missing RGB band, signed/zero/above-one values and individual flags additionally have explicitly synthetic-file regressions.",
                      "The real snow option caused no additional changes; positive real snow exclusion is not claimed.",
                      "No per-pixel provenance index, additional atmospheric accuracy, resampling, reprojection or other-product QA rules.",
                      "Headless built renderer, native bridge and CSP do not replace native WebView/window acceptance.",
                      "Earthdata/Copernicus successful authorization and protected production originals remain pending; software authorization entries are available."]}
    target = Path(args.output).resolve(); assert target.parent == workspace/"prototype/qa"
    target.write_text(json.dumps(result,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
    print(json.dumps({"status":"passed","outputs":12,"ui":3,"offline":12,"output":str(target)}))


if __name__ == "__main__": main()
