"""Real acceptance of curated public STAC APIs and reviewed direct GeoTIFF presets.

Uses a frozen repository runtime, a new loopback store and explicitly selected
original numeric bands. STAC discovery/pagination evidence is separate from local
file verification. Publisher JSON is checked against immutable raw documents;
it is never replaced with synthetic datasetPreset metadata. No accounts, models,
desktop windows or public services are operated by this harness.
"""
import argparse
import base64
from datetime import datetime, timezone
import hashlib
import importlib.util
import io
import json
import math
from pathlib import Path
import re
import shutil
import socket
import subprocess
import time
import traceback
from urllib.error import HTTPError
from urllib.parse import urljoin
from urllib.request import Request, urlopen

import numpy as np
from PIL import Image
import rasterio

HELPERS = {"itemContains", "assetKey", "assetContains", "publisherNoData", "sampleCoordinates",
           "requireDiscoveryComplete", "expectedShape", "expectedBytes", "expectedCollectionLicense",
           "expectedPixelValues", "expectedSha256", "expectedSha1", "expectedMd5", "originalSampleEvidence", "requireAOIUsable"}


def same_original_value(want, got):
    """Compare an original numerical value without changing either API contract.

    Rasterio helper serializes nonfinite values as lower-case strings; native
    STAC raster serialization uses NaN/Infinity/-Infinity. These spellings
    denote the same original Float sample, rather than a calibrated substitute.
    """
    if isinstance(want, str) or isinstance(got, str):
        return isinstance(want, str) and isinstance(got, str) and want.lower() == got.lower()
    return want == got or isinstance(want, float) and isinstance(got, (int, float)) and math.isclose(got, want, rel_tol=1e-7, abs_tol=1e-7)


