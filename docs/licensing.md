# Licensing decision — pending maintainer choice

The repository already declares **Apache-2.0**. This work preserves that license
and copies its existing text into every framework archive. No new project license
or dependency-license exception has been selected on the maintainer's behalf.

There are two independent choices:

## 1. License for Incular's own code

| Option | Effect |
| --- | --- |
| Keep Apache-2.0 (recommended) | Preserve the current grant, including its explicit patent terms, and avoid a relicensing exercise. |
| Offer MIT OR Apache-2.0 | Let recipients choose either license, a familiar Rust arrangement. Requires a correct MIT notice, updates to all manifests/docs, and rights to offer all contributed/adapted code under both terms. |

Both allow commercial use subject to their terms. Changing the license for
Incular does not change licenses of dependencies or copied third-party material.
Keep copyright/attribution notices. The authoritative [Apache text](https://www.apache.org/licenses/LICENSE-2.0)
defines its patent and redistribution terms; compare the [MIT text](https://spdx.org/licenses/MIT.html)
before selecting the dual-license option. Contributor/provenance confirmation is
required for relicensing; source inspection cannot prove authorship rights.

## 2. Existing dependency licenses

`cargo deny` currently rejects these because the repository's allow-list omits
their licenses, not because the tool established a legal incompatibility:

| Dependency | Path | License |
| --- | --- | --- |
| clipboard-win 5.4.1 | arboard / Windows clipboard | BSL-1.0 |
| error-code 3.4.0 | clipboard-win | BSL-1.0 |
| ksni 0.3.6 | Linux tray implementation | Unlicense |

Recommended: accept these three specific dependencies after reviewing the texts,
with narrow exceptions rather than a global policy relaxation. BSL-1.0 here is
the **Boost Software License**, not the Business Source License. Its notice
requirements differ for source and machine-executable distributions. The
Unlicense is a public-domain dedication with fallback permission language.
See the [Boost license](https://spdx.org/licenses/BSL-1.0.html) and
[Unlicense](https://spdx.org/licenses/Unlicense.html).

Alternative: reject one or both licenses and replace/remove the associated
clipboard/tray dependencies, then retest the affected native behavior. That is
a functionality/dependency change rather than an automatic license conversion.

The proposed policy additions, **not applied**, are:

```toml
[[licenses.exceptions]]
name = "clipboard-win"
version = "=5.4.1"
allow = ["BSL-1.0"]

[[licenses.exceptions]]
name = "error-code"
version = "=3.4.0"
allow = ["BSL-1.0"]

[[licenses.exceptions]]
name = "ksni"
version = "=0.3.6"
allow = ["Unlicense"]
```

After the choice, apply the policy, rerun `cargo deny check`, synchronize license
files if changed, and repackage. Publication remains blocked until this decision.
