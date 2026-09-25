"""Measure the upstream Electron workload with every descendant process included."""
import argparse
import ctypes as c
from ctypes import wintypes as w
import hashlib
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
measure.user.GetWindowTextW.argtypes = [w.HWND, w.LPWSTR, c.c_int]
measure.kernel.TerminateProcess.argtypes = [w.HANDLE, w.UINT]


def ready(pid, title):
    found = []
    callback_type = c.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)

    @callback_type
    def callback(hwnd, _):
        owner = w.DWORD()
        measure.user.GetWindowThreadProcessId(hwnd, c.byref(owner))
        if owner.value == pid:
            text = c.create_unicode_buffer(1024)
            measure.user.GetWindowTextW(hwnd, text, len(text))
            if text.value == title:
                found.append(True)
        return True

    measure.user.EnumWindows(callback, 0)
    return bool(found)


def tree(pid):
    result = {pid}
    pending = [pid]
    while pending:
        for item in measure.children(pending.pop()):
            if item["pid"] not in result:
                result.add(item["pid"])
                pending.append(item["pid"])
    return result


def launch(executable, output, run, title):
    trace, handles, cpu_totals = [], {}, {}
    env = dict(os.environ)
    for key in ("ELECTRON_RUN_AS_NODE", "ELECTRON_CAPTURE"):
        env.pop(key, None)
    with (output / f"run-{run}.log").open("w") as log:
        process = subprocess.Popen([str(executable)], stdout=log, stderr=log, env=env,
                                   creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            deadline = time.perf_counter() + 45
            while not ready(process.pid, title):
                if process.poll() is not None or time.perf_counter() > deadline:
                    raise RuntimeError("Electron did not signal valid workload and viewport readiness")
                time.sleep(.1)
            bounds = measure.visible(process.pid)
            started = time.perf_counter()
            idle_at, samples = None, []
            while time.perf_counter() - started < 180:
                if process.poll() is not None or not measure.visible(process.pid):
                    raise RuntimeError("Electron exited or its window became hidden/minimized")
                pids = tree(process.pid)
                current = {"elapsedSeconds": time.perf_counter() - started,
                           "privateWorkingSetBytes": 0, "workingSetBytes": 0,
                           "privateCommitBytes": 0, "processIds": sorted(pids)}
                for pid in sorted(pids):
                    if pid not in handles:
                        handles[pid] = measure.kernel.OpenProcess(0x411, False, pid)
                        if not handles[pid]:
                            raise c.WinError(c.get_last_error())
                    value = measure.sample(handles[pid], started)
                    for key in ("privateWorkingSetBytes", "workingSetBytes", "privateCommitBytes"):
                        current[key] += value[key]
                    cpu_totals[pid] = value["cpuSeconds"]
                # Include final CPU time of exited descendants; retain handles
                # so a PID cannot silently be reused during the measurement.
                for pid in handles.keys() - pids:
                    times = [w.FILETIME() for _ in range(4)]
                    if measure.kernel.GetProcessTimes(handles[pid], *(c.byref(t) for t in times)):
                        cpu_totals[pid] = sum((t.dwHighDateTime << 32) | t.dwLowDateTime for t in times[2:]) / 10_000_000
                current["cpuSeconds"] = sum(cpu_totals.values())
                current["cpuPercent"] = None
                if trace:
                    previous = trace[-1]
                    current["cpuPercent"] = 100 * (current["cpuSeconds"] - previous["cpuSeconds"]) / (current["elapsedSeconds"] - previous["elapsedSeconds"])
                trace.append(current)
                window = [s for s in trace if s["elapsedSeconds"] >= current["elapsedSeconds"] - 16.5]
                stable = measure.stable(window, 15) and all(s["processIds"] == current["processIds"] for s in window)
                if idle_at is not None:
                    if not stable:
                        idle_at, samples = None, []
                    else:
                        samples.append(current)
                        if len(samples) == 10:
                            metrics = ("privateWorkingSetBytes", "workingSetBytes", "privateCommitBytes", "cpuPercent")
                            return {"run": run, "idleQualified": True, "samples": samples,
                                    "idleDetectedAtSeconds": idle_at,
                                    "visibleClientRectsDpiVirtualized": bounds,
                                    "medians": {key: statistics.median(s[key] for s in samples) for key in metrics}}
                elif current["elapsedSeconds"] >= 30 and stable:
                    idle_at = current["elapsedSeconds"]
                    print(f"Electron launch {run}: idle at {idle_at:.1f}s, {len(pids)} processes", flush=True)
                time.sleep(1)
            raise RuntimeError("Electron failed strict idle criteria after 180 seconds")
        finally:
            (output / f"trace-{run}.json").write_text(json.dumps(trace, indent=2))
            # Only terminate this launch and descendants for which we hold
            # identity-preserving process handles; never target by image name.
            if process.poll() is None:
                for pid in tree(process.pid):
                    if pid not in handles:
                        handle = measure.kernel.OpenProcess(0x411, False, pid)
                        if handle:
                            handles[pid] = handle
                process.terminate()
                process.wait(timeout=10)
            for handle in handles.values():
                measure.kernel.TerminateProcess(handle, 0)
                measure.kernel.CloseHandle(handle)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "benchmarks/desktop/results/windows/electron")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    exe = args.executable.resolve()
    reference = json.loads((ROOT / "benchmarks/desktop/reference/quickgui-macos.json").read_text(encoding="utf-8"))
    result = {"framework": "Electron", "version": "44.2.0",
              "measuredAt": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
              "workload": reference["workload"], "idlePolicy": reference["methodology"]["idlePolicy"],
              "metric": "Windows private working set, summed over browser and all descendants",
              "notes": "CPU is aggregate percent of one core. Summed total working sets double-count shared pages; private resident does not. GPU/OS-compositor memory outside the process tree is excluded.",
              "executableSha256": hashlib.sha256(exe.read_bytes()).hexdigest(),
              "bundleBytes": sum(p.stat().st_size for p in exe.parent.rglob('*') if p.is_file()),
              "runs": []}
    try:
        for run in range(1, 4):
            value = launch(exe, args.output, run, reference["workload"]["readyTitle"])
            result["runs"].append(value)
            print(value["medians"], flush=True)
        result["summary"] = {key: {"median": statistics.median(run["medians"][key] for run in result["runs"]),
                                  "min": min(run["medians"][key] for run in result["runs"]),
                                  "max": max(run["medians"][key] for run in result["runs"])}
                             for key in result["runs"][0]["medians"]}
    except Exception as error:
        result["failure"] = str(error)
        raise
    finally:
        (args.output / "electron-windows.json").write_text(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
