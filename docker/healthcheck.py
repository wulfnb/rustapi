"""Require the supervisor processes, real application window, and web viewer."""
import subprocess
import urllib.request

subprocess.run(
    ["supervisorctl", "-c", "/etc/rustapi/supervisord.conf", "status"],
    check=True, capture_output=True, timeout=3,
)
windows = subprocess.check_output(["xwininfo", "-root", "-tree"], timeout=3)
if b"RustAPI - API Testing Client" not in windows:
    raise SystemExit("RustAPI window is missing")
with urllib.request.urlopen("http://127.0.0.1:6080/vnc.html", timeout=3) as response:
    if response.status != 200:
        raise SystemExit("noVNC viewer is unavailable")
