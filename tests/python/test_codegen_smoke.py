"""Validate cross-platform sample orchestration without requiring the external SDKs."""
import contextlib
import http.server
import io
import json
from pathlib import Path
import runpy
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]


class CodegenSmokeTests(unittest.TestCase):
    def invoke(self, platform="posix", default_directory=False, fail=False, omit_request=False):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            build = directory / "target/debug"
            build.mkdir(parents=True)
            captured = {}
            calls = []
            closed = []

            def server(_address, handler):
                captured["handler"] = handler
                return SimpleNamespace(server_address=("127.0.0.1", 12345),
                                       serve_forever=lambda: None,
                                       shutdown=lambda: closed.append("shutdown"),
                                       server_close=lambda: closed.append("close"))

            def command(command, **kwargs):
                calls.append(command)
                self.assertEqual(kwargs["timeout"], 180)
                if fail:
                    return subprocess.CompletedProcess(command, 9, "fixture output", "fixture failure")
                executable = Path(command[0]).name
                if executable.startswith("codegen-fixture"):
                    output = Path(command[1])
                    for case in ("json", "text", "form", "urlencoded", "binary", "options"):
                        for extension in ("sh", "py", "mjs", "rs"):
                            content = f"curl http://fixture/request-{case}"
                            if case == "form" and extension == "sh":
                                content = "set -eu\nbody=$(mktemp)\n" + content
                            (output / f"request-{case}.{extension}").write_text(content, encoding="utf-8")
                    captured["handler"].log_message(None)
                    return subprocess.CompletedProcess(command, 0, "", "")
                if executable == "cargo":
                    self.assertIn("--offline", command)
                    self.assertNotIn("--extern", command)
                    project = Path(command[command.index("--manifest-path") + 1]).parent
                    manifest = (project / "Cargo.toml").read_text(encoding="utf-8")
                    self.assertIn('[dependencies.reqwest]', manifest)
                    self.assertIn('[dependencies.tokio]', manifest)
                    self.assertIn(str(ROOT).replace('\\', '\\\\'), manifest)
                    self.assertTrue((project / "Cargo.lock").is_file())
                    suffix = ".exe" if platform == "nt" else ""
                    (build / f"rustapi-codegen-sample{suffix}").write_bytes(b"fixture binary")
                    return subprocess.CompletedProcess(command, 0, "", "")
                if omit_request:
                    return subprocess.CompletedProcess(command, 0, "", "")
                # Each runner must address the expected sample, not a different language.
                globals_ = captured["handler"].do_POST.__globals__
                case = globals_["case"]
                if globals_["language"] == "sh":
                    self.assertEqual(executable, "git-bash")
                    self.assertTrue(Path(command[1]).is_file())
                if not executable.startswith("rustapi-codegen-sample"):
                    self.assertIn(f"request-{case}", str(command[-1]))
                headers = {"x-quote": 'quote-"-dollar-$(echo nope)'}
                bodies = {
                    "json": b'{"ready":true,"nil":null}',
                    "text": "${never}\x01🦀".encode(),
                    "form": (
                        b'--fixture\r\nContent-Disposition: form-data; name="name"\r\n'
                        b'Content-Type: text/plain; charset=utf-8\r\n\r\nAda & friends\r\n'
                        b'--fixture\r\nContent-Disposition: form-data; name="upload"; filename="upload.bin"\r\n'
                        b'Content-Type: application/x-fixture\r\n\r\n\0\xff\x80\x01\r\n\r\n'
                        b'--fixture\r\nContent-Disposition: form-data; name="upload"; filename="second.bin"\r\n'
                        b'Content-Type: application/x-fixture\r\n\r\n\x07\x06\x05\x04\r\n--fixture--\r\n'
                    ),
                    "urlencoded": b"name=Ada+%26+friends",
                    "binary": bytes([0, 255, 128, 1, 13, 10]),
                    "options": b"",
                }
                body = bodies[case]
                headers["Content-Length"] = str(len(body))
                if case == "form":
                    headers["content-type"] = "multipart/form-data; boundary=fixture"
                request = captured["handler"].__new__(captured["handler"])
                request.path = f"/{case}?q=a%20b%26c&api_key=key%20with%20spaces"
                request.headers = headers
                request.rfile = io.BytesIO(body)
                request.wfile = io.BytesIO()
                request.send_response = lambda status: self.assertEqual(status, 200)
                request.send_header = lambda *_args: None
                request.end_headers = lambda: None
                request.do_OPTIONS() if case == "options" else request.do_POST()
                self.assertEqual(json.loads(request.wfile.getvalue()), {"ok": True})
                return subprocess.CompletedProcess(command, 0, "", "")

            argv = ["scripts/codegen-smoke.py", *([] if default_directory else [str(build)])]
            with contextlib.chdir(directory), patch.object(sys, "argv", argv):
                with patch.dict(sys.modules, {"os": SimpleNamespace(name=platform, environ={"RUSTAPI_BASH": "git-bash"})}):
                    with patch("http.server.ThreadingHTTPServer", side_effect=server), patch("subprocess.run", side_effect=command):
                        with contextlib.redirect_stdout(io.StringIO()) as output:
                            try:
                                result = runpy.run_path(str(ROOT / "scripts/codegen-smoke.py"), run_name="__main__")
                            finally:
                                self.assertEqual(closed, ["shutdown", "close"])
            self.assertEqual(len(result["records"]), 24)
            self.assertIn("Validated 24 generated HTTP requests", output.getvalue())
            self.assertEqual(sum(c[0] == "cargo" for c in calls), 6)
            self.assertFalse((build / "rustapi-codegen-sample").exists())
            self.assertFalse((build / "rustapi-codegen-sample.exe").exists())

    def test_posix_default_directory_executes_and_validates_all_languages(self):
        self.invoke(default_directory=True)

    def test_windows_rust_samples_use_cargo_for_native_linker_configuration(self):
        self.invoke(platform="nt")

    def test_command_failure_is_reported_and_always_closes_the_server(self):
        with self.assertRaisesRegex(RuntimeError, "fixture failure"):
            self.invoke(fail=True)

    def test_a_sample_that_sends_no_request_fails_validation(self):
        with self.assertRaises(AssertionError):
            self.invoke(omit_request=True)
