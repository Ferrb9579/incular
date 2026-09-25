"""Measure the real native release app; standard library only, Windows x64.

Windows private working set is NOT macOS physical footprint. Record private
commit and total working set too; do not calculate cross-platform speedups.
"""
import argparse
import ctypes as c
from ctypes import wintypes as w
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
SIZE_T = c.c_size_t
kernel = c.WinDLL("kernel32", use_last_error=True)
psapi = c.WinDLL("psapi", use_last_error=True)
user = c.WinDLL("user32", use_last_error=True)


class Memory(c.Structure):
    _fields_ = [("cb", w.DWORD), ("faults", w.DWORD)] + [
        (name, SIZE_T) for name in ("peakWs", "ws", "peakPaged", "paged",
                                  "peakNonPaged", "nonPaged", "pagefile", "peakPagefile", "private")]


class ProcessEntry(c.Structure):
    _fields_ = [("size", w.DWORD), ("usage", w.DWORD), ("pid", w.DWORD),
                ("heap", SIZE_T), ("module", w.DWORD), ("threads", w.DWORD),
                ("parent", w.DWORD), ("priority", w.LONG), ("flags", w.DWORD),
                ("exe", w.WCHAR * 260)]


kernel.OpenProcess.argtypes = [w.DWORD, w.BOOL, w.DWORD]
kernel.OpenProcess.restype = w.HANDLE
kernel.CloseHandle.argtypes = [w.HANDLE]
kernel.GetProcessTimes.argtypes = [w.HANDLE] + [c.POINTER(w.FILETIME)] * 4
kernel.CreateToolhelp32Snapshot.argtypes = [w.DWORD, w.DWORD]
kernel.CreateToolhelp32Snapshot.restype = w.HANDLE
kernel.Process32FirstW.argtypes = [w.HANDLE, c.POINTER(ProcessEntry)]
kernel.Process32NextW.argtypes = [w.HANDLE, c.POINTER(ProcessEntry)]
psapi.GetProcessMemoryInfo.argtypes = [w.HANDLE, c.POINTER(Memory), w.DWORD]
psapi.QueryWorkingSet.argtypes = [w.HANDLE, c.c_void_p, w.DWORD]
user.IsWindowVisible.argtypes = [w.HWND]
user.IsIconic.argtypes = [w.HWND]
user.GetWindowThreadProcessId.argtypes = [w.HWND, c.POINTER(w.DWORD)]
user.GetClientRect.argtypes = [w.HWND, c.POINTER(w.RECT)]


def children(pid):
    snapshot = kernel.CreateToolhelp32Snapshot(2, 0)
    if snapshot == c.c_void_p(-1).value:
        raise c.WinError(c.get_last_error())
    entry = ProcessEntry()
    entry.size = c.sizeof(entry)
    found = []
    try:
        more = kernel.Process32FirstW(snapshot, c.byref(entry))
        while more:
            if entry.parent == pid:
                found.append({"pid": entry.pid, "name": entry.exe})
            more = kernel.Process32NextW(snapshot, c.byref(entry))
    finally:
        kernel.CloseHandle(snapshot)
    return found


def visible(pid):
    found = []
    callback_type = c.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)

    @callback_type
    def callback(hwnd, _):
        owner = w.DWORD()
        user.GetWindowThreadProcessId(hwnd, c.byref(owner))
        if owner.value == pid and user.IsWindowVisible(hwnd) and not user.IsIconic(hwnd):
            rect = w.RECT()
            user.GetClientRect(hwnd, c.byref(rect))
            if rect.right > 0 and rect.bottom > 0:
                found.append([rect.right, rect.bottom])
        return True

    user.EnumWindows(callback, 0)
    return found