def reviewed_asset_transports(original, source, plan):
    """Independently check exact product-scoped publisher transport receipts.

    This is acceptance-side verification of preserved original hrefs, not a
    runtime adapter and not permission for arbitrary bucket/path rewriting.
    """
    dataset_id = plan["dataset"]["id"]
    year = plan["request"]["year"]
    original_assets = original["assets"]
    result = []
    if dataset_id == "maxar-open-legacy":
        assert original["id"] == source["id"] and original["collection"] == "10300100EB15FF00"
        assert original["properties"]["datetime"] == source["startDatetime"] == source["endDatetime"]
        assert {"ms_analytic", "pan_analytic", "visual", "data-mask"} == set(original_assets)
        return result  # Original publisher relative paths are resolved, not rewritten.
    if dataset_id in {"jaxa-amsr2-soil-moisture", "jaxa-sgli-lst"}:
        assert all(asset["href"].startswith("./") and ".." not in asset["href"] for asset in original_assets.values())
        return result  # Original public JAXA HTTPS hrefs are not rewritten.
    if dataset_id in {"deafrica-fractional-cover", "deafrica-wofs-annual"}:
        name = "fc_ls_summary_annual" if dataset_id == "deafrica-fractional-cover" else "wofs_ls_summary_annual"
        assert year == 2025 and source["id"] == f"{name}_{source['region']}_{year}--P1Y"
        assert original["properties"]["odc:product"] == name
        assert original["properties"]["odc:region_code"] == source["region"]
        assert set(original_assets) == {layer["id"] for layer in plan["dataset"]["layers"]}
        for key, metadata in original_assets.items():
            assert metadata["href"] == f"{source['id']}_{key}.tif"
        return result
    for key, metadata in original_assets.items():
        original_href = metadata["href"]
        if dataset_id == "worldcereal":
            assert original_href.startswith("s3://deafrica-input-datasets/esa_worldcereal/")
            wanted_href = original_href.replace("s3://deafrica-input-datasets/", "https://deafrica-input-datasets.s3.af-south-1.amazonaws.com/", 1)
            adapter = "deafrica-worldcereal-public-s3"
        elif dataset_id in {"wsf-settlements", "gmw-mangroves"}:
            wanted_key = f"wsf{year}" if dataset_id == "wsf-settlements" else "mangrove"
            assert key == wanted_key and len(original_assets) == 1
            assert original_href.startswith(f"s3://deafrica-input-datasets/wsf_{year}/" if dataset_id == "wsf-settlements" else f"s3://deafrica-input-datasets/gmw/{year}/")
            wanted_href = source["stac"]["url"].removesuffix(".stac-item.json") + ".tif"
            assert original_href.replace("s3://deafrica-input-datasets/", "https://deafrica-input-datasets.s3.af-south-1.amazonaws.com/", 1) == wanted_href
            adapter = "deafrica-settlements-mangrove-public-s3"
        elif dataset_id == "cgls-landcover":
            assert original_href == f"s3://deafrica-input-datasets/gls/{year}/cgls_landcover_{year}_{key}.tif"
            if key != "permanentwater_cover_fraction":
                continue  # All other original S3 declarations remain unsupported.
            wanted_href = f"https://deafrica-input-datasets.s3.af-south-1.amazonaws.com/cgls_landcover/{year}/cgls_landcover_{year}_{key}.tif"
            adapter = "deafrica-cgls-reviewed-distribution-path"
        else:
            raise AssertionError("Standalone publisher product has no reviewed acceptance transport contract")
        result.append({"assetKey": key, "originalHref": original_href, "href": wanted_href, "adapter": adapter})
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exe", required=True, type=Path)
    parser.add_argument("--run", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--cases", required=True, type=Path)
    parser.add_argument("--port", default=4662, type=int)
    parser.add_argument("--boundary-cases", type=Path,
                        help="Read-only invalid resolve/connect requests; require explicit failure and no new file jobs")
    parser.add_argument("--recheck", action="store_true")
    parser.add_argument("--fresh-dataset", action="append", default=[],
                        help="With --recheck, create fresh reviewed metadata and a new original transfer for this dataset; preserve previous receipts")
    parser.add_argument("--require-prior-rejection", action="append", default=[],
                        help="For a replaced dataset, require the previous stored inspection to fail reviewed-provenance validation")
    parser.add_argument("--prior-rejection-route", choices=["inspect", "snapshot"], default="inspect",
                        help="Use snapshot read when strict startup rejects retaining an obsolete job in the active index")
    args = parser.parse_args()
    repository = Path(__file__).resolve().parents[1]
    run = args.run.resolve()
    assert run.parent == repository / ".verification" and (run.name.startswith("public-datasets-") or run.name == "new-public-channels-native-20261012")
    assert args.recheck or not run.exists(), "Fresh acceptance requires a new isolated store"
    assert args.exe.resolve().is_relative_to(repository)
    assert 1024 <= args.port <= 65535
    with socket.socket() as check:
        check.bind(("127.0.0.1", args.port))
    reader_spec = importlib.util.spec_from_file_location("public_dataset_reader", repository / "scripts" / "verify-public-dataset-connectors.py")
    reader = importlib.util.module_from_spec(reader_spec)
    reader_spec.loader.exec_module(reader)
    run.mkdir(exist_ok=args.recheck)
    prior = None
    if args.recheck:
        prior = json.loads((run / "verification.json").read_text(encoding="utf8"))
        assert Path(prior["run"]).samefile(run)
        shutil.copyfile(run / "verification.json", run / f"verification-before-{time.time_ns()}.json")
    hash_ = reader.sha256_file(args.exe)
    frozen = run / ("runtime-" + hash_[:16] + args.exe.suffix)
    shutil.copyfile(args.exe, frozen)
    assert reader.sha256_file(frozen) == hash_
    store = run / "core"
    base = f"http://127.0.0.1:{args.port}"
    cases = json.loads(args.cases.read_text(encoding="utf8"))
    assert isinstance(cases, list) and cases
    assert not args.fresh_dataset or args.recheck
    assert set(args.fresh_dataset).issubset({case["datasetId"] for case in cases})
    assert set(args.require_prior_rejection).issubset(set(args.fresh_dataset))
    report = {"schema": "geod-curated-public-source-native-acceptance/v1", "status": "running",
              "checkedAt": datetime.now(timezone.utc).isoformat(), "binarySha256": hash_,
              "run": str(run), "reader": f"Rasterio {rasterio.__version__} / GDAL {rasterio.__gdal_version__}",
              "credentialsRead": False, "modelCalls": 0, "earthEngineComputeRequests": 0,
              "nativeWindowTested": False, "cases": [], "errors": []}
    if prior:
        report["previousBinarySha256"] = prior["binarySha256"]
    args.report.parent.mkdir(parents=True, exist_ok=True)

    def save():
        value = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
        (run / "verification.json").write_text(value, encoding="utf8")
        args.report.write_text(value, encoding="utf8")

    def api(route, body=None):
        request = Request(base + route, data=json.dumps(body).encode() if body is not None else None,
                          headers={"X-GeoD-Client": "geod-global", "Content-Type": "application/json"},
                          method="POST" if body is not None else "GET")
        try:
            with urlopen(request, timeout=90) as response:
                return json.loads(response.read())
        except HTTPError as error:
            raise RuntimeError(f"{route} HTTP {error.code}: {error.read().decode('utf8', 'replace')}") from error

    def start():
        out = (run / "runtime.stdout.log").open("ab")
        err = (run / "runtime.stderr.log").open("ab")
        process = subprocess.Popen([str(frozen), "serve", "--data-dir", str(store), "--port", str(args.port)],
                                   cwd=repository, stdin=subprocess.DEVNULL, stdout=out, stderr=err,
                                   creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        out.close(); err.close()
        for _ in range(150):
            try:
                api("/health"); return process
            except Exception:
                assert process.poll() is None, "Private runtime exited during startup"
                time.sleep(.1)
        process.terminate(); process.wait(timeout=15)
        raise RuntimeError("Private runtime did not become ready")

    def verified_document(receipt):
        checksum = receipt["sha256"]
        path = store / "stac" / ("document-" + checksum + ".json")
        assert reader.sha256_file(path) == checksum, "Immutable metadata bytes no longer match their receipt"
        return json.loads(path.read_text(encoding="utf8"))

    def original_stac_proof(item, expected_search=None, expected_item_source=None, expected_plan=None):
        provenance = item["provenance"]
        raw = verified_document({"sha256": item["documentSha256"]})
        original = next(feature for feature in raw["features"] if feature["id"] == item["itemId"]) if raw.get("type") == "FeatureCollection" else raw
        assert original.get("type") == "Feature" and original["id"] == item["itemId"]
        assert original.get("collection") == item["collectionId"]
        assert item["properties"] == original["properties"] and "datasetPreset" not in item["properties"]
        assert item["bbox"] == original.get("bbox") and item["geometry"] == original.get("geometry")
        for asset in item["assets"]:
            assert asset["metadata"] == original["assets"][asset["key"]]
        query = provenance["search"]
        if expected_search is not None:
            assert original["collection"] == expected_search["collectionId"]
            for field in ("connectionId", "collectionId", "bounds", "datetime"):
                assert query[field] == expected_search[field], "Saved source query differs from reviewed preset query"
        else:
            assert expected_item_source is not None and raw.get("type") == "Feature" and query is None
            assert expected_item_source["stac"]["kind"] == "item"
            assert provenance["documentUrl"] == expected_item_source["stac"]["url"]
            assert expected_plan is not None
            reviewed = provenance["reviewedDataset"]
            assert reviewed["schemaVersion"] == 2
            assert reviewed["dataset"] == expected_plan["dataset"]
            assert reviewed["request"] == expected_plan["request"]
            assert reviewed["layer"] == next(layer for layer in expected_plan["dataset"]["layers"]
                                              if layer["id"] == expected_plan["request"]["layer"])
            assert reviewed["sourceId"] == expected_item_source["id"]
            assert reviewed["sourceTitle"] == expected_item_source["title"]
            assert reviewed["sourceBounds"] == expected_item_source["bounds"]
            assert reviewed["declarationKind"].startswith("reviewed-client-preset;")
            dataset_id = expected_plan["dataset"]["id"]
            annual = dataset_id in {"deafrica-fractional-cover", "deafrica-wofs-annual"}
            if not annual and dataset_id not in {"jaxa-amsr2-soil-moisture", "jaxa-sgli-lst"}:
                source_id = expected_item_source["id"]
                assert source_id == original["id"], "Client source identity must match the pinned publisher item for this indexed product"
            tolerance = 1e-8 if annual else 0
            assert all(abs(a - b) <= tolerance for a, b in zip(expected_item_source["bounds"], original["bbox"]))
            for native_key, standard_key, annual_key in [("startDatetime", "start_datetime", "dtr:start_datetime"), ("endDatetime", "end_datetime", "dtr:end_datetime")]:
                if dataset_id == "maxar-open-legacy":
                    assert item[native_key] is None and item["datetime"] == original["properties"]["datetime"] == expected_item_source[native_key]
                    continue  # A real acquisition instant is not a fabricated STAC interval.
                declared = original["properties"].get(standard_key)
                if annual and declared is None:
                    declared = original["properties"].get(annual_key)
                assert item[native_key] == declared and expected_item_source[native_key] == declared
            expected_transports = reviewed_asset_transports(original, expected_item_source, expected_plan)
            by_key = {transport["assetKey"]: transport for transport in expected_transports}
            for asset in item["assets"]:
                transport = by_key.get(asset["key"])
                if transport:
                    assert asset["originalHref"] == transport["originalHref"] and asset["href"] == transport["href"]
                    assert asset["transportAdapter"] == transport["adapter"]
                elif annual or dataset_id in {"jaxa-amsr2-soil-moisture", "jaxa-sgli-lst", "maxar-open-legacy"}:
                    assert asset.get("originalHref") is None and asset.get("transportAdapter") is None
                    assert asset["href"] == urljoin(provenance["documentUrl"], original["assets"][asset["key"]]["href"])
                elif dataset_id == "cgls-landcover":
                    assert not asset["eligible"] and asset.get("transportAdapter") is None
                    assert asset["href"] == original["assets"][asset["key"]]["href"]
            assert sorted(provenance.get("assetTransports", []), key=lambda value: value["assetKey"]) == sorted(expected_transports, key=lambda value: value["assetKey"])
            record_path = store / "stac" / ("snapshot-" + item["id"] + ".json")
            assert reader.sha256_file(record_path) == item["id"], "Independent reviewed binding receipt hash differs from snapshot identity"
            archived_record = json.loads(record_path.read_text(encoding="utf8"))
            assert archived_record["reviewedDataset"] == reviewed and archived_record.get("assetTransports", []) == provenance.get("assetTransports", [])
        request = provenance.get("documentRequest")
        if request is not None:
            assert request["url"] == provenance["documentUrl"] and request["method"] in {"POST", "GET"}
        else:
            assert expected_search is None, "An API search must retain its actual request receipt"
        if request is not None and request["method"] == "POST":
            body = request["body"]
            assert body["collections"] == [query["collectionId"]] and body["bbox"] == query["bounds"]
            assert body.get("datetime") == query["datetime"]
        documents = []
        originals = []
        for receipt in provenance["metadataDocuments"]:
            value = verified_document(receipt)
            originals.append(value)
            documents.append(receipt)
        collection = provenance["collection"]
        if collection is not None:
            assert collection["id"] == item["collectionId"]
            declarations = [entry for doc in originals for entry in doc.get("collections", [doc])]
            assert any(entry == collection for entry in declarations), "Saved collection is not the immutable publisher declaration"
        else:
            assert expected_search is None, "An API query must preserve its source collection declaration"
        return {"kind": ("publisher STAC item/collection/query documents" if expected_search is not None else
                         "original publisher standalone STAC Feature; reviewed client item source binding is separate from original properties"),
                "originalPropertiesAndAssetsUnchanged": True, "metadataDocumentBytesAndHashesVerified": True,
                "itemDocumentSha256": item["documentSha256"], "documents": documents,
                "request": request, "search": query,
                "collectionId": item["collectionId"], "collectionLicenseRaw": collection.get("license") if collection else None,
                "publisherCollectionDocumentAvailable": collection is not None,
                "reviewedStandaloneSource": expected_item_source,
                "separateReviewedBindingAndControlledTransportsVerified": expected_item_source is not None}

    def original_raster_proof(item, source, plan):
        raw = verified_document({"sha256": item["documentSha256"]})
        assert raw["kind"] == "direct-raster" and raw["href"] == source["stac"]["url"]
        proof = raw["datasetPreset"]
        assert proof["dataset"] == plan["dataset"] and proof["request"] == plan["request"]
        assert proof["sourceId"] == source["id"] and proof["sourceBounds"] == source.get("bounds")
        assert item["properties"] == {"datasetPreset": proof} and item["bbox"] == source.get("bounds")
        assert item["datetime"] is None and item["startDatetime"] is None and item["endDatetime"] is None
        assert len(item["assets"]) == 1 and item["assets"][0]["metadata"] == raw
        catalog = proof.get("catalogRaster")
        documents = item["provenance"]["metadataDocuments"]
        if catalog:
            assert plan["dataset"]["id"] == "usgs-3dep" and catalog == source["catalogRaster"]
            assert documents == [{"url": catalog["catalogUrl"], "sha256": catalog["catalogSha256"]}]
            publisher = verified_document(documents[0])
            assert publisher["total"] == len(publisher["items"]) <= 100 and publisher["errors"] == []
            matches = [record for record in publisher["items"] if record["sourceId"] == source["id"]]
            assert matches == [catalog["record"]]
            assert catalog["record"]["downloadURL"] == raw["href"]
            assert catalog["record"]["sizeInBytes"] == raw["file:size"]
            assert proof["declarationKind"] == "reviewed-client-catalog-raster; exact publisher catalog record archived separately"
        else:
            assert proof["declarationKind"].startswith("reviewed-client-preset;") and not documents
        return {"kind": "reviewed client direct-raster declaration; not publisher STAC", "exactCatalogBytesAndSelectedRecordVerified": bool(catalog),
                "itemDocumentSha256": item["documentSha256"], "documents": documents, "declarationKind": proof["declarationKind"],
                "originalNumericalFileUrlVerified": True, "publicationIsNotObservation": True}

    process = start()
    try:
        report["datasets"] = api("/datasets")
        ids = {dataset["id"] for dataset in report["datasets"]}
        if args.boundary_cases:
            report["boundaryChecks"] = []
            for boundary in json.loads(args.boundary_cases.read_text(encoding="utf8")):
                before = api("/jobs")
                row = {"request": boundary["request"], "name": boundary["name"], "routes": [], "status": "passed"}
                for route in boundary.get("routes", ["/datasets/resolve", "/datasets/connect"]):
                    try:
                        api(route, boundary["request"])
                    except RuntimeError as error:
                        assert "HTTP " + str(boundary.get("expectedHttpStatus", 400)) in str(error)
                        row["routes"].append({"route": route, "error": str(error), "explicitRejection": True})
                    else:
                        raise AssertionError(f"Invalid {boundary['name']} was accepted at {route}")
                    assert api("/jobs") == before, "Rejected metadata input created original-file jobs"
                row["createdNoJobs"] = True
                report["boundaryChecks"].append(row)
            save()
        for index, case in enumerate(cases):
            label = f"{index + 1:02d}-{case['datasetId']}-{case.get('layer', 'default')}"
            record = {"request": case, "status": "running"}
            report["cases"].append(record); save()
            try:
                assert case["datasetId"] in ids, "Curated source absent from this frozen runtime"
                request = {key: value for key, value in case.items() if key not in HELPERS}
                selection_identity = {key: value for key, value in case.items() if key != "sampleCoordinates"}
                previous_case = next((row for row in prior["cases"]
                                      if {key: value for key, value in row["request"].items() if key != "sampleCoordinates"} == selection_identity
                                      and row.get("job", {}).get("settled")), None) if prior else None
                old = None if case["datasetId"] in args.fresh_dataset else previous_case
                if previous_case and old is None:
                    record["freshReplacementForPriorJob"] = {
                        "jobId": previous_case["job"]["id"], "sha256": previous_case["job"]["sha256"],
                        "snapshotId": previous_case["source"]["snapshot"]["id"],
                        "reason": "Explicit fresh-dataset acceptance after a reviewed metadata correction; previous original and immutable metadata retained in private evidence"
                    }
                    if case["datasetId"] in args.require_prior_rejection:
                        rejection_route = ("/stac/snapshots/" + previous_case["source"]["snapshot"]["id"]
                                           if args.prior_rejection_route == "snapshot" else
                                           "/stac/jobs/" + previous_case["job"]["id"] + "/inspect")
                        try:
                            api(rejection_route)
                        except RuntimeError as error:
                            assert "Public dataset provenance differs from its reviewed preset" in str(error)
                            record["supersededSourceRejectedForRead"] = {
                                "jobId": previous_case["job"]["id"], "route": rejection_route, "error": str(error),
                                "originalFileRetained": Path(previous_case["job"]["outputPath"]).is_file()
                            }
                        else:
                            raise AssertionError("Incorrect previous declaration was still accepted for inspection")
                record["plan"] = api("/datasets/resolve", request)
                before = api("/jobs")
                connected = old["connectionResult"] if old else api("/datasets/connect", request)
                record["connectionResult"] = connected
                assert api("/jobs") == before, "Discovery unexpectedly started a transfer"
                assert connected["complete"] and not connected["failures"], "Metadata connection failed for at least one reviewed source"
                record["discoveryStartedNoDownloads"] = True
                candidates = []
                searched_snapshot_ids = set()
                queries = []
                for search in connected.get("searches", []):
                    page = search["page"]
                    assert isinstance(page["complete"], bool) and isinstance(page["limitReached"], bool)
                    assert not page["complete"] or page["nextCursor"] is None and not page["limitReached"]
                    source = next(source for source in record["plan"]["sources"] if source["id"] == search["sourceId"])
                    expected = {**source["search"], "connectionId": search["connectionId"]}
                    queries.append({"sourceId": search["sourceId"], "connectionId": search["connectionId"],
                                    "returnedItems": len(page["items"]), "complete": page["complete"],
                                    "limitReached": page["limitReached"], "nextCursorPresent": page["nextCursor"] is not None})
                    for item in page["items"]:
                        assert api("/stac/snapshots/" + item["id"]) == item
                        proof = original_stac_proof(item, expected)
                        searched_snapshot_ids.add(item["id"])
                        for asset in item["assets"]:
                            candidates.append((item, asset, proof))
                for connection in connected["stacConnections"]:
                    for identifier in connection["snapshotIds"]:
                        if identifier in searched_snapshot_ids:
                            continue
                        item = api("/stac/snapshots/" + identifier)
                        source = next(source for source in connected["plan"]["sources"]
                                      if source.get("stac", {}).get("kind") in {"item", "raster"}
                                      and source["stac"]["url"] == item["provenance"]["documentUrl"])
                        record["resolvedPlan"] = record.get("resolvedPlan", record["plan"])
                        record["plan"] = connected["plan"]
                        proof = (original_raster_proof(item, source, record["plan"]) if source["stac"]["kind"] == "raster"
                                 else original_stac_proof(item, expected_item_source=source, expected_plan=record["plan"]))
                        for asset in item["assets"]:
                            candidates.append((item, asset, proof))
                record["discoveryPages"] = queries
                record["discoveryComplete"] = all(query["complete"] for query in queries)
                record["discoveryMode"] = record["plan"]["dataset"].get("discovery", "reviewed-direct-files")
                if case.get("requireDiscoveryComplete"):
                    assert record["discoveryComplete"], "This explicitly bounded acceptance query did not enumerate all returned source pages"
                matching = [(item, asset, proof) for item, asset, proof in candidates if asset["eligible"]
                            and ("assetKey" not in case or asset["key"] == case["assetKey"])
                            and ("assetContains" not in case or case["assetContains"] in asset["href"])
                            and ("itemContains" not in case or case["itemContains"] in item["itemId"])]
                assert matching, "No real original matched the explicit acceptance selection"
                item, asset, proof = matching[0]
                label += "-" + re.sub(r"[^A-Za-z0-9._-]", "_", asset["key"])
                if "expectedCollectionLicense" in case:
                    assert proof.get("collectionLicenseRaw") == case["expectedCollectionLicense"]
                record["source"] = {"snapshot": item, "asset": asset}
                record["provenanceCheck"] = proof
                save()  # Publish the actual discovery response before the original transfer.
                pin = {"snapshotId": item["id"], "assetKey": asset["key"]}
                project = next(project for project in api("/projects") if project["id"] == old["project"]["id"]) if old else api("/stac/project", {"name": "Live curated acceptance · " + label, "bounds": case["bounds"], "selections": [pin]})
                assert len(project["stacItems"]) == 1
                record["project"] = project
                started = time.monotonic()
                queued = {"jobs": [api("/jobs/" + old["job"]["id"])]} if old else api("/stac/downloads", {"projectId": project["id"], "selections": [pin]})
                assert len(queued["jobs"]) == 1
                job = queued["jobs"][0]
                deadline = time.monotonic() + 1800
                last_update = time.monotonic()
                while time.monotonic() < deadline:
                    job = api("/jobs/" + job["id"])
                    if job["status"] in {"failed", "cancelled", "interrupted"}:
                        record["unfinishedJob"] = job
                        raise RuntimeError("Native original transfer failed: " + str(job.get("error")))
                    if job["status"] == "succeeded" and job.get("settled"):
                        break
                    if time.monotonic() - last_update >= 45:
                        print(json.dumps({"source": label, "phase": "downloading-original", "bytes": job["bytesDownloaded"], "total": job["totalBytes"]}), flush=True)
                        last_update = time.monotonic()
                    time.sleep(.4)
                else:
                    record["unfinishedJob"] = job
                    raise RuntimeError("Native transfer did not settle within the30minute observation")
                record["downloadSeconds"] = old["downloadSeconds"] if old else round(time.monotonic() - started, 3)
                record["settledTransferReusedForFinalInspection"] = bool(old)
                record["job"] = job; save()
                path = Path(job["outputPath"]).resolve(strict=True)
                assert path.parent.samefile(store / "assets")
                assert path.stat().st_size == job["bytesDownloaded"] and job["totalBytes"] in {None, path.stat().st_size}
                assert reader.sha256_file(path) == job["sha256"]
                publisher_integrity = {"source": "unchanged publisher asset metadata", "declaredSha256": False, "declaredSize": False}
                multihash = asset["metadata"].get("checksum:multihash")
                if multihash is not None:
                    assert isinstance(multihash, str) and len(multihash) == 68 and multihash[:4].lower() == "1220"
                    assert job["sha256"] == multihash[4:].lower(), "Complete original differs from publisher SHA256 multihash"
                    publisher_integrity.update(declaredSha256=True, sha256=multihash[4:].lower(), sha256Matched=True)
                publisher_size = asset["metadata"].get("bdc:size")
                if publisher_size is not None:
                    assert isinstance(publisher_size, int) and publisher_size == path.stat().st_size
                    publisher_integrity.update(declaredSize=True, size=publisher_size, sizeMatched=True)
                record["publisherIntegrityCheck"] = publisher_integrity
                if "expectedBytes" in case:
                    assert path.stat().st_size == case["expectedBytes"], "Downloaded object size changed from independently probed publisher original"
                if "expectedSha256" in case:
                    assert job["sha256"] == case["expectedSha256"], "Fresh native original differs from independently downloaded original"
                    record["independentReferenceSha256Matched"] = True
                if "expectedMd5" in case:
                    md5 = hashlib.md5()
                    with path.open("rb") as stream:
                        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                            md5.update(chunk)
                    assert md5.hexdigest() == case["expectedMd5"], "Fresh complete original differs from the independently archived publisher MD5"
                    record["publisherMd5Matched"] = {"md5": md5.hexdigest(), "declarationSource": "Actual independently fetched publisher record; not an invented STAC asset field or multipart ETag"}
                if "expectedSha1" in case:
                    sha1 = hashlib.sha1()
                    with path.open("rb") as stream:
                        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                            sha1.update(chunk)
                    assert sha1.hexdigest() == case["expectedSha1"], "Fresh complete original differs from publisher SHA1 sidecar"
                    record["publisherSha1SidecarMatched"] = {"sha1": sha1.hexdigest(), "declarationSource": "Actual independently fetched publisher checksum sidecar; not an invented STAC asset field"}
                declaration = case.get("publisherNoData")
                if declaration is None:
                    preset = item["properties"].get("datasetPreset")
                    if preset and preset["layer"].get("nodata") is not None:
                        declaration = {"value": preset["layer"]["nodata"], "sourceUrl": preset["dataset"]["documentationUrl"], "declarationKind": preset["declarationKind"]}
                record["independentRaster"] = reader.inspect_all(path, declaration)
                independent = record["independentRaster"]
                with rasterio.open(path) as dataset:
                    record["independentStorage"] = {"storageLayout": "tile" if dataset.is_tiled else "strip",
                                                    "compression": dataset.compression.value if dataset.compression else None,
                                                    "blockShapes": [list(shape) for shape in dataset.block_shapes],
                                                    "colorInterpretations": [value.name for value in dataset.colorinterp],
                                                    "overviewFactors": dataset.overviews(1)}
                if "expectedShape" in case:
                    assert [independent["height"], independent["width"]] == case["expectedShape"]
                asset_grid = asset["metadata"]
                if "proj:shape" in asset_grid:
                    assert asset_grid["proj:shape"] == [independent["height"], independent["width"]], "Original file differs from its asset-specific publisher grid"
                if "proj:code" in asset_grid:
                    assert asset_grid["proj:code"] == independent["crs"]
                if "proj:transform" in asset_grid:
                    declared_transform = asset_grid["proj:transform"]
                    assert len(declared_transform) in {6, 9}
                    assert all(math.isclose(got, wanted, rel_tol=1e-10, abs_tol=1e-10)
                               for got, wanted in zip(independent["transform"], declared_transform[:6]))
                record["assetSpecificPublisherGridVerified"] = any(key in asset_grid for key in ("proj:shape", "proj:code", "proj:transform"))
                if case["datasetId"] in {"deafrica-fractional-cover", "deafrica-wofs-annual", "wsf-settlements", "gmw-mangroves", "cgls-landcover"}:
                    publisher_grid = item["properties"]
                    assert publisher_grid["proj:shape"] == [independent["height"], independent["width"]]
                    assert "EPSG:" + str(publisher_grid["proj:epsg"]) == independent["crs"]
                    assert all(math.isclose(got, wanted, rel_tol=1e-10, abs_tol=1e-10)
                               for got, wanted in zip(independent["transform"], publisher_grid["proj:transform"][:6]))
                    record["itemSpecificPublisherGridVerified"] = True
                native = api("/stac/jobs/" + job["id"] + "/inspect")
                record["nativeInspection"] = {key: value for key, value in native.items() if key != "previewDataUrl"}
                record["nativeInspection"]["previewAvailable"] = bool(native.get("previewDataUrl"))
                assert native["sha256"] == job["sha256"] and native["jobId"] == job["id"]
                assert [native["width"], native["height"]] == [independent["width"], independent["height"]]
                assert native["storageLayout"] == record["independentStorage"]["storageLayout"]
                assert native["width"] * native["height"] <= 8_000_000_000
                assert max(native["width"], native["height"]) <= (131_072 if native["storageLayout"] == "tile" else 65_536)
                assert len(native["bands"]) == independent["count"]
                for native_band, original_band in zip(native["bands"], independent["bands"]):
                    assert native_band["dataType"].lower() == original_band["dtype"].lower()
                    assert same_original_value(original_band["nodata"], native_band["nodata"]), "Native inspection changed embedded NoData declaration"
                assert native["crs"] == independent["crs"] and native["bounds"] and native["transform"]
                for got, wanted in zip(native["bounds"], independent["bounds"]):
                    assert math.isclose(got, wanted, rel_tol=1e-10, abs_tol=1e-6)
                assert native["previewDataUrl"], "Raw preview missing; successful download is not complete local inspection acceptance"
                preview = base64.b64decode(native["previewDataUrl"].split(",", 1)[1], validate=True)
                with Image.open(io.BytesIO(preview)) as image:
                    assert image.size == (native["previewWidth"], native["previewHeight"])
                    image.verify()
                (run / (label + "-preview.png")).write_bytes(preview)
                record["previewFile"] = label + "-preview.png"
                record["previewSha256"] = hashlib.sha256(preview).hexdigest()
                record["nativeInspection"]["previewDataUrl"] = "saved in private run directory"
                record["pixels"] = []
                with rasterio.open(path) as dataset:
                    coords = {(0, 0), (dataset.width - 1, dataset.height - 1), (dataset.width // 2, dataset.height // 2),
                              (dataset.width // 3, dataset.height // 3), (dataset.width * 2 // 3, dataset.height * 2 // 3)}
                    useful = reader.aoi_usable_sample(dataset, case["bounds"], declaration["value"] if declaration else None)
                    if case.get("requireAOIUsable"):
                        assert useful, "No unmasked finite original sample was established inside the requested area"
                    if useful is None:
                        first = independent["bands"][0]["firstUsableSample"]
                        useful = {**first, "scope": "One unmasked original pixel elsewhere; requested-area validity was not established"} if first else None
                    assert useful, "Original contains no usable first-band sample"
                    record["usefulPixelSample"] = useful
                    coords.add((useful["column"], useful["row"]))
                    for col, row in case.get("sampleCoordinates", []):
                        assert isinstance(col, int) and isinstance(row, int) and 0 <= col < dataset.width and 0 <= row < dataset.height
                        coords.add((col, row))
                    for col, row in sorted(coords):
                        actual = api(f"/stac/jobs/{job['id']}/pixel?column={col}&row={row}")
                        expected = dataset.read(window=((row, row + 1), (col, col + 1)))[:, 0, 0]
                        assert actual["sha256"] == job["sha256"] and len(actual["values"]) == dataset.count
                        for band, value in enumerate(expected):
                            want, got = reader.scalar(value), actual["values"][band]
                            assert same_original_value(want, got), "Native pixel changed original numeric/NaN value"
                            nodata = dataset.nodatavals[band]
                            missing = nodata is not None and (value == nodata or np.isnan(value) and np.isnan(nodata))
                            assert actual["noData"][band] == bool(missing)
                        record["pixels"].append(actual)
                    for wanted in case.get("expectedPixelValues", []):
                        actual = next(pixel for pixel in record["pixels"] if pixel["column"] == wanted["column"] and pixel["row"] == wanted["row"])
                        assert actual["values"] == wanted["values"] and actual["noData"] == wanted["noData"], "Representative publisher class/mask sample changed"
                assert reader.sha256_file(path) == job["sha256"], "Inspection modified original bytes"
                record["originalSha256Verified"] = True
                record["status"] = "passed"
                print(json.dumps({"source": label, "status": "passed", "bytes": path.stat().st_size,
                                  "shape": [native["height"], native["width"]], "discoveryComplete": record["discoveryComplete"],
                                  "downloadSeconds": record["downloadSeconds"]}), flush=True)
            except Exception as error:
                record["status"] = "failed"
                record["error"] = f"{type(error).__name__}: {error}"
                record["traceback"] = traceback.format_exc()
                report["errors"].append({"source": label, "error": record["error"]})
                print(json.dumps(report["errors"][-1]), flush=True)
            save()
        assert not any(job["status"] in {"queued", "running"} for job in api("/jobs")), "Private store still contains unfinished transfers"
        water_rows = {row["request"]["layer"]: row for row in report["cases"]
                      if row["request"]["datasetId"] == "deafrica-wofs-annual" and row["status"] == "passed"}
        if set(water_rows) == {"frequency", "count_wet", "count_clear"}:
            compared, no_clear, maximum_error = 0, 0, 0.
            with rasterio.open(water_rows["frequency"]["job"]["outputPath"]) as frequency, rasterio.open(water_rows["count_wet"]["job"]["outputPath"]) as wet, rasterio.open(water_rows["count_clear"]["job"]["outputPath"]) as clear:
                assert frequency.transform == wet.transform == clear.transform and frequency.crs == wet.crs == clear.crs
                for _, window in frequency.block_windows(1):
                    f, w, c = frequency.read(1, window=window), wet.read(1, window=window), clear.read(1, window=window)
                    usable = np.isfinite(f) & (c > 0) & (w >= 0)
                    assert np.all(w[(w >= 0) & (c >= 0)] <= c[(w >= 0) & (c >= 0)])
                    assert not np.any(np.isfinite(f) & (c == 0)), "No clear observations fabricated a finite water frequency"
                    difference = np.abs(f[usable].astype(np.float64) - w[usable] / c[usable])
                    if difference.size:
                        maximum_error = max(maximum_error, float(difference.max()))
                        assert np.all(difference <= 1e-7)
                    compared += int(usable.sum())
                    no_clear += int((c == 0).sum())
            report["freshNativeWaterRatioVerification"] = {"comparedOriginalPixels": compared, "zeroClearCountPixelsWithNaNFrequency": no_clear, "maximumAbsoluteDifference": maximum_error, "withinFloat32Precision": True, "usedFreshNativeFiles": True}
        process.terminate(); process.wait(timeout=15); process = start()
        restored = {project["id"]: project for project in api("/projects")}
        for record in report["cases"]:
            if record["status"] != "passed":
                continue
            assert restored[record["project"]["id"]]["stacItems"] == record["project"]["stacItems"]
            assert api("/stac/snapshots/" + record["source"]["snapshot"]["id"]) == record["source"]["snapshot"]
            assert api("/jobs/" + record["job"]["id"])["sha256"] == record["job"]["sha256"]
            probe = record["pixels"][0]
            assert api(f"/stac/jobs/{record['job']['id']}/pixel?column={probe['column']}&row={probe['row']}") == probe
            reopened = api("/stac/jobs/" + record["job"]["id"] + "/inspect")
            assert reopened["sha256"] == record["job"]["sha256"] and reopened["width"] == record["nativeInspection"]["width"] and reopened["height"] == record["nativeInspection"]["height"]
            assert hashlib.sha256(base64.b64decode(reopened["previewDataUrl"].split(",", 1)[1], validate=True)).hexdigest() == record["previewSha256"]
            record["reopenedOriginalPreviewVerified"] = True
            record["reopenedProjectAndSourceVerified"] = True
        report["totalOriginalBytes"] = sum(record.get("job", {}).get("bytesDownloaded", 0) for record in report["cases"])
        report["totalDecodedOriginalPixels"] = sum(band["allPixelsDecoded"] for record in report["cases"] for band in record.get("independentRaster", {}).get("bands", []))
        report["totalNativePixelCoordinates"] = sum(len(record.get("pixels", [])) for record in report["cases"])
        report["sourceZipPackaging"] = "Source ZIP packaging is not implemented; no original ZIP is claimed"
        report["discoveryScope"] = "Returned page coverage is recorded separately; a verified selected original does not prove exhaustive source coverage or scientific AOI coverage"
        report["status"] = "passed" if not report["errors"] else "partial"
    finally:
        if process.poll() is None:
            process.terminate(); process.wait(timeout=15)
        report["ownRuntimeStopped"] = True
        save()
    assert report["status"] == "passed", json.dumps(report["errors"])


if __name__ == "__main__":
    main()
