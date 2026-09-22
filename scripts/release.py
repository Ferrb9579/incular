"""Local release checks. This script never uploads or publishes a crate (Python 3.9+)."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tarfile
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]


def metadata(full=False):
    command = ["cargo", "metadata", "--format-version", "1", "--locked", "--all-features"]
    if not full:
        command.append("--no-deps")
    return json.loads(subprocess.check_output(command, cwd=ROOT))


def publishable():
    return [p for p in metadata()["packages"] if p["publish"] != []]


def manifest_check():
    license_bytes = (ROOT / "LICENSE").read_bytes()
    packages = publishable()
    names = {p["name"] for p in packages}
    for package in packages:
        directory = Path(package["manifest_path"]).parent
        for key in ("description", "license", "repository", "readme", "documentation", "rust_version"):
            if not package.get(key):
                raise ValueError(f"{package['name']}: missing {key}")
        if (directory / "LICENSE").read_bytes() != license_bytes:
            raise ValueError(f"{package['name']}: license copy differs from root")
        if not package["metadata"].get("docs", {}).get("rs"):
            raise ValueError(f"{package['name']}: missing docs.rs metadata")
        for dependency in package["dependencies"]:
            if dependency["name"] in names and dependency["req"] == "*":
                raise ValueError(f"{package['name']}: unversioned internal dependency")
    remaining = {p["name"]: p for p in packages}
    layers = []
    while remaining:
        layer = sorted(name for name, p in remaining.items() if not any(
            d["name"] in remaining and d["kind"] != "dev" for d in p["dependencies"]))
        if not layer:
            raise ValueError("normal/build dependency cycle")
        layers.append(layer)
        for name in layer:
            del remaining[name]
    print(json.dumps({"packages": len(packages), "normal_build_layers": layers}, indent=2))


def archives():
    report = []
    for package in publishable():
        stem = f"{package['name']}-{package['version']}"
        path = ROOT / "target/package" / (stem + ".crate")
        with tarfile.open(path) as archive:
            members = archive.getmembers()
            names = {m.name for m in members}
            for member in members:
                if not member.name.startswith(stem + "/") or ".." in Path(member.name).parts or not member.isfile():
                    raise ValueError(f"unexpected archive entry: {member.name}")
            for required in ("LICENSE", "README.md", "Cargo.toml", "src/lib.rs"):
                if stem + "/" + required not in names:
                    raise ValueError(f"{stem}: missing {required}")
            if archive.extractfile(stem + "/LICENSE").read() != (ROOT / "LICENSE").read_bytes():
                raise ValueError(f"{stem}: stale license")
            normalized = archive.extractfile(stem + "/Cargo.toml").read().decode()
            for target in re.findall(r'^path\s*=\s*"([^"]+)"', normalized, re.MULTILINE):
                if stem + "/" + target not in names:
                    raise ValueError(f"{stem}: target missing from archive: {target}")
            # Reject stale archives, including changed rustdoc source/README.
            directory = Path(package["manifest_path"]).parent
            for member in members:
                relative = member.name[len(stem) + 1:]
                if relative.startswith(("src/", "examples/")) or relative == "README.md":
                    if archive.extractfile(member).read() != (directory / relative).read_bytes():
                        raise ValueError(f"{stem}: stale archive entry {relative}; repackage")
        report.append({"crate": stem, "bytes": path.stat().st_size,
                       "sha256": hashlib.sha256(path.read_bytes()).hexdigest()})
    output = ROOT / "target/release-archives.json"
    output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Verified {len(report)} self-contained archives; hashes: {output}")


def registry():
    report = []
    for package in publishable():
        name = package["name"]
        request = urllib.request.Request("https://crates.io/api/v1/crates/" + name,
                                         headers={"User-Agent": "Incular release readiness (github.com/Ferrb9579/incular)"})
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                data = json.load(response)
            row = {"name": name, "status": "exists", "version": data["crate"]["max_version"],
                   "repository": data["crate"].get("repository")}
        except urllib.error.HTTPError as error:
            if error.code != 404:
                raise
            row = {"name": name, "status": "not_found_at_check_time"}
        report.append(row)
        print(f"{name}: {row['status']}", flush=True)
        time.sleep(1)
    (ROOT / "target/release-registry.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")


def inventory():
    data = metadata(full=True)
    packages = [{k: p.get(k) for k in ("name", "version", "license", "repository", "source")}
                for p in data["packages"] if p["source"] is not None]
    packages.sort(key=lambda p: (p["name"], p["version"]))
    output = ROOT / "docs/dependency-inventory.json"
    output.write_text(json.dumps({"scope": "Cargo.lock, all features and targets; not a license clearance",
                                  "packages": packages}, indent=2) + "\n", encoding="utf-8")
    print(f"Recorded {len(packages)} third-party packages in {output}")


def links():
    paths = [ROOT / name for name in ("README.md", "CONTRIBUTING.md", "SECURITY.md", "CHANGELOG.md", "ROADMAP.md", "CODE_OF_CONDUCT.md")]
    paths += list((ROOT / "docs").glob("*.md")) + list((ROOT / "crates").glob("*/README.md"))
    for path in paths:
        text = re.sub(r"```.*?```", "", path.read_text(encoding="utf-8"), flags=re.DOTALL)
        for link in re.findall(r"\]\(([^)]+)\)", text):
            if "://" in link or link.startswith(("#", "mailto:")):
                continue
            target = link.split("#", 1)[0]
            if target and not (path.parent / target).exists():
                raise ValueError(f"{path.relative_to(ROOT)}: broken link {link}")
    print(f"Local Markdown links checked in {len(paths)} files")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["metadata", "archives", "registry", "inventory", "links", "package"])
    parser.add_argument("--allow-dirty", action="store_true", help="Only for local pre-commit package rehearsal")
    args = parser.parse_args()
    if args.action == "package":
        command = ["cargo", "package", "--workspace", "--exclude", "incular-devtools-ui", "--locked"]
        if args.allow_dirty:
            command.append("--allow-dirty")
        subprocess.run(command, cwd=ROOT, check=True)
        archives()
    else:
        {"metadata": manifest_check, "archives": archives, "registry": registry,
         "inventory": inventory, "links": links}[args.action]()


if __name__ == "__main__":
    main()
