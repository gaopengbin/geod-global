"""Exercise the real stdio MCP binary against an already running local runtime.

Uses only Python's standard library. --allow-write explicitly creates one real
crop job; the default proves read-only discovery, validation and inspection.
The JSON report contains no local paths or image payloads.
"""
import argparse
import json
import queue
import subprocess
import threading
import time
from pathlib import Path


class Client:
    def __init__(self, executable, server, allow_write=False):
        command = [str(Path(executable).resolve()), "serve-mcp", "--server", server]
        if allow_write:
            command.append("--allow-write")
        self.process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=subprocess.PIPE, text=True, encoding="utf-8")
        self.lines = queue.Queue()
        self.errors = []
        self.sequence = 0

        def read_stdout():
            for line in self.process.stdout:
                self.lines.put(line)
            self.lines.put(None)

        def read_stderr():
            self.errors.extend(self.process.stderr.readlines())

        threading.Thread(target=read_stdout, daemon=True).start()
        threading.Thread(target=read_stderr, daemon=True).start()
        self.info = self.request("initialize", {"protocolVersion": "2025-11-25", "capabilities": {},
                                "clientInfo": {"name": "geod-verification", "version": "1"}})["result"]
        self.notify("notifications/initialized")

    def notify(self, method, params=None):
        message = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            message["params"] = params
        self.process.stdin.write(json.dumps(message, ensure_ascii=False) + "\n")
        self.process.stdin.flush()

    def request(self, method, params=None):
        self.sequence += 1
        message = {"jsonrpc": "2.0", "id": self.sequence, "method": method}
        if params is not None:
            message["params"] = params
        self.process.stdin.write(json.dumps(message, ensure_ascii=False) + "\n")
        self.process.stdin.flush()
        while True:
            line = self.lines.get(timeout=150)
            assert line is not None, "MCP exited before a response: " + "".join(self.errors)
            response = json.loads(line)  # Every stdout line must be protocol JSON.
            assert response["jsonrpc"] == "2.0"
            if response.get("id") == self.sequence:
                return response
            assert "id" not in response, "Unexpected response ID"

    def call(self, name, arguments=None):
        response = self.request("tools/call", {"name": name, "arguments": arguments or {}})
        assert "error" not in response, response
        result = response["result"]
        assert not result.get("isError", False), result
        content = result["structuredContent"]
        assert json.loads(result["content"][0]["text"]) == content
        return content

    def close(self):
        self.process.stdin.close()
        try:
            assert self.process.wait(timeout=20) == 0, "".join(self.errors)
            assert self.lines.get(timeout=2) is None, "Non-protocol trailing stdout"
        finally:
            if self.process.poll() is None:
                self.process.terminate()
                self.process.wait(timeout=10)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", required=True)
    parser.add_argument("--server", default="http://127.0.0.1:4318")
    parser.add_argument("--recipe", required=True)
    parser.add_argument("--report", required=True)
    parser.add_argument("--allow-write", action="store_true")
    args = parser.parse_args()
    recipe = json.loads(Path(args.recipe).read_text(encoding="utf-8"))
    source_id = recipe["source"]["jobId"]
    report = {"schemaVersion": "geod-mcp-verification/v1", "sourceId": source_id,
              "checks": [], "writeEnabled": args.allow_write}
    readonly = Client(args.executable, args.server)
    try:
        report["protocolVersion"] = readonly.info["protocolVersion"]
        names = [tool["name"] for tool in readonly.request("tools/list")["result"]["tools"]]
        assert len(names) == 7 and "geod_recipe_run" not in names, names
        denied = readonly.request("tools/call", {"name": "geod_recipe_run", "arguments": {"recipe": recipe}})
        assert denied["error"]["code"] == -32602
        report["checks"].append("read_only_discovery_and_dispatch")
        assert readonly.call("geod_health")["runtime"]["status"] == "ok"
        assert isinstance(readonly.call("geod_jobs_list", {"limit": 1})["jobs"], list)
        assert isinstance(readonly.call("geod_recipes_list", {"limit": 1})["recipes"], list)
        source = readonly.call("geod_job_status", {"id": source_id})
        assert source["status"] == "succeeded" and source["settled"]
        assert source["sha256"] == recipe["source"]["sha256"]
        inspection = readonly.call("geod_raster_inspect", {"id": source_id})
        assert "previewDataUrl" not in inspection and inspection["previewOmitted"]
        plan = readonly.call("geod_recipe_plan", {"recipe": recipe})
        assert plan["plan"]["width"] > 0 and plan["plan"]["height"] > 0
        report["plan"] = plan["plan"]
        x = (inspection["bounds"][0] + inspection["bounds"][2]) / 2
        y = (inspection["bounds"][1] + inspection["bounds"][3]) / 2
        pixel = readonly.call("geod_raster_pixel", {"id": source_id, "x": x, "y": y})
        assert 0 <= pixel["value"] <= 11 and pixel["sha256"] == source["sha256"]
        report["sourcePixel"] = pixel
        report["checks"].append("real_source_inspect_plan_pixel")
        invalid = json.loads(json.dumps(recipe))
        invalid["source"]["sha256"] = "0" * 64
        failed = readonly.request("tools/call", {"name": "geod_recipe_plan", "arguments": {"recipe": invalid}})
        assert failed["result"]["isError"]
        report["checks"].append("pin_mismatch_is_tool_error")
    finally:
        readonly.close()

    if args.allow_write:
        writable = Client(args.executable, args.server, True)
        try:
            names = [tool["name"] for tool in writable.request("tools/list")["result"]["tools"]]
            assert len(names) == 12 and "geod_recipe_run" in names
            submitted = writable.call("geod_recipe_run", {"recipe": recipe})
            job_id = submitted["jobId"]
            assert submitted["poll"]["tool"] == "geod_job_status"
            deadline = time.monotonic() + 150
            while True:
                job = writable.call("geod_job_status", {"id": job_id})
                if job["settled"] and job["status"] not in ("running", "queued"):
                    break
                assert time.monotonic() < deadline, "Crop did not settle"
                time.sleep(0.2)
            assert job["status"] == "succeeded", job
            output = writable.call("geod_raster_inspect", {"id": job_id})
            assert output["width"] == plan["plan"]["width"]
            assert output["height"] == plan["plan"]["height"]
            assert output["sha256"] == job["sha256"]
            assert "previewDataUrl" not in output
            report["output"] = {"jobId": job_id, "status": job["status"], "settled": job["settled"],
                                "sha256": job["sha256"], "width": output["width"], "height": output["height"]}
            report["checks"].append("explicit_write_real_crop_and_settled_inspection")
        finally:
            writable.close()
        reopened = Client(args.executable, args.server)
        try:
            persisted = reopened.call("geod_job_status", {"id": job_id})
            assert persisted["sha256"] == report["output"]["sha256"] and persisted["settled"]
            report["checks"].append("server_ownership_survives_mcp_disconnect")
        finally:
            reopened.close()
    report["checks"].append("protocol_only_stdout_and_clean_eof_exit")
    destination = Path(args.report)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False))


if __name__ == "__main__":
    main()
