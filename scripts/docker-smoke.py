"""Check the non-root GUI, noVNC WebSocket, restart, and persistent profile."""
import base64
import os
import socket
import subprocess
import sys
import time
import urllib.request
import uuid

image = sys.argv[1]
name = "rustapi-smoke-" + uuid.uuid4().hex[:12]
volume = name + "-data"


def docker(*args):
    return subprocess.check_output(["docker", *args], text=True, timeout=120).strip()


def wait_healthy():
    deadline = time.monotonic() + 120
    while time.monotonic() < deadline:
        status = docker("inspect", "--format", "{{.State.Health.Status}}", name)
        if status == "healthy":
            return
        if status == "unhealthy":
            break
        time.sleep(2)
    raise RuntimeError("Container did not become healthy")


try:
    docker("volume", "create", volume)
    docker("run", "-d", "--name", name, "--shm-size=256m", "-p", "127.0.0.1::6080", "-v", volume + ":/data", image)
    wait_healthy()
    assert docker("exec", name, "id", "-u") == "1000"
    docker("exec", name, "python3", "/usr/local/bin/check-portal.py")
    port = int(docker("port", name, "6080/tcp").rsplit(":", 1)[1])
    with urllib.request.urlopen(f"http://127.0.0.1:{port}/vnc.html", timeout=5) as response:
        assert response.status == 200
        assert b"noVNC" in response.read()
    # Upgrade all the way through websockify and require the VNC server greeting.
    with socket.create_connection(("127.0.0.1", port), timeout=10) as stream:
        key = base64.b64encode(os.urandom(16)).decode()
        stream.sendall((f"GET /websockify HTTP/1.1\r\nHost: localhost:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: binary\r\n\r\n").encode())
        received = b""
        while b"RFB " not in received:
            chunk = stream.recv(4096)
            assert chunk, "VNC connection closed before handshake"
            received += chunk
            assert len(received) < 65536
        assert b"101 Switching Protocols" in received
    docker("exec", "-e", "RUSTAPI_DATA_DIR=/tmp/isolated-smoke", name, "rustapi", "--smoke-test")
    docker("exec", name, "sh", "-c", "printf persistent > /data/smoke-marker")
    docker("restart", name)
    wait_healthy()
    assert docker("exec", name, "cat", "/data/smoke-marker") == "persistent"
    print("Docker GUI, file chooser portal, WebSocket, non-root runtime, and persistence passed")
except Exception:
    subprocess.run(["docker", "logs", name], check=False, timeout=30)
    raise
finally:
    subprocess.run(["docker", "rm", "-f", name], check=False, timeout=30)
    subprocess.run(["docker", "volume", "rm", volume], check=False, timeout=30)
