#!/usr/bin/env python3
"""Assemble a reviewable release; never install it or run downloaded code."""
import hashlib
import json
import pathlib
import shutil
import subprocess
import tarfile
import tomllib

root = pathlib.Path(__file__).resolve().parent.parent
manifest = json.loads((root / "manifest.json").read_text())
cargo = tomllib.loads((root / "Cargo.toml").read_text())
assert manifest["version"] == cargo["package"]["version"], "Manifest/Cargo versions differ"
metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--offline", "--format-version=1"], cwd=root))
licenses = root / "licenses"
licenses.mkdir(exist_ok=True)
index = ["# Rust dependency licenses\n", "Notices for the locked dependency graph, including optional/build-time crates. Enabled runtime dependencies are compiled into bin/omafm.\n"]
for package in metadata["packages"]:
    if package["name"] == "omafm":
        continue
    source = pathlib.Path(package["manifest_path"]).parent
    destination = licenses / f'{package["name"]}-{package["version"]}'
    destination.mkdir(exist_ok=True)
    files = [f for f in source.iterdir() if f.is_file() and f.name.lower().startswith(("license", "copying", "notice"))]
    if not files:
        raise RuntimeError(f'No license files for {package["name"]}')
    for file in files:
        shutil.copyfile(file, destination / file.name)
    index.append(f'- {package["name"]} {package["version"]}: {package["license"]}; {package.get("repository", "")}\n')
(licenses / "README.md").write_text("\n".join(index))
info = {
    "version": manifest["version"],
    "target": "x86_64-unknown-linux-gnu",
    "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
    "binarySha256": hashlib.sha256((root / "bin/omafm").read_bytes()).hexdigest(),
    "sourceSha256": {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
                     for p in sorted([root / "Cargo.toml", root / "Cargo.lock", *root.glob("src/*.rs")])},
}
(root / "bin/build-info.json").write_text(json.dumps(info, indent=2) + "\n")
destination = root / "dist" / manifest["id"]
destination.mkdir(parents=True, exist_ok=True)
for name in ["manifest.json", "BarWidget.qml", "Panel.qml", "Receiver.qml", "Bookmarks.qml", "README.md",
             "PUBLISHING.md", "LICENSE", "preview.png", "VALIDATION.md", "Cargo.toml", "Cargo.lock", "bin", "licenses", "src", "tests", "scripts"]:
    source = root / name
    if source.is_dir():
        shutil.copytree(source, destination / name, dirs_exist_ok=True)
    else:
        shutil.copy2(source, destination / name)
assert not any(p.is_symlink() for p in destination.rglob("*")), "Plugin cannot contain symlinks"
archive = root / "dist" / f'omafm-{manifest["version"]}-linux-x86_64.tar.gz'
with tarfile.open(archive, "w:gz") as tar:
    tar.add(destination, arcname=manifest["id"])
archive.with_suffix(archive.suffix + ".sha256").write_text(hashlib.sha256(archive.read_bytes()).hexdigest() + "  " + archive.name + "\n")
