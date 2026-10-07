"""Independent source-format checks for actual Agent vector acceptance cases.

Uses archived original responses for GML/OSM and independently refetches ArcGIS
GeoJSON plus Esri JSON. No acquisition, model call, credential or desktop action.
"""
from collections import Counter
import importlib.util
import json
from pathlib import Path
import subprocess
from urllib.parse import urlencode
from urllib.request import Request
import hashlib


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def positions(value):
    if isinstance(value, list):
        return [tuple(value)] if value and isinstance(value[0], (int, float)) else [p for child in value for p in positions(child)]
    if isinstance(value, dict):
        if "x" in value and "y" in value:
            return [(value["x"], value["y"])]
        return [p for key in ("coordinates", "geometries", "paths", "rings", "points") for p in positions(value.get(key, []))]
    return []


def verify(binary, store, inspection, service, record, output, public, report):
    asset, source = inspection["asset"], inspection["asset"].get("remoteSource")
    original = Path(record["path"]).read_bytes()
    assert hashlib.sha256(original).hexdigest() == asset["sourceSha256"]
    if (source and source.get("wfs")) or asset.get("osmSource"):
        command = subprocess.run([str(binary), "vectors", "inspect", "--id", asset["id"], "--data-dir", str(store)],
            capture_output=True, check=True, timeout=60, creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        assert json.loads(command.stdout) == inspection
        (output / "fresh-native-inspection.json").write_bytes(command.stdout)
        # CLI inspect is pretty-printed. Check the hash against the actual native
        # export, not whitespace introduced by that presentation layer.
        export_path = output / "native-export.geojson"
        subprocess.run([str(binary), "vectors", "export", "--id", asset["id"], "--out", str(export_path), "--data-dir", str(store)],
            capture_output=True, check=True, timeout=60, creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        exported = export_path.read_bytes()
        assert json.loads(exported) == inspection["geojson"]
        assert hashlib.sha256(exported).hexdigest() == asset["geojsonSha256"]
        # Original-format checkers also validate the literal GeoJSON field.
        # Frame their input around the real exported bytes; never reserialize it.
        fields = [json.dumps(key).encode() + b":" + json.dumps(value, ensure_ascii=False).encode()
            for key, value in inspection.items() if key != "geojson"]
        framed = b"{" + b",".join(fields + [b'"geojson":' + exported]) + b"}"
        assert json.loads(framed) == inspection
        if source:
            checker = module("agent_wfs_reference", "verify-wfs-public.py")
            result = checker.verify_snapshot(original, framed, record, service, output)
            report["protocolSourceReference"] = {"protocol": "WFS 2", **result}
        else:
            checker = module("agent_osm_reference", "verify-overpass-public.py")
            osm = asset["osmSource"]
            result, exported = checker.verify_snapshot(original, framed, record, osm["preset"], osm["requestedBounds"], service)
            (output / "export.geojson").write_bytes(exported)
            report["protocolSourceReference"] = {"protocol": "Overpass", **result}
        return inspection["geojson"]["features"]
    assert source and source.get("arcgis")
    features = inspection["geojson"]["features"]
    by_id = {f["id"]: f for f in features}
    refetched = []
    for index, receipt in enumerate(source["pages"]):
        body = urlencode(receipt["parameters"]).encode()
        with public.open(Request(receipt["url"], data=body), timeout=45) as response:
            assert response.status == 200 and response.geturl() == receipt["url"]
            raw = response.read(20 * 1024 * 1024 + 1)
            assert len(raw) <= 20 * 1024 * 1024
        value = json.loads(raw)
        assert "error" not in value and len(value["features"]) == receipt["returned"]
        assert all(feature == by_id[feature["id"]] for feature in value["features"])
        refetched.extend(value["features"])
        (output / f"arcgis-refetched-{index}.geojson").write_bytes(raw)
        report["publicRefetches"].append({"url": receipt["url"], "returned":len(value["features"]),
            "sha256": hashlib.sha256(raw).hexdigest(), "matchesAcquisitionReceiptBytes": hashlib.sha256(raw).hexdigest() == receipt["sha256"]})
    metadata = source["arcgis"]
    assert sorted(f["id"] for f in refetched) == metadata["objectIds"]
    query = service["url"].rstrip("/") + "/" + source["collectionId"] + "/query"
    parameters = {"f":"json", "objectIds":",".join(map(str, metadata["objectIds"])),
        "outFields":"*", "outSR":"4326", "returnGeometry":"true"}
    with public.open(Request(query, data=urlencode(parameters).encode()), timeout=45) as response:
        raw = response.read(20 * 1024 * 1024 + 1)
        assert response.status == 200 and len(raw) <= 20 * 1024 * 1024
    esri = json.loads(raw)
    assert "error" not in esri and len(esri["features"]) == len(features)
    field = metadata["layer"]["objectIdField"]
    for feature in esri["features"]:
        native = by_id[feature["attributes"][field]]
        assert feature["attributes"] == native["properties"]
        assert Counter(positions(feature["geometry"])) == Counter(positions(native["geometry"]))
    (output / "arcgis-independent-esri.json").write_bytes(raw)
    report["protocolSourceReference"] = {"protocol":"ArcGIS", "completeGeoJsonEquality":True,
        "independentEsriAttributesAndCoordinatesEqual":True, "features":len(features)}
    # Match the native source membership order after independently checking every ID.
    return [by_id[ident] for ident in metadata["objectIds"]]
