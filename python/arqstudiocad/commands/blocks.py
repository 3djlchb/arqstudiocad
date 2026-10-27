"""BLOCK and INSERT (ADR 0023 D2): make a block from objects selected, and
place a block, following the cursor until its insertion point is picked.
BEDIT, BSAVE and BCLOSE (D3): edit a block's definition alone, in a block
editor of its own, and save it back into the drawing. The Blocks palette
starts INSERT and BEDIT with the block already chosen (ADR 0025 D5), loads
templates and inserts their blocks, copying each first (D6, D7)."""

import fnmatch
import math
from collections.abc import Callable, Generator, Sequence
from dataclasses import replace
from pathlib import Path
from typing import Any

from arqstudiocad.commands import (
    Background,
    BlockClash,
    BlockDefinitionState,
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
    """BLOCK: the Block Definition dialog (ADR 0023 D2a), as AutoCAD's;
    without an interface, -BLOCK's prompts. The dialog closes to pick the
    base point or select objects and opens again; a name the drawing uses
    is offered to be redefined. One undo step."""
    if ctx.ui is None:
        yield from block_prompts(ctx)
        return
    document = ctx.document()
    template = ctx.session.template
    state = BlockDefinitionState(
        ids=ctx.session.selection(document), units=document.insertion_units
    )
    while True:
        answer = ctx.ui.define_block(document, state)
        if answer is None:
            raise CommandCancelledError()
        action, state = answer
        if action == "pick_point":
            state = replace(state, base=(yield ctx.point(tr("Specify insertion base point"))))
            continue
        if action == "select_objects":
            ctx.session.clear_selection(document)
            state = replace(state, ids=(yield from select_objects(ctx, document)))
            continue
        if state.base_on_screen:
            state = replace(state, base=(yield ctx.point(tr("Specify insertion base point"))))
        if state.objects_on_screen:
            ctx.session.clear_selection(document)
            state = replace(state, ids=(yield from select_objects(ctx, document)))
        if not state.ids:
            ctx.message(tr("No objects are selected."))
            continue
        name = state.name.strip()
        taken = _named(document, name)
        if taken is not None:
            references = document.block_references.get(taken, 0)
            if not ctx.ui.ask_redefine_block(taken, references):
                continue
            name = taken
        elif template is not None and template.is_open:
            found = _named(template, name)
            if found is not None and not ctx.ui.ask_block_name_in_template(found, template.name):
                continue
        try:
            _, changes = document.define_block(
                name,
                state.base,
                state.ids,
                objects=state.objects or "convert",
                description=state.description.strip(),
                units=state.units,
                scale_uniformly=state.scale_uniformly,
                explodable=state.explodable,
                redefine=taken is not None,
            )
        except InvalidOperationError as error:
            ctx.message(tr("{name} cannot be made: {reason}", name=name, reason=error))
            continue
        ctx.session.report(document, changes)
        ctx.session.clear_selection(document)
        ctx.session.block_used(document, name, CREATED)
        if taken is not None:
            ctx.message(
                tr("Block {name} redefined from {count} objects.", name=name, count=len(state.ids))
            )
        else:
            ctx.message(
                tr("Block {name} made from {count} objects.", name=name, count=len(state.ids))
            )
        if state.open_editor:
            _edit(ctx, document, name)
        return


def block_prompts(ctx: CommandContext) -> CommandScript:
    """-BLOCK: a block named and based at a point, made from objects
    selected, asked at the command line; a reference to it takes their
    place (AutoCAD's "Convert to block"). A name in use is refused."""
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


def rename(ctx: CommandContext, chosen: str | None = None) -> CommandScript:
    """RENAME: a block of the drawing gets another name, every reference
    following (ADR 0025 step 8c), as chosen in the Rename dialog with
    ``chosen`` selected first. Only blocks are renamed for now."""
    if ctx.ui is None:
        yield from rename_prompts(ctx)
        return
    document = ctx.document()
    if not document.block_names:
        raise CommandCancelledError(tr("The drawing has no blocks to rename."))
    while True:
        answer = ctx.ui.rename_block(document, chosen)
        if answer is None:
            raise CommandCancelledError()
        old, new = answer
        problem = _renamed(ctx, document, old, new)
        if problem is None:
            return
        # Said, then the dialog again with that block.
        ctx.message(problem)
        chosen = old


def rename_prompts(ctx: CommandContext) -> CommandScript:
    """-RENAME: the kind of object, its name and the new one, asked at the
    command line as AutoCAD's; only blocks for now."""
    document = ctx.document()
    yield ctx.choose(tr("Object type to rename"), [Option("B", tr("Block"))], default="B")
    while (old := _named(document, (yield ctx.ask(tr("Old block name"))).strip())) is None:
        ctx.message(tr("There is no such block."))
    problem = _not_renamable(ctx, document, old)
    if problem is not None:
        raise CommandCancelledError(problem)
    while True:
        new = (yield ctx.ask(tr("New block name"))).strip()
        problem = _renamed(ctx, document, old, new)
        if problem is None:
            return
        ctx.message(problem)


def rename_named(name: str) -> Command:
    """RENAME with block ``name`` chosen first, as the Blocks palette's
    context menu starts it."""
    return Command("RENAME", lambda ctx: rename(ctx, name), repeat=False)


def _not_renamable(ctx: CommandContext, document: CadDocument, name: str) -> str | None:
    """Why block ``name`` of ``document`` cannot be renamed now; None if it
    can."""
    if document.is_dynamic_block(name):
        # Its anonymous references wait for the parametric engine (04.1).
        return tr("{name} is a dynamic block: it cannot be renamed yet.", name=name)
    for editor in ctx.session.block_editors(document):
        edited = ctx.session.block_editor_of(editor)
        if edited is not None and edited[1].lower() == name.lower():
            return tr("Close the block editor of {name} before renaming it.", name=name)
    return None


def _renamed(ctx: CommandContext, document: CadDocument, old: str, new: str) -> str | None:
    """Renames block ``old`` of ``document`` to ``new``, saying so; why not,
    if it cannot be."""
    found = _named(document, old)
    if found is None:
        return tr("There is no such block.")
    problem = _not_renamable(ctx, document, found)
    if problem is not None:
        return problem
    taken = _named(document, new)
    if taken is not None and taken.lower() != found.lower():
        return tr("A block named {name} exists in the drawing.", name=taken)
    if new == found:
        return tr("{name} is already called so.", name=found)
    try:
        ctx.session.rename_block(document, found, new)
    except InvalidOperationError:
        return tr("{name} is not a valid block name.", name=new)
    ctx.message(tr("Block {old} renamed to {new}.", old=found, new=new))
    return None


def selectblock(ctx: CommandContext) -> CommandScript:
    """SELECTBLOCK: selects the references to a block placed in the
    drawing, replacing the selection; the view stays where it is (ADR 0025
    step 8d, maintainer 2026-10-26). Asks the block's name."""
    document = ctx.document()
    if not document.block_names:
        raise CommandCancelledError(tr("The drawing has no blocks."))
    while (name := _named(document, (yield ctx.ask(tr("Block name"))).strip())) is None:
        ctx.message(tr("There is no such block."))
    _select_references(ctx, document, name)


def select_named(name: str) -> Command:
    """SELECTBLOCK with block ``name`` already chosen, as the Blocks
    palette's context menu starts it."""

    def run(ctx: CommandContext) -> CommandScript:
        document = ctx.document()
        found = _named(document, name)
        if found is None:
            raise CommandCancelledError(tr("There is no such block."))
        _select_references(ctx, document, found)
        return
        yield

    return Command("SELECTBLOCK", run, repeat=False)


def _select_references(ctx: CommandContext, document: CadDocument, name: str) -> None:
    """Selects block ``name``'s references in model space, alone, and says
    how many others could not be: those that draw nothing, those inside
    other blocks (selected as a whole) and those in layouts."""
    shown, hidden, nested, in_layouts = document.find_block_references(name)
    ctx.session.clear_selection(document)
    ctx.session.select(document, shown)
    if len(shown) == 1:
        ctx.message(tr("1 reference to {name} selected.", name=name))
    else:
        ctx.message(tr("{count} references to {name} selected.", count=len(shown), name=name))
    if hidden:
        ctx.message(tr("Not selected: {count} on frozen or off layers.", count=hidden))
    if nested:
        ctx.message(tr("Not selected: {count} inside other blocks.", count=nested))
    if in_layouts:
        ctx.message(tr("Not selected: {count} in layouts.", count=in_layouts))


def purge(ctx: CommandContext) -> CommandScript:
    """PURGE: unused blocks chosen in the Purge dialog are purged, nested
    items too when asked (ADR 0025 step 8e, maintainer 2026-10-26). Only
    blocks for now."""
    if ctx.ui is None:
        yield from purge_prompts(ctx)
        return
    document = ctx.document()
    if not document.unused_blocks:
        raise CommandCancelledError(tr("No unused blocks to purge."))
    answer = ctx.ui.purge_blocks(document)
    if answer is None:
        raise CommandCancelledError()
    names, nested = answer
    _purge(ctx, document, names, nested)


def purge_prompts(ctx: CommandContext) -> CommandScript:
    """-PURGE: the kind of objects, the names (wildcards allowed, ``*``
    every one) and whether to purge nested items, asked at the command line
    as AutoCAD's; only blocks for now."""
    document = ctx.document()
    yield ctx.choose(
        tr("Type of unused objects to purge"), [Option("B", tr("Blocks"))], default="B"
    )
    typed = (yield ctx.ask(tr("Names to purge"), default="*")).strip() or "*"
    nested = yield ctx.choose(
        tr("Purge nested items?"), [Option("Y", tr("Yes")), Option("N", tr("No"))], default="Y"
    )
    patterns = [p.strip().casefold() for p in typed.split(",") if p.strip()]
    found = [
        n
        for n in document.unused_blocks
        if any(fnmatch.fnmatchcase(n.casefold(), p) for p in patterns)
    ]
    dynamic = [n for n in found if document.is_dynamic_block(n)]
    if dynamic:
        ctx.message(tr("Not purged: {names} (dynamic blocks).", names=", ".join(dynamic)))
    names = [n for n in found if n not in dynamic]
    if not names:
        raise CommandCancelledError(tr("No unused blocks to purge."))
    _purge(ctx, document, names, nested == "Y")


def purge_named(name: str) -> Command:
    """PURGE of block ``name`` alone, nested items included, once confirmed,
    as the Blocks palette's context menu starts it."""

    def run(ctx: CommandContext) -> CommandScript:
        document = ctx.document()
        found = _named(document, name)
        if found is None:
            raise CommandCancelledError(tr("There is no such block."))
        if document.is_dynamic_block(found):
            raise CommandCancelledError(
                tr("{name} is a dynamic block: it cannot be purged yet.", name=found)
            )
        if found not in document.unused_blocks:
            raise CommandCancelledError(tr("{name} is in use: it cannot be purged.", name=found))
        if ctx.ui is not None and not ctx.ui.ask_purge_block(found):
            raise CommandCancelledError()
        _purge(ctx, document, [found], nested=True)
        return
        yield

    return Command("PURGE", run, repeat=False)


def _purge(ctx: CommandContext, document: CadDocument, names: list[str], nested: bool) -> None:
    """Purges blocks ``names`` of ``document`` in one undo step, saying which
    went; a block open in a block editor stays."""
    edited = {
        found[1].lower()
        for editor in ctx.session.block_editors(document)
        if (found := ctx.session.block_editor_of(editor)) is not None
    }
    kept = [n for n in names if n.lower() in edited]
    if kept:
        ctx.message(tr("Not purged: {names} (open in a block editor).", names=", ".join(kept)))
    names = [n for n in names if n.lower() not in edited]
    if not names:
        raise CommandCancelledError()
    try:
        purged, changes = document.purge_blocks(names, nested)
    except InvalidOperationError as error:
        raise CommandCancelledError(tr("Nothing was purged: {reason}", reason=error)) from None
    ctx.session.report(document, changes)
    if len(purged) == 1:
        ctx.message(tr("1 block purged: {names}.", names=purged[0]))
    else:
        ctx.message(
            tr("{count} blocks purged: {names}.", count=len(purged), names=", ".join(purged))
        )


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
            origin = yield from _read(ctx, load, loaded)
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


def copyblock(ctx: CommandContext) -> CommandScript:
    """COPYBLOCK: copies a block of the Blocks palette's template into the
    drawing without inserting it, with the blocks, layers and styles it
    needs (ADR 0025 D6, step 8b). One undo step."""
    document = ctx.document()
    template = ctx.session.template
    if template is None or not template.is_open:
        raise CommandCancelledError(tr("No template is loaded."))
    while (name := _named(template, (yield ctx.ask(tr("Block name"))).strip())) is None:
        ctx.message(tr("There is no such block."))
    yield from copy_into(ctx, document, template, name)


def copy_named(
    name: str,
    source: CadDocument | None,
    load: Path | None = None,
    loaded: Callable[[CadDocument], None] | None = None,
    redefine: bool = False,
) -> Command:
    """COPYBLOCK with block ``name`` of ``source`` already chosen, as the
    Blocks palette's context menu starts it (Copy to drawing, Redefine from
    template with ``redefine``); with ``load``, the source is that file,
    read first, as for a recent block whose file is not open."""

    def run(ctx: CommandContext) -> CommandScript:
        document = ctx.document()
        origin = source
        if load is not None:
            origin = yield from _read(ctx, load, loaded)
        found = _named(origin, name) if origin is not None else None
        if origin is None or found is None:
            raise CommandCancelledError(tr("There is no such block."))
        yield from copy_into(ctx, document, origin, found, redefine)

    return Command("COPYBLOCK", run, repeat=False)


def copy_into(
    ctx: CommandContext,
    document: CadDocument,
    source: CadDocument,
    name: str,
    redefine: bool = False,
) -> CommandScript:
    """Copies block ``name`` of ``source`` into ``document``, asking about
    the clashes, or redefining the drawing's block of that name with
    ``redefine`` (the other clashes still asked); one undo step."""
    clashes = block_clashes(document, source, name)
    if _named(document, name) is not None and all(c.name != name for c in clashes):
        # The same block: the drawing's is used, nothing to copy (D6).
        ctx.message(tr("The drawing already has block {name}.", name=name))
        return
    choices: dict[str, ClashChoice] = {}
    if redefine:
        choices = {c.name: ClashChoice("redefine") for c in clashes if c.name == name}
    asked = [c for c in clashes if c.name not in choices]
    if asked:
        choices.update((yield from _resolve(ctx, asked)))
    document.begin_group("COPYBLOCK")
    try:
        copied = _copied(ctx, document, name, source, choices)
    except BaseException:
        ctx.session.report(document, document.cancel_group())
        raise
    document.end_group()
    if redefine:
        ctx.message(tr("Block {name} redefined from {source}.", name=copied, source=source.name))
    else:
        ctx.message(tr("Block {name} copied to the drawing.", name=copied))


def _read(
    ctx: CommandContext, path: Path, loaded: Callable[[CadDocument], None] | None
) -> Generator[Prompt | Background, Any, CadDocument]:
    """The file at ``path``, read in the background for the Blocks palette
    and given to ``loaded``; never in a tab nor written."""
    app = ctx.session.app
    try:
        document: CadDocument = yield ctx.background(
            lambda: app.open_document(path), tr("Reading {name}…", name=path.name)
        )
    except CadError as error:
        raise CommandCancelledError(
            tr("{name} cannot be read: {reason}", name=path.name, reason=error)
        ) from None
    if loaded is not None:
        loaded(document)
    return document


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
    Command("-BLOCK", block_prompts, ("-B",)),
    Command("INSERT", insert, ("I",)),
    Command("BEDIT", bedit, ("BE",)),
    Command("BSAVE", bsave),
    Command("BCLOSE", bclose),
    Command("COPYBLOCK", copyblock),
    Command("RENAME", rename, ("REN",)),
    Command("-RENAME", rename_prompts, ("-REN",)),
    Command("SELECTBLOCK", selectblock),
    Command("PURGE", purge, ("PU",)),
    Command("-PURGE", purge_prompts, ("-PU",)),
)
