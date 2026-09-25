"""Exploratory driver isolation. These short samples are not strict idle benchmarks."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import statistics
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("measure", Path(__file__).with_name("run-windows.py"))
measure = importlib.util.module_from_spec(spec)
spec.loader.exec_module(measure)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--backends", nargs="+", default=["vulkan", "dx12", "gl"])
    parser.add_argument("--stages", nargs="+", default=["window", "instance", "surface", "adapter", "device", "submit", "configure", "present"])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    results = []
    for backend in args.backends:
        for stage in args.stages:
            name = f"{backend}-{stage}"
            ready = (args.output / (name + ".ready")).resolve()
            ready.unlink(missing_ok=True)
            env = dict(os.environ, WGPU_BACKEND=backend, INCULAR_GPU_PROBE_STAGE=stage, INCULAR_BENCH_READY=str(ready))
            env.pop("WGPU_ADAPTER_NAME", None)
            with (args.output / (name + ".log")).open("w") as log:
                launched = time.perf_counter()
                process = subprocess.Popen([str(args.executable.resolve())], env=env, stdout=log, stderr=log, creationflags=subprocess.CREATE_NO_WINDOW)
                handle = None
                try:
                    deadline = time.perf_counter() + 45
                    while not ready.exists():
                        if process.poll() is not None or time.perf_counter() > deadline:
                            raise RuntimeError(f"{name} failed readiness")
                        time.sleep(.1)
                    ready_seconds = time.perf_counter() - launched
                    handle = measure.kernel.OpenProcess(0x410, False, process.pid)
                    time.sleep(3)
                    started = time.perf_counter()
                    samples = [measure.sample(handle, started)]
                    for _ in range(5):
                        time.sleep(1)
                        current = measure.sample(handle, started)
                        previous = samples[-1]
                        current["cpuPercent"] = 100 * (current["cpuSeconds"] - previous["cpuSeconds"]) / (current["elapsedSeconds"] - previous["elapsedSeconds"])
                        samples.append(current)
                    row = {"backend": backend, "stage": stage, "readySeconds": ready_seconds,
                           "probeEnvironment": {key: value for key, value in env.items() if key.startswith("INCULAR_GPU_PROBE_")},
                           "samples": samples,
                           "medians": {key: statistics.median(s[key] for s in samples[1:]) for key in ("privateWorkingSetBytes", "workingSetBytes", "privateCommitBytes", "cpuPercent")}}
                    results.append(row)
                    print(name, row["medians"], flush=True)
                    (args.output / "stages.json").write_text(json.dumps(results, indent=2))
                finally:
                    if process.poll() is None:
                        process.terminate()
                        process.wait(timeout=10)
                    if handle:
                        measure.kernel.CloseHandle(handle)
                    ready.unlink(missing_ok=True)


if __name__ == "__main__":
    main()
