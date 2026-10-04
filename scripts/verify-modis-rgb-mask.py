"""Offline native acceptance of MODIS RGB quality screening using real, previously
downloaded public COGs. Every output DN and preview pixel is independently read
with Rasterio/GDAL and NumPy. Writes only a fresh private QA workspace.
"""
import argparse, atexit, base64, copy, hashlib, io, json, shutil, subprocess, time, urllib.request, zipfile
from datetime import datetime, timezone
from pathlib import Path
import numpy as np
import rasterio
from PIL import Image

KEYS = ["red", "green", "blue", "modis_qc", "modis_state"]
GUIDE = "https://landweb.modaps.eosdis.nasa.gov/data/userguide/MOD09_User_Guide_V61.pdf"
FILL = -28672

def sha(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as f:
        for block in iter(lambda: f.read(1024 * 1024), b""): h.update(block)
    return h.hexdigest()

def dump(path, data):
    Path(path).write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

def read_layers(jobs):
    arrays = []
    for job in jobs:
        assert sha(job["outputPath"]) == job["sha256"]
        with rasterio.open(job["outputPath"]) as ds: arrays.append(ds.read(1))
    return arrays

def expected_rgb(arrays, mask):
    rgb = np.stack(arrays[:3]); input_valid = np.all(rgb != FILL, axis=0)
    if not mask: return rgb, None
    qc, state = arrays[3:]
    assert qc.dtype == np.uint32 and state.dtype == np.uint16
    clear = ((state & 3) == 0) & ((state & (4 | 768 | 1024 | 8192)) == 0)
    accepted = clear & (qc != 4294967295) & (state != 65535)
    if mask["policy"] == "clear_best":
        accepted &= ((qc & 3) == 0) & (((qc >> 2) & 15) == 0) & (((qc >> 10) & 15) == 0) & (((qc >> 14) & 15) == 0)
    if mask["excludeSnow"]: accepted &= ((state & 4096) == 0) & ((state & 32768) == 0)
    rgb[:, ~accepted] = FILL
    counts = {"examinedPixels": accepted.size, "rejectedPixels": int((~accepted).sum()),
              "inputCommonValidPixels": int(input_valid.sum()), "removedValidPixels": int((input_valid & ~accepted).sum())}
    return rgb, counts

def preview_check(data, rgb, path):
    pw, ph = data["previewWidth"], data["previewHeight"]
    sy = np.arange(ph, dtype=np.int64) * rgb.shape[1] // ph
    sx = np.arange(pw, dtype=np.int64) * rgb.shape[2] // pw
    samples = rgb[:, sy[:, None], sx[None, :]].astype(np.int32)
    valid = np.all(samples != FILL, axis=0); rgba = np.zeros((ph, pw, 4), dtype=np.uint8)
    for band in range(3):
        values = np.sort(samples[band][valid]); bounds = [0, 0] if not values.size else values[[(values.size-1)*2//100, (values.size-1)*98//100]].tolist()
        assert bounds == data["composite"]["displayRanges"][band]
        low, high = bounds
        rgba[:, :, band][valid] = 128 if low == high else np.floor(np.clip((samples[band][valid]-low)/(high-low),0,1)*255+0.5).astype(np.uint8)
    rgba[:, :, 3][valid] = 255
    png = base64.b64decode(data["previewDataUrl"].split(",",1)[1]); Path(path).write_bytes(png)
    assert np.array_equal(np.asarray(Image.open(io.BytesIO(png)).convert("RGBA")), rgba)
    assert data["composite"]["validSampleCount"] == int(valid.sum())
    return {"width":pw,"height":ph,"rgbaPixelsCompared":pw*ph,"pngSha256":hashlib.sha256(png).hexdigest()}

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument("--root",required=True);p.add_argument("--source",default=".verification/modis-quality-processing-20261004");p.add_argument("--exe",required=True);p.add_argument("--port",type=int,default=4619);a=p.parse_args()
    root=Path(a.root).resolve();source=Path(a.source).resolve();exe_source=Path(a.exe).resolve()
    assert root.parent == Path(".verification").resolve() and root.name.startswith("modis-rgb-mask-") and not root.exists()
    assert source.parent == root.parent
    receipt_path=source/"native-processing-verification.json";old=json.loads(receipt_path.read_text(encoding="utf-8"));assert old["status"]=="passed"
    old_jobs=json.loads((source/"jobs.json").read_text(encoding="utf-8"));old_projects=json.loads((source/"projects.json").read_text(encoding="utf-8"))
    projects={c["name"]:c["project"] for c in old["cases"]}
    root.mkdir();(root/"assets").mkdir();binary_sha=sha(exe_source);exe=root/f"runtime-{binary_sha[:16]}.exe";shutil.copy2(exe_source,exe);assert sha(exe)==binary_sha
    selected={j["id"]:copy.deepcopy(j) for j in old["originals"]}
    for case in ["single","mosaic"]:
        for key in KEYS:
            row=next(r for r in old["outputs"] if r["case"]==case and r["key"]==key)
            selected[row["job"]["id"]]=copy.deepcopy(old_jobs[row["job"]["id"]])
    before=[]
    for job in selected.values():
        src=Path(job["outputPath"]);assert sha(src)==job["sha256"]
        before.append({"path":str(src),"sha256":job["sha256"],"mtimeNs":src.stat().st_mtime_ns})
        target=root/"assets"/f"{job['id']}.tif";shutil.copy2(src,target);job["outputPath"]=str(target)
        if job.get("manifestPath"):
            metadata=root/"assets"/f"{job['id']}.metadata.json";shutil.copy2(job["manifestPath"],metadata);job["manifestPath"]=str(metadata)
    dump(root/"jobs.json",selected);dump(root/"projects.json",old_projects)
    dump(root/"proxy-settings.json",{"mode":"custom","url":"http://127.0.0.1:9"})
    report={"schema":"geod-modis-rgb-mask-native/v1","status":"running","checkedAt":datetime.now(timezone.utc).isoformat(),"qaOnly":True,
        "nativeBinarySha256":binary_sha,"nativeBinary":str(exe),"sourceReceiptSha256":sha(receipt_path),"definition":GUIDE,
        "independentReader":f"Rasterio {rasterio.__version__} / GDAL {rasterio.__gdal_version__}","network":"provider traffic disabled via loopback:9","cases":[],"controls":[]}
    dump(root/"input-snapshots.json",{"jobs":selected,"projects":old_projects,"sourceFiles":before,"sourceReceiptSha256":sha(receipt_path)})
    process=None;base=f"http://127.0.0.1:{a.port}";opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
    def stop():
        nonlocal process
        if process and process.poll() is None:process.terminate();process.wait(timeout=20)
        process=None
    atexit.register(stop)
    def api(path,body=None):
        req=urllib.request.Request(base+path,data=json.dumps(body).encode() if body is not None else None,headers={"Content-Type":"application/json","X-GeoD-Client":"geod-global"})
        with opener.open(req,timeout=120) as res:return json.load(res)
    def start():
        nonlocal process
        process=subprocess.Popen([str(exe),"serve","--data-dir",str(root),"--port",str(a.port)],stdout=(root/"runtime.stdout.log").open("ab"),stderr=(root/"runtime.stderr.log").open("ab"),creationflags=getattr(subprocess,"CREATE_NO_WINDOW",0))
        for _ in range(100):
            assert process.poll() is None,(root/"runtime.stderr.log").read_text(encoding="utf-8")
            try: assert Path(api("/health")["storageRoot"]).samefile(root);return
            except OSError:time.sleep(.1)
        raise AssertionError("QA service did not start")
    def wait(job):
        deadline=time.monotonic()+240
        while time.monotonic()<deadline:
            job=api(f"/jobs/{job['id']}");assert job["status"] not in ["failed","cancelled","interrupted"],job
            if job["status"]=="succeeded" and job["settled"]:return job
            time.sleep(.2)
        raise AssertionError("Job did not settle")
    def cli(*args,success=True):
        r=subprocess.run([str(exe),"scientific-rgb",*args,"--server",base],capture_output=True,text=True,encoding="utf-8",timeout=300,creationflags=getattr(subprocess,"CREATE_NO_WINDOW",0))
        if success:assert r.returncode==0,r.stderr;return json.loads(r.stdout)
        assert r.returncode!=0;return r.stderr.strip()
    try:
        start()
        item="MYD09A1.A2025177.h08v05.061.2025189031924"
        originals=[next(j for j in selected.values() if j["kind"]=="download" and j["itemId"]==item and j["assetKey"]==k) for k in KEYS]
        single=[next(r["job"] for r in old["outputs"] if r["case"]=="single" and r["key"]==k) for k in KEYS]
        single=[selected[j["id"]] for j in single]
        draft={k:projects["single"][k] for k in ["bounds","scenes"]};draft["name"]="QA · same-scene quality mask · polygon with hole";draft["geometry"]=projects["polygon"]["geometry"]
        polygon_project=api("/projects",draft);polygon=[wait(api(f"/projects/{polygon_project['id']}/mosaics",{"assetKey":key})) for key in KEYS]
        dump(root/"polygon-inputs.json",{"project":polygon_project,"jobs":polygon})
        for label,layers,project in [("original",originals,None),("single",single,projects["single"]["id"]),("polygon",polygon,polygon_project["id"])]:
            arrays=read_layers(layers)
            for policy,snow in [(None,False),("clear",False),("clear_best",False),("clear_best",True)]:
                name=f"QA · {label} · {policy or 'unmasked'} · snow {snow}";request={"jobIds":[j["id"] for j in layers[:3]],"name":name}
                if project:request["projectId"]=project
                mask={"qcJobId":layers[3]["id"],"stateJobId":layers[4]["id"],"policy":policy,"excludeSnow":snow} if policy else None
                if mask:request["qualityMask"]=mask
                slug=f"{label}-{policy or 'unmasked'}-{snow}";request_path=root/f"{slug}-request.json";dump(request_path,request)
                plan=cli("plan","--request",str(request_path));job=cli("run","--request",str(request_path));assert job["status"]=="succeeded" and sha(job["outputPath"])==job["sha256"]
                expected,counts=expected_rgb(arrays,mask)
                with rasterio.open(layers[0]["outputPath"]) as src,rasterio.open(job["outputPath"]) as out:
                    assert out.crs==src.crs and out.transform==src.transform and out.bounds==src.bounds
                    assert out.count==3 and out.dtypes==("int16",)*3 and out.nodata==FILL and out.scales==(.0001,)*3 and out.offsets==(0.,)*3
                    assert np.array_equal(out.read(),expected)
                    assert sha(job["outputPath"])==job["sha256"]
                output=job["rgbOutput"]
                assert output.get("qualityMask")==counts
                assert output["channelValidPixels"]==[int((band!=FILL).sum()) for band in expected]
                assert output["commonValidPixels"]==int(np.all(expected!=FILL,axis=0).sum())
                assert output["samplesSha256"]==[hashlib.sha256(band.astype('<i2').tobytes()).hexdigest() for band in expected]
                metadata=api(f"/jobs/{job['id']}/rgb");assert metadata["artifact"]["sha256"]==job["sha256"]
                preview=preview_check(metadata,expected,root/f"{slug}-preview.png")
                samples=[]
                for choose in [np.all(expected!=FILL,axis=0),~np.all(expected!=FILL,axis=0)]:
                    points=np.argwhere(choose)
                    if len(points):
                        for row,col in [points[0],points[len(points)//2],points[-1]]:
                            xy=[metadata["bounds"][0]+(int(col)+.5)*metadata["pixelSize"][0],metadata["bounds"][3]-(int(row)+.5)*metadata["pixelSize"][1]]
                            sample=cli("pixel","--id",job["id"],"--x",str(xy[0]),"--y",str(xy[1]));assert sample["values"]==expected[:,row,col].tolist()
                            samples.append(sample)
                package=cli("package","--id",job["id"]);assert sha(package["path"])==package["sha256"]
                with zipfile.ZipFile(package["path"]) as z:
                    assert z.testzip() is None and len(z.namelist())==5
                    assert hashlib.sha256(z.read(f"{job['id']}.tif")).hexdigest()==job["sha256"]
                    manifest=json.loads(z.read(f"{job['id']}.metadata.json"));assert manifest["spec"]==job["rgbSpec"] and manifest["output"]["samples"]==output
                    assert ("quality-layer pins" in z.read("README.txt").decode())==bool(mask)
                entry={"case":label,"policy":policy,"excludeSnow":snow,"request":request,"plan":plan,"job":job,"allDnCompared":expected.size,"counts":counts,"preview":preview,"pixels":samples,"package":package}
                report["cases"].append(entry);dump(root/"native-verification.json",report)
                print(json.dumps({"case":label,"policy":policy,"snow":snow,"samples":expected.size,"removed":counts and counts["removedValidPixels"]}),flush=True)
        # Independently generated mosaics cannot attest to same-scene selection.
        multi=[selected[next(r["job"]["id"] for r in old["outputs"] if r["case"]=="mosaic" and r["key"]==k)] for k in KEYS]
        bad={"jobIds":[j["id"] for j in multi[:3]],"qualityMask":{"qcJobId":multi[3]["id"],"stateJobId":multi[4]["id"],"policy":"clear","excludeSnow":False}}
        dump(root/"independent-mosaic-control.json",bad);message=cli("plan","--request",str(root/"independent-mosaic-control.json"),success=False)
        assert "coupled" in message;report["controls"].append({"name":"independent multi-scene QA rejected","message":message})
        for label,key,value in [("wrong-period QC","qcJobId",next(j["id"] for j in selected.values() if j["kind"]=="download" and j["assetKey"]=="modis_qc" and 'A2025169' in j["itemId"])),("duplicate quality IDs","stateJobId",originals[3]["id"])]:
            wrong={"jobIds":[j["id"] for j in originals[:3]],"qualityMask":{"qcJobId":originals[3]["id"],"stateJobId":originals[4]["id"],"policy":"clear","excludeSnow":False}}
            wrong["qualityMask"][key]=value;path=root/f"{label.replace(' ','-')}-control.json";dump(path,wrong)
            report["controls"].append({"name":label,"message":cli("plan","--request",str(path),success=False)})
        stop();start()
        for entry in report["cases"]:
            job=api(f"/jobs/{entry['job']['id']}");assert job["rgbSpec"]==entry["job"]["rgbSpec"] and job["rgbOutput"]==entry["job"]["rgbOutput"]
            assert cli("inspect","--id",job["id"])["artifact"]["sha256"]==job["sha256"]
        for pin in before:assert sha(pin["path"])==pin["sha256"] and Path(pin["path"]).stat().st_mtime_ns==pin["mtimeNs"]
        report.update(status="passed",originalSourceFilesUnchanged=True,restartRulesAndResultsUnchanged=True)
        report["scope"]="Original same-scene RGB, same-scene rectangular and polygon-with-hole clips; independent multi-scene mosaics rejected, coupled quality-aware mosaics pending"
        dump(root/"native-verification.json",report)
        dump(root/"ui-fixture.json",{"originals":originals,"single":single,"polygon":polygon,"polygonProject":polygon_project,"cases":report["cases"],"nativeBinarySha256":binary_sha,"nativeBinary":str(exe)})
        print(json.dumps({"status":"passed","cases":len(report["cases"]),"dnSamples":sum(c["allDnCompared"] for c in report["cases"]),"previewPixels":sum(c["preview"]["rgbaPixelsCompared"] for c in report["cases"])}),flush=True)
    finally:stop()

if __name__=="__main__":main()
