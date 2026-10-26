"""BLOCK and INSERT (ADR 0023 D2): make a block from objects selected, and
place a block, following the cursor until its insertion point is picked.
BEDIT, BSAVE and BCLOSE (D3): edit a block's definition alone, in a block
editor of its own, and save it back into the drawing. The Blocks palette
starts INSERT and BEDIT with the block already chosen (ADR 0025 D5), loads
templates and inserts their blocks, copying each first (D6, D7)."""

import math
from collections.abc import Callable, Generator, Sequence
from pathlib import Path
from typing import Any

from arqstudiocad.commands import (
    Background,
    BlockClash,
    ClashChoice,
    Command,
    CommandCancelledError,
    CommandContext,
    CommandScript,
    Option,
    Prompt,
)
from arqstudiocad.commands.modify import select_objects
from arqstudiocad.core import CadDocument, CadError, InvalidOperationError
from arqstudiocad.events import Drag
from arqstudiocad.i18n import tr
from arqstudiocad.recent_blocks import CREATED, FROM_TEMPLATE, INSERTED

#: Where INSERT first puts the block it shows following the cursor.
ORIGIN = (0.0, 0.0)


def block(ctx: CommandContext) -> CommandScript:
    """A block named and based at a point, made from objects selected; a
    reference to it takes their place (AutoCAD's "Convert to block")."""
    document = ctx.document()
    template = ctx.session.template
    while True:
        name = (yield ctx.ask(tr("Block name"))).strip()
        taken = _named(document, name)
        if taken is not None:
            ctx.message(tr("A block named {name} exists in the drawing.", name=name))
            if ctx.ui is not None:
                # Said in a dialog as well (maintainer, 2026-10-25).
                ctx.ui.report_block_name_taken(taken)
            continue
        found = _named(template, name) if template is not None and template.is_open else None
        if template is None or found is None:
            break
        # Two files, no clash yet: inserting it from the template will ask
        # (maintainer, 2026-10-25).
        ctx.message(
            tr(
                "The template {template} has a block named {name}: inserting it "
                "from the template will ask what to do.",
                template=template.name,
                name=found,
            )
        )
        if ctx.ui is None or ctx.ui.ask_block_name_in_template(found, template.name):
            break
    base = yield ctx.point(tr("Specify base point"))
    ids = yield from select_objects(ctx, document)
    if not ids:
        return
    try:
        _, changes = document.create_block(name, base, ids)
    except InvalidOperationError:
        raise CommandCancelledError(tr("{name} is not a valid block name.", name=name)) from None
    ctx.session.report(document, changes)
    ctx.session.block_used(document, name, CREATED)
    ctx.message(tr("Block {name} made from {count} objects.", name=name, count=len(ids)))


def insert(ctx: CommandContext) -> CommandScript:
    """A block placed at a point, scaled and turned (Scale and Rotate before
    the point); it follows the cursor until the point is picked. One undo
    step."""
    document = ctx.document()
    names = document.block_names
    if not names:
        raise CommandCancelledError(tr("The drawing has no blocks to insert."))
    if ctx.ui is not None:
        name = ctx.ui.choose_block(names, tr("Insert block"))
        if name is None:
            raise CommandCancelledError()
    else:
        while (name := _named(document, (yield ctx.ask(tr("Block name"))).strip())) is None:
            ctx.message(tr("There is no such block."))
    yield from place_block(ctx, document, name)


