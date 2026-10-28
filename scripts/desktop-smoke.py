"""Run the real native event loop with an isolated profile and bounded timeout."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile

binary = str(Path(sys.argv[1]).resolve())
with tempfile.TemporaryDirectory(prefix="rustapi-smoke-") as profile:
    result = subprocess.run(
        [binary, "--smoke-test"],
        env={**os.environ, "RUSTAPI_DATA_DIR": profile},
        capture_output=True, text=True, timeout=60,
    )
    print(result.stdout)
    print(result.stderr, file=sys.stderr)
    result.check_returncode()
    if "RustAPI desktop smoke test passed" not in result.stdout:
        raise SystemExit("Native event loop did not complete")
    if not (Path(profile) / "collections").is_dir():
        raise SystemExit("Application did not initialize its profile")
    for filename in ("state.json", "history.json"):
        if not (Path(profile) / filename).is_file():
            raise SystemExit(f"Application did not persist {filename} on shutdown")
