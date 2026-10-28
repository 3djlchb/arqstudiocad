"""Budget data (ADR 0027): CURRENCIES edits the drawing's register of
currencies, the ones blocks' costs may use, in the Currencies dialog.
BLOCKDATA edits a block's data (category, type, material, supplier and
cost) in the Block Data dialog, and -BLOCKDATA at the command line (step
4a, maintainer 2026-10-27): the block is the one of the references
selected, one picked, or one named."""

from collections.abc import Generator
from typing import Any

from arqstudiocad.commands import (
    Command,
    CommandCancelledError,
    CommandContext,
    CommandScript,
    Option,
    Prompt,
)
from arqstudiocad.core import CadBlockData, CadDocument, InvalidOperationError
from arqstudiocad.currencies import format_amount, parse_amount, register_of, shown_name
from arqstudiocad.i18n import tr

#: What clears a field at the command line, as AutoCAD's prompts take it.
CLEAR = "."


def currencies(ctx: CommandContext) -> CommandScript:
    """CURRENCIES: the drawing's currencies, edited in the Currencies dialog
    and written in one undo step; without an interface, listed."""
    document = ctx.document()
    if ctx.ui is None:
        listed, principal = document.currencies
        for currency in listed:
            state = tr("principal") if currency.code == principal else ""
            if not currency.is_active:
                state = tr("inactive")
            text = f"{currency.code} — {shown_name(currency.name)}"
            ctx.message(f"{text} ({state})" if state else text)
        return
    if not edit_currencies(ctx, document):
        raise CommandCancelledError()
    yield from ()


def edit_currencies(ctx: CommandContext, document: CadDocument) -> bool:
    """The Currencies dialog, and what it proposes written in one undo step,
    or why the core refused it said; False if it was cancelled. BLOCK's
    Budget data section opens it too (ADR 0027 step 4b)."""
    assert ctx.ui is not None
    answer = ctx.ui.edit_currencies(document)
    if answer is None:
        return False
    try:
        changes = document.set_currencies(*answer)
    except InvalidOperationError as error:
        ctx.message(tr("The currencies were not changed: {reason}", reason=error))
        return True
    if changes.document_changed:
        ctx.session.report(document, changes)
        ctx.message(tr("Currencies changed."))
    return True


def block_data(ctx: CommandContext) -> CommandScript:
    """BLOCKDATA: a block's data, edited in the Block Data dialog and
    written in one undo step; without an interface, asked as -BLOCKDATA."""
    document = ctx.document()
    name = yield from _block(ctx, document)
    if ctx.ui is None:
        yield from _ask_data(ctx, document, name)
        return
    data = ctx.ui.edit_block_data(document, name)
    if data is None:
        raise CommandCancelledError()
    _write(ctx, document, name, data)


def block_data_prompts(ctx: CommandContext) -> CommandScript:
    """-BLOCKDATA: the block, then each field at the command line; Enter
    keeps a value, a point clears it."""
    document = ctx.document()
    name = yield from _block(ctx, document)
    yield from _ask_data(ctx, document, name)


def block_data_named(name: str) -> Command:
    """BLOCKDATA with block ``name`` already chosen, as the Blocks palette's
    context menu starts it."""

    def run(ctx: CommandContext) -> CommandScript:
        document = ctx.document()
        found = _named(document, name)
        if found is None:
            raise CommandCancelledError(tr("There is no such block."))
        if ctx.ui is None:
            yield from _ask_data(ctx, document, found)
            return
        data = ctx.ui.edit_block_data(document, found)
        if data is None:
            raise CommandCancelledError()
        _write(ctx, document, found, data)

    return Command("BLOCKDATA", run, repeat=False)


def clear_data_named(name: str) -> Command:
    """Clears block ``name``'s budget data, once confirmed, in one undo
    step, as the Blocks palette's Clear block data starts it (maintainer,
    2026-10-28)."""

    def run(ctx: CommandContext) -> CommandScript:
        document = ctx.document()
        found = _named(document, name)
        if found is None:
            raise CommandCancelledError(tr("There is no such block."))
        if ctx.ui is not None and not ctx.ui.ask_clear_block_data(found):
            raise CommandCancelledError()
        try:
            changes = document.set_block_data(found, CadBlockData())
        except InvalidOperationError as error:
            raise CommandCancelledError(
                tr("The block data was not changed: {reason}", reason=error)
            ) from None
        if not changes.blocks_modified:
            raise CommandCancelledError(tr("{name} has no block data.", name=found))
        ctx.session.report(document, changes)
        ctx.message(tr("Block data of {name} cleared.", name=found))
        yield from ()

    return Command("BLOCKDATA", run, repeat=False)


