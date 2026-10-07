"""Independently verify a positive live-vector Agent case through real MCP.

Refetch public receipts, compare every original feature/property/coordinate,
reconstruct all features through paged MCP nodes, then repeat with both data-dir
and loopback owners while their public proxy is an owned rejecting trap.
No model call, new extraction, user store, desktop or publication is performed.
"""
import argparse
from collections import deque
from datetime import datetime, timezone
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import json
import os
from pathlib import Path
import socket
import subprocess
import threading
import time
from urllib.request import Request, ProxyHandler, build_opener
from shapely.geometry import shape, box


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


transport = module("vector_mcp_transport", "verify-mcp.py")
sha = lambda value: hashlib.sha256(value).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", required=True, type=Path)
    parser.add_argument("--executable", required=True, type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    case = args.case.resolve(strict=True)
    assert case.is_relative_to(root / ".verification") and case.name == "case"
    native = json.loads((case / "native-acceptance.json").read_text(encoding="utf-8"))
    assert native["status"] == "passed" and not native["modelPerformedExtraction"]
    inspection = json.loads((case / "original-inspection.json").read_text(encoding="utf-8"))
    service = json.loads((case / "saved-service.json").read_text(encoding="utf-8"))
    asset, geojson = inspection["asset"], inspection["geojson"]
    store = case / "core"
    binary = args.executable.resolve(strict=True)
    assert binary.is_relative_to(root)
    output = case.parent / "vector-data-verification"
    if output.exists():
        output = case.parent / f"vector-data-verification-{time.time_ns()}"
    output.mkdir()
    report = {"schema":"geod-agent-vector-data-acceptance/v1", "status":"pending",
              "startedAt":datetime.now(timezone.utc).isoformat(), "binarySha256":sha(binary.read_bytes()),
              "actualModelCallsHere":0, "usedUserDesktop":False, "nativeWindowTested":False,
              "newFilesExtractedHere":0, "published":False, "backends":[], "publicRefetches":[]}
    def save():
        (output / "acceptance.json").write_text(json.dumps(report, ensure_ascii=False, indent=2),encoding="utf-8")
    original_proxy = (store / "proxy-settings.json").read_bytes()
    registry_before = {f:sha((store / f).read_bytes()) for f in ("vectors.json", "feature-services.json")}
    registry = json.loads((store / "vectors.json").read_text(encoding="utf-8"))
    source_file = Path(registry[asset["id"]]["path"])
    assert source_file.parent.samefile(store / "vectors")
    assert sha(source_file.read_bytes()) == asset["sourceSha256"]
    public = build_opener()
    refetched = []
    seen = {}
    trap_hits = []
    class Trap(BaseHTTPRequestHandler):
        def hit(self):
            trap_hits.append(self.path)
            self.send_error(502,"Public requests disabled during verified local reads")
        do_CONNECT = hit
        do_GET = hit
        do_POST = hit
        def log_message(self,*_):
            pass
    trap = ThreadingHTTPServer(("127.0.0.1",0),Trap)
    trap_thread = threading.Thread(target=trap.serve_forever,daemon=True)
    trap_thread.start()
    server, client = None, None
    log = None
    try:
        source = asset.get("remoteSource") or asset["osmSource"]
        protocol = "WFS 2" if source.get("wfs") else "ArcGIS" if source.get("arcgis") else "Overpass" if asset.get("osmSource") else "OGC API Features"
        if protocol != "OGC API Features":
            checker = module("agent_vector_protocol_reference", "vector-agent-protocol-reference.py")
            refetched = checker.verify(binary, store, inspection, service, registry[asset["id"]], output, public, report)
        for receipt in source.get("pages", []) if protocol == "OGC API Features" else []:
            assert receipt["url"].startswith("https://demo.pygeoapi.io/stable/collections/lakes/items?")
            with public.open(Request(receipt["url"],headers={"Accept":"application/geo+json, application/json"}),timeout=30) as response:
                assert response.status == 200 and response.geturl() == receipt["url"]
                raw=response.read(20*1024*1024+1)
                assert len(raw)<=20*1024*1024
            page=json.loads(raw)
            assert page["numberReturned"] == receipt["returned"] == len(page["features"])
            report["publicRefetches"].append({"url":receipt["url"],"sha256":sha(raw),"bytes":len(raw),"returned":len(page["features"]),
                "matchesAcquisitionReceiptBytes":sha(raw)==receipt["sha256"]})
            for feature in page["features"]:
                identity=json.dumps(feature.get("id"),sort_keys=True) if "id" in feature else None
                if identity is not None and identity in seen:
                    assert feature == seen[identity]
                    continue
                if identity is not None:
                    seen[identity]=feature
                refetched.append(feature)
        assert refetched == geojson["features"], "Native saved geometry/attributes differ from independently refetched originals"
        assert len(refetched)==asset["featureCount"]
        if "numberMatched" in source:
            assert len(refetched)==source["numberMatched"]
        coordinates=0
        def positions(value):
            if isinstance(value,list):
                if value and isinstance(value[0],(int,float)):
                    return 1
                return sum(positions(v) for v in value)
            if isinstance(value,dict):
                return positions(value.get("coordinates",[]))+sum(positions(v) for v in value.get("geometries",[]))
            return 0
        for feature in refetched:
            geom=shape(feature["geometry"])
            assert geom.is_valid and geom.intersects(box(*source["requestedBounds"]))
            coordinates+=positions(feature["geometry"])
        assert coordinates==asset["coordinateCount"]
        report["independent"]={"protocol":protocol,"features":len(refetched),"coordinatePositions":coordinates,"attributesAndCoordinates":"independent original format reference" if protocol in ("WFS 2","Overpass") else "exact equality with independently refetched original pages",
            "geometryValidation":"Shapely/GEOS validity and requested bbox intersection; complete original geometry is retained","sourceBytes":asset["bytes"],"sourceSha256":asset["sourceSha256"]}
        (output / "refetched-features.json").write_text(json.dumps(refetched,ensure_ascii=False),encoding="utf-8")
        (store / "proxy-settings.json").write_text(json.dumps({"mode":"custom","url":f"http://127.0.0.1:{trap.server_port}"}),encoding="utf-8")
        loopback=build_opener(ProxyHandler({}))
        with socket.socket() as sock:
            sock.bind(("127.0.0.1",0))
            port=sock.getsockname()[1]
        origin=f"http://127.0.0.1:{port}"
        for mode in ("data-dir","loopback"):
            calls=[]
            if mode=="loopback":
                log=(output / "runtime.log").open("wb")
                server=subprocess.Popen([str(binary),"serve","--data-dir",str(store),"--port",str(port)],stdout=log,stderr=log,
                    creationflags=getattr(subprocess,"CREATE_NO_WINDOW",0))
                for attempt in range(100):
                    assert server.poll() is None
                    try:
                        with loopback.open(origin+"/health",timeout=2) as response:
                            health=json.load(response)
                        assert Path(health["storageRoot"]).samefile(store)
                        break
                    except OSError:
                        if attempt==99: raise
                        time.sleep(0.1)
                client=transport.Client(binary,server=origin)
            else:
                client=transport.Client(binary,data_dir=store)
            tools=client.request("tools/list")["result"]["tools"]
            reads={"geod_feature_services","geod_feature_collections","geod_vectors_list","geod_vector_inspect","geod_vector_features","geod_vector_node"}
            assert reads <= {t["name"] for t in tools}
            assert all(t["annotations"]["readOnlyHint"] and not t["annotations"]["openWorldHint"] for t in tools if t["name"] in reads)
            denied=client.request("tools/call",{"name":"geod_feature_query","arguments":{}})
            assert denied["error"]["code"]==-32602
            def call(name,args):
                value=client.call(name,args)
                calls.append({"name":name,"arguments":args,"result":value})
                return value
            def pages(name,args,key):
                values,offset=[],0
                while True:
                    page=call(name,{**args,"offset":offset,"limit":2})
                    values+=page[key]
                    if page["nextOffset"] is None:
                        assert page["complete"]
                        break
                    assert page["nextOffset"]>offset
                    offset=page["nextOffset"]
                return values
            assert pages("geod_feature_services",{},"services")[0]["id"]==service["id"]
            assert pages("geod_feature_collections",{"id":service["id"]},"collections")==service["collections"]
            assert pages("geod_vectors_list",{},"vectors")[0]["id"]==asset["id"]
            metadata=call("geod_vector_inspect",{"id":asset["id"]})
            assert metadata["verified"] and metadata["asset"]==asset
            summaries=pages("geod_vector_features",{"id":asset["id"]},"features")
            assert [s["index"] for s in summaries]==list(range(len(refetched)))
            rebuilt=[]
            for index,feature in enumerate(refetched):
                tasks=deque([(None,None,"")])
                result=None
                while tasks:
                    parent,key,pointer=tasks.popleft()
                    offset=0
                    value=None
                    while True:
                        node=call("geod_vector_node",{"id":asset["id"],"feature":index,"pointer":pointer,"offset":offset,"limit":20})
                        assert node["source"]["verified"] and node["source"]["geojsonSha256"]==asset["geojsonSha256"]
                        kind=node["nodeType"]
                        if offset==0:
                            value={} if kind=="object" else [None]*node["page"]["total"] if kind=="array" else "" if kind=="string" else node["value"]
                            if parent is None: result=value
                            else: parent[key]=value
                        if kind in ("object","array"):
                            for entry in node["page"]["entries"]:
                                entry_key=entry["key"] if kind=="object" else entry["index"]
                                data=entry["data"]
                                value[entry_key]=data.get("inline")
                                if data.get("valueOmitted"):
                                    tasks.append((value,entry_key,data["read"]["arguments"]["pointer"]))
                            next_offset=node["page"]["nextOffset"]
                        elif kind=="string":
                            value+=node["text"]
                            if parent is None: result=value
                            else: parent[key]=value
                            next_offset=node["nextOffset"]
                        else: next_offset=None
                        if next_offset is None: break
                        assert next_offset>offset
                        offset=next_offset
                assert result==feature
                rebuilt.append(result)
            assert rebuilt==refetched
            client.close();client=None
            if server:
                server.terminate();server.wait(timeout=20);server=None
                log.close();log=None
            assert {f:sha((store/f).read_bytes()) for f in registry_before}==registry_before
            assert sha(source_file.read_bytes())==asset["sourceSha256"]
            assert not trap_hits, "Read tools attempted an external request"
            (output / (mode+"-calls.json")).write_text(json.dumps(calls,ensure_ascii=False,indent=2),encoding="utf-8")
            report["backends"].append({"mode":mode,"calls":len(calls),"featuresReconstructed":len(rebuilt),"completeOriginalEquality":True,
                "sourceAndRegistriesUnchanged":True,"publicProxyRequests":len(trap_hits),"writeToolDenied":True})
        report["status"]="passed"
        report["limitations"]=["Live model acceptance is separate; only its recorded route was tested",
            "Public demo availability and licensing are not guaranteed","Hash/geometry equality does not establish scientific accuracy or feature completeness outside the requested bbox"]
    except Exception as error:
        report.update(status="failed",error=str(error))
        raise
    finally:
        if client:
            client.close()
        if server and server.poll() is None:
            server.terminate();server.wait(timeout=20)
        if log:log.close()
        (store / "proxy-settings.json").write_bytes(original_proxy)
        trap.shutdown();trap.server_close()
        report["finishedAt"]=datetime.now(timezone.utc).isoformat()
        save()
        print(json.dumps({"status":report["status"],"directory":str(output),"backends":report["backends"]}),flush=True)


if __name__=="__main__":
    main()
