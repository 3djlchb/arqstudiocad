<!--
Thank you. Nothing here is bureaucracy: each line exists because it saves a
round trip. Delete what does not apply, and write in English or Spanish.
Gracias. Borra lo que no aplique; puedes escribir en inglés o en español.
-->

## What this changes, and why

<!-- The problem first, the fix second. If it closes an issue: "Closes #12". -->

## How it was verified

<!--
Not "tests pass" -- what you actually checked. Usually one of:

  - a new test that fails without the change (the surest kind);
  - a real drawing, named, before and after;
  - a measurement: entity counts, pixels, milliseconds.

If it touches rendering or packaging: on which OS, GPU and graphics backend.
-->

## Checklist

- [ ] The local checks are green (`pre-commit run --all-files`, or the commands in AGENTS.md)
- [ ] New behaviour comes with a test, and I checked it fails without the change
- [ ] It respects the layer rules of the technical contract (Rust owns the document; Python coordinates; PySide6 presents)
- [ ] Code, comments and commit messages are in English
- [ ] If it touches the interface: user-facing strings go through `tr()`
- [ ] If it touches a translated prompt: the English key stays visible, `[Referencia(R)]`
- [ ] If it changes `README.md` or `CONTRIBUTING.md`: the Spanish version is updated too
- [ ] If it copies or adapts third-party code: licence checked and recorded in `THIRD_PARTY.md`
- [ ] If it is an architectural decision: an ADR is included
