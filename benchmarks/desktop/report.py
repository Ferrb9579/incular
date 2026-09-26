"""Generate the current comparison from retained benchmark evidence."""
import json
from pathlib import Path

root = Path(__file__).resolve().parent

def read(relative):
    return json.loads((root / relative).read_text(encoding="utf-8-sig"))

native = read("results/memory-disk/after.json")
electron = read("results/windows/electron-ui-electron-final/electron-windows.json")
bundle = read("results/bundle-dist/bundle-manifest.json")
lines = [
    "# Latest desktop benchmark results", "",
    "Windows AMD Radeon 610M, three idle-qualified launches each. Decimal MB; CPU is percent of one core. Incular uses the latest distribution build; Electron is the retained previous measurement on this host and was not rerun in this experiment.", "",
    "| Application | Private resident MB | Private committed MB | Idle CPU | Qualified launches |",
    "| --- | ---: | ---: | ---: | ---: |",
]
for name, result in [("Incular", native), ("Electron", electron)]:
    summary = result["summary"]
    qualified = sum(run["idleQualified"] for run in result["runs"])
    lines.append(f"| {name} | {summary['privateWorkingSetBytes']['median'] / 1e6:.2f} | {summary['privateCommitBytes']['median'] / 1e6:.2f} | {summary['cpuPercent']['median']:.2f}% | {qualified}/{len(result['runs'])} |")
lines += [
    "",
    f"The latest Incular distribution bundle is **{sum(item['bytes'] for item in bundle) / 1e6:.2f} MB**, including the VC runtime DLL. The memory results above use this executable. The retained Electron installed bundle is {electron['bundleBytes'] / 1e6:.2f} MB.",
    "",
    "Incular still exceeds Electron's measured private resident RAM. QuickGUI's macOS chart uses a different OS and memory metric, so it is not a same-machine comparison.",
    "",
    "- [Hardware findings and methodology](AMD-HARDWARE.md)",
    "- [Paired memory and disk reductions](MEMORY-DISK.md)",
    "- [Bundle reduction and dependency patches](BUNDLE-SIZE.md)",
    "- [Latest raw results and validation](results/README.md)",
    "- [Electron visual comparison](results/bundle-dist/visuals/compare-issue-tracker.png)",
]
(root / "RESULTS.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
print(root / "RESULTS.md")
