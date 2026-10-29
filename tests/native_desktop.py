"""Real Linux window/file-dialog regression test; run under Xvfb and D-Bus."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser()
parser.add_argument("binary", type=Path)
parser.add_argument("--output", type=Path, default=Path("coverage/native"))
args = parser.parse_args()
binary = args.binary.resolve()
args.output.mkdir(parents=True, exist_ok=True)
for name in ("native-start.png", "native-context.png", "native-chooser.png", "native-failure.png"):
    (args.output / name).unlink(missing_ok=True)


def xdo(*arguments, check=True):
    return subprocess.run(["xdotool", *map(str, arguments)], text=True, capture_output=True, check=check, timeout=10).stdout.strip()


def wait_for(predicate, description, timeout=30):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        if app.poll() is not None:
            raise RuntimeError(f"Application exited while waiting for {description}")
        time.sleep(0.1)
    raise RuntimeError(f"Timed out waiting for {description}")


def window(pattern):
    ids = xdo("search", "--onlyvisible", "--name", pattern, check=False).splitlines()
    return ids[-1] if ids else None


def click(client, x, y, button=1):
    # Re-enter the client after GTK releases its pointer grab.
    xdo("mousemove", 0, 0)
    xdo("mousemove", "--sync", "--window", client, x, y)
    time.sleep(0.15)
    xdo("click", button)


def geometry(client):
    return dict(line.split("=", 1) for line in xdo("getwindowgeometry", "--shell", client).splitlines())


def choose_file(pattern, path=None):
    dialog = wait_for(lambda: window(pattern), "native file dialog")
    xdo("windowactivate", "--sync", dialog)
    time.sleep(0.3)
    if path is None:
        xdo("key", "Escape")
    else:
        xdo("key", "ctrl+l")
        time.sleep(0.1)
        xdo("key", "ctrl+a")
        xdo("type", "--clearmodifiers", "--delay", "1", str(path))
        time.sleep(0.1)
        xdo("key", "Return")
        time.sleep(0.2)
        if window(pattern):
            subprocess.run(["scrot", "--overwrite", str(args.output / "native-chooser.png")], check=True)
            size = geometry(dialog)
            click(dialog, int(size["WIDTH"]) - 50, int(size["HEIGHT"]) - 25)
    wait_for(lambda: window(pattern) is None, "file dialog to close")


def import_file(main, path=None):
    xdo("windowactivate", "--sync", main)
    xdo("windowfocus", "--sync", main)
    time.sleep(0.3)
    click(main, int(geometry(main)["WIDTH"]) - 50, 20)
    choose_file("^Import collection$", path)


def export_file(main, path=None):
    xdo("windowactivate", "--sync", main)
    xdo("windowfocus", "--sync", main)
    time.sleep(0.3)
    click(main, 80, 142, 3)
    time.sleep(0.2)
    subprocess.run(["scrot", "--overwrite", str(args.output / "native-context.png")], check=True)
    click(main, 125, 191)
    choose_file("^Export collection$", path)


with tempfile.TemporaryDirectory(prefix="rustapi-native-") as temporary:
    directory = Path(temporary)
    profile = directory / "profile"
    imported = directory / "input.json"
    exported = directory / "exported.json"
    fixture = {"id": "native-collection", "name": "Native imported", "description": "", "version": "1.0.0", "requests": [], "folders": [], "variables": {}, "created_at": "2026-10-28T00:00:00Z", "updated_at": "2026-10-28T00:00:00Z"}
    original = json.dumps(fixture)
    imported.write_text(original)
    log = (args.output / "application.log").open("w")
    manager = subprocess.Popen(["openbox"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    # Start the window manager before winit creates its first hidden window.
    for _ in range(100):
        if xdo("get_desktop", check=False):
            break
        time.sleep(0.1)
    started = time.monotonic()
    app = subprocess.Popen([str(binary)], env={**os.environ, "RUSTAPI_DATA_DIR": str(profile)}, stdout=log, stderr=subprocess.STDOUT)
    try:
        main = wait_for(lambda: window("RustAPI - API Testing Client"), "main window", timeout=90)
        wait_for(lambda: xdo("getactivewindow", check=False), "window manager")
        xdo("windowactivate", "--sync", main)
        time.sleep(0.5)
        subprocess.run(["scrot", "--overwrite", str(args.output / "native-start.png")], check=True)
        print(f"Main window ready after {time.monotonic() - started:.1f}s", flush=True)
        print("Main window:", xdo("getwindowgeometry", "--shell", main), flush=True)
        import_file(main, imported)
        saved = profile / "collections" / "Native imported.json"
        wait_for(saved.is_file, "imported collection to persist")
        assert imported.read_text() == original
        assert json.loads(saved.read_text())["id"] == fixture["id"]
        import_file(main)
        export_file(main, exported)
        wait_for(exported.is_file, "exported collection")
        assert json.loads(exported.read_text())["name"] == "My Collection"
        export_file(main)
        xdo("windowactivate", "--sync", main)
        xdo("key", "alt+F4")
        assert app.wait(timeout=20) == 0
        assert (profile / "state.json").is_file()
        assert (profile / "history.json").is_file()
        print("Real import/export dialogs, cancellation, source preservation, and shutdown persistence passed", flush=True)
    except Exception:
        print("Files:", list(directory.rglob("*")), flush=True)
        for name in ("wchan", "status"):
            path = Path(f"/proc/{app.pid}/{name}")
            if path.exists():
                print(name, path.read_text(), flush=True)
        for identifier in xdo("search", "--name", ".", check=False).splitlines():
            print("Visible window:", identifier, xdo("getwindowname", identifier, check=False), flush=True)
        subprocess.run(["scrot", "--pointer", "--overwrite", str(args.output / "native-failure.png")], check=False)
        raise
    finally:
        if app.poll() is None:
            app.terminate()
            app.wait(timeout=15)
        if manager.poll() is None:
            manager.terminate()
            manager.wait(timeout=10)
        log.close()
