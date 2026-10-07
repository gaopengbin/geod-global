"""Two live status inquiries, one owned native transfer, one deliberate network fault.

Windows-only opt-in acceptance. The source TLS connection is passed through intact:
this does not replace source bytes or inject queue records. No user store is opened.
"""
from pathlib import Path
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse
import hashlib
import json
import os
import socket
import socketserver
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
import winreg

ROOT = Path(__file__).resolve().parents[2]
HOST = "sentinel-cogs.s3.us-west-2.amazonaws.com"
MODEL = "deepseek-v4-flash"
RUN = ROOT / ".verification" / f"agent-project-task-model-fault-{time.time_ns()}"
RUN.mkdir()
secret = os.environ.get("LAOGAO_API_KEY")
if not secret:
    with winreg.OpenKey(winreg.HKEY_CURRENT_USER, "Environment") as key:
        secret = winreg.QueryValueEx(key, "LAOGAO_API_KEY")[0]
assert isinstance(secret, str) and secret
opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
trip = threading.Event()
lock = threading.Lock()
traffic = []
model_requests = []
configured_proxy = urllib.request.getproxies().get("https")
upstream_proxy = urlparse(configured_proxy) if configured_proxy else None
if upstream_proxy:
    assert upstream_proxy.scheme == "http" and upstream_proxy.hostname and upstream_proxy.port
    assert not upstream_proxy.username and not upstream_proxy.password


def headers(stream):
    data = bytearray()
    while not data.endswith(b"\r\n\r\n"):
        byte = stream.recv(1)
        if not byte:
            raise ConnectionError("Tunnel headers ended early")
        data.extend(byte)
        if len(data) > 8192:
            raise ValueError("Tunnel headers exceeded limit")
    return bytes(data)