def sample(handle, started):
    memory = Memory()
    memory.cb = c.sizeof(memory)
    if not psapi.GetProcessMemoryInfo(handle, c.byref(memory), memory.cb):
        raise c.WinError(c.get_last_error())
    # PSAPI_WORKING_SET_BLOCK.Shared is bit 8; entries are pointer-sized.
    buffer = (SIZE_T * (memory.ws // 4096 + 32768))()
    if not psapi.QueryWorkingSet(handle, buffer, c.sizeof(buffer)):
        raise c.WinError(c.get_last_error())
    private_ws = sum(not (buffer[i] & (1 << 8)) for i in range(1, buffer[0] + 1)) * 4096
    times = [w.FILETIME() for _ in range(4)]
    if not kernel.GetProcessTimes(handle, *(c.byref(t) for t in times)):
        raise c.WinError(c.get_last_error())
    cpu = sum((t.dwHighDateTime << 32) | t.dwLowDateTime for t in times[2:]) / 10_000_000
    return {"elapsedSeconds": time.perf_counter() - started,
            "privateWorkingSetBytes": private_ws, "workingSetBytes": memory.ws,
            "privateCommitBytes": memory.private, "cpuSeconds": cpu, "cpuPercent": None}


def stable(samples, seconds):
    if len(samples) < 2 or samples[-1]["elapsedSeconds"] - samples[0]["elapsedSeconds"] < seconds:
        return False
    if any(s["cpuPercent"] is None or s["cpuPercent"] > 1 for s in samples):
        return False
    for metric in ("privateWorkingSetBytes", "privateCommitBytes", "workingSetBytes"):
        values = [s[metric] for s in samples]
        if max(values) - min(values) > max(1_000_000, statistics.median(values) * .01):
            return False
    return True


def launch(executable, output, run, diagnostic=False, backend="default", adapter=None, after_smoke=False):
    ready = output / f"ready-{run}.txt"
    ready.unlink(missing_ok=True)
    env = dict(os.environ, INCULAR_BENCH_READY=str(ready.resolve()))
    for key in ("INCULAR_BENCH_CAPTURE", "INCULAR_BENCH_SMOKE", "INCULAR_BENCH_PROFILE", "WGPU_BACKEND", "WGPU_ADAPTER_NAME"):
        env.pop(key, None)
    if backend != "default":
        env["WGPU_BACKEND"] = backend
    if adapter:
        env["WGPU_ADAPTER_NAME"] = adapter
    if after_smoke:
        env["INCULAR_BENCH_SMOKE"] = "1"
    trace = []
    with (output / f"run-{run}.log").open("w") as log:
        launched = time.perf_counter()
        process = subprocess.Popen([str(executable)], cwd=ROOT, env=env,
                                   stdout=log, stderr=log, creationflags=subprocess.CREATE_NO_WINDOW)
        handle = None
        try:
            deadline = time.perf_counter() + 30
            while not ready.exists():
                if process.poll() is not None:
                    raise RuntimeError(f"App exited: see run-{run}.log")
                if time.perf_counter() > deadline:
                    raise RuntimeError("Three presented frames not observed in 30 seconds")
                time.sleep(.1)
            ready_seconds = time.perf_counter() - launched
            bounds = visible(process.pid)
            if not bounds:
                raise RuntimeError("No visible, non-minimized app window")
            handle = kernel.OpenProcess(0x410, False, process.pid)
            if not handle:
                raise c.WinError(c.get_last_error())
            started = time.perf_counter()
            idle_at = None
            samples = []
            while time.perf_counter() - started < 180:
                if process.poll() is not None or not visible(process.pid):
                    raise RuntimeError("App exited, hidden or minimized during measurement")
                helpers = children(process.pid)
                if helpers:
                    raise RuntimeError(f"Unexpected helpers; extend process aggregation before measuring: {helpers}")
                current = sample(handle, started)
                if trace:
                    previous = trace[-1]
                    current["cpuPercent"] = 100 * (current["cpuSeconds"] - previous["cpuSeconds"]) / (current["elapsedSeconds"] - previous["elapsedSeconds"])
                trace.append(current)
                window = [s for s in trace if s["elapsedSeconds"] >= current["elapsedSeconds"] - 16.5]
                if idle_at is not None:
                    if not diagnostic and not stable(window, 15):
                        idle_at = None
                        samples = []
                    else:
                        samples.append(current)
                        if len(samples) == 10:
                            metrics = ("privateWorkingSetBytes", "workingSetBytes", "privateCommitBytes", "cpuPercent")
                            return {"run": run, "pid": process.pid, "visibleClientRectsDpiVirtualized": bounds,
                                    "idleQualified": not diagnostic,
                                    "readySeconds": ready_seconds,
                                    "idleDetectedAtSeconds": idle_at, "samples": samples,
                                    "medians": {m: statistics.median(s[m] for s in samples) for m in metrics},
                                    "startupSamples": trace[:-10]}
                elif current["elapsedSeconds"] >= 30 and (diagnostic or stable(window, 15)):
                    idle_at = current["elapsedSeconds"]
                    print(f"Launch {run}: {'diagnostic sampling' if diagnostic else 'idle'} at {idle_at:.1f}s", flush=True)
                time.sleep(1)
            raise RuntimeError("No stable idle measurement within 180 seconds")
        finally:
            (output / f"trace-{run}.json").write_text(json.dumps(trace, indent=2))
            if handle:
                kernel.CloseHandle(handle)
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=10)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--executable", type=Path, default=ROOT / "target/release/examples/issue_tracker.exe")
    parser.add_argument("--output", type=Path, default=ROOT / "benchmarks/desktop/results/windows")
    parser.add_argument("--diagnostic", action="store_true", help="Record post-warmup activity; NEVER qualifies as an idle result")
    parser.add_argument("--backend", choices=["default", "vulkan", "dx12", "gl"], default="default",
                        help="Explicit WGPU backend; default clears inherited WGPU_BACKEND")
    parser.add_argument("--adapter", help="Explicit WGPU_ADAPTER_NAME match, recorded in results")
    parser.add_argument("--after-smoke", action="store_true", help="Validate idle after interaction smoke; not the initial workload comparison")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    executable = args.executable.resolve()
    reference = json.loads((ROOT / "benchmarks/desktop/reference/quickgui-macos.json").read_text(encoding="utf-8"))
    dataset = ROOT / "examples/issue_tracker/issues.json"
    assert hashlib.sha256(dataset.read_bytes()).hexdigest() == reference["workload"]["datasetSha256"]
    result = {"measuredAt": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
              "platform": platform.platform(), "architecture": platform.machine(),
              "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "workingTreeDirty": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT)),
              "rustc": subprocess.check_output(["rustc", "-Vv"], text=True),
              "workload": reference["workload"], "metric": "Windows private working set",
              "mode": "diagnostic (not idle)" if args.diagnostic else "strict idle",
              "backendOverride": args.backend,
              "adapterOverride": args.adapter,
              "afterInteractionSmoke": args.after_smoke,
              "executableBytes": executable.stat().st_size,
              "executableSha256": hashlib.sha256(executable.read_bytes()).hexdigest(),
              "idlePolicy": reference["methodology"]["idlePolicy"],
              "notes": "CPU is percent of one logical core. All three memory metrics must remain stable. No child processes allowed. No GC/cache purge. GPU/OS compositor memory excluded. Not equivalent to macOS physical footprint.",
              "runs": []}
    for run in range(1, 4):
        try:
            measured = launch(executable, args.output, run, args.diagnostic, args.backend, args.adapter, args.after_smoke)
        except Exception as error:
            result["failure"] = str(error)
            (args.output / "incular-windows.json").write_text(json.dumps(result, indent=2))
            raise
        result["runs"].append(measured)
        (args.output / "incular-windows.json").write_text(json.dumps(result, indent=2))
        print(f"Launch {run}: {measured['medians']}", flush=True)
    result["summary"] = {metric: {
        "median": statistics.median(run["medians"][metric] for run in result["runs"]),
        "min": min(run["medians"][metric] for run in result["runs"]),
        "max": max(run["medians"][metric] for run in result["runs"])}
        for metric in result["runs"][0]["medians"]}
    (args.output / "incular-windows.json").write_text(json.dumps(result, indent=2))
    print(json.dumps(result["summary"], indent=2))


if __name__ == "__main__":
    main()
