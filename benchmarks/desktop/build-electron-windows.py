"""Build the pinned, vendored Electron reference; all build output stays in target/."""
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]
REFERENCE = ROOT / "benchmarks/desktop/reference/electron"
BUILD = ROOT / "target/desktop-benchmark/electron-source"


def run(*args):
    subprocess.run([str(arg) for arg in args], cwd=BUILD, check=True)


def main():
    BUILD.mkdir(parents=True, exist_ok=True)
    for name in ("package.json", "package-lock.json"):
        shutil.copyfile(REFERENCE / name, BUILD / name)
    for name in ("electron", "web"):
        shutil.copytree(REFERENCE / name, BUILD / name, dirs_exist_ok=True)
    shutil.copyfile(REFERENCE / "windows-main.ts", BUILD / "windows-main.ts")
    shutil.copyfile(REFERENCE.parent / "workload.ts", BUILD / "workload.ts")
    shutil.copyfile(ROOT / "examples/issue_tracker/issues.json", BUILD / "web/issues.generated.json")
    app = BUILD / "app"
    app.mkdir(exist_ok=True)
    (app / "package.json").write_text(json.dumps({
        "name": "benchmark-electron", "version": "1.0.0", "main": "main.js"
    }))
    shutil.copyfile(BUILD / "web/index.html", app / "index.html")
    run("npm.cmd", "ci", "--no-audit", "--no-fund")
    binary = BUILD / "node_modules/.bin"
    run(binary / "esbuild.cmd", "windows-main.ts", "--bundle", "--platform=node",
        "--format=cjs", "--external:electron", "--outfile=app/main.js")
    run(binary / "esbuild.cmd", "web/app.ts", "--bundle", "--platform=browser",
        "--outfile=app/app.js")
    run(binary / "electron-packager.cmd", "app", "Benchmark Electron", "--platform=win32",
        "--arch=x64", "--electron-version=44.2.0", "--asar", "--overwrite",
        "--out=" + str(ROOT / "target/desktop-benchmark/electron-package"))


if __name__ == "__main__":
    main()
