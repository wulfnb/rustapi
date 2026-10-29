"""Package a native binary; macOS gets an ad-hoc signed app, ZIP, and DMG."""
import argparse
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import tarfile
import zipfile

parser = argparse.ArgumentParser()
parser.add_argument("--binary", type=Path, required=True)
parser.add_argument("--platform", choices=["macos", "linux", "windows"], required=True)
parser.add_argument("--arch", choices=["arm64", "x86_64"], required=True)
args = parser.parse_args()
name = f"RustAPI-{args.platform}-{args.arch}"
dist = Path("dist")
dist.mkdir(exist_ok=True)
stage = dist / name
if stage.exists():
    shutil.rmtree(stage)
stage.mkdir()
for notice in ("LICENSE", "NOTICE"):
    shutil.copy2(notice, stage)

if args.platform == "macos":
    app = stage / "RustAPI.app"
    contents = app / "Contents"
    (contents / "MacOS").mkdir(parents=True)
    binary = contents / "MacOS" / "rustapi"
    shutil.copy2(args.binary, binary)
    with (contents / "Info.plist").open("wb") as output:
        # Read from Cargo metadata so release tags cannot silently change the app version.
        import json
        metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--no-deps", "--format-version=1"]))
        version = metadata["packages"][0]["version"]
        plistlib.dump({
            "CFBundleName": "RustAPI", "CFBundleDisplayName": "RustAPI",
            "CFBundleIdentifier": "org.rustapi.desktop", "CFBundleExecutable": "rustapi",
            "CFBundlePackageType": "APPL", "CFBundleShortVersionString": version,
            "CFBundleVersion": version, "NSHighResolutionCapable": True,
        }, output)
    binary.chmod(0o755)
    subprocess.run(["codesign", "--force", "--sign", "-", str(app)], check=True)
    subprocess.run(["codesign", "--verify", "--strict", str(app)], check=True)
    shutil.copy2("README.md", stage)
    subprocess.run(["ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", str(stage), str(dist / (name + ".zip"))], check=True)
    dmg_stage = dist / (name + "-dmg")
    if dmg_stage.exists():
        shutil.rmtree(dmg_stage)
    dmg_stage.mkdir()
    shutil.copytree(app, dmg_stage / app.name)
    for notice in ("LICENSE", "NOTICE"):
        shutil.copy2(notice, dmg_stage)
    (dmg_stage / "Applications").symlink_to("/Applications")
    subprocess.run(["hdiutil", "create", "-volname", "RustAPI", "-srcfolder", str(dmg_stage), "-ov", "-format", "UDZO", str(dist / (name + ".dmg"))], check=True)
    subprocess.run(["hdiutil", "verify", str(dist / (name + ".dmg"))], check=True)
else:
    binary = stage / ("rustapi.exe" if args.platform == "windows" else "rustapi")
    shutil.copy2(args.binary, binary)
    binary.chmod(0o755)
    shutil.copy2("README.md", stage)
    if args.platform == "windows":
        with zipfile.ZipFile(dist / (name + ".zip"), "w", zipfile.ZIP_DEFLATED) as archive:
            for path in stage.rglob("*"):
                archive.write(path, path.relative_to(dist))
    else:
        with tarfile.open(dist / (name + ".tar.gz"), "w:gz") as archive:
            archive.add(stage, arcname=name)

# Test the actual archived distribution, not just the staging directory.
unpacked = dist / (name + "-unpacked")
if unpacked.exists():
    shutil.rmtree(unpacked)
unpacked.mkdir()
if args.platform == "macos":
    subprocess.run(["ditto", "-x", "-k", str(dist / (name + ".zip")), str(unpacked)], check=True)
    packaged_binary = unpacked / name / "RustAPI.app" / "Contents" / "MacOS" / "rustapi"
elif args.platform == "windows":
    with zipfile.ZipFile(dist / (name + ".zip")) as archive:
        archive.extractall(unpacked)
    packaged_binary = unpacked / name / "rustapi.exe"
else:
    with tarfile.open(dist / (name + ".tar.gz")) as archive:
        archive.extractall(unpacked, filter="data")
    packaged_binary = unpacked / name / "rustapi"
print(packaged_binary)
if "GITHUB_OUTPUT" in os.environ:
    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        output.write(f"binary={packaged_binary}\n")
