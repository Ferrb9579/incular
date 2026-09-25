# Latest desktop benchmark results

Windows AMD Radeon 610M, three idle-qualified launches each. Decimal MB; CPU is percent of one core. These are the retained release-build memory measurements, not a new memory measurement of the distribution build.

| Application | Private resident MB | Private committed MB | Idle CPU | Qualified launches |
| --- | ---: | ---: | ---: | ---: |
| Incular | 111.22 | 148.59 | 0.00% | 3/3 |
| Electron | 104.40 | 196.98 | 0.00% | 3/3 |

The latest Incular distribution bundle is **11.79 MB**, including the VC runtime DLL. Its memory has not been rebenchmarked. The retained Electron installed bundle is 386.14 MB.

Incular still exceeds Electron's measured private resident RAM. QuickGUI's macOS chart uses a different OS and memory metric, so it is not a same-machine comparison.

- [Hardware findings and methodology](AMD-HARDWARE.md)
- [Bundle reduction and dependency patches](BUNDLE-SIZE.md)
- [Latest raw results and validation](results/README.md)
- [Electron visual comparison](results/bundle-dist/visuals/compare-issue-tracker.png)
