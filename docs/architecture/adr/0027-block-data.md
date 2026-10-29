# 0027. Block data and the drawing's currencies

Date: 2026-10-27 (accepted 2026-10-29) · Status: Accepted · Builds on [0006](0006-i18n.md),
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

### D3a — How amounts are written *(maintainer, 2026-10-27)*

As accounting standards allow, the drawing's register also says how its
amounts are written:

- **thousands** separated by a comma, a point or a space;
- **decimals** separated by a point or a comma, never the thousands one.

A drawing starts with 1,250.50. The Currencies dialog chooses both in two
lists, with an example. Amounts are shown in this format and typed in it:
thousands are optional when typed, but in groups of three when written. The
core always stores an amount as decimal text with a point (D4), so changing
the format changes no cost.

Each currency may also write its amounts its own way, as its country does
*(maintainer, 2026-10-27)*:

- a currency follows the drawing's format, **General**, or has its own
  separators, with the same rules;
- a currency added from the catalog starts with its country's usual
  format. Where several countries share it (the euro), it starts with the
  most common one, and the user may change it or return it to General.

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
- showing a field as a label in the drawing;
- **measured quantities** for budget line items *(maintainer,
  2026-10-28)*, so an item is priced by what a block measures, not only
  by how many references it has. Each item has a unit (each, m, m², m³,
  kg). The quantities would be:
  - a block's perimeter and area, from its 2D contents (closed
    polylines, hatches, circles);
  - its height and volume, from its 3D solids, once the owned kernel
    measures them (`arq-kernel` step 3, milestone 02.8);
  - its weight, from its volume and its material's density;
  - each reference measured with its own scale.

  Each block says which of these its line item is priced by: its number
  of references, its perimeter, its area, its volume, and so on. That
  choice, and the unit, are part of the block's data and can be changed.

  **Dynamic blocks** need this most *(maintainer, 2026-10-28)*. Each
  reference has its own dimensions, set by its parameters (a door's
  width, a wall's length), so its quantities come from that reference,
  not from the definition. When a parameter changes, the quantities
  follow on their own. This joins the parametric engine (milestone 04.1),
  which recomputes a dynamic block when a parameter changes.

  This comes after the budget table, with its own ADR. That ADR decides:
  - which geometry counts;
  - how drawing units become an item's units;
  - where densities live;
  - whether quantities are computed when needed or stored;
  - how a dynamic block's parameters reach its quantities.

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
- **Step 3, the Currencies dialog (2026-10-27)** *(maintainer: a table
  with boxes, the whole ISO catalog with a search, the command alone for
  now)*:
  - **CURRENCIES** (Spanish `MONEDAS`) opens the Currencies dialog
    through the new UI service `edit_currencies`. It writes what the
    dialog proposes in one undo step, and says why when the core refuses.
    Without an interface, it lists the register.
  - **The dialog** (`ui/currencies_dialog.py`) shows one row per currency:
    - an Active box, and a Principal choice drawn as a radio button (the
      style's own radio is faint over a table in the dark theme);
    - the code, the name and the symbol (the last two edited in place),
      the decimals, and how many blocks' costs use it.
  - **What stays:** a currency a cost uses, or the principal one, stays
    active and cannot be removed; only an active one can be principal.
  - **Adding:** **Add from catalog…** opens the catalog with a search by
    code or name; **Own currency…** asks for a code (3 to 10 capital
    letters or digits, not taken), a name, a symbol and the decimals.
  - **The catalog** (`arqstudiocad/currencies.py`) holds ISO 4217's 154
    current currencies, without funds, metals or testing codes; BGN left
    with Bulgaria's euro (2026). These are facts compiled for the project,
    not copied from another data set, so `THIRD_PARTY.md` does not change.
  - **Names:** a catalog name is kept in English in the drawing and shown
    through `tr()`, so each reader sees it in their language; a name the
    user writes is shown as written. Every catalog name has its Spanish
    translation.
  - Opening it from the Budget data section comes in step 4, and from
    the Options dialog later.
- **The number format, D3a (2026-10-27):**
  - **Core:** `Currencies` gains `thousands` and `decimal`, checked as
    D3a says. The register writes them as the pairs `thousands` and
    `decimal`, named `comma`, `point` or `space`, because a value that is
    only a space would not survive every reader. A register without them
    reads as 1,250.50.
  - **Python:** `CadDocument.number_format`; `set_currencies` takes
    `thousands` and `decimal`, None keeping them.
    `arqstudiocad.currencies` adds:
    - `format_amount` and `parse_amount`, the latter accepting the
      no-break spaces too when thousands are spaced;
    - `CurrencyRegister`, the register, its principal currency and the
      format: what the UI service `edit_currencies` now returns.
  - **The Currencies dialog** gains a General number format group with
    the two lists and a live example. Choosing a separator the other list
    has moves that list to its other choice.
  - **A currency's own format:**
    - **Core:** `Currency::format` is `None` for General, or its
      separators, checked like the drawing's. The register writes them as
      `thousands` and `decimal` pairs after the currency's code: pairs
      before the first code are the drawing's. A currency with one alone
      does not read. `Currencies::format_of(code)` gives the format that
      applies.
    - **Python:** `CadCurrency` takes `thousands` and `decimal` (both or
      neither), and `CurrencyRegister.format_of(code)`.
    - **The catalog:** each currency gives its country's usual format,
      1,250.50 unless stated; `CatalogCurrency.currency()` makes the
      register's currency from it.
    - **The dialog:** a Format column lists General (with the drawing's
      example), then 1,250.50, 1.250,50, 1 250,50 and 1 250.50.
- **Step 4a, the block data editors (2026-10-28)** *(maintainer, 2026-10-27:
  a cost is typed in its currency's format; the block is the one of the
  references selected, one picked, or one named; two pull requests)*:
  - **The Block Data dialog** (`ui/block_data_dialog.py`) holds:
    - Category and Type, editable lists suggesting the values the
      drawing's blocks use (each once ignoring case, as the first block by
      name spells it);
    - Material and Supplier;
    - Cost: an amount typed in the format of the chosen currency, and the
      active currencies, the principal one first.
  - **How the dialog behaves:**
    - changing the currency writes the amount again in the new currency's
      format;
    - an amount in another format, or with more decimals than its
      currency has, keeps OK disabled and says why;
    - a cost in a currency that is not active (a drawing edited
      elsewhere) is shown as such and must be moved to another currency.
  - **BLOCKDATA** (Spanish `DATOSBLOQUE`) edits the block of the selected
    references when they are all of one block. Otherwise it asks to
    select a reference, or **Name** to type one. It opens the dialog
    through the new UI service `edit_block_data`, and writes the data in
    one undo step; it says nothing when the data is the same.
  - **-BLOCKDATA** (`-DATOSBLOQUE`) asks each field at the command line:
    Enter keeps a value and a point clears it. The currency comes before
    the cost, so the cost is read in its format. Without an interface,
    BLOCKDATA asks the same way.
  - **The Blocks palette:**
    - the context menu gains **Block data…** for the drawing's blocks,
      dynamic ones included;
    - a **Budget data** section below Selected block shows the fields the
      block has, and its cost with its currency's symbol, in that
      currency's format (`S/ 1.250,50`).
  - `arqstudiocad.currencies` gains `register_of(document)`, and
    `CurrencyRegister.get` and `cost_text`.
- **Step 4b, the Budget data section (2026-10-28):**
  - The Block Data dialog's fields became one widget, `BlockDataFields`,
    which the Block Definition dialog also shows.
  - **The section:** an optional Budget data section below the groups.
    - It is folded by default, and unfolded when it holds data or was
      unfolded before the dialog closed to pick.
    - The data is **optional** *(maintainer, 2026-10-28)*: folded or
      open, an empty section makes the block. Only data typed wrong
      holds OK back, saying "Budget data: …", and the section stays open
      while it does, so it never hides why.
    - It has a **Currencies…** button: the dialog closes with the new
      action `currencies`, BLOCK edits the currencies as CURRENCIES does
      (its own undo step), then opens the dialog again.
  - `BlockDefinitionState` gains `data` and `data_shown`.
  - **BLOCK** writes the block and its data in one undo step (a group
    named BLOCK). If the core refuses the data, nothing is made and the
    dialog opens again.
  - **A redefinition:** empty data leaves the block's data as it is; data
    given replaces it. Clearing a block's data is BLOCKDATA's.
  - **-BLOCK** asks no budget data: -BLOCKDATA does.
- **Clearing a block's data at once (2026-10-28)** *(maintainer: editing
  and clearing must both be at hand; a small pull request of its own)*:
  - **The Block Data dialog** gains **Clear data**: it empties every
    field. OK then clears the block's data, and Cancel keeps it.
  - **The Blocks palette's context menu** gains **Clear block data**.
    - It is enabled only for a block with data, including data that does
      not read.
    - It asks first, No being the default, through the new UI service
      `ask_clear_block_data`.
    - It clears the data in one undo step (BLOCKDATA), saying so.
    - `clear_data_named` starts it.
  - **The core:** `set_block_data` skipped data that did not read when
    given empty data, taking it for "no data". Now data that does not
    read is always replaced, so it can be cleared.
- **Step 5, the Properties panel (2026-10-28):**
  - With references of **one** block selected (`selected_block`), the
    panel gains a **Block data** group after General
    (`block_data_section`).
    - It shows "Applies to: every reference of X", because the data
      belongs to the definition.
    - Category, Type, Material and Supplier are edited in place.
    - Cost is typed in its currency's format.
    - Currency lists the active currencies.
    - **Clear block data** works on a double click, asks first (No by
      default), and shows only when the block has data.
  - **Every field is optional.** Each edit is one undo step (BLOCKDATA).
    A cost typed wrong is said in the command line and changes nothing.
  - **Choosing a currency:** when the block has a cost, choosing another
    currency moves the cost to it. Without a cost, the currency chosen
    waits, by block, until a cost is typed.
  - Two blocks, or anything that is not a reference, show no group.
  - With this step every step of D8 is done, and the maintainer accepted
    the ADR (2026-10-29). The budget table follows in ADR 0028.

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
