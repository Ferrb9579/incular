"""Short exploratory backend probes, not substitutes for the strict benchmark."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("measure", Path(__file__).with_name("run-windows.py"))
measure = importlib.util.module_from_spec(spec)
spec.loader.exec_module(measure)

parser = argparse.ArgumentParser()
parser.add_argument("--executable", type=Path, default=ROOT / "target/release/examples/issue_tracker.exe")
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--backends", nargs="+", default=["default", "dx12", "vulkan"])
parser.add_argument("--ready", action="store_true", help="Use the benchmark's three-frame readiness handshake")
args = parser.parse_args()
args.output.parent.mkdir(parents=True, exist_ok=True)
results = []
for backend in args.backends:
    env = dict(os.environ)
    for key in ("WGPU_BACKEND", "INCULAR_BENCH_PROFILE", "INCULAR_BENCH_SMOKE", "INCULAR_BENCH_READY", "INCULAR_BENCH_CAPTURE"):
        env.pop(key, None)
    if backend != "default":
        env["WGPU_BACKEND"] = backend
    ready = args.output.with_name(f"{args.output.stem}-{backend}.ready")
    if args.ready:
        ready.unlink(missing_ok=True)
        env["INCULAR_BENCH_READY"] = str(ready.resolve())
    with args.output.with_name(f"{args.output.stem}-{backend}.log").open("w") as log:
        process = subprocess.Popen([str(args.executable.resolve())], stdout=log, stderr=log,
                                   env=env, creationflags=subprocess.CREATE_NO_WINDOW)
        handle = None
        try:
            if args.ready:
                deadline = time.perf_counter() + 30
                while not ready.exists():
                    if process.poll() is not None or time.perf_counter() > deadline:
                        raise RuntimeError("Readiness failed")
                    time.sleep(.1)
            time.sleep(8)
            if process.poll() is not None:
                raise RuntimeError(f"{backend} exited with {process.returncode}")
            handle = measure.kernel.OpenProcess(0x410, False, process.pid)
            started = time.perf_counter()
            before = measure.sample(handle, started)
            time.sleep(5)
            after = measure.sample(handle, started)
            after["cpuPercent"] = 100 * (after["cpuSeconds"] - before["cpuSeconds"]) / (after["elapsedSeconds"] - before["elapsedSeconds"])
            after["backendOverride"] = backend
            after["threeFrameReadiness"] = args.ready
            results.append(after)
            print(json.dumps(after), flush=True)
            args.output.write_text(json.dumps(results, indent=2))
        finally:
            if handle:
                measure.kernel.CloseHandle(handle)
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=10)
