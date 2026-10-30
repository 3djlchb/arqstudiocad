# 0028. The budget table

Date: 2026-10-29 · Status: Proposed · Builds on [0013](0013-application-shell.md),
[0025](0025-blocks-palette.md), [0027](0027-block-data.md), and the
[master plan](../../MASTER_PLAN.md) §16 (new technologies).

## Context

Each block definition can carry budget data: category, type, material,
supplier and a cost in a currency of the drawing's register, written in
that currency's number format (ADR 0027). The Blocks palette already counts
a block's references, anonymous copies included (ADR 0025 step 8d). What is
missing is the budget itself: each block's quantity times its unit cost,
grouped, totalled, and taken out of the program. ADR 0027 D7 left this for
its own ADR. The maintainer decided its shape on 2026-10-29.

What the code has (main `2891a15`):

- `CadDocument.blocks_data`: every block's data, by name.
- `CadDocument.block_references`: every block's references anywhere in the
  drawing, counted in one pass. It does not separate model space from
  layouts, and does not multiply nested references by the references of
  the block holding them.
- `CadDocument.find_block_references(name)`: one block's references,
  split into shown, hidden, nested and in layouts.
- `arqstudiocad.currencies`: `register_of`, `CurrencyRegister.format_of`
  and `cost_text`, and `format_amount`.
- The only runtime Python dependency is PySide6.

## Decision

### D1 — A Budget panel *(maintainer)*

The budget is shown in a dockable **Budget** panel, as the Blocks palette
is. It follows the drawing as it changes. A command **BUDGET** (Spanish
`PRESUPUESTO`) shows it, and View → Palettes gets a toggle.

Inserting the budget into the drawing as a table entity comes later: DWG
tables are another workstream (milestone 03.4).

### D2 — Grouped by category *(maintainer)*

**Rows and order:**

- Each block with a cost is one row, grouped **by category**. Each
  category has its subtotal.
- Inside a category, rows are sorted by type, then by block name.
- A block with a cost but no category goes under **No category**.

