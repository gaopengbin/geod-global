"""Verify real MCP sessions against an existing, completed WCS verification store.

This intentionally reuses a downloaded file; it does not claim a new public
acquisition through MCP. A rejecting local proxy checks that the offline workflow
does not contact the provider. No endpoint or account is selected by default.
"""
import argparse
from datetime import datetime, timezone
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import json
from pathlib import Path
import subprocess
import threading
import time
from urllib.request import build_opener, ProxyHandler, Request

SPEC = importlib.util.spec_from_file_location("mcp_qa", Path(__file__).with_name("verify-mcp.py"))
qa = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(qa)

READ = {"geod_projects_list", "geod_project_get", "geod_wcs_connections", "geod_wcs_coverages",
        "geod_wcs_description", "geod_wcs_plan", "geod_wcs_inspect", "geod_wcs_pixel"}
WRITE = {"geod_wcs_connect", "geod_wcs_describe", "geod_wcs_prepare", "geod_wcs_project_save",
         "geod_wcs_download", "geod_wcs_forget"}


def require(ok, message):
    if not ok:
        raise AssertionError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", required=True)
    parser.add_argument("--data-dir", required=True)
    parser.add_argument("--project", required=True)
    parser.add_argument("--job", required=True)
    parser.add_argument("--plan", required=True)
    parser.add_argument("--report", required=True)
    parser.add_argument("--port", type=int, default=4366)
    args = parser.parse_args()
    require(1 <= args.port <= 65535, "Choose a valid local verification port")
    root = Path(args.data_dir).resolve(strict=True)
    # Only a deliberate test store may have proxy routing temporarily changed.
    require(".verification" in root.parts, "Use a store under .verification, never the user's workspace")
    binary = Path(args.executable).resolve(strict=True)
    proxy_file = root / "proxy-settings.json"
    proxy_before = proxy_file.read_bytes() if proxy_file.exists() else None
    registry_before = {name: sha((root / name).read_bytes())
                       for name in ("jobs.json", "projects.json", "wcs-connections.json")}
    saved_jobs = json.loads((root / "jobs.json").read_text(encoding="utf-8"))
    require(all(j["status"] not in ("running", "queued") for j in saved_jobs.values()),
            "Use a settled verification store; do not recover an active task as a test side effect")
    lease = subprocess.run([str(binary), "wcs", "list", "--data-dir", str(root)],
                           stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30,
                           creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    require(lease.returncode == 0, "Verification store is unavailable or already owned; proxy configuration was not changed")
    report = {"schemaVersion": "geod-wcs-mcp-verification/v1", "status": "running",
              "startedAt": datetime.now(timezone.utc).isoformat(), "binarySha256": sha(binary.read_bytes()),
              "jobId": args.job, "projectId": args.project, "planId": args.plan,
              "successfulPublicGetCoverageTransfers": 0, "successfulPublicMetadataRequests": 0,
              "sessions": [], "independentPixelChecks": [], "checks": []}
    attempts = []

    class DenyProxy(BaseHTTPRequestHandler):
        def blocked(self):
            attempts.append({"method": self.command, "target": self.path})
            self.send_error(502, "External access disabled during offline MCP acceptance")

        do_CONNECT = blocked
        do_GET = blocked
        do_POST = blocked

        def log_message(self, *_):
            pass

    trap = ThreadingHTTPServer(("127.0.0.1", 0), DenyProxy)
    threading.Thread(target=trap.serve_forever, daemon=True).start()
    server = None
    opener = build_opener(ProxyHandler({}))
    origin = f"http://127.0.0.1:{args.port}"

    def api(path):
        with opener.open(Request(origin + path, headers={"X-GeoD-Client": "geod-global"}), timeout=3) as response:
            return json.load(response)

    def client(mode, write=False):
        return qa.Client(binary, origin if mode == "loopback" else None, write,
                         data_dir=root if mode == "direct" else None)

    def session(mode, write=False):
        c = client(mode, write)
        try:
            names = [t["name"] for t in c.request("tools/list")["result"]["tools"]]
            require(len(names) == len(set(names)), "Duplicate tool discovery names")
            require(READ.issubset(names), "Missing WCS read tools")
            require(WRITE.issubset(names) if write else WRITE.isdisjoint(names), "Incorrect mutation discovery")
            if not write:
                for name in WRITE:
                    response = c.request("tools/call", {"name": name, "arguments": {}})
                    require(response.get("error", {}).get("code") == -32602, "Read-only dispatch allowed a write")
            health = c.call("geod_health")
            require(health["runtime"]["status"] == "ok", "Runtime health failed")
            plan = c.call("geod_wcs_plan", {"id": args.plan})
            description = c.call("geod_wcs_description", {"id": plan["description"]["id"]})
            require(description == plan["description"], "Saved coverage declarations differ")
            project = c.call("geod_project_get", {"id": args.project})
            require(any(p["planId"] == args.plan for p in project["wcsItems"]), "Project lost its plan")
            summaries = c.call("geod_projects_list", {"limit": 100})
            summary = next(p for p in summaries["projects"] if p["id"] == args.project)
            require(summary["wcsItemCount"] == len(project["wcsItems"]), "Project summary selection count differs")
            connections = c.call("geod_wcs_connections", {"limit": 100})
            connection = next(v for v in connections["connections"] if v["id"] == description["connectionId"])
            catalog = []
            offset = 0
            while True:
                page = c.call("geod_wcs_coverages", {"id": connection["id"], "offset": offset, "limit": 1})
                catalog += page["coverages"]
                require(page["total"] == connection["coverageCount"], "Catalog total differs")
                if page["nextOffset"] is None:
                    break
                require(page["nextOffset"] > offset, "Catalog pagination did not advance")
                offset = page["nextOffset"]
            require(len(catalog) == connection["coverageCount"], "Catalog pagination dropped coverages")
            require(any(v["id"] == description["coverageId"] for v in catalog), "Saved coverage missing from catalog")
            job = c.call("geod_job_status", {"id": args.job})
            require(job["status"] == "succeeded" and job["settled"], "Actual file job not complete")
            require(job["wcsSource"]["planId"] == args.plan, "Task source pin changed")
            inspection = c.call("geod_wcs_inspect", {"id": args.job})
            require("previewDataUrl" not in inspection and inspection["previewOmitted"], "Image leaked into tool text")
            require(inspection["sha256"] == job["sha256"], "Inspection checksum differs")
            require((inspection["width"], inspection["height"]) == (plan["width"], plan["height"]), "Grid dimensions differ")
            path = Path(job["outputPath"]).resolve(strict=True)
            # Native Windows paths use the extended-length prefix; compare the
            # actual directory identity, rather than lexical prefix spellings.
            require(path.parent.samefile(root / "assets") and path.name == f"{args.job}.tif",
                    "Output escaped the verification store")
            require(sha(path.read_bytes()) == job["sha256"], "Actual file checksum differs")
            # Independent numerical decoder, separate from the native/MCP code.
            import rasterio
            with rasterio.open(path) as ds:
                require(ds.width == plan["width"] and ds.height == plan["height"], "Independent dimensions differ")
                require(ds.crs.to_string() == inspection["crs"], "Independent CRS differs")
                samples = ds.read()
                points = [(0, 0), (ds.width - 1, 0), (ds.width // 2, ds.height // 2),
                          (0, ds.height - 1), (ds.width - 1, ds.height - 1)]
                for column, row in points:
                    actual = c.call("geod_wcs_pixel", {"id": args.job, "column": column, "row": row})
                    expected = [float(samples[band, row, column]) for band in range(ds.count)]
                    require(actual["values"] == expected, "MCP original samples differ from rasterio")
                    require(actual["sha256"] == job["sha256"], "Pixel source hash differs")
                    report["independentPixelChecks"].append({"mode": mode, "writes": write,
                                                           "column": column, "row": row, "values": expected})
            if write:
                prepared = c.call("geod_wcs_prepare", {"request": {"descriptionId": description["id"],
                                                                  "bounds": plan["requestedBounds"]}})
                require(prepared == plan, "Native plan preparation changed the pinned selection")
                saved = c.call("geod_wcs_project_save", {"request": {"projectId": args.project,
                              "bounds": project["bounds"], "selections": [{"planId": prepared["id"]}]}})
                require(saved == project, "Appending an existing selection changed the project")
                queued = c.call("geod_wcs_download", {"request": {"projectId": args.project,
                                                                  "selections": [{"planId": args.plan}]}})
                require(len(queued["jobs"]) == 1 and queued["jobs"][0]["jobId"] == args.job,
                        "Native queue failed to reuse the checksum-verified completed file")
                reused = queued["jobs"][0]
                require(reused["poll"]["tool"] == "geod_job_status", "Missing per-job polling instruction")
                require(reused["job"]["status"] == "succeeded" and reused["job"]["settled"], "Reused file not settled")
                # Native source checks reject a private service before network access.
                response = c.request("tools/call", {"name": "geod_wcs_connect", "arguments": {
                    "request": {"name": "reject private test", "url": "http://127.0.0.1:9/wcs"}}})
                require(response["result"]["isError"], "Private source bypassed native URL policy")
            require(not attempts, "An offline MCP workflow attempted external access")
            report["sessions"].append({"mode": mode, "writes": write,
                                       "protocolVersion": c.info["protocolVersion"], "discoveredTools": len(names),
                                       "catalogEntries": len(catalog), "jobStatus": job["status"],
                                       "settled": job["settled"], "bytes": job["bytesDownloaded"],
                                       "sha256": job["sha256"], "width": inspection["width"],
                                       "height": inspection["height"]})
        finally:
            c.close()

    try:
        proxy_file.write_text(json.dumps({"mode": "custom", "url": f"http://127.0.0.1:{trap.server_port}"}), encoding="utf-8")
        session("direct")
        session("direct", True)
        # Refuse to interact with an unrelated service occupying this port.
        import socket
        with socket.socket() as probe:
            require(probe.connect_ex(("127.0.0.1", args.port)) != 0, "Verification port is already occupied")
        server = subprocess.Popen([str(binary), "serve", "--data-dir", str(root), "--port", str(args.port)],
                                  stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
                                  creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        deadline = time.monotonic() + 20
        while True:
            require(server.poll() is None, "Verification runtime exited before readiness")
            try:
                require(api("/health")["status"] == "ok", "Verification runtime is unhealthy")
                break
            except OSError:
                require(time.monotonic() < deadline, "Verification runtime did not become ready")
                time.sleep(.1)
        session("loopback")
        session("loopback", True)
        require(all(j["status"] not in ("running", "queued") for j in api("/jobs")), "Cannot stop a runtime with active jobs")
        server.terminate()
        server.wait(timeout=10)
        server = None
        session("direct")
        report["registryBytesUnchanged"] = all(sha((root / name).read_bytes()) == value for name, value in registry_before.items())
        require(report["registryBytesUnchanged"], "Offline idempotent calls changed persisted records")
        report["blockedProxyAttempts"] = attempts
        report["checks"] = ["read_only_discovery_and_dispatch", "saved_catalog_full_pagination",
                            "exact_pinned_description_plan_project", "native_original_file_inspection",
                            "independent_original_pixel_samples", "local_plan_prepare",
                            "idempotent_project_append", "checksum_verified_download_reuse_and_settlement",
                            "private_source_rejection", "direct_and_loopback_stdio",
                            "protocol_only_stdout_clean_eof", "reconnect_registry_and_file_recovery",
                            "no_external_requests_through_rejecting_proxy"]
        report["status"] = "passed"
        report["finishedAt"] = datetime.now(timezone.utc).isoformat()
        target = Path(args.report)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(json.dumps({"status": report["status"], "stdioSessions": len(report["sessions"]),
                          "independentPixelChecks": len(report["independentPixelChecks"]),
                          "externalRequests": len(attempts), "newPublicDownloads": 0}))
    finally:
        if server is not None:
            server.terminate()
            server.wait(timeout=10)
        trap.shutdown()
        trap.server_close()
        if proxy_before is None:
            proxy_file.unlink(missing_ok=True)
        else:
            proxy_file.write_bytes(proxy_before)


if __name__ == "__main__":
    main()
