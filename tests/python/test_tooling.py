import contextlib
import io
import json
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[2]


def run_script(relative, args=(), environment=None):
    with patch.object(__import__("sys"), "argv", [relative, *map(str, args)]):
        with patch.dict(os.environ, environment or {}, clear=False):
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                return runpy.run_path(str(ROOT / relative), run_name="__main__")


class PackageTests(unittest.TestCase):
    def test_native_archives_preserve_binary_notices_and_clean_stale_files(self):
        for platform in ("linux", "windows", "macos"):
            with self.subTest(platform=platform), tempfile.TemporaryDirectory() as directory:
                base = Path(directory)
                for name in ("README.md", "LICENSE", "NOTICE", "CONTRIBUTING.md"):
                    shutil.copy2(ROOT / name, base)
                (base / "input.bin").write_bytes(b"native-binary-fixture")
                github_output = base / "github-output"
                external_calls = []

                def external(command, **kwargs):
                    self.assertTrue(kwargs["check"])
                    external_calls.append(command)
                    if command[0] == "codesign":
                        app = Path(command[-1])
                        self.assertEqual((app / "Contents/Resources/LICENSE").read_bytes(), (ROOT / "LICENSE").read_bytes())
                    elif command[0] == "ditto":
                        if "-x" in command:
                            with zipfile.ZipFile(command[-2]) as archive:
                                archive.extractall(command[-1])
                        else:
                            stage = Path(command[-2])
                            with zipfile.ZipFile(command[-1], "w") as archive:
                                for path in stage.rglob("*"):
                                    archive.write(path, path.relative_to(stage.parent))
                    elif command[1] == "create":
                        Path(command[-1]).write_bytes(b"dmg-fixture")
                    else:
                        self.assertEqual(command[:2], ["hdiutil", "verify"])
                        self.assertTrue(Path(command[-1]).is_file())
                    return subprocess.CompletedProcess(command, 0, "", "")

                with contextlib.chdir(base), patch("subprocess.run", side_effect=external), patch("subprocess.check_output", return_value=b'{"packages":[{"version":"0.1.0"}]}'):
                    icon_file = base / "assets" / "icons" / "RustAPIIcon.icns"
                    for iteration in range(2):
                        if iteration == 0 and platform == "macos":
                            icon_file.parent.mkdir(parents=True, exist_ok=True)
                            icon_file.write_bytes(b"icns-fixture")
                        elif icon_file.exists():
                            icon_file.unlink()
                        run_script("scripts/package.py", ["--binary", "input.bin", "--platform", platform, "--arch", "arm64"], {"GITHUB_OUTPUT": str(github_output)})
                    output_path = Path(github_output.read_text().splitlines()[-1].split("=", 1)[1])
                    self.assertEqual(output_path.read_bytes(), b"native-binary-fixture")
                    notices = base / "dist" / f"RustAPI-{platform}-arm64"
                    for name in ("LICENSE", "NOTICE", "CONTRIBUTING.md"):
                        self.assertEqual((notices / name).read_bytes(), (ROOT / name).read_bytes())
                    with patch.dict(os.environ, {}, clear=True):
                        run_script("scripts/package.py", ["--binary", "input.bin", "--platform", platform, "--arch", "x86_64"])
                    if platform == "macos":
                        self.assertTrue(any(c[:2] == ["codesign", "--verify"] for c in external_calls))
                        self.assertTrue(any(c[:2] == ["hdiutil", "verify"] for c in external_calls))

    def test_missing_license_refuses_an_incomplete_distribution(self):
        with tempfile.TemporaryDirectory() as directory, contextlib.chdir(directory):
            with self.assertRaises(FileNotFoundError):
                run_script("scripts/package.py", ["--binary", "missing", "--platform", "linux", "--arch", "x86_64"])

    def test_disk_image_verification_retries_only_temporary_resource_failures(self):
        for mode, expected_attempts in (("recover", 2), ("exhausted", 5), ("corrupt", 1), ("unknown", 1)):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as directory:
                base = Path(directory)
                (base / "input.bin").write_bytes(b"native-binary-fixture")
                (base / "README.md").write_text("fixture")
                attempts = []

                def external(command, **kwargs):
                    self.assertTrue(kwargs["check"])
                    if command[:2] == ["hdiutil", "verify"]:
                        attempts.append(command)
                        self.assertTrue(kwargs["capture_output"])
                        self.assertTrue(kwargs["text"])
                        if mode != "recover" or len(attempts) == 1:
                            stderr = {"recover": "Resource temporarily unavailable",
                                      "exhausted": "Resource temporarily unavailable",
                                      "corrupt": "checksum mismatch", "unknown": None}[mode]
                            raise subprocess.CalledProcessError(1, command, stderr=stderr)
                    return subprocess.CompletedProcess(command, 0, "Verified DMG\n", "")

                with contextlib.chdir(base), patch("subprocess.run", side_effect=external), patch("time.sleep") as sleep:
                    with patch("subprocess.check_output", return_value=b'{"packages":[{"version":"0.1.0"}]}'):
                        args = ["--binary", "input.bin", "--platform", "macos", "--arch", "arm64"]
                        if mode == "recover":
                            run_script("scripts/package.py", args)
                        else:
                            with self.assertRaises(subprocess.CalledProcessError):
                                run_script("scripts/package.py", args)
                    self.assertEqual(len(attempts), expected_attempts)
                    self.assertEqual([call.args[0] for call in sleep.call_args_list],
                                     [2 ** attempt for attempt in range(expected_attempts - 1)])


