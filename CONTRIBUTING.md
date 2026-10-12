# Contributing to ArqStudioCAD

**English** | [Español](CONTRIBUTING.es.md)

Patches, bug reports, drawings that misbehave and translations are welcome.
ArqStudioCAD is GPL-3.0-or-later; by contributing you agree your work ships
under that licence. Contributors are listed in [AUTHORS](AUTHORS), and the
project follows the [Contributor Covenant](CODE_OF_CONDUCT.md).

**Write issues and pull requests in English or Spanish, whichever you
prefer** — both are read. Only the code itself, its comments and the commit
messages have to be English, so that any contributor can read them.

You do not need any permission on this repository to contribute: fork it, push
your change to a branch of your fork, and open a pull request against `main`.

> The project is pre-alpha. Before a large change, open an issue first so we
> can agree on the approach — it saves you work.

## Getting it running

Follow [Building from source](README.md#building-from-source), then install
the local checks once:

```sh
cargo install cargo-deny --locked
uv run pre-commit install
```

From then on every commit runs formatting, linters, type checks and the
dependency licence check. The full list of commands is in [AGENTS.md](AGENTS.md).
**Testing is local**: CI on GitHub runs only on demand, so a pull request is
expected to arrive with the checks already green on your machine.

## House rules

The binding rules are in the [technical contract](docs/architecture/technical-contract.md);
these are the ones a first contribution usually meets:

- **Each layer owns its part.** Rust owns the document and all geometry;
  Python coordinates commands; PySide6 only presents. A command never edits
  document internals or widgets directly, and no algorithm is duplicated
  between Rust and Python.
- **Every change to the drawing goes through the core API**, inside a
  transaction, so undo and redo are exact and a failed operation leaves the
  drawing untouched.
- **Commands are headless actions**, never logic attached to a key or a mouse
  event. That makes them testable without a GUI and scriptable.
- **The DWG/DXF document is the model.** What ArqStudioCAD does not understand
  about a drawing (proxies, XDATA, dictionaries, 3D solids) must survive a
  round trip untouched. opencadcodec types never leave `arq-document`.
- **Only `crates/arq-python` may depend on Python**, and no crate depends on Qt.
- **Coordinates are `f64` model units and angles are radians**; NaN and
  infinity are rejected.
- **New behaviour comes with a test, at the right layer.** A test that passes
  with the bug still in place proves nothing — check that it fails when you
  revert your fix. A check that inspects nothing is a failure, not a pass.
- **Code, comments, docstrings and commit messages are in English.** The
  interface is translated (see below). `README.md` and `README.es.md` change
  together.
- **Copied or adapted code** must be under a licence compatible with
  GPL-3.0-or-later and is recorded in [THIRD_PARTY.md](THIRD_PARTY.md).
- **Architectural decisions** (a new dependency, a change of approach) come
  with an ADR in [docs/architecture/adr/](docs/architecture/adr/).
- **AI-assisted contributions are welcome.** You are responsible for every
  line: read it, run it, test it. [AGENTS.md](AGENTS.md) holds the rules for
  coding agents.

## Branches, commits and pull requests

- One topic per pull request; small is easier to review and to revert.
- Commit messages: an imperative summary line in English (`Add LINE command`),
  then a body that explains *why* when it is not obvious.
- In the pull request, say **how you verified** the change — the template asks
  for it. The most convincing evidence is a test that fails without the change,
  a named drawing before and after, or a measurement.
- Maintainers merge into `main` once the checks are green and the review is done.

## Reporting a drawing that opens, draws or saves wrong

This is the most valuable kind of report — and impossible to act on without
the file. Attach the DWG/DXF (zip it if GitHub refuses the extension). If it
is client work you cannot share, a copy reduced to the entities that misbehave
is worth more than the real one anyway.

## Translating ArqStudioCAD

**Everything you read is translatable; everything you type is English.**

An AutoCAD user types what their fingers already know — `L`, `TR`, `Z` — and it
must work in every interface language. Menus, dialogs, panels and the *text*
of prompts are translated; command names and the option keys inside prompts
are not. The full rules are in [ADR 0006](docs/architecture/adr/0006-i18n.md).

The one hard rule for translators: a prompt is parsed against its English
option keys, so a translation must keep each key visible —

```json
"Specify rotation angle or [Reference]:":
    "Especifique el ángulo de rotación o [Referencia(R)]:"
```

A language is a folder under `python/arqstudiocad/locales/<code>/` with a
`meta.json` (its code and its own name, e.g. `"name": "Español"`) and a
`ui.json` (`{"English source": "translation"}`); copy `es/` as a model. No code
change is needed: the language appears in *Tools > Language* by itself.
Partial translations are fine: a missing string falls back to English, and no
release is ever blocked on a translation. Keep `{placeholders}` such as
`{app}` or `{version}` exactly as they are — the tests check it. Localized
command names (`LINEA`, in a future `commands.json`) may be *added*, but never
replace or shadow an English name.

---

The contribution model is adapted from [IngeCAD](https://github.com/ingelibre/ingecad)
(GPL-3.0-or-later).