def place_block(
    ctx: CommandContext,
    document: CadDocument,
    name: str,
    scale: float = 1.0,
    rotation: float = 0.0,
    on_screen: bool = True,
    explode: bool = False,
    source: CadDocument | None = None,
    choices: dict[str, ClashChoice] | None = None,
    at: tuple[float, float] | None = None,
) -> CommandScript:
    """Places block ``name``: on screen, following the cursor until its
    point is picked, with Scale and Rotate still offered; otherwise at
    ``at`` (the origin by default) at once. With ``explode`` the reference
    is then replaced by its contents, as EXPLODE does (ADR 0025 D5). A
    block of ``source`` (a template) is first copied into the drawing, as
    ``choices`` resolve its clashes (D6). One undo step either way; the
    Blocks palette's Recent source lists the block (D1)."""
    used = name
    how = FROM_TEMPLATE if source is not None and source == ctx.session.template else INSERTED
    if not on_screen:
        document.begin_group("INSERT")
        try:
            name = _copied(ctx, document, name, source, choices)
            point = at if at is not None else ORIGIN
            placed, changes = document.insert_block(name, point, scale, rotation)
            ctx.session.report(document, changes)
            if explode:
                ctx.session.report(document, document.explode_entities([placed])[1])
        except BaseException:
            ctx.session.report(document, document.cancel_group())
            raise
        document.end_group()
        ctx.session.block_used(source or document, used, how)
        return
    document.begin_group("INSERT")
    placed = False
    try:
        name = _copied(ctx, document, name, source, choices)
        # The block, at the origin, follows the cursor as a drag preview.
        shown, changes = document.insert_block(name, ORIGIN, scale, rotation)
        ctx.session.report(document, changes)
        options = [Option("S", tr("Scale")), Option("R", tr("Rotate"))]
        while True:
            answer = yield ctx.point(
                tr("Specify insertion point"),
                base=ORIGIN,
                options=options,
                drag=Drag("move", (shown,), band=False),
            )
            if answer == "S":
                scale = yield from _number(ctx, tr("Specify scale factor"), scale, positive=True)
            elif answer == "R":
                degrees = yield from _number(
                    ctx, tr("Specify rotation angle"), math.degrees(rotation)
                )
                rotation = math.radians(degrees)
            else:
                ctx.session.report(document, document.move_entities([shown], answer))
                if explode:
                    ctx.session.report(document, document.explode_entities([shown])[1])
                placed = True
                return
            # Shown again with the new scale or rotation.
            ctx.session.report(document, document.delete_entities([shown]))
            shown, changes = document.insert_block(name, ORIGIN, scale, rotation)
            ctx.session.report(document, changes)
    finally:
        if placed:
            document.end_group()
            ctx.session.block_used(source or document, used, how)
        else:
            # Cancelled: nothing was inserted.
            ctx.session.report(document, document.cancel_group())


def bedit(ctx: CommandContext, chosen: str | None = None) -> CommandScript:
    """Opens a block of the drawing in a block editor (ADR 0023 D3): its
    definition alone, at its own coordinates, where every tool works. The
    block is chosen in the Edit Block Definition dialog, ``chosen`` first
    (the block double-clicked)."""
    document = ctx.document()
    if ctx.session.block_editor_of(document) is not None:
        raise CommandCancelledError(tr("Close this block editor before editing another block."))
    names = document.block_names
    if not names:
        raise CommandCancelledError(tr("The drawing has no blocks to edit."))
    if ctx.ui is not None:
        name = ctx.ui.choose_block_to_edit(document, chosen)
        if name is None:
            raise CommandCancelledError()
    else:
        while (name := _named(document, (yield ctx.ask(tr("Block name"))).strip())) is None:
            ctx.message(tr("There is no such block."))
    _edit(ctx, document, name)


def _edit(ctx: CommandContext, document: CadDocument, name: str) -> None:
    """Opens block ``name`` in a block editor, unless it is dynamic."""
    if document.is_dynamic_block(name):
        # Its parameters wait for the parametric engine (D3a).
        raise CommandCancelledError(
            tr("{name} is a dynamic block: it cannot be edited yet.", name=name)
        )
    ctx.session.open_block_editor(document, name)
    ctx.message(tr("Editing block {name}.", name=name))


def edit_clicked(name: str) -> Command:
    """BEDIT for the block reference double-clicked, as AutoCAD's: the Edit
    Block Definition dialog opens with block ``name`` chosen (ADR 0023 D3,
    maintainer, 2026-10-25)."""
    return Command("BEDIT", lambda ctx: bedit(ctx, name), repeat=False)


def insert_named(
    name: str,
    scale: float,
    rotation: float,
    on_screen: bool,
    explode: bool = False,
    source: CadDocument | None = None,
    at: tuple[float, float] | None = None,
    load: Path | None = None,
    loaded: Callable[[CadDocument], None] | None = None,
) -> Command:
    """INSERT with block ``name`` already chosen, scaled and turned
    (``rotation`` in radians), exploded if asked, as the Blocks palette
    starts it: from its Insert button or a drop on the drawing, at ``at``
    when the drop came first. A block of ``source`` (a template, another
    drawing) is copied into the drawing first, in the same undo step; a
    block whose name the drawing uses for other contents is asked about,
    never resolved silently (ADR 0025 D6). With ``load``, the source is
    that file, read first in the background and given to ``loaded``, as
    for a recent block whose file is not open (D1)."""

    def run(ctx: CommandContext) -> CommandScript:
        document = ctx.document()
        origin = source
        if load is not None:
            app, file = ctx.session.app, load
            try:
                origin = yield ctx.background(
                    lambda: app.open_document(file), tr("Reading {name}…", name=file.name)
                )
            except CadError as error:
                raise CommandCancelledError(
                    tr("{name} cannot be read: {reason}", name=file.name, reason=error)
                ) from None
            if loaded is not None:
                loaded(origin)
        found = _named(origin if origin is not None else document, name)
        if found is None:
            raise CommandCancelledError(tr("There is no such block."))
        choices: dict[str, ClashChoice] = {}
        if origin is not None:
            clashes = block_clashes(document, origin, found)
            if clashes:
                choices = yield from _resolve(ctx, clashes)
        ctx.message(tr("Inserting block {name}.", name=found))
        yield from place_block(
            ctx,
            document,
            found,
            scale,
            rotation,
            on_screen and at is None,
            explode,
            origin,
            choices,
            at,
        )

    return Command("INSERT", run, repeat=False)


