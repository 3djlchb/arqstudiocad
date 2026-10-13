"""The properties panel (ADR 0013, D5.6), a property grid after
OpenCADStudio's (ADR 0015).

Until selection exists (milestone 2.5) it describes the active drawing, in
sections: General (name, file, format, unsaved changes), Contents (entity
and layer counts, current layer) and Geometry (extents).
"""

from PySide6.QtWidgets import QDockWidget, QWidget

from arqstudiocad.core import CadDocument
from arqstudiocad.events import Event
from arqstudiocad.i18n import tr
from arqstudiocad.session import Session
from arqstudiocad.ui.event_bridge import QtEventBridge
from arqstudiocad.ui.property_grid import PropertyGrid, Section

#: The AutoCAD release that introduced each drawing format.
RELEASES = {
    "AC1009": "R12",
    "AC1012": "R13",
    "AC1014": "R14",
    "AC1015": "2000",
    "AC1018": "2004",
    "AC1021": "2007",
    "AC1024": "2010",
    "AC1027": "2013",
    "AC1032": "2018",
}


def version_text(code: str) -> str:
    """``AC1032`` → ``AutoCAD 2018 (AC1032)``; an unknown code as it is."""
    release = RELEASES.get(code)
    return code if release is None else tr("AutoCAD {release} ({code})", release=release, code=code)


def drawing_properties(document: CadDocument) -> list[Section]:
    """The sections the panel shows for ``document``."""
    bounds = document.get_bounds()
    extents = "—" if bounds is None else f"{bounds.width:.4f} x {bounds.height:.4f}"
    path = document.path
    return [
        Section(
            "general",
            tr("General"),
            [
                (tr("Name"), document.name),
                (tr("File"), tr("(not saved)") if path is None else str(path)),
                (tr("Format"), version_text(document.version)),
                (tr("Unsaved changes"), tr("Yes") if document.is_modified else tr("No")),
            ],
        ),
        Section(
            "contents",
            tr("Contents"),
            [
                (tr("Entities"), str(document.entity_count)),
                (tr("Layers"), str(len(document.get_layers()))),
                (tr("Current layer"), document.current_layer.name),
            ],
        ),
        Section("geometry", tr("Geometry"), [(tr("Extents"), extents)]),
    ]


class PropertiesPanel(QDockWidget):
    """What the active drawing is, as a property grid."""

    def __init__(
        self, session: Session, events: QtEventBridge, parent: QWidget | None = None
    ) -> None:
        super().__init__(parent)
        self.setObjectName("properties_panel")
        self._session = session
        self.grid = PropertyGrid()
        self.setWidget(self.grid)
        for signal in (
            events.active_document_changed,
            events.document_changed,
            events.document_saved,
        ):
            signal.connect(self.refresh)
        self.retranslate()

    def retranslate(self) -> None:
        self.setWindowTitle(tr("Properties"))
        self.refresh()

    def refresh(self, event: Event | None = None) -> None:
        document = self._session.active
        if document is None or not document.is_open:
            self.grid.show_sections(tr("No drawing open."), [])
        else:
            self.grid.show_sections(tr("Drawing"), drawing_properties(document))

    def rows(self) -> list[tuple[str, str]]:
        """Every (name, value) the panel shows, for tests."""
        return [row for _title, rows in self.grid.sections() for row in rows]
