# 0027. Block data and the drawing's currencies

Date: 2026-10-27 · Status: Proposed · Builds on [0006](0006-i18n.md),
[0017](0017-dwg-dxf-engine-strategy.md), [0018](0018-native-format.md),
[0023](0023-blocks.md), [0025](0025-blocks-palette.md), and technical
contract sections 3, 4, 6, 10 and 13.

## Context

The maintainer's drawings are mostly blocks, and the maintainer wants to
build budgets from them (2026-10-26): the Blocks palette can already count
and select a block's references (ADR 0025, step 8d), but a block carries
nothing a budget needs. Each block definition should carry its own data:
category, type, material, cost and supplier. On 2026-10-27 the maintainer
added that the cost is not always in US dollars: the drawing keeps a
register of the currencies it uses, so that a budget can later be broken
down by currency.

AutoCAD's answer for per-reference values is attributes. They do not fit
here: a block's price and supplier are the same for every reference, and
attributes would be repeated, and edited, in each one. AutoCAD and progeCAD
keep application data on any object in its extension dictionary, holding
XRECORDs, and keep data they do not understand when they save.

What the code has today (main `4c906a9`):

- The codec fork exposes `CadDocument::extension_dictionary_handle(owner)`
  and `ensure_extension_dictionary(owner)`, and the `Dictionary` and
  `XRecord` objects with typed entries (`XRecordValue`). The drawing's root
  dictionary is `header.named_objects_dict_handle`.
  `table_style_verify.rs` already reads an extension dictionary.
- `Document::purge_blocks` (step 8e) removes a record's extension
  dictionary and everything it holds, so purging a block removes its data.
- `block_copy.rs` (step 7a) clears the extension dictionaries of the
  entities it copies and does not copy the record's. A copied block would
  lose its data.
- The change set reports `blocks_modified` for a block whose contents
  changed (contract §6). Nothing reports a change of document settings.

## Decision

### D1 — The data belongs to the block definition *(maintainer, 2026-10-26)*

Each block definition may carry **block data**: *category*, *type*,
*material*, *supplier* and *cost*. Every field is optional; a block with
none has no data. The data belongs to the definition, not to each
reference: every reference of a block costs the same. A budget's quantity
is the number of references, as SELECTBLOCK and the palette count them,
including anonymous copies (ADR 0025, step 8d). Attributes stay for what
differs between references (a door's code, a room's number).

Category and type are free text; the editors suggest the values the drawing
already uses. Fixed, editable lists may come later with the Options dialog.
Material and supplier are free text.

### D2 — The cost has a currency from the drawing's register *(maintainer, 2026-10-27)*

The cost is an amount and a currency, for example `120.00 USD` or
`450.00 PEN`. The amount:

- is exact: it is kept as a whole number of the currency's minor units
  (cents), never as a binary floating-point number;
- has as many decimals as its currency (2 for USD and PEN, 0 for JPY);
- is zero or more.

The currency is one of the drawing's **active** currencies (D3).

### D3 — The drawing's currency register *(maintainer, 2026-10-27)*

The drawing keeps a register of currencies, inside the DWG, so it travels
with the drawing and its budget. Each currency has:

- a code: ISO 4217 (`USD`, `PEN`, `EUR`…) or the user's own, three to ten
  capital letters or digits, unique in the register;
- a name, a symbol (`$`, `S/`, `€`) and its number of decimals (0 to 4);
- whether it is active.

The rules:

- **Default:** a drawing without a register has USD (US dollar, `$`, two
  decimals), active and principal. Nothing is written into the file until
  the user changes the register or gives a block a cost, so opening and
  saving a drawing does not add objects to it.
- **Active currencies:** several can be active; only they are offered for
  a block's cost.
- **The principal currency:** exactly one, always active. It is proposed
  first for a new cost.
- **Adding:** the user picks a currency from a built-in ISO 4217 catalog
  (code, name, symbol, decimals), or creates one of their own.
- **Refused changes:** removing a currency, deactivating it or changing its
  decimals while a block's cost uses it is refused, saying how many blocks
  use it. Deactivating the principal currency, or the last active one, is
  refused too.
- **A cost in an unknown currency:** a block can arrive with a currency the
  register does not hold: copied from another drawing (D6) or edited in
  another program. Copying adds that currency to the register, active. A
  drawing read with such a cost shows it as it is, and the editors ask for
  the currency to be added before that cost is changed.

Exchange rates are not part of this ADR: they come with the budget table,
which will subtotal by currency.

### D4 — Stored as XRECORDs, in the DWG and the DXF

- **Block data:** an XRECORD in the block record's extension dictionary.
- **The register:** an XRECORD in a dictionary of our own in the drawing's
  root dictionary.

The key of our dictionary entries is one constant in `arq-document`. Each
XRECORD starts with a format version (an integer). It is followed by
name–value pairs as strings: a field's name, then its value. The amount is
written as decimal text with its currency's decimals, `"120.00"`. Pairs let
a later version add fields. Fields a version does not know are kept as they
are when it rewrites the record.

Reading never fails on our data: an XRECORD that does not parse is reported
and left untouched, and the block shows no data. Saving reads the file back
and compares every block's data and the register (ADR 0017 D5), in DWG and
DXF.

### D5 — Editing it, as one undo step each

- **The core:**
  - `arq-document`: `BlockData`, `Money` and the register, read and written
    as D4 says, all or nothing.
  - `arq-core`: setting a block's data is one undo step, named BLOCKDATA;
    changing the register is one undo step, named CURRENCIES.
- **The change set (contract §6):**
  - a block whose data changed is reported in `blocks_modified`, whose
    meaning widens to "contents or data changed";
  - a change of the register is reported by the existing
    `document_changed` (amended in step 2: no new field is needed).
