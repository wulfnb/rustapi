"""Execute generated cURL, Python, JavaScript and Rust against a local fixture server.

Requires requests, Node 20+, curl, Bash (Git Bash on Windows) and the repository's Rust toolchain.
Run cargo build --locked --examples first. Accepts the target/debug directory.
"""
import http.server
import json
import os
import shutil
import tomllib
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

def sample_project(directory):
    # Let Cargo propagate platform-native linker paths (notably Windows import libs).
    root = Path(__file__).resolve().parents[1]
    config = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    project = directory / "rust-sample"
    (project / "src").mkdir(parents=True)
    manifest = '[package]\nname = "rustapi-codegen-sample"\nversion = "0.0.0"\nedition = "2021"\n'
    manifest += f'\n[dependencies.rustapi]\npath = {json.dumps(str(root))}\n'
    for name in ("reqwest", "tokio"):
        manifest += f"\n[dependencies.{name}]\n"
        for key, value in config["dependencies"][name].items():
            manifest += f"{key} = {json.dumps(value)}\n"
    (project / "Cargo.toml").write_text(manifest, encoding="utf-8")
    # Retain the repository's resolved versions; Cargo only adds this temporary root.
    shutil.copyfile(root / "Cargo.lock", project / "Cargo.lock")
    return project

def run(command):
    result = subprocess.run(command, capture_output=True, text=True, timeout=180)
    if result.returncode:
        raise RuntimeError(f"{command[0]} failed:\n{result.stdout}\n{result.stderr}")

try:
    with tempfile.TemporaryDirectory(prefix="rustapi-codegen-") as temp:
        directory = Path(temp)
        run([str(build / "examples" / f"codegen-fixture{exe}"), temp, base])
        project = sample_project(directory)
        for case in ("json", "text", "form", "urlencoded", "binary", "options"):
            for language in ("sh", "py", "mjs", "rs"):
                source = directory / f"request-{case}.{language}"
                before = len(records)
                if language == "rs":
                    binary = build / f"rustapi-codegen-sample{exe}"
                    shutil.copyfile(source, project / "src/main.rs")
                    run(["cargo", "build", "--offline", "--manifest-path", str(project / "Cargo.toml"), "--target-dir", str(build.parent), "--bin", "rustapi-codegen-sample"])
                    run([str(binary)])
                    binary.unlink()
                elif language == "sh":
                    # Multipart exports are shell programs (mktemp/trap/printf/cat),
                    # not argv lists. Windows CI supplies Git Bash explicitly.
                    run([os.environ.get("RUSTAPI_BASH", "bash"), str(source)])
                else:
                    run([{"py": sys.executable, "mjs": "node"}[language], str(source)])
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
                    assert bytes([7, 6, 5, 4]) in body
                    assert body.count(b'name="upload"') == 2
                    assert body.lower().count(b"content-type: application/x-fixture") == 2
                    assert b"content-type: text/plain; charset=utf-8" in body.lower()
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
