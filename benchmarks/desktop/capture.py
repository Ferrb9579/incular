"""Capture a separate validation launch, leaving measured launches untouched."""
import argparse
import os
from pathlib import Path
import struct
import subprocess
import time
import zlib

root = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser()
parser.add_argument("--executable", type=Path, default=root / "target/release/examples/issue_tracker.exe")
parser.add_argument("--output", type=Path, default=root / "benchmarks/desktop/results/windows")
parser.add_argument("--backend", choices=["default", "vulkan", "dx12", "gl"], default="default")
parser.add_argument("--adapter")
parser.add_argument("--smoke-only", action="store_true", help="Validate interactions without requiring surface readback")
parser.add_argument("--states", action="store_true", help="Capture initial, filtered, completed, searched, and empty UI states")
args = parser.parse_args()
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=True)
ppm = output / "issue-tracker.ppm"
ppm.unlink(missing_ok=True)
ready = output / "capture-ready.txt"
ready.unlink(missing_ok=True)
env = dict(os.environ, INCULAR_BENCH_CAPTURE=str(ppm), INCULAR_BENCH_READY=str(ready))
env.pop("INCULAR_BENCH_STATE_CAPTURES", None)
if args.states:
    env["INCULAR_BENCH_STATE_CAPTURES"] = str(output)
    env["INCULAR_BENCH_SMOKE"] = "1"
if args.smoke_only:
    env.pop("INCULAR_BENCH_CAPTURE", None)
    env["INCULAR_BENCH_SMOKE"] = "1"
env.pop("INCULAR_BENCH_PROFILE", None)
env.pop("WGPU_BACKEND", None)
env.pop("WGPU_ADAPTER_NAME", None)
if args.backend != "default":
    env["WGPU_BACKEND"] = args.backend
if args.adapter:
    env["WGPU_ADAPTER_NAME"] = args.adapter
with (output / "capture.log").open("w") as log:
    process = subprocess.Popen([str(args.executable.resolve())],
                               env=env, stdout=log, stderr=log,
                               creationflags=subprocess.CREATE_NO_WINDOW)
    try:
        deadline = time.monotonic() + 30
        while not ready.exists():
            if process.poll() is not None or time.monotonic() > deadline:
                raise RuntimeError("Capture failed; inspect capture.log")
            time.sleep(.2)
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=10)
if args.smoke_only and not args.states:
    print(f"Interaction smoke passed: {output / 'capture.log'}")
    raise SystemExit(0)
for ppm in ([ppm] if not args.states else sorted(output.glob('*.ppm'))):
    with ppm.open("rb") as source:
        assert source.readline() == b"P6\n"
        width, height = map(int, source.readline().split())
        assert source.readline() == b"255\n"
        pixels = source.read()
    assert len(pixels) == width * height * 3
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    scanlines = b"".join(b"\x00" + pixels[y * width * 3:(y + 1) * width * 3] for y in range(height))
    png = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
           + chunk(b"IDAT", zlib.compress(scanlines)) + chunk(b"IEND", b""))
    ppm.with_suffix(".png").write_bytes(png)
    print(f"Captured {width} x {height}: {ppm.with_suffix('.png')}")