class SourceTunnel(socketserver.BaseRequestHandler):
    def handle(self):
        remote = None
        record = {"target": HOST, "encryptedSourceBytesForwarded": 0,
                  "controlledAbort": False, "status": "connecting"}
        with lock:
            traffic.append(record)
        try:
            self.request.settimeout(10)
            request = headers(self.request)
            if request.split(b"\r\n", 1)[0] != f"CONNECT {HOST}:443 HTTP/1.1".encode():
                self.request.sendall(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n")
                raise ValueError("Unrelated source host refused")
            if upstream_proxy:
                remote = socket.create_connection((upstream_proxy.hostname, upstream_proxy.port), timeout=15)
                remote.sendall(f"CONNECT {HOST}:443 HTTP/1.1\r\nHost: {HOST}:443\r\n\r\n".encode())
                response = headers(remote)
                assert response.split(b"\r\n", 1)[0].split()[1] == b"200"
            else:
                remote = socket.create_connection((HOST, 443), timeout=15)
            self.request.sendall(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            remote.settimeout(.5)
            self.request.settimeout(.5)
            record["status"] = "forwarding"
            stop = threading.Event()

            def client_to_source():
                try:
                    while not trip.is_set() and not stop.is_set():
                        try:
                            chunk = self.request.recv(16384)
                        except socket.timeout:
                            continue
                        if not chunk:
                            break
                        remote.sendall(chunk)
                except OSError:
                    pass
                finally:
                    stop.set()

            sender = threading.Thread(target=client_to_source, daemon=True)
            sender.start()
            try:
                while not trip.is_set() and not stop.is_set():
                    try:
                        chunk = remote.recv(16384)
                    except socket.timeout:
                        continue
                    if not chunk:
                        break
                    if record["encryptedSourceBytesForwarded"] >= 65536 and trip.wait(2):
                        break
                    self.request.sendall(chunk)
                    record["encryptedSourceBytesForwarded"] += len(chunk)
            finally:
                record["controlledAbort"] = trip.is_set()
                stop.set()
                for stream in (remote, self.request):
                    try:
                        stream.shutdown(socket.SHUT_RDWR)
                    except OSError:
                        pass
                sender.join(timeout=2)
            record["status"] = "closed"
        except Exception as error:
            record["status"] = "transport-error"
            record["errorType"] = type(error).__name__
        finally:
            if remote:
                remote.close()


class OwnedSourceServer(socketserver.ThreadingTCPServer):
    daemon_threads = True


with socket.socket() as reservation:
    reservation.bind(("127.0.0.1", 0))
    relay_port = reservation.getsockname()[1]
relay = f"http://127.0.0.1:{relay_port}"


class Forwarder(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_POST(self):
        if self.path == "/task-fault/trip":
            trip.set()
            self.send_response(200)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        if self.path != "/v1/chat/completions" or self.headers.get("Authorization") != "Bearer " + secret:
            self.send_error(403)
            return
        length = int(self.headers.get("Content-Length", "0"))
        if not 0 < length < 8_000_000:
            self.send_error(413)
            return
        body = self.rfile.read(length)
        value = json.loads(body)
        if value.get("model") != MODEL:
            self.send_error(403)
            return
        record = {"model": MODEL, "toolDefinitionCount": len(value.get("tools", [])), "status": "running"}
        with lock:
            if len(model_requests) >= 10:
                self.send_error(429)
                return
            model_requests.append(record)
        started = time.monotonic()
        try:
            request = urllib.request.Request(relay + self.path, data=body, method="POST",
                headers={"Authorization": "Bearer " + secret, "Content-Type": "application/json"})
            with opener.open(request, timeout=100) as response:
                record["httpStatus"] = response.status
                self.send_response(response.status)
                self.send_header("Content-Type", response.headers.get("Content-Type", "application/json"))
                self.end_headers()
                while True:
                    chunk = response.read1(16384)
                    if not chunk:
                        break
                    self.wfile.write(chunk)
                    self.wfile.flush()
            record["status"] = "completed"
        except urllib.error.HTTPError as error:
            record["httpStatus"] = error.code
            record["status"] = "http-error"
            self.send_response(error.code)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(b'{"error":{"message":"Owned acceptance model route refused the request"}}')
        except Exception as error:
            record["status"] = "transport-error"
            record["errorType"] = type(error).__name__
            try:
                self.send_error(502)
            except OSError:
                pass
        finally:
            record["elapsedSeconds"] = round(time.monotonic() - started, 3)


source = OwnedSourceServer(("127.0.0.1", 0), SourceTunnel)
capture = ThreadingHTTPServer(("127.0.0.1", 0), Forwarder)
capture.daemon_threads = True
threads = [threading.Thread(target=server.serve_forever, daemon=True) for server in (source, capture)]
for thread in threads:
    thread.start()
record = {"schema": "geod-agent-project-task-model-launch/v1", "scenario": "active-to-controlled-network-failure",
          "status": "running", "modelRoutes": [MODEL], "modelKeyInFilesOrArgs": False,
          "jobStatesInjected": False, "usedUserDesktop": False, "published": False}


def save():
    (RUN / "launch.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")


def get(path, auth=False):
    request = urllib.request.Request(relay + path, headers={"Authorization": "Bearer " + secret} if auth else {})
    with opener.open(request, timeout=20) as response:
        return json.load(response)


tunnel = subprocess.Popen(["ssh.exe", "-N", "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=yes",
    "-o", "ExitOnForwardFailure=yes", "-o", "ServerAliveInterval=30", "-o", "ServerAliveCountMax=3",
    "-i", str(Path.home() / ".ssh/laogao_tencent_ed25519"), "-L",
    f"127.0.0.1:{relay_port}:127.0.0.1:9094", "ubuntu@62.234.147.130"],
    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, creationflags=subprocess.CREATE_NO_WINDOW)
record["ownedTunnelPid"] = tunnel.pid
save()
try:
    for _ in range(60):
        assert tunnel.poll() is None
        try:
            with socket.create_connection(("127.0.0.1", relay_port), timeout=.3):
                break
        except OSError:
            time.sleep(.2)
    assert get("/api/status").get("success")
    prices = get("/api/pricing")["data"]
    assert MODEL in {model["id"] for model in get("/v1/models", True)["data"]}
    price = next(value for value in prices if value["model_name"] == MODEL)
    record["liveRouteDiscoveryAndConsumerPricingVerified"] = True
    record["consumerPrice"] = {key: price.get(key) for key in ("model_name", "quota_type", "model_ratio", "completion_ratio")}
    env = os.environ.copy()
    env.update({"GEOD_AGENT_TEST_KEY": secret, "GEOD_AGENT_TEST_MODEL": MODEL,
        "GEOD_AGENT_TEST_BASE_URL": f"http://127.0.0.1:{capture.server_port}/v1",
        "GEOD_AGENT_TEST_NATIVE_VAULT": "1", "GEOD_AGENT_CONNECTION_QA": str(RUN / "case"),
        "GEOD_AGENT_TASK_FAULT_SCENARIO": "1", "GEOD_AGENT_TASK_JOBS": str(ROOT / ".verification/cli-e2e-roundtrip/store/jobs.json"),
        "GEOD_AGENT_TASK_PROXY": f"http://127.0.0.1:{source.server_address[1]}",
        "GEOD_AGENT_TASK_FAULT_CONTROL_PORT": str(capture.server_port),
        "NO_PROXY": "127.0.0.1,localhost", "NODE_USE_ENV_PROXY": "1"})
    print(json.dumps({"run": str(RUN), "status": "running", "model": MODEL, "plannedTurns": 2}), flush=True)
    with (RUN / "native.log").open("wb") as log:
        process = subprocess.Popen(["cargo", "test", "--locked", "--offline", "-p", "geod-global-desktop",
            "--features", "custom-protocol", "agent::task_status_model_tests::live_transfer::live_active_failed_tasks_are_native_read_only_and_resume",
            "--", "--ignored", "--exact", "--nocapture"], cwd=ROOT, env=env,
            stdout=log, stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW)
        record["nativeProcessPid"] = process.pid
        save()
        try:
            code = process.wait(timeout=360)
        except subprocess.TimeoutExpired:
            process.terminate()
            process.wait(timeout=20)
            raise RuntimeError("Bounded task status acceptance timed out")
    record["nativeExit"] = code
    record["status"] = "passed" if code == 0 else "failed"
    if code == 0:
        native = json.loads((RUN / "case/native-acceptance.json").read_text(encoding="utf-8"))
        assert native["modelTurns"] == 2 and not native["jobStatesInjected"]
        assert any(value["controlledAbort"] and value["encryptedSourceBytesForwarded"] > 65536 for value in traffic)
        assert model_requests and all(value["status"] == "completed" and value["httpStatus"] == 200 for value in model_requests)
        calls = [entry for entry in native["turns"][-1]["selected"]["entries"] if entry["type"] == "tool"]
        record["nativeModelToolCalls"] = len(calls)
        record["failedModelToolCalls"] = sum(entry["status"] != "completed" for entry in calls)
        assert record["failedModelToolCalls"] == 0
except Exception as error:
    record["status"] = "failed"
    record["errorType"] = type(error).__name__
    raise
finally:
    trip.set()
    for server in (source, capture):
        server.shutdown()
        server.server_close()
    for thread in threads:
        thread.join(timeout=5)
    tunnel.terminate()
    tunnel.wait(timeout=15)
    record["ownedTunnelStopped"] = tunnel.poll() is not None
    record["ownedServersStopped"] = all(not thread.is_alive() for thread in threads)
    record["modelRequests"] = model_requests
    record["sourceTunnels"] = traffic
    save()
    cleanup = subprocess.run([sys.executable, str(ROOT / ".verification/verify-owned-agent-vault.py"), str(RUN)],
        capture_output=True, creationflags=subprocess.CREATE_NO_WINDOW)
    record["ownedVaultCleanupReadVerified"] = cleanup.returncode == 0
    scanned = 0
    for file in RUN.rglob("*"):
        if file.is_file():
            assert secret.encode() not in file.read_bytes(), "Model key was found in an owned acceptance file"
            scanned += 1
    record["ownedFilesScannedForModelKey"] = scanned
    save()
    assert cleanup.returncode == 0, "Owned acceptance vault cleanup requires repair"
    print(json.dumps({"status": record["status"], "modelRequests": len(model_requests),
                      "nativeModelToolCalls": record.get("nativeModelToolCalls"),
                      "ownedTunnelStopped": record["ownedTunnelStopped"],
                      "ownedVaultCleanupReadVerified": record["ownedVaultCleanupReadVerified"]}), flush=True)
raise SystemExit(record["nativeExit"])
