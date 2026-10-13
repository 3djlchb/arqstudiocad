"""The properties panel (ADR 0013, D5.6).

Until selection exists (milestone 2.5) it describes the active drawing: name,
file, format version, entity and layer counts, current layer, extents and
whether it has unsaved changes.
"""

from PySide6.QtWidgets import QDockWidget, QFormLayout, QLabel, QWidget

from arqstudiocad.core import CadDocument
from arqstudiocad.events import Event
from arqstudiocad.i18n import tr
from arqstudiocad.session import Session
from arqstudiocad.ui.event_bridge import QtEventBridge

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


def drawing_properties(document: CadDocument) -> list[tuple[str, str]]:
    """The rows the panel shows for ``document``, as (label, value)."""
    bounds = document.get_bounds()
    extents = "—" if bounds is None else f"{bounds.width:.4f} x {bounds.height:.4f}"
    path = document.path
    return [
        (tr("Name"), document.name),
        (tr("File"), tr("(not saved)") if path is None else str(path)),
        (tr("Format"), version_text(document.version)),
        (tr("Entities"), str(document.entity_count)),
        (tr("Layers"), str(len(document.get_layers()))),
        (tr("Current layer"), document.current_layer.name),
        (tr("Extents"), extents),
        (tr("Unsaved changes"), tr("Yes") if document.is_modified else tr("No")),
    ]


class PropertiesPanel(QDockWidget):
    """What the active drawing is."""

    def __init__(
        self, session: Session, events: QtEventBridge, parent: QWidget | None = None
    ) -> None:
        super().__init__(parent)
        self.setObjectName("properties_panel")
        self._session = session
        self._form = QFormLayout()
        body = QWidget()
        body.setLayout(self._form)
        self.setWidget(body)
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
        while self._form.rowCount():
            self._form.removeRow(0)
        document = self._session.active
        if document is None or not document.is_open:
            self._form.addRow(QLabel(tr("No drawing open.")))
            return
        for label, value in drawing_properties(document):
            shown = QLabel(value)
            shown.setWordWrap(True)
            self._form.addRow(label, shown)

    def rows(self) -> list[tuple[str, str]]:
        """What the panel shows, for tests: (label, value); a lone message has
        an empty label."""
        shown: list[tuple[str, str]] = []
        for row in range(self._form.rowCount()):
            label = self._form.itemAt(row, QFormLayout.ItemRole.LabelRole)
            field = self._form.itemAt(row, QFormLayout.ItemRole.FieldRole)
            spanning = self._form.itemAt(row, QFormLayout.ItemRole.SpanningRole)
            texts = [
                widget.text() if isinstance(widget, QLabel) else ""
                for widget in (
                    None if item is None else item.widget() for item in (label, field, spanning)
                )
            ]
            shown.append((texts[0], texts[1] or texts[2]))
        return shown