def block_clashes(document: CadDocument, source: CadDocument, name: str) -> list[BlockClash]:
    """The blocks copying ``name`` from ``source`` would bring whose names
    ``document`` uses for other contents, ready for the clash dialog."""
    taken = {n.casefold() for n in document.block_names + source.block_names}
    found = []
    for clash in document.block_clashes(name, source):
        here = _named(document, clash) or clash
        suggested = _free_name(clash, taken)
        dynamic = source.is_dynamic_block(clash) or document.is_dynamic_block(here)
        found.append(BlockClash(clash, source, document, here, dynamic, suggested))
    return found


def _free_name(name: str, taken: set[str]) -> str:
    """``name_2``, ``name_3``… the first not ``taken`` (case-folded)."""
    number = 2
    while f"{name}_{number}".casefold() in taken:
        number += 1
    return f"{name}_{number}"


def _resolve(
    ctx: CommandContext, clashes: Sequence[BlockClash]
) -> Generator[Prompt | Background, Any, dict[str, ClashChoice]]:
    """A choice for each clash: from the clash dialog, or asked at the
    command line without one. A dynamic block can only be used as the
    drawing has it (maintainer, 2026-10-25)."""
    if ctx.ui is not None:
        answer = ctx.ui.resolve_block_clashes(clashes)
        if answer is None:
            raise CommandCancelledError()
        return answer
    choices: dict[str, ClashChoice] = {}
    for clash in clashes:
        if clash.dynamic:
            ctx.message(
                tr(
                    "{name} is a dynamic block: the drawing's is used.",
                    name=clash.drawing_name,
                )
            )
            choices[clash.name] = ClashChoice("keep")
            continue
        key = yield ctx.choose(
            tr("The drawing has another block named {name}", name=clash.drawing_name),
            [
                Option("U", tr("Use the drawing's")),
                Option("R", tr("Redefine")),
                Option("C", tr("Copy with another name")),
            ],
            default="U",
        )
        if key == "R":
            choices[clash.name] = ClashChoice("redefine")
        elif key == "C":
            new = (yield ctx.ask(tr("New block name"), default=clash.suggested)).strip()
            choices[clash.name] = ClashChoice("rename", new or clash.suggested)
        else:
            choices[clash.name] = ClashChoice("keep")
    return choices


def load_template(path: Path | None, loaded: Callable[[CadDocument], None]) -> Command:
    """Opens a template for the Blocks palette in the background, as OPEN
    opens a drawing (ADR 0025 D7): ``path``, or a file the user chooses.
    The template is never shown in a tab nor written; ``loaded`` gets it."""

    def run(ctx: CommandContext) -> CommandScript:
        chosen = path
        if chosen is None:
            if ctx.ui is None:
                chosen = Path((yield ctx.ask(tr("Template to load"))))
            else:
                chosen = ctx.ui.choose_template_path()
                if chosen is None:
                    raise CommandCancelledError()
        app = ctx.session.app
        file = chosen
        try:
            template = yield ctx.background(
                lambda: app.open_document(file),
                tr("Loading template {name}…", name=file.name),
            )
        except CadError as error:
            raise CommandCancelledError(
                tr("{name} cannot be loaded as a template: {reason}", name=file.name, reason=error)
            ) from None
        loaded(template)
        count = len(template.block_names)
        ctx.message(tr("Template {name} loaded: {count} blocks.", name=file.name, count=count))

    return Command("BLOCKSPALETTE", run, repeat=False)


def read_files(
    paths: Sequence[Path], read: Callable[[dict[Path, CadDocument | str]], None]
) -> Command:
    """Opens the files of recent blocks for the Blocks palette in the
    background, as a template is (ADR 0025 D1): ``read`` gets each file's
    document, or why it cannot be read. None is shown in a tab nor written."""

    def run(ctx: CommandContext) -> CommandScript:
        app = ctx.session.app

        def task() -> dict[Path, CadDocument | str]:
            found: dict[Path, CadDocument | str] = {}
            for path in paths:
                try:
                    found[path] = app.open_document(path)
                except Exception as error:  # one file's failure, not the others'
                    found[path] = str(error)
            return found

        read((yield ctx.background(task, tr("Reading the recent blocks' files…"))))

    return Command("BLOCKSPALETTE", run, repeat=False)


