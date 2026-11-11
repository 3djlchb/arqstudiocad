# Security policy

ArqStudioCAD is a desktop CAD application. It has no server, no accounts and
no network features of its own. Its main exposure is the files it opens: a
DWG or DXF drawing from someone else is untrusted input. The decisions behind
this policy are in [ADR 0037](docs/architecture/adr/0037-security-baseline.md).

## Supported versions

There is no release yet. Security fixes go to `main`; once releases exist,
the latest one will be supported.

## Reporting a vulnerability

Please report it privately, not in a public issue:

- through GitHub's [private vulnerability reporting](https://github.com/3djlchb/arqstudiocad/security/advisories/new), or
- by e-mail to **arqstudiocad29@gmail.com**, with "Security" in the subject.

Include what you did, what happened and, if you can, a file that shows it.
Zip a drawing if GitHub refuses its extension. Expect an acknowledgement
within a week. We will tell you what we found, credit you in the advisory if
you wish, and publish the advisory once a fix is on `main`.

**What counts as a vulnerability:** anything a file or another input can do
beyond what the user asked for:

- memory corruption or code execution;
- reading or writing files the user did not choose;
- running a program;
- a crash, hang or memory blowup that could be used against other users.

A drawing that only opens, draws or saves wrong is an ordinary bug: report
it as [CONTRIBUTING.md](CONTRIBUTING.md) explains.

## How the project keeps it secure

- **Untrusted files.** The readers are in Rust, with `unsafe` kept to the
  graphics and Python boundaries. The DWG/DXF readers are fuzzed weekly in
  CI ([fuzz/](fuzz/README.md), ADR 0037 D2), and hand-made hostile drawings
  open in every `cargo test`. Every input that breaks them stays as a
  regression test.
- **Dependencies.**
  - `cargo deny` checks every Rust dependency's license and advisories on
    every pull request.
  - `pip-audit` checks every locked Python package.
  - Lock files pin exact versions.
- **CI.**
  - Workflows run with a read-only token.
  - Every action is pinned to a commit.
  - Checkouts keep no credentials.
  - Dependabot proposes updates to the pinned actions monthly.
- **Graphics drivers.** They are probed in a child process, so a driver
  that crashes does not take the application down (ADR 0007).
- **Signed installers.** Planned before the first release (milestone 04.6):
  SignPath Foundation for Windows, Apple's notarization for macOS.

## Principles for what is not built yet

Plugins, automation (scripts) and external references will open or run
things the user did not write. Each will have an ADR of its own when it is
designed (ADR 0014), and each must keep to these principles:

1. **A file is data, never code.** Opening a drawing never runs anything
   inside it. Embedded scripts or macros are kept, not executed.
2. **Code runs only with the user's consent.** That means a plugin the user
   installed or a script the user started, never one that came inside a
   file.
3. **Least access.** Each runs with only the access it needs: files the user
   chose, no network unless stated.
4. **Limits.** Reading, loading references and running scripts have bounded
   time and memory, so a hostile input cannot hang the application.
5. **References stay where the user put them.** An external reference path
   in a drawing is never followed outside the folders the user allows
   without asking.
