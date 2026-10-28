"""Budget data (ADR 0027): CURRENCIES edits the drawing's register of
currencies, the ones blocks' costs may use, in the Currencies dialog. The
block data commands come in step 4."""

from arqstudiocad.commands import Command, CommandCancelledError, CommandContext, CommandScript
from arqstudiocad.core import InvalidOperationError
from arqstudiocad.currencies import shown_name
from arqstudiocad.i18n import tr


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
    answer = ctx.ui.edit_currencies(document)
    if answer is None:
        raise CommandCancelledError()
    try:
        changes = document.set_currencies(*answer)
    except InvalidOperationError as error:
        raise CommandCancelledError(
            tr("The currencies were not changed: {reason}", reason=error)
        ) from None
    if changes.document_changed:
        ctx.session.report(document, changes)
        ctx.message(tr("Currencies changed."))
    yield from ()


#: Budget data commands.
BUDGET_COMMANDS = (Command("CURRENCIES", currencies),)
