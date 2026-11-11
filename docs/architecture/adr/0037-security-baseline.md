# 0037. Security baseline: policy, least-privilege CI, audited dependencies, fuzzed readers

Date: 2026-11-11 · Status: Proposed

## Context

On 2026-11-11 the maintainer asked for every change needed to keep the
application secure. The repository is public and shared by every
development machine. What stood then (checked 2026-11-11):

- **Exposure.** A desktop application with no server, accounts or network
  features. Its exposure is the files it opens: a DWG/DXF from someone else
  is untrusted input to the readers (AcadRust, through `arq-document`).
- **What was in place.** The core is Rust, with `unsafe` only in the GPU
  viewport and the PyO3 bindings. `cargo deny` checked Rust licenses and
  advisories in CI. GPU probes run in child processes (ADR 0007). No code
  evaluates text from outside (`eval`, `exec`, `shell=True`).
- **What was missing:**
  - **Reporting:** no `SECURITY.md`, so no private way to report.
  - **CI:** the workflows declared no `permissions`, so their token could
    write. Actions were referenced by moving tags, and checkouts kept
    their credentials in `.git`.
  - **Python:** no check of Python packages against advisories. A first
    run of `pip-audit` found 25 advisories in two development-only
    packages, `filelock` 4.1.1 and `virtualenv` 21.14.6 (through
    `pre-commit`).
  - **Readers:** no fuzzing of the DWG/DXF readers. `proptest` covered
    only geometry.
  - **Installers:** unsigned (`packaging/macos/build.sh` says so).

## Decision

The maintainer chose the recommended option on each point (2026-11-11).

**D1 — Policy.** A `SECURITY.md` at the root:
- private reports through GitHub's private vulnerability reporting or the
  project's address;
- what counts as a vulnerability, a hostile file's crash, hang or memory
  blowup included;
- how the project keeps secure;
- principles for plugins, automation and external references (a file is
  data, never code; code runs only with the user's consent; least access;
  limits; references stay where the user put them).

The detailed design of each of those comes in its own ADR when it is
designed, not before (ADR 0014).

**D2 — Fuzzing of the readers.** `cargo-fuzz` (libFuzzer, MIT/Apache-2.0) in
a `fuzz/` crate outside the workspace, the only place that uses Rust
nightly. The application keeps building on the pinned stable toolchain. CI
runs the fuzzers weekly and by hand. Every input that crashes, hangs or
blows up memory is fixed and kept as a regression test in the stable
`cargo test`. This is the next pull request after this one.

**D3 — Dependencies and CI.**
- **Python audit:** `pip-audit` (Apache-2.0, PyPA) runs in CI through
  `uvx` at a fixed version, not as a dependency of the project. It checks
  every locked Python package, development tools included, from a hashed
  export of `uv.lock`. A finding fails the pull request until the package
  is upgraded, or the advisory is shown not to apply and ignored by its ID
  with the reason written next to it.
- **Workflows:**
  - `permissions: contents: read`;
  - every action pinned to a commit SHA, its release in a comment;
  - `persist-credentials: false` on every checkout.
- **Dependabot** proposes one grouped pull request a month for the pinned
  actions, reviewed like any other. Rust and Python packages stay with
  `cargo deny`, `pip-audit` and upgrades by hand.
- **Repository settings,** set by the maintainer on GitHub: private
  vulnerability reporting and Dependabot alerts on.

**D4 — Signed installers** before the first release (milestone 04.6):
SignPath Foundation, free for open-source projects, for Windows; an Apple
Developer account and notarization for macOS. Until then the installers
stay unsigned, as packaging says.

## Consequences

- **Reporting:** a private channel exists, and a hostile file is treated
  as a security matter.
- **CI tokens:** a compromised action or step can no longer write to the
  repository through the token, and a moved tag cannot change what runs.
  Pinned actions need Dependabot's monthly pull request to stay current.
- **Python advisories** now fail CI. This pull request upgrades `filelock`
  to 4.2.10 and `virtualenv` to 21.15.6, after which none remain.
- **New tools** (master plan §16): `pip-audit` and `cargo-fuzz` are
  development tools only. Neither ships with the application, both are
  maintained by their ecosystems' own groups, and each can be replaced
  without touching the core.
- **Fuzzing (D2)** adds nightly Rust in one crate and one workflow only.
  Its findings become stable regression tests.
- **Release criteria:** the first release gains signed installers among
  its criteria (milestone 04.6).

## Note (2026-11-11): D2 done

The fuzzers are in [`fuzz/`](../../../fuzz/README.md):
- `read_dwg` and `read_dxf` open their input as the application does
  (`Document::read`, then index, measure, draw);
- they run on nightly-2026-10-27;
- `.github/workflows/fuzz.yml` runs them weekly, by hand, and for a minute
  on pull requests touching the readers.

**First finding.** Within seconds the DWG fuzzer found a drawing whose
layer table lacks "0". Building the model then panicked, outside the
reader's guard. Every read now adds a missing layer "0", as AUDIT would,
and the drawing opens recovered. After the fix, about 11,400 more DWG
inputs and 10,500 DXF inputs found nothing.

**Hostile drawings.** Seventeen drawings made by hand
(`tools/make_hostile_drawings.py`) open in every `cargo test`
(`crates/arq-core/tests/hostile_files.rs`), and are kept byte for byte
from git and the hooks.

**Licenses.** A finding derived from a third-party sample is not committed;
a test that builds the same damage stands for it. `deny.toml` allows NCSA
(LLVM's libFuzzer) for `libfuzzer-sys` alone. It is a fuzzing tool, never
shipped.
