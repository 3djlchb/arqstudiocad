"""The 3D commands of milestone 02.8 (ADR 0024), in the ribbon's 3D tab:
MASSPROP so far.

MASSPROP reports the volume, surface area, bounding box and centroid of the
solids selected, together, as AutoCAD's MASSPROP reports a selection;
block references count the solids inside them (maintainer, 2026-11-01).
Moments of inertia, and writing the report to a file, come later. The
figures show every digit the kernel guarantees: precision is lowered only
for files read from or written for another application (maintainer,
2026-11-01).
"""

import math
from collections.abc import Callable

from arqstudiocad.commands import Command, CommandCancelledError, CommandContext, CommandScript
from arqstudiocad.commands.modify import done_selecting, select_objects
from arqstudiocad.core import CadMassProperties, CadSolidMeasures
from arqstudiocad.i18n import tr

#: Significant digits shown: the kernel refines each measure until it
#: settles to 1e-11 (ADR 0024 step 3), so ten hold.
DIGITS = 10
#: Significant digits a double holds.
DOUBLE_DIGITS = 15


def refusals() -> dict[str, Callable[[int], str]]:
    """Why entities were not measured, by the API's reason, in the order
    said. Written as literal tr() calls so the translation test sees every
    one."""
    return {
        "not_a_solid": lambda n: tr("{count} were not solids.", count=n),
        "untranslated": lambda n: tr("{count} solids cannot be read yet.", count=n),
        "empty": lambda n: tr("{count} solids have no faces.", count=n),
        "not_closed": lambda n: tr("{count} do not close around a volume.", count=n),
        "unsupported": lambda n: tr("{count} have faces that cannot be measured yet.", count=n),
        "not_converged": lambda n: tr("{count} could not be measured precisely.", count=n),
        "uneven_scale": lambda n: tr("{count} block references are scaled unevenly.", count=n),
    }


def massprop(ctx: CommandContext) -> CommandScript:
    """MASSPROP, AutoCAD's: the mass properties of the solids selected."""
    document = ctx.document()
    ids = yield from select_objects(ctx, document, edits=False)
    if not ids:
        return
    measures: CadSolidMeasures = yield ctx.background(
        lambda: document.measure_solids(ids), tr("Measuring solids…")
    )
    done_selecting(ctx, document, ids)
    reasons = [why for _, why, _ in measures.refused]
    for why, said in refusals().items():
        if count := reasons.count(why):
            ctx.message(said(count))
    if measures.total is None:
        raise CommandCancelledError(tr("No solid could be measured."))
    for line in report(measures.total, measures.solids):
        ctx.message(line)


def report(total: CadMassProperties, solids: int) -> list[str]:
    """The lines of MASSPROP's report, its labels aligned."""
    low, high = total.bounds
    size = max(b - a for a, b in zip(low, high, strict=True))
    largest = max(abs(c) for c in (*low, *high))
    # A coordinate's digits count across the solids, not from the origin.
    length, area, volume = _places(size, largest), _places(total.area), _places(total.volume)
    labels = {
        "solids": tr("Solids:"),
        "volume": tr("Volume:"),
        "area": tr("Surface area:"),
        "bounds": tr("Bounding box:"),
        "centroid": tr("Centroid:"),
    }
    width = max(len(label) for label in labels.values()) + 2

    def row(label: str, value: str) -> str:
        return label.ljust(width) + value

    lines = [
        f"{'-' * 16}   {tr('SOLIDS')}   {'-' * 16}",
        row(labels["solids"], str(solids)),
        row(labels["volume"], _fixed(total.volume, volume)),
        row(labels["area"], _fixed(total.area, area)),
    ]
    for index, axis in enumerate("XYZ"):
        span = f"{axis}: {_fixed(low[index], length)}  --  {_fixed(high[index], length)}"
        lines.append(row(labels["bounds"] if index == 0 else "", span))
    for index, axis in enumerate("XYZ"):
        at = f"{axis}: {_fixed(total.centroid[index], length)}"
        lines.append(row(labels["centroid"] if index == 0 else "", at))
    return lines


def _places(scale: float, largest: float = 0.0) -> int:
    """Decimal places that show ``DIGITS`` significant digits of
    ``scale``, but no more than a double keeps of ``largest`` (a coordinate
    far from the origin)."""
    places = DIGITS - 1 - math.floor(math.log10(scale)) if scale > 0.0 else DIGITS
    if largest > 0.0:
        places = min(places, DOUBLE_DIGITS - 1 - math.floor(math.log10(largest)))
    return max(0, places)


def _fixed(value: float, places: int) -> str:
    # Adding zero turns a rounded -0.0 into 0.0.
    return f"{round(value, places) + 0.0:.{places}f}"


#: The 3D commands.
SOLID_COMMANDS = (Command("MASSPROP", massprop),)

__all__ = ["SOLID_COMMANDS", "report"]
