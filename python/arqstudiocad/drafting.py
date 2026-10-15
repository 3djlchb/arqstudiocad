"""How points are picked: running object snaps, ORTHO, the cursor
(ADR 0016 D5). No Qt: the status bar, the viewport and the commands share
it through the session, and every change is a ``DraftingChanged`` event.
"""

from arqstudiocad.events import DraftingChanged, EventBus
from arqstudiocad.points import DEFAULT_SNAPS, SNAP_KEYS, Point


class Drafting:
    """The drafting settings of one window."""

    def __init__(self, bus: EventBus) -> None:
        self._bus = bus
        self._osnap = True
        self._snaps = DEFAULT_SNAPS
        self._ortho = False
        #: Where the cursor is, in world coordinates: the direction of a
        #: distance typed alone (direct distance entry).
        self.cursor: Point | None = None
        #: A snap typed at a point prompt (``END``…): it alone applies to the
        #: next point, whatever the running snaps.
        self.override: str | None = None

    @property
    def osnap(self) -> bool:
        """Whether the running snaps are on (F3)."""
        return self._osnap

    @property
    def snaps(self) -> tuple[str, ...]:
        """The running snaps chosen, by AutoCAD's key, in AutoCAD's order."""
        return tuple(k for k in SNAP_KEYS if k in self._snaps)

    @property
    def ortho(self) -> bool:
        """Whether ORTHO is on (F8)."""
        return self._ortho

    def active_snaps(self) -> tuple[str, ...]:
        """The snaps that apply to the next point: the typed one, else the
        running ones when they are on, else none."""
        if self.override is not None:
            return (self.override,)
        return self.snaps if self._osnap else ()

    def set_osnap(self, on: bool) -> None:
        if on != self._osnap:
            self._osnap = on
            self._publish()

    def set_snaps(self, keys: set[str] | frozenset[str]) -> None:
        """Chooses the running snaps; unknown keys are an error."""
        unknown = set(keys) - set(SNAP_KEYS)
        if unknown:
            raise ValueError(f"unknown object snaps: {sorted(unknown)}")
        if frozenset(keys) != self._snaps:
            self._snaps = frozenset(keys)
            self._publish()

    def set_ortho(self, on: bool) -> None:
        if on != self._ortho:
            self._ortho = on
            self._publish()

    def _publish(self) -> None:
        self._bus.publish(DraftingChanged(self._osnap, self.snaps, self._ortho))
