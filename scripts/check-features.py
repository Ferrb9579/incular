"""Compile supported facade feature sets; no tests or GUI applications are run."""
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    for features in ("", "desktop", "controls", "material", "devtools", "desktop,controls,material,devtools"):
        command = ["cargo", "check", "-p", "incular", "--lib", "--locked", "--no-default-features"]
        if features:
            command += ["--features", features]
        print("Checking features: " + (features or "none"), flush=True)
        subprocess.run(command, cwd=ROOT, check=True)
    subprocess.run(["cargo", "check", "-p", "incular", "--lib", "--locked"], cwd=ROOT, check=True)


if __name__ == "__main__":
    main()
