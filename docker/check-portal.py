"""Require the file-chooser portal on the running application's D-Bus session."""
import os
from pathlib import Path
import subprocess

for process in Path("/proc").iterdir():
    if not process.name.isdigit():
        continue
    try:
        if (process / "comm").read_text().strip() != "rustapi":
            continue
        variables = dict(
            item.split("=", 1)
            for item in (process / "environ").read_bytes().decode().split("\0")
            if "=" in item
        )
    except (OSError, UnicodeError):
        continue
    result = subprocess.run(
        ["dbus-send", "--session", "--print-reply", "--dest=org.freedesktop.portal.Desktop",
         "/org/freedesktop/portal/desktop", "org.freedesktop.DBus.Introspectable.Introspect"],
        env={**os.environ, **variables}, capture_output=True, timeout=30, check=True,
    )
    if b"org.freedesktop.portal.FileChooser" not in result.stdout:
        raise SystemExit("Desktop portal does not expose FileChooser")
    print("File-chooser portal is available on the RustAPI session")
    break
else:
    raise SystemExit("Could not find the running RustAPI session")
