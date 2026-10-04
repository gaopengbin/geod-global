"""Actual stdio MCP quality-mask creation through directory and loopback modes."""
import argparse, importlib.util, json, subprocess, time, urllib.request
from pathlib import Path
import numpy as np
import rasterio
spec=importlib.util.spec_from_file_location("rgb_mcp",Path(__file__).with_name("verify-scientific-rgb-mcp.py"));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
spec2=importlib.util.spec_from_file_location("mask_science",Path(__file__).with_name("verify-modis-rgb-mask.py"));science=importlib.util.module_from_spec(spec2);spec2.loader.exec_module(science)
spec4=importlib.util.spec_from_file_location("landsat_science",Path(__file__).with_name("verify-landsat-rgb-mask.py"));landsat_science=importlib.util.module_from_spec(spec4);spec4.loader.exec_module(landsat_science)
spec3=importlib.util.spec_from_file_location("coupled_science",Path(__file__).with_name("verify-modis-coupled.py"));coupled_science=importlib.util.module_from_spec(spec3);spec3.loader.exec_module(coupled_science)
spec5=importlib.util.spec_from_file_location("landsat_coupled",Path(__file__).with_name("verify-landsat-coupled.py"));landsat_coupled=importlib.util.module_from_spec(spec5);spec5.loader.exec_module(landsat_coupled)

