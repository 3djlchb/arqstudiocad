# 0039. Number format: a decimal point, as every CAD program

Date: 2026-11-15 · Status: Accepted (maintainer, 2026-11-15)

## Context

The properties panel wrote and read its numbers with the system's locale
(`QLocale()`), and the number boxes of the block dialogs did too. On a
desktop in Spanish (Ecuador), whose locale in Qt writes a decimal comma,
the panel showed a line's length as `2,82842712475` and read `2,5` as two
and a half. Three tests of the panel failed on such a machine, and passed
on CI, in English.

A comma is already taken. On the command line it separates a point's
coordinates (`10,5` is X 10, Y 5), as in AutoCAD. A number the panel shows
should be one the command line reads the same way. A drawing's numbers
should also read the same on every machine that opens it.

Money already has its own rule. Each drawing writes amounts with its own
thousands and decimal separators, by default a comma for thousands and a
point for decimals, as in Ecuador ([ADR 0028](0028-budget-table.md),
maintainer, 2026-10-27).

## Decision

*(maintainer, 2026-11-15: "the format AutoCAD and ProgeCAD have, standard
for all CAD programs")*

### D1 — In the interface, a point and no thousands separator

Every number that is not money, shown or typed in the interface (command
line, properties panel, dialogs), uses:

- a point for decimals: `2.5`, `12345.6789`;
- no thousands separator;
- ASCII digits;
- the same rule whatever the system's language.

A decimal comma or a thousands separator typed in a number field is
refused, with the field's usual message. One module holds the rule,
`arqstudiocad.ui.number_format`: `format_number`, `parse_number`, and
`with_cad_numbers` for a spin box. Qt's C locale provides it, with the
group separator neither written nor accepted.

### D2 — Precision is the drawing's

How many decimals a length or an angle shows, and in which units, comes
from the drawing, as in AutoCAD (`LUNITS`/`LUPREC`, `AUNITS`/`AUPREC`),
never from the system. Until those are read, the panel shows up to 12
significant digits and the extents 4 decimals, as today.

### D3 — What is printed is the drawing's choice

A drawing's printed annotations choose their own format, as money does:
dimensions their decimal separator (`DIMDSEP`, milestone 04.2), amounts
their thousands and decimal separators (ADR 0028, unchanged). Choosing a
comma there changes nothing that is typed.

### D4 — Files

DWG and DXF keep numbers with a point, as the codec already writes them.

## Consequences

- The panel and the dialogs read the same everywhere, and what they show
  can be typed on the command line.
- The tests no longer depend on the machine's language. A fixture
  (`decimal_comma_locale`) makes the default locale Spanish (Ecuador) for
  the tests of this rule, so CI, in English, would catch a return to the
  system's locale.
- Units and precision from the drawing (D2) and dimensions' separator (D3)
  are later work, in their milestones.
