# 0022. Text entities in milestone 02.8

Date: 2026-10-18 · Status: Accepted · Builds on [0016](0016-editing-workflow.md),
[0017](0017-dwg-dxf-engine-strategy.md), [0018](0018-native-format.md)

## Context

Single-line (TEXT) and multiline (MTEXT) texts are read and saved without
loss, but they are read-only: they cannot be moved, copied, rotated, scaled
or mirrored ("These objects cannot be transformed yet: TEXT"), and they are
drawn as their outline box. The maintainer's drawings hold many of them.
The roadmap puts "texts" in milestone 02.8 and "texts and fonts" in phase 04
without saying which part belongs where.

## Decision

Each choice marked *(maintainer)* was made by the maintainer on 2026-10-18.

### D1 — Scope in 02.8: edit and transform *(maintainer)*

For TEXT and MTEXT drawn in plan (normal `(0, 0, ±1)`):

- **Transform as a whole**: MOVE, COPY, ROTATE, SCALE, MIRROR and the grip
  modes, through the engine's own transformation of the entity (as blocks
  and polylines, ADR 0016 D1). Texts on a tilted plane stay read-only.
- **One grip** at the text's anchor (D3); stretching it moves the text.
- **Properties**: content, height and rotation (degrees) are editable;
  style, justification, width factor and oblique angle are shown, not
  edited. Each edit is one undo step.
- **TEXT command** (alias `DT`, Spanish `TEXTO`): start point, height (the
  drawing's TEXTSIZE, then the last height used), rotation (0), then one
  line per Enter, each below the previous one, until an empty line, as
  AutoCAD's single-line text. New texts use the current text style
  (TEXTSTYLE) and layer, left-justified on the baseline.

Texts are drawn with their letters in one built-in single-stroke font
(amended 2026-10-18, *(maintainer)*): LibreCAD's ISO 3098 `standard.lff`
(GPL-2.0-or-later, 10 KB, Latin-1 letters and the usual CAD symbols),
laid out in Rust from each text's height, rotation, justification, width
factor, oblique angle and OCS; MTEXT shows its paragraphs as plain lines;
block attributes are drawn too. The text's box (picking, selection, zoom
and the spatial index) is the box of its letters. The drawing's own fonts
(SHX, TrueType), MTEXT formatting and word wrapping, and an in-place text
editor are phase 04; until then a text may look wider or narrower than in
the CAD that wrote it.

### D2 — Mirrored texts stay readable

As AutoCAD with its default `MIRRTEXT = 0`: a mirrored text keeps reading
left to right and upright. Its anchor is mirrored; of the two upright frames
that fit the mirrored text, the one whose up direction is closest to the
original's is kept; when that frame reverses the reading direction, the
horizontal justification is mirrored too (left ↔ right; MTEXT: the
attachment column), so the text stays on the mirrored side without knowing
its width.

### D3 — The anchor

A TEXT's anchor is its insertion point when it is left-justified on the
baseline, and its alignment point otherwise; an MTEXT's anchor is its
insertion point. Aligned and Fit TEXTs, whose height and rotation follow
from their two points, transform as a whole but their height and rotation
are shown, not edited.

### D4 — MTEXT content

An MTEXT's content is editable only when it holds no formatting codes
(`{\f…}`, `\H…`, `\C…`…); otherwise it is shown as stored and not edited,
so no formatting is lost before the MTEXT editor of phase 04. Paragraph
breaks (`\P`) and the escaped characters `\\`, `\{` and `\}` are not
formatting (amended 2026-10-19, D6): such content is read and written as
plain lines of text.

### D6 — Basic multiline text in 02.8 *(maintainer, 2026-10-19)*

As other CADs, both single-line (TEXT) and multiline (MTEXT) texts are
created in 02.8; the formatted MTEXT stays in phase 04.

- **MTEXT command** (aliases `T` and `MT`, Spanish `TEXTOM`): first corner,
  then the opposite corner, with the options Height, Justify (the nine
  attachment points, `TL` to `BR`) and Rotation; then the text, in a window
  of several lines (or line by line on the command line without an
  interface, until an empty line). The rectangle's width is the MTEXT's
  width; its anchor is the rectangle's point that the justification names.
  It uses the current text style and layer and is one undo step.
- **Word wrapping**: paragraphs wrap at word boundaries to the MTEXT's width
  (a word longer than the width stays whole on its line).
- **Editing**: Properties shows an MTEXT's paragraphs and edits them in the
  same window (double click on Contents); its width is shown.
- **Phase 04** keeps bold, italic, fonts, colors and sizes within the text,
  columns, the in-place editor and the drawing's own fonts.

### D5 — Ownership

Texts stay in the engine's database and are edited there through the
adapter, as blocks and polylines (ADR 0018 D2: the own model takes them in
stage 3). The Python API gains a read-only text description of an entity and
operations to create texts and change their content, height and rotation;
they are all-or-nothing like every core operation.

## Consequences

- The maintainer's texts can be read, moved and edited with the rest of
  the drawing; phase 04 keeps the drawing's own fonts and rich MTEXT.
- A font file enters the core as data: it adds no library, follows the
  rule for new technologies (master plan §16: replaceable, compatible
  license, no API change) and is recorded in `THIRD_PARTY.md`.
- MIRROR changes a text's justification when it must reverse it; the
  mirrored drawing looks as AutoCAD's with `MIRRTEXT = 0`.
- MTEXT with formatting stays read-only in content until phase 04;
  multiline text made in ArqStudioCAD, which holds only paragraphs, is
  edited freely.
