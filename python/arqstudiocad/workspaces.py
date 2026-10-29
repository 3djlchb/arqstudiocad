"""Workspaces: a discipline's tools shown in a layout (ADR 0015).

A discipline is data: its ribbon tabs, shown by the Ribbon layout, and, for
2D drafting only so far, its toolbars, shown by the Classic layout; each
lists command names. The window turns the names into the one action of each
command (ADR 0013, D5.1).
Titles are functions calling ``tr()``, so they follow the language. No Qt
here: discipline plugins declare theirs the same way.
"""

from collections.abc import Callable, Iterator
from dataclasses import dataclass
from typing import Literal, get_args

from arqstudiocad.i18n import tr

Layout = Literal["ribbon", "classic"]
#: The layouts, the first one the default (ADR 0015: Ribbon on first run).
LAYOUTS: tuple[Layout, ...] = get_args(Layout)

#: A title in the interface language, asked for whenever it is shown.
Title = Callable[[], str]


@dataclass(frozen=True)
class Toolbar:
    """A Classic toolbar; ``None`` in ``commands`` is a separator."""

    name: str
    title: Title
    commands: tuple[str | None, ...]


@dataclass(frozen=True)
class RibbonGroup:
    """Buttons with a title underneath, inside a ribbon tab."""

    name: str
    title: Title
    commands: tuple[str, ...]


#: The context of the block editor's contextual ribbon tab.
BLOCK_EDITOR = "block_editor"


@dataclass(frozen=True)
class RibbonTab:
    name: str
    title: Title
    groups: tuple[RibbonGroup, ...]
    #: A contextual tab (AutoCAD's) shows only in this context, e.g. while a
    #: block editor is the active document; ``None``: always shown.
    context: str | None = None


@dataclass(frozen=True)
class Discipline:
    """The tools of one discipline (2D drafting, BIM architecture…).

    Every discipline has a ribbon; only those with toolbars (2D drafting)
    can also be shown in the Classic layout (ADR 0015)."""

    name: str
    title: Title
    ribbon: tuple[RibbonTab, ...]
    toolbars: tuple[Toolbar, ...] = ()

    def __post_init__(self) -> None:
        if not self.ribbon:
            raise ValueError(f"discipline {self.name!r} has no ribbon")

    @property
    def layouts(self) -> tuple[Layout, ...]:
        """The layouts it can be shown in, the default first."""
        return LAYOUTS if self.toolbars else ("ribbon",)

    def commands(self) -> set[str]:
        """Every command the discipline shows, in either layout."""
        names = {name for toolbar in self.toolbars for name in toolbar.commands if name}
        for tab in self.ribbon:
            for group in tab.groups:
                names.update(group.commands)
        return names


def layout_title(layout: Layout) -> str:
    return tr("Ribbon") if layout == "ribbon" else tr("Classic")


class WorkspaceRegistry:
    """The disciplines that can be chosen, in the order registered."""

    def __init__(self) -> None:
        self._disciplines: dict[str, Discipline] = {}

    def register(self, discipline: Discipline) -> None:
        if discipline.name in self._disciplines:
            raise ValueError(f"discipline {discipline.name!r} is already registered")
        self._disciplines[discipline.name] = discipline

    def get(self, name: str) -> Discipline | None:
        return self._disciplines.get(name)

    def __iter__(self) -> Iterator[Discipline]:
        return iter(self._disciplines.values())

    def __len__(self) -> int:
        return len(self._disciplines)


#: 2D drafting, the discipline of milestone 2.4.
CAD = Discipline(
    name="cad",
    title=lambda: tr("2D drafting"),
    ribbon=(
        RibbonTab(
            "home",
            lambda: tr("Home"),
            (
                RibbonGroup("file", lambda: tr("File"), ("NEW", "OPEN", "SAVE", "SAVEAS", "CLOSE")),
                RibbonGroup("draw", lambda: tr("Draw"), ("LINE", "PLINE", "CIRCLE", "ARC")),
                RibbonGroup(
                    "modify",
                    lambda: tr("Modify"),
                    ("MOVE", "COPY", "ROTATE", "SCALE", "MIRROR", "ERASE", "EXPLODE"),
                ),
                RibbonGroup("history", lambda: tr("History"), ("UNDO", "REDO")),
                RibbonGroup("layers", lambda: tr("Layers"), ("LAYER",)),
            ),
        ),
        # Blocks, as AutoCAD's Insert tab (ADR 0023 D4).
        RibbonTab(
            "insert",
            lambda: tr("Insert"),
            (RibbonGroup("block", lambda: tr("Block"), ("INSERT", "BLOCK", "BEDIT")),),
        ),
        # Annotation tools, as AutoCAD's Annotate tab: texts first; dimensions,
        # leaders and tables join them in phase 04.
        RibbonTab(
            "annotate",
            lambda: tr("Annotate"),
            (RibbonGroup("text", lambda: tr("Text"), ("MTEXT", "TEXT")),),
        ),
        RibbonTab(
            "view",
            lambda: tr("View"),
            (
                RibbonGroup("navigate", lambda: tr("Navigate"), ("ZOOM",)),
                # Palettes, each button pressed while its palette is open (ADR 0025 D2).
                RibbonGroup(
                    "palettes",
                    lambda: tr("Palettes"),
                    ("BLOCKSPALETTE", "LAYER", "PROPERTIES", "BUDGET"),
                ),
            ),
        ),
        # AutoCAD's contextual Block Editor tab: only while a block editor
        # is the active document (ADR 0023 D3).
        RibbonTab(
            "block_editor",
            lambda: tr("Block Editor"),
            (RibbonGroup("block_editor", lambda: tr("Block Editor"), ("BSAVE", "BCLOSE")),),
            context=BLOCK_EDITOR,
        ),
    ),
    toolbars=(
        Toolbar(
            "standard",
            lambda: tr("Standard"),
            ("NEW", "OPEN", "SAVE", None, "UNDO", "REDO", None, "ZOOM"),
        ),
        Toolbar(
            "draw",
            lambda: tr("Draw"),
            (
                "LINE",
                "PLINE",
                "CIRCLE",
                "ARC",
                None,
                "INSERT",
                "BLOCK",
                "BEDIT",
                None,
                "MTEXT",
                "TEXT",
            ),
        ),
        Toolbar(
            "modify",
            lambda: tr("Modify"),
            ("ERASE", "COPY", "MIRROR", None, "MOVE", "ROTATE", "SCALE", None, "EXPLODE"),
        ),
        Toolbar("layers", lambda: tr("Layers"), ("LAYER",)),
    ),
)


def builtin_workspaces() -> WorkspaceRegistry:
    registry = WorkspaceRegistry()
    registry.register(CAD)
    return registry