- **Python:** `CadDocument.block_data(name)`,
  `CadDocument.set_block_data(name, data)`, `CadDocument.currencies` and
  `CadDocument.set_currencies(register)`. The values a field already has
  in the drawing, for the suggestions, are listed too.
- **The Block Definition dialog** (ADR 0023 D2a) gains an optional
  **Budget data** section, folded by default. It holds:
  - Category and Type, with suggestions;
  - Material and Supplier;
  - Cost: an amount, and a currency chosen among the active ones (the
    principal first);
  - a **Currencies…** button.
- **The Blocks palette:**
  - the context menu gains **Block data…**, which opens the same fields in
    a small dialog;
  - the Selected block section shows the block's data.
- **The Properties panel:** with one or more references of a single block
  selected, a **Block data** group shows the definition's data, editable,
  and says that a change applies to every reference.
- **Commands** (English names in code, Spanish in the catalog):
  - **BLOCKDATA** (`DATOSBLOQUE`) opens the block-data dialog for a block
    named or picked;
  - **-BLOCKDATA** (`-DATOSBLOQUE`) asks for the fields at the command
    line;
  - **CURRENCIES** (`MONEDAS`) opens the Currencies dialog. It lists the
    register with a box for each active currency and a choice of the
    principal one, and buttons to add from the catalog, add your own, and
    remove. The Options dialog will later open it too.
- **The ISO 4217 catalog** is data in the Python package: code, symbol
  and decimals. Its names are translated through the locale catalogs
  (ADR 0006). The catalog lists facts from ISO 4217. Anything copied from
  another data set (for example Unicode CLDR) is recorded in
  `THIRD_PARTY.md` and must be GPL-compatible.

### D6 — The data follows the block

- **The block editor** (ADR 0023 D3) keeps the record, so it keeps the
  data when it saves; a test proves it.
- **COPYBLOCK and the Template source** (ADR 0025 D6) copy the data with
  the block, and add its currency to the target's register when missing
  (D3). On a clash answered "Replace", the data comes from the source.
- **RENAME** keeps the data, and **PURGE** removes it with the block.
- **EXPLODE** removes a reference, not a definition: the data stays.

### D7 — What waits

These come later, each with its own ADR or roadmap entry:

- the budget table itself: quantities × cost, subtotals by currency,
  exchange rates, and export to CSV and spreadsheets;
- a library of block data shared between drawings;
- fixed, editable lists for category and type;
- showing a field as a label in the drawing.

### D8 — Order of work

Each step is its own pull request with tests:

1. This ADR and the roadmap row.
2. **The core:** `BlockData`, `Money` and the register in `arq-document`,
   read and written as XRECORDs and checked on save. Then the undo steps,
   the change set field and the Python API. The block editor, COPYBLOCK,
   RENAME and PURGE follow D6. Round trips in DWG and DXF. The maintainer's
   drawings read and saved with no change when nothing is edited.
3. **The Currencies dialog**, CURRENCIES and the ISO 4217 catalog.
4. **The Budget data section** of the Block Definition dialog, BLOCKDATA
   and -BLOCKDATA, the palette's Block data… and its Selected block
   section.
5. **The Properties panel's** Block data group.

## Implementation notes

- **Step 2, the core (2026-10-27):**
  - `arq-document` (`block_data.rs`) adds `BlockData`, `Money`, `Currency`
    and `Currencies`:
    - `Document::block_data`, `blocks_data` and `set_block_data`;
    - `Document::currencies`, `currency_uses` and `set_currencies`.
  - The XRECORDs are keyed `ARQSTUDIOCAD_BLOCKDATA`, in the block record's
    extension dictionary, and `ARQSTUDIOCAD_CURRENCIES`, in the root
    dictionary. Each starts with version 1 (group code 70), followed by
    pairs: a name (code 1), then a value (code 300).
  - A new error, `InvalidBlockData`, covers data that cannot be written or
    does not read.
  - `arq-core`: `set_block_data` is one undo step, BLOCKDATA;
    `set_currencies` is one undo step, CURRENCIES.
  - **Python:** `CadBlockData` and `CadCurrency`, and on `CadDocument`:
    - `block_data`, `blocks_data` and `set_block_data`, the cost given as
      decimal text;
    - `currencies` (the list and the principal code), `currency_uses` and
      `set_currencies`.
  - **A change from D5:** the register's change is reported by the change
    set's existing `document_changed`, which already means "a
    document-level setting changed". No new field was needed, so the
    contract, the binding and the events are unchanged. A block whose data
    changed is reported in `blocks_modified`, as D5 says.
  - **Saving** compares every block's data and the register when it reads
    the file back.
  - **D6:**
    - COPYBLOCK, and with it the Template source, copies a block's data
      and adds its currency to the register;
    - a redefinition takes the source's data, none included;
    - RENAME and the block editor keep the data, and PURGE removes it.
  - Tests:
    - in Rust, amounts, refusals, undo, unknown pairs, unreadable data,
      round trips in DWG and DXF, and D6;
    - in Python, the API.

## Consequences

- **Budgets become possible** from what the drawing already holds: the
  palette's counts give the quantities, and the definitions give the cost.
- **Interoperability:** the data lives where other CAD programs keep data
  they do not understand.
  - The maintainer checks that progeCAD keeps it: a drawing saved with
    block data is opened, saved and read back.
  - A program that drops unknown XRECORDs loses the data, but not the
    drawing.
- **The change set reports the register** through its existing
  `document_changed` (see the step 2 note), so the contract does not
  change.
- **COPYBLOCK changes:** it must copy one extension dictionary, the
  record's, while it still clears those of the copied entities.
- **The currency register is document data,** not a user preference: two
  users opening the same drawing see the same currencies.