def edit_named(name: str) -> Command:
    """BEDIT with block ``name`` already chosen, as the Blocks palette
    starts it."""

    def run(ctx: CommandContext) -> CommandScript:
        document = ctx.document()
        if ctx.session.block_editor_of(document) is not None:
            raise CommandCancelledError(tr("Close this block editor before editing another block."))
        found = _named(document, name)
        if found is None:
            raise CommandCancelledError(tr("There is no such block."))
        _edit(ctx, document, found)
        return
        yield

    return Command("BEDIT", run, repeat=False)


def bsave(ctx: CommandContext) -> CommandScript:
    """Saves the block editor into its block, as one undo step of the
    drawing; the editor stays open."""
    yield from save_block(ctx, _editor(ctx))


def bclose(ctx: CommandContext) -> CommandScript:
    """Closes the block editor, asking first whether to save the block."""
    yield from close_block_editor(ctx, _editor(ctx))


def save_block(ctx: CommandContext, editor: CadDocument) -> CommandScript:
    """Puts what ``editor`` holds into its block (also SAVE in an editor)."""
    found = ctx.session.block_editor_of(editor)
    assert found is not None
    document, name = found
    try:
        ctx.session.save_block(editor)
    except InvalidOperationError:
        raise CommandCancelledError(
            tr("{name} cannot hold a reference to itself.", name=name)
        ) from None
    ctx.message(tr("Block {name} saved in {drawing}.", name=name, drawing=document.name))
    return
    yield  # a command script, though it asks nothing


def close_block_editor(ctx: CommandContext, editor: CadDocument) -> CommandScript:
    """Closes ``editor`` (also CLOSE in an editor, or closing its drawing),
    asking first whether to save its changes into the block."""
    if editor.is_modified:
        ctx.session.activate(editor)
        if ctx.ui is not None:
            answer = ctx.ui.ask_save_changes(editor.name)
        else:
            key = yield ctx.choose(
                tr("Save changes to {name}?", name=editor.name),
                [Option("Y", tr("Yes")), Option("N", tr("No"))],
                default="Y",
            )
            answer = "yes" if key == "Y" else "no"
        if answer == "cancel":
            raise CommandCancelledError()
        if answer == "yes":
            yield from save_block(ctx, editor)
    name = editor.name
    ctx.session.close(editor)
    ctx.message(tr("Closed {name}", name=name))


def _editor(ctx: CommandContext) -> CadDocument:
    """The active document, which must be a block editor."""
    document = ctx.document()
    if ctx.session.block_editor_of(document) is None:
        raise CommandCancelledError(tr("This is not a block editor."))
    return document


def _copied(
    ctx: CommandContext,
    document: CadDocument,
    name: str,
    source: CadDocument | None,
    choices: dict[str, ClashChoice] | None = None,
) -> str:
    """Block ``name`` in the drawing: copied from ``source`` (a template,
    another drawing) first, when there is one, as ``choices`` resolve its
    clashes, within the INSERT group (ADR 0025 D6)."""
    if source is None:
        return name
    chosen = choices or {}
    try:
        copied, changes = document.copy_block(
            name,
            source,
            keep=[n for n, c in chosen.items() if c.action == "keep"],
            redefine=[n for n, c in chosen.items() if c.action == "redefine"],
            rename={n: c.name for n, c in chosen.items() if c.action == "rename"},
        )
    except CadError as error:
        raise CommandCancelledError(
            tr(
                "{name} cannot be copied from {source}: {reason}",
                name=name,
                source=source.name,
                reason=error,
            )
        ) from None
    ctx.session.report(document, changes)
    return copied


def _named(document: CadDocument, name: str) -> str | None:
    """The block called ``name``, ignoring case, as the drawing spells it."""
    return next((n for n in document.block_names if n.lower() == name.lower()), None)


def _number(
    ctx: CommandContext, message: str, default: float, positive: bool = False
) -> Generator[Prompt | Background, Any, float]:
    """A number typed at the command line, ``default`` on Enter."""
    while True:
        typed = yield ctx.ask(message, default=f"{default:.10g}")
        try:
            value = float(typed)
        except ValueError:
            ctx.message(tr("{text} is not a number.", text=typed))
            continue
        if not math.isfinite(value) or (positive and value <= 0.0):
            ctx.message(tr("The value must be a number greater than zero."))
            continue
        return value


#: Block commands, with AutoCAD's aliases.
BLOCK_COMMANDS = (
    Command("BLOCK", block, ("B",)),
    Command("INSERT", insert, ("I",)),
    Command("BEDIT", bedit, ("BE",)),
    Command("BSAVE", bsave),
    Command("BCLOSE", bclose),
)