class DesktopSmokeTests(unittest.TestCase):
    def invoke(self, *, marker=True, collections=True, state=True, history=True, returncode=0):
        def launch(command, **kwargs):
            profile = Path(kwargs["env"]["RUSTAPI_DATA_DIR"])
            self.assertNotEqual(str(profile), os.environ.get("RUSTAPI_DATA_DIR"))
            self.assertEqual(command[1:], ["--smoke-test"])
            if collections:
                (profile / "collections").mkdir()
            for filename, present in (("state.json", state), ("history.json", history)):
                if present:
                    (profile / filename).write_text("{}")
            return subprocess.CompletedProcess(command, returncode, "RustAPI desktop smoke test passed" if marker else "", "diagnostic")
        with patch("subprocess.run", side_effect=launch):
            run_script("scripts/desktop-smoke.py", ["target/debug/rustapi"])

    def test_success_uses_an_isolated_profile_and_requires_shutdown_persistence(self):
        self.invoke()

    def test_startup_failures_and_missing_evidence_are_reported(self):
        for options in ({"marker": False}, {"collections": False}, {"state": False}, {"history": False}):
            with self.subTest(options=options), self.assertRaises(SystemExit):
                self.invoke(**options)
        with self.assertRaises(subprocess.CalledProcessError):
            self.invoke(returncode=1)


class HealthTests(unittest.TestCase):
    def invoke(self, window=True, status=200, supervisor=True):
        response = SimpleNamespace(status=status)
        with patch("subprocess.run", side_effect=None if supervisor else subprocess.CalledProcessError(3, "supervisorctl")):
            with patch("subprocess.check_output", return_value=b"RustAPI - API Testing Client" if window else b"no window"):
                with patch("urllib.request.urlopen") as request:
                    request.return_value.__enter__.return_value = response
                    run_script("docker/healthcheck.py")
                    request.assert_called_once_with("http://127.0.0.1:6080/vnc.html", timeout=3)

    def test_health_requires_supervisor_application_and_viewer(self):
        self.invoke()
        for options in ({"window": False}, {"status": 500}):
            with self.subTest(options=options), self.assertRaises(SystemExit):
                self.invoke(**options)
        with self.assertRaises(subprocess.CalledProcessError):
            self.invoke(supervisor=False)


