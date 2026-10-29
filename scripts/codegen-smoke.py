"""Execute generated cURL, Python, JavaScript and Rust against a local fixture server.

Requires requests, Node 20+, curl, bash and the repository's Rust toolchain.
Run cargo build --locked --examples first. Accepts the target/debug directory.
"""
import http.server
import json
import os
import shlex
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
from urllib.parse import parse_qs, urlsplit

build = Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug").resolve()
exe = ".exe" if os.name == "nt" else ""
records = []

class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        url = urlsplit(self.path)
        body = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        records.append((url.path[1:], parse_qs(url.query), dict(self.headers), body))
        self.send_response(200)
        self.send_header("Content-Length", "11")
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(b'{"ok":true}')

    do_OPTIONS = do_POST
    def log_message(self, *_args):
        pass

server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
thread = threading.Thread(target=server.serve_forever, daemon=True)
thread.start()
base = f"http://127.0.0.1:{server.server_address[1]}"

def dependency(name):
    return max((build / "deps").glob(f"lib{name}-*.rlib"), key=lambda p: p.stat().st_mtime)

def run(command):
    result = subprocess.run(command, capture_output=True, text=True, timeout=60)
    if result.returncode:
        raise RuntimeError(f"{command[0]} failed:\n{result.stdout}\n{result.stderr}")

try:
    with tempfile.TemporaryDirectory(prefix="rustapi-codegen-") as temp:
        directory = Path(temp)
        run([str(build / "examples" / f"codegen-fixture{exe}"), temp, base])
        for case in ("json", "text", "form", "urlencoded", "binary", "options"):
            for language in ("sh", "py", "mjs", "rs"):
                source = directory / f"request-{case}.{language}"
                before = len(records)
                if language == "rs":
                    binary = directory / f"sample{exe}"
                    run(["rustc", "--edition=2021", "--crate-name=sample", str(source), "-L", f"dependency={build / 'deps'}", "--extern", f"reqwest={dependency('reqwest')}", "--extern", f"tokio={dependency('tokio')}", "-o", str(binary)])
                    run([str(binary)])
                    binary.unlink()
                elif language == "sh" and os.name == "nt":
                    run(shlex.split(source.read_text().replace("\\\n", "")))
                else:
                    run([{"sh": "bash", "py": sys.executable, "mjs": "node"}[language], str(source)])
                assert len(records) == before + 1, (case, language, "request count")
                actual, query, headers, body = records[-1]
                assert actual == case and query == {"q": ["a b&c"], "api_key": ["key with spaces"]}, (case, language, query)
                headers = {k.lower(): v for k, v in headers.items()}
                assert headers["x-quote"] == 'quote-"-dollar-$(echo nope)', (case, language)
                if case == "json":
                    assert json.loads(body)["ready"] is True and json.loads(body)["nil"] is None
                elif case == "text":
                    assert b"${never}" in body and b"\x01" in body and "🦀" in body.decode()
                elif case == "form":
                    assert b"Ada & friends" in body and bytes([0, 255, 128, 1, 13, 10]) in body
                    assert "multipart/form-data; boundary=" in headers["content-type"]
                elif case == "urlencoded":
                    assert parse_qs(body.decode()) == {"name": ["Ada & friends"]}
                elif case == "binary":
                    assert body == bytes([0, 255, 128, 1, 13, 10])
                print(f"PASS {language} {case}")
finally:
    server.shutdown()
    server.server_close()
print(f"Validated {len(records)} generated HTTP requests")
