"""Real Aedes, Socket.IO and grpc-js interoperability; all listeners are loopback."""
import base64
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
BINARY = Path(sys.argv[1]).resolve()
NODE_MODULES = Path(sys.argv[2]).resolve()
environment = dict(os.environ, RUSTAPI_PROTOCOL_NODE_MODULES=str(NODE_MODULES))
tls_directory = tempfile.TemporaryDirectory(prefix="rustapi-protocol-tls-")
key = str(Path(tls_directory.name) / "key.pem")
cert = str(Path(tls_directory.name) / "cert.pem")
subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", key, "-out", cert, "-days", "1", "-subj", "/CN=localhost"], check=True, capture_output=True)
environment.update(RUSTAPI_TEST_TLS_KEY=key, RUSTAPI_TEST_TLS_CERT=cert)
server = subprocess.Popen(["node", str(Path(__file__).with_name("servers.mjs"))], env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
try:
    assert select.select([server.stdout], [], [], 15)[0], "Protocol servers did not start"
    line = server.stdout.readline()
    assert line, server.stderr.read()
    ports = json.loads(line)
    with tempfile.TemporaryDirectory(prefix="rustapi-protocol-interop-") as temporary:
        directory = Path(temporary)
        def run(profile, expect_tls_rejection=False):
            path = directory / "session.json"
            path.write_text(json.dumps(profile))
            result = subprocess.run([str(BINARY), "protocol", "run", str(path), "--workspace", str(directory / "workspace")], env=environment, text=True, capture_output=True, timeout=15)
            if expect_tls_rejection:
                assert result.returncode != 0, "Untrusted MQTT certificate was accepted"
                assert "tls" in result.stderr.lower() or "certificate" in result.stderr.lower(), result.stderr
                return []
            assert result.returncode == 0, result.stderr
            return [json.loads(line) for line in result.stdout.splitlines()]

        mqtt = run({"protocol":"Mqtt", "request":{"url":f"mqtt://127.0.0.1:{ports['mqtt']}"}, "duration_seconds":5, "max_events":1, "topic":"rustapi/interop", "qos":1, "messages":["hello 🦀"]})
        payload = next(event["data"] for event in mqtt if event["kind"] == "message")
        assert base64.b64decode(payload["base64"]).decode() == "hello 🦀"
        run({"protocol":"Mqtt", "request":{"url":f"mqtts://localhost:{ports['mqtts']}"}, "duration_seconds":5, "max_events":1}, expect_tls_rejection=True)
        socket = run({"protocol":"SocketIo", "request":{"url":f"http://127.0.0.1:{ports['socket']}"}, "duration_seconds":5, "max_events":1, "socket_event":"echo", "socket_auth":'{"token":"validator-token"}', "messages":['{"hello":"🦀"}']})
        assert next(event["data"] for event in socket if event["kind"] == "event") == ["reply", {"received":{"hello":"🦀"}}]
        descriptor = str(Path(__file__).with_name("echo.proto"))
        profile = {"protocol":"Grpc", "request":{"url":f"grpc://127.0.0.1:{ports['grpc']}","body":'{"text":"hello 🦀"}',"auth":{"auth_type":"BearerToken","bearer_token":"validator-token"}}, "duration_seconds":5,"max_events":1,"grpc_descriptor":descriptor,"grpc_method":"fixture.Echo/Unary"}
        unary = run(profile)
        assert next(event["data"] for event in unary if event["kind"] == "message") == {"text":"hello 🦀"}
        profile["grpc_method"] = "fixture.Echo/Stream"
        profile["max_events"] = 3
        stream = run(profile)
        assert [event["data"]["text"] for event in stream if event["kind"] == "message"] == ["hello 🦀-0", "hello 🦀-1", "hello 🦀-2"]
        profile["max_events"] = 1
        limited = run(profile)
        assert len([event for event in limited if event["kind"] == "message"]) == 1
        print("6 protocol interoperability cases passed (Aedes, MQTT TLS rejection, Socket.IO, grpc-js unary/stream/limit)")
finally:
    server.terminate()
    try:
        server.wait(timeout=5)
    except subprocess.TimeoutExpired:
        server.kill()
        server.wait(timeout=5)
    tls_directory.cleanup()