def _block(ctx: CommandContext, document: CadDocument) -> Generator[Prompt, Any, str]:
    """The block whose data to edit: that of the references selected, when
    they are all of one block; else one picked, or named with N."""
    if not document.block_names:
        raise CommandCancelledError(tr("The drawing has no blocks."))
    name = _selected_block(ctx, document)
    while name is None:
        answer = yield ctx.select(tr("Select a block reference"), [Option("N", tr("Name"))])
        if answer == "N":
            name = _named(document, (yield ctx.ask(tr("Block name"))).strip())
            if name is None:
                ctx.message(tr("There is no such block."))
            continue
        if not ctx.session.selection(document):
            raise CommandCancelledError()
        name = _selected_block(ctx, document)
        if name is None:
            ctx.message(tr("Select references of one block."))
            ctx.session.deselect(document, ctx.session.selection(document))
    return name


def _selected_block(ctx: CommandContext, document: CadDocument) -> str | None:
    """The block every selected entity is a reference of, if there is one."""
    chosen = ctx.session.selection(document)
    if not chosen:
        return None
    blocks = {dict(e.details).get("Block name") for e in document.get_entities(chosen)}
    if len(blocks) != 1:
        return None
    (block,) = blocks
    return _named(document, block) if isinstance(block, str) else None


def _named(document: CadDocument, name: str) -> str | None:
    """The block called ``name``, ignoring case, as the drawing spells it."""
    return next((n for n in document.block_names if n.casefold() == name.casefold()), None)


def _ask_data(ctx: CommandContext, document: CadDocument, name: str) -> CommandScript:
    """Each field of block ``name``'s data at the command line, then the
    data written."""
    try:
        data = document.block_data(name) or CadBlockData()
    except InvalidOperationError:
        ctx.message(tr("The block's data could not be read: saving replaces it."))
        data = CadBlockData()
    texts = {}
    for field, label in (
        ("category", tr("Category")),
        ("type", tr("Type")),
        ("material", tr("Material")),
        ("supplier", tr("Supplier")),
    ):
        texts[field] = yield from _text(ctx, label, getattr(data, field))
    register = register_of(document)
    active = [c.code for c in register.currencies if c.is_active]
    current = data.currency if data.currency in active else register.principal
    code = yield ctx.choose(tr("Currency"), [Option(c, c) for c in active], default=current)
    thousands, decimal = register.format_of(code)
    shown = format_amount(data.cost, thousands, decimal) if data.cost is not None else None
    while True:
        typed = (yield ctx.ask(tr("Cost (a point for none)"), default=shown, optional=True)).strip()
        if typed in ("", CLEAR):
            cost = None
            break
        try:
            cost = parse_amount(typed, thousands, decimal)
        except ValueError as error:
            ctx.message(str(error))
            continue
        break
    _write(
        ctx,
        document,
        name,
        CadBlockData(
            category=texts["category"],
            type=texts["type"],
            material=texts["material"],
            supplier=texts["supplier"],
            cost=cost,
            currency=code if cost is not None else None,
        ),
    )


def _text(ctx: CommandContext, label: str, current: str) -> Generator[Prompt, Any, str]:
    """A text field: Enter keeps ``current``, a point clears it."""
    typed = (yield ctx.ask(label, default=current or None, optional=True)).strip()
    return "" if typed == CLEAR else typed


def _write(ctx: CommandContext, document: CadDocument, name: str, data: CadBlockData) -> None:
    """Writes block ``name``'s data in one undo step, saying so; nothing
    when it is unchanged."""
    try:
        changes = document.set_block_data(name, data)
    except InvalidOperationError as error:
        raise CommandCancelledError(
            tr("The block data was not changed: {reason}", reason=error)
        ) from None
    if changes.blocks_modified:
        ctx.session.report(document, changes)
        ctx.message(tr("Block data of {name} saved.", name=name))


#: Budget data commands.
BUDGET_COMMANDS = (
    Command("CURRENCIES", currencies),
    Command("BLOCKDATA", block_data),
    Command("-BLOCKDATA", block_data_prompts),
)