def main():
    p=argparse.ArgumentParser();p.add_argument("--root",required=True);p.add_argument("--port",type=int,default=4623);a=p.parse_args();root=Path(a.root).resolve()
    coupled=root.name.startswith(("modis-coupled-","landsat-coupled-"));landsat=root.name.startswith(("landsat-rgb-mask-","landsat-coupled-"));policy="cloud_free_conservative" if landsat else "clear_best"
    assert root.parent==Path(".verification").resolve() and root.name.startswith(("modis-rgb-mask-","modis-coupled-","landsat-rgb-mask-","landsat-coupled-"))
    native=json.loads((root/"native-verification.json").read_text(encoding="utf-8"));assert native["status"]=="passed"
    fixture=json.loads((root/"ui-fixture.json").read_text(encoding="utf-8"));exe=fixture["nativeBinary"];assert module.sha(exe)==native["nativeBinarySha256"]
    layers=fixture["cases"]["fallback"]["jobs"] if coupled else fixture["single"]
    request={"jobIds":[j["id"] for j in layers[:3]],"qualityMask":{"qaPixelJobId" if landsat else "qcJobId":layers[3]["id"],"qaRadsatJobId" if landsat else "stateJobId":layers[4]["id"],"policy":policy,"excludeSnow":True}}
    if coupled:
        reference=next(c for c in native["cases"] if c["case"]=="fallback" and c["policy"]==policy and c["excludeSnow"])
        jobs=json.loads((root/"jobs.json").read_text(encoding="utf-8"))
        expected,counts,*_=(landsat_coupled if landsat else coupled_science).coherent_oracle(reference["job"]["rgbSpec"],jobs)
    elif landsat:expected,counts=landsat_science.expected_rgb(*landsat_science.read_layers(layers),request["qualityMask"])
    else:expected,counts=science.expected_rgb(science.read_layers(layers),request["qualityMask"])
    report={"schema":"geod-landsat-coupled-mcp/v1" if landsat and coupled else "geod-landsat-rgb-mask-mcp/v1" if landsat else "geod-modis-coupled-mcp/v1" if coupled else "geod-modis-rgb-mask-mcp/v1","qaOnly":True,"nativeBinarySha256":native["nativeBinarySha256"],"nativeReceiptSha256":module.sha(root/"native-verification.json"),"cases":[]}
    def discovery(c,write):
        tools=c.request("tools/list")["result"]["tools"];names={t["name"] for t in tools};assert "geod_rgb_plan" in names
        assert ("geod_rgb_run" in names)==write and ("geod_rgb_package" in names)==write
        schema=next(t for t in tools if t["name"]=="geod_rgb_plan")["inputSchema"]["properties"]["request"]["properties"]["qualityMask"]
        variants=schema.get("oneOf",[schema]);chosen=next(v for v in variants if v["properties"]["policy"]["enum"]==(["cloud_free","cloud_free_conservative"] if landsat else ["clear","clear_best"]))
        assert chosen["additionalProperties"] is False
        assert chosen["required"]==(["qaPixelJobId","qaRadsatJobId","policy"] if landsat else ["qcJobId","stateJobId","policy"])
        if not write:assert c.request("tools/call",{"name":"geod_rgb_run","arguments":{"request":request}})["error"]["code"]==-32602
        return schema
    def inspect(c,job):
        data=c.call("geod_rgb_inspect",{"id":job["id"]});assert data["previewOmitted"] and data["artifact"]["sha256"]==job["sha256"]
        with rasterio.open(job["outputPath"]) as ds:
            assert np.array_equal(ds.read(),expected)
            pixels=[]
            for row,col in [(0,0),(ds.height//2,ds.width//2),(ds.height-1,ds.width-1)]:
                x,y=ds.xy(row,col);point=c.call("geod_rgb_pixel",{"id":job["id"],"x":x,"y":y});assert point["values"]==expected[:,row,col].tolist();pixels.append(point)
        assert job["rgbOutput"]["qualityMask"]==counts and module.sha(job["outputPath"])==job["sha256"]
        return {"job":job,"metadata":data,"pixels":pixels,"samplesCompared":expected.size}
    def settle(c,id):
        # Landsat selection decodes complete 60-million-pixel originals in a
        # development runtime; use the same bounded wait as native acceptance.
        deadline=time.monotonic()+(600 if landsat and coupled else 180)
        while time.monotonic()<deadline:
            job=c.call("geod_job_status",{"id":id})
            if job["settled"] and job["status"] not in ["queued","running"]:assert job["status"]=="succeeded",job;return job
            time.sleep(.2)
        raise AssertionError("MCP job did not settle")
    with module.client(exe,data_dir=root) as c:
        discovery(c,False);plan=c.call("geod_rgb_plan",{"request":request});assert plan["spec"]["qualityMask"]["policy"]==policy
        if coupled:assert plan["spec"]["qualityMask"]==reference["job"]["rgbSpec"]["qualityMask"]
        wrong={**request,"qualityMask":{**request["qualityMask"],"policy":"unrecognized"}}
        assert c.request("tools/call",{"name":"geod_rgb_plan","arguments":{"request":wrong}})["error"]["code"]==-32602
        wrong={**request,"qualityMask":{**request["qualityMask"],"qaRadsatJobId" if landsat else "stateJobId":request["qualityMask"]["qaPixelJobId" if landsat else "qcJobId"]}}
        assert c.request("tools/call",{"name":"geod_rgb_plan","arguments":{"request":wrong}})["error"]["code"]==-32602
    with module.client(exe,data_dir=root,allow_write=True) as c:
        discovery(c,True);submitted=c.call("geod_rgb_run",{"request":{**request,"name":"QA · direct MCP quality-screened RGB"}});job=settle(c,submitted["jobId"])
        entry=inspect(c,job);entry["mode"]="direct";entry["package"]=c.call("geod_rgb_package",{"id":job["id"]});report["cases"].append(entry)
    base=f"http://127.0.0.1:{a.port}";process=subprocess.Popen([exe,"serve","--data-dir",str(root),"--port",str(a.port)],stdout=(root/"mcp.stdout.log").open("ab"),stderr=(root/"mcp.stderr.log").open("ab"),creationflags=getattr(subprocess,"CREATE_NO_WINDOW",0))
    try:
        for _ in range(100):
            assert process.poll() is None
            try:
                req=urllib.request.Request(base+"/health",headers={"X-GeoD-Client":"geod-global"})
                with urllib.request.urlopen(req,timeout=3) as r:assert Path(json.load(r)["storageRoot"]).samefile(root)
                break
            except OSError:time.sleep(.1)
        with module.client(exe,server=base,allow_write=True) as c:
            discovery(c,True);submitted=c.call("geod_rgb_run",{"request":{**request,"name":"QA · loopback MCP quality-screened RGB"}})
            assert submitted["job"]["status"] in ["queued","running"]
        with module.client(exe,server=base) as c:
            discovery(c,False);job=settle(c,submitted["jobId"]);entry=inspect(c,job)
        with module.client(exe,server=base,allow_write=True) as c:entry["package"]=c.call("geod_rgb_package",{"id":job["id"]})
        entry.update(mode="loopback",disconnectedBeforeSettlement=True,reconnected=True);report["cases"].append(entry)
    finally:
        if process.poll() is None:process.terminate();process.wait(timeout=20)
    report.update(status="passed",writesDeniedInReadOnlyMode=True,invalidPolicyAndDuplicateFlagsRejected=True,cleanProtocolEof=True)
    science.dump(root/"mcp-verification.json",report);print(json.dumps({"status":"passed","cases":len(report["cases"]),"samples":sum(c["samplesCompared"] for c in report["cases"])}))
if __name__=="__main__":main()