**Columns:** Item (the block's type, or its name when it has no type),
Material, Supplier, Unit, Quantity, Unit cost, Amount.

- The unit is "each" (`und`) until measured quantities exist (ADR 0027 D7).
- Amounts are shown with the currency's symbol, in the currency's number
  format (ADR 0027 D3a).

**Totals:** amounts in different currencies are never added together.
- A category with costs in two currencies shows one subtotal per currency.
- The budget ends with one total per currency.
- Exchange rates come later.

### D3 — Quantities: model space, nested references multiplied *(maintainer)*

A block's quantity is its number of references in **model space**,
including those inside other blocks. A nested reference counts once for
each placement of the block holding it, so a dining set holding four
chairs, placed three times, counts twelve chairs. This goes down any
number of levels.

**What counts:**

- Anonymous copies count for their block, as SELECTBLOCK and the palette
  count them.
- References only in layouts (paper space) do not count.
- References on off or frozen layers count: they are in the drawing,
  only hidden.

**The core:** this needs a new query, because `block_references` neither
separates layouts nor multiplies. `Document::block_quantities()` returns,
for every block, its references in model space, counted as above. It is
one pass over the drawing plus the nesting graph, without recursion that
could loop: a block cannot hold itself (ADR 0023).

### D4 — Blocks without a cost are listed apart *(maintainer)*

**Without a cost:**

- A block with references in model space but no cost is listed in a
  section **Without cost**, with its quantity.
- It is never added to a total.
- The panel says how many there are, so a price is not forgotten.

**Without data:** blocks with no data at all are listed there too.

**Not listed:** blocks with no references in model space.

### D5 — Scope

The budget covers the whole drawing. A **Selection only** option counts
only the selected references, nested ones included. This allows a budget
per area: select a floor, then read its budget.

### D6 — Export to CSV and to Excel *(maintainer)*

**Export…** writes the budget as shown (groups, rows, subtotals, totals,
and the Without cost section) to one of two formats.

**CSV:**

- Plain text, one row per line.
- Amounts are written with the decimal separator of their currency's
  format and without a thousands separator.
- The field separator is a semicolon when that decimal separator is a
  comma, otherwise a comma, as spreadsheets expect.
- Written as UTF-8 with a byte order mark, so spreadsheets read accents.

**Excel (`.xlsx`):**

- Amounts are numbers with a number format showing their currency's
  symbol and decimals, so they can be summed in the spreadsheet.
- Subtotals and totals are written as values: no formulas that a reader
  might break.

**The Excel library:** writing `.xlsx` needs one, so the master plan's
§16 rule applies:

- **The library:** `openpyxl`, which writes and reads `.xlsx`.
- **Objective:** a budget the maintainer's clients open in Excel, with
  formats.
- **The core stays independent:** it is used only by the export, in the
  Python application layer. The Rust core and the integration contract do
  not change.
- **License:** MIT, and its only dependency, `et_xmlfile`, is MIT too.
  Both are GPL-compatible; they are recorded in `THIRD_PARTY.md`.
- **Upkeep:** pure Python, maintained for many years, widely used and
  documented.
- **Replaceable:** it sits behind one function, `export_xlsx`. Another
  writer could take its place, or CSV would remain.
- **How it ships:** it becomes a runtime dependency, pinned to a range,
  and is imported only when exporting to Excel. Without it, Excel export
  is unavailable with a message, and CSV still works.

### D7 — What waits

- Measured quantities (perimeter, area, volume, weight), and dynamic
  blocks' quantities following their parameters (ADR 0027 D7).
- Exchange rates, and a total in one currency.
- Inserting the budget as a table in the drawing.
- Budget templates, headers (project, client, date) and printing.
- Filters by layer.

### D8 — Order of work

Each step is its own pull request with tests:

1. This ADR and the roadmap row. ADR 0027 is accepted, and milestone
   02.11 marked done.
2. **The core:**
   - `Document::block_quantities` (D3), and its selection variant (D5);
   - the Python API;
   - tests with nesting, anonymous copies, layouts and the maintainer's
     drawings.
   - Done: `Document::block_quantities` and `block_quantities_of(ids)`,
     in Python `CadDocument.block_quantities` and `block_quantities_of`.
     A MINSERT counts each of its copies. Only blocks INSERT can place
     are listed: an anonymous copy whose block is unknown is not, though
     what it holds counts. A block holding itself through others, which
     only a broken file has, is counted where it is placed; what it holds
     is not, and the count does not loop.
3. **The panel** (D1, D2, D4, D5), the BUDGET command and its toggle.
   - Done. The maintainer chose (2026-10-29) to dock it across the
     bottom, above the command window, closed at first, and to end each
     category with a Subtotal row per currency, as the export will write
     it.
   - The budget itself is built apart from the panel,
     `arqstudiocad.budget.budget_of`, for the export to reuse.
   - Categories that differ only in case are one group, spelled as its
     first block by name spells it.
   - The panel reads the drawing only while it is open, and catches up
     when opened again.
4. **Export** (D6): CSV, then Excel with `openpyxl`.
   - Done, in one pull request. The maintainer chose (2026-10-29) an
     **Export…** button in the Budget panel, writing what the panel
     shows, and the command **BUDGETEXPORT** (Spanish
     `EXPORTARPRESUPUESTO`): of the objects selected, as BLOCKDATA takes
     them, or of the whole drawing. One Save dialog offers both types.
   - Both formats add a **Currency** column, so every amount says its
     currency.
   - The CSV field separator follows the drawing's general number format
     (a semicolon when its decimal separator is a comma), not each
     currency's: one file has one separator, and an amount holding it is
     quoted.
   - `openpyxl` 3.1.5 (MIT) and `et_xmlfile` 2.0.0 (MIT) are recorded in
     `THIRD_PARTY.md`; `types-openpyxl` is a development dependency for
     mypy. Workbooks open in Excel, LibreOffice Calc and ONLYOFFICE.
   - **Packaging:** PyInstaller finds the import inside `export_xlsx`, so
     the Windows, macOS, `.deb` and `.rpm` packages carry it. The Flatpak
     manifest, built offline and blocked for now, will need both as
     Python modules; until then Excel export says it is unavailable there
     and CSV works.
   - With this step every step of D8 is done.

## Consequences

- **A budget comes straight from the drawing.** Changing a block's cost,
  or placing more references, changes the budget at once.
- **A new quantity query in the core.** The palette's counts stay as they
  are, so the two numbers differ on purpose: the palette counts
  everywhere, the budget counts model space with nesting. The panel says
  which.
- **A second runtime Python dependency,** limited to export and optional
  at import.
- **Groundwork for later:** the panel and the export are what measured
  quantities and dynamic blocks will fill (ADR 0027 D7).
