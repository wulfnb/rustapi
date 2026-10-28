import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "docker/start-desktop.sh"


class DesktopLauncherTests(unittest.TestCase):
    executed = set()

    def launch(self, failures, arguments=()):
        with tempfile.TemporaryDirectory(prefix="rustapi-shell-") as temporary:
            directory = Path(temporary)
            scripts = {
                "xdpyinfo": f'''#!{sys.executable}
import os
from pathlib import Path
path = Path(os.environ["TEST_ATTEMPTS"])
count = int(path.read_text()) + 1 if path.exists() else 1
path.write_text(str(count))
raise SystemExit(1 if count <= int(os.environ["TEST_FAILURES"]) else 0)
''',
                "sleep": "#!/bin/sh\nexit 0\n",
                "dbus-run-session": f"#!{sys.executable}\nimport json, sys\nprint(json.dumps(sys.argv[1:]))\n",
            }
            for name, text in scripts.items():
                path = directory / name
                path.write_text(text)
                path.chmod(0o755)
            result = subprocess.run(
                ["bash", "-x", str(SCRIPT), *arguments],
                env={**os.environ, "PATH": str(directory) + os.pathsep + os.environ["PATH"], "PS4": "+RUSTAPI_LINE:${LINENO}: ", "TEST_ATTEMPTS": str(directory / "attempts"), "TEST_FAILURES": str(failures)},
                capture_output=True, text=True, timeout=30,
            )
            self.executed.update(map(int, re.findall(r"^\++RUSTAPI_LINE:(\d+):", result.stderr, re.MULTILINE)))
            return result, int((directory / "attempts").read_text())

    def test_ready_display_executes_the_application_and_preserves_arguments(self):
        result, attempts = self.launch(0, ("--version", "a value with spaces"))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(attempts, 1)
        self.assertEqual(json.loads(result.stdout), ["--", "rustapi", "--version", "a value with spaces"])

    def test_display_readiness_retries_are_bounded(self):
        result, attempts = self.launch(2)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(attempts, 3)
        result, attempts = self.launch(100)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(attempts, 100)
        self.assertIn("X display failed to start", result.stderr)

    @classmethod
    def tearDownClass(cls):
        executable = {
            number for number, line in enumerate(SCRIPT.read_text().splitlines(), 1)
            if line.strip() and not line.lstrip().startswith("#") and line.strip() not in ("fi", "done")
        }
        missing = executable - cls.executed
        report = {"file": "docker/start-desktop.sh", "total": len(executable), "covered": len(executable - missing), "missing": sorted(missing)}
        if destination := os.environ.get("RUSTAPI_COVERAGE_DIR"):
            output = Path(destination)
            output.mkdir(parents=True, exist_ok=True)
            (output / "shell.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    unittest.main()