class FakeProcess:
    def __init__(self, pid, comm="rustapi", environment=b"DBUS_SESSION_BUS_ADDRESS=unix:path=/test\0", error=None):
        self.name, self.comm, self.environment, self.error = pid, comm, environment, error

    def __truediv__(self, name):
        if name == "comm":
            def read():
                if self.error:
                    raise self.error
                return self.comm
            return SimpleNamespace(read_text=read)
        return SimpleNamespace(read_bytes=lambda: self.environment)


class PortalTests(unittest.TestCase):
    def invoke(self, processes, output=b"org.freedesktop.portal.FileChooser"):
        def call(command, **kwargs):
            self.assertEqual(kwargs["env"]["DBUS_SESSION_BUS_ADDRESS"], "unix:path=/test")
            self.assertEqual(command[0], "dbus-send")
            return subprocess.CompletedProcess(command, 0, output, b"")
        with patch("pathlib.Path.iterdir", return_value=iter(processes)), patch("subprocess.run", side_effect=call):
            run_script("docker/check-portal.py")

    def test_portal_finds_the_application_session_and_skips_unreadable_processes(self):
        self.invoke([FakeProcess("self"), FakeProcess("1", comm="other"), FakeProcess("2", error=PermissionError()), FakeProcess("3", environment=b"\xff"), FakeProcess("4")])

    def test_missing_application_or_file_chooser_is_an_error(self):
        for processes, output in (([], b""), ([FakeProcess("4")], b"no FileChooser")):
            with self.subTest(processes=processes), self.assertRaises(SystemExit):
                self.invoke(processes, output)


class DockerSmokeTests(unittest.TestCase):
    def invoke(self, statuses=("starting", "healthy", "healthy"), frames=None, clock=None):
        statuses = iter(statuses)
        calls = []

        def docker(command, **kwargs):
            calls.append(command)
            self.assertEqual(command[0], "docker")
            if command[1] == "inspect":
                return next(statuses)
            if command[1] == "port":
                return "127.0.0.1:34567"
            if command[1] == "exec" and "id" in command:
                return "1000"
            if command[1] == "exec" and "cat" in command:
                return "persistent"
            return "fixture"

        frames = iter(frames if frames is not None else [b"HTTP/1.1 101 Switching Protocols\r\n\r\n", b"\x82\x0cRFB 003.008\n"])
        response = SimpleNamespace(status=200, read=lambda: b"noVNC viewer")
        with patch("subprocess.check_output", side_effect=docker), patch("subprocess.run") as cleanup:
            with patch("urllib.request.urlopen") as request, patch("socket.create_connection") as connect:
                request.return_value.__enter__.return_value = response
                stream = connect.return_value.__enter__.return_value
                stream.recv.side_effect = lambda _: next(frames)
                with patch("time.sleep"), patch("time.monotonic", side_effect=clock or range(10000)):
                    try:
                        run_script("scripts/docker-smoke.py", ["rustapi:fixture"])
                    finally:
                        cleanup_commands = [call.args[0] for call in cleanup.call_args_list]
                        self.assertTrue(any(c[:3] == ["docker", "rm", "-f"] for c in cleanup_commands))
                        self.assertTrue(any(c[:3] == ["docker", "volume", "rm"] for c in cleanup_commands))
        self.assertTrue(any(c[1] == "restart" for c in calls))
        self.assertTrue(any("127.0.0.1::6080" in c for c in calls))
        self.assertIn(b"GET /websockify", stream.sendall.call_args.args[0])

    def test_docker_browser_gui_restart_persistence_and_loopback_binding(self):
        self.invoke()

    def test_failed_health_and_websocket_always_cleanup_task_resources(self):
        for options, error in (({"statuses": ["unhealthy"]}, RuntimeError), ({"clock": [0, 121]}, RuntimeError), ({"frames": [b""]}, AssertionError), ({"frames": [b"x" * 65537]}, AssertionError), ({"frames": [b"HTTP/1.1 200 OK RFB "]}, AssertionError)):
            with self.subTest(options=options), self.assertRaises(error):
                self.invoke(**options)


if __name__ == "__main__":
    unittest.main()
