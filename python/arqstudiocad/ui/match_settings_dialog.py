"""MATCHPROP's Property Settings, as AutoCAD's: which properties the source
gives, basic and special, each a check box (docs/design/matchprop-command.md).
Only the properties the drawing model holds are offered."""

from collections.abc import Collection

from PySide6.QtWidgets import (
    QCheckBox,
    QDialog,
    QDialogButtonBox,
    QGridLayout,
    QGroupBox,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.commands.matchprop import BASIC, SPECIAL, setting_label
from arqstudiocad.i18n import tr


class MatchSettingsDialog(QDialog):
    """A check box per property, the chosen ones checked; OK and Cancel."""

    def __init__(self, chosen: Collection[str], parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setObjectName("match_settings_dialog")
        self.setWindowTitle(tr("Property Settings"))
        self.boxes: dict[str, QCheckBox] = {}
        layout = QVBoxLayout(self)
        for title, names in ((tr("Basic Properties"), BASIC), (tr("Special Properties"), SPECIAL)):
            group = QGroupBox(title)
            grid = QGridLayout(group)
            for k, name in enumerate(names):
                box = QCheckBox(setting_label(name))
                box.setChecked(name in chosen)
                self.boxes[name] = box
                grid.addWidget(box, k // 2, k % 2)
            layout.addWidget(group)
        buttons = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Ok | QDialogButtonBox.StandardButton.Cancel
        )
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)
        layout.addWidget(buttons)

    def chosen(self) -> frozenset[str]:
        return frozenset(name for name, box in self.boxes.items() if box.isChecked())


def ask_match_settings(parent: QWidget | None, chosen: Collection[str]) -> frozenset[str] | None:
    """The properties checked, or None if the window was cancelled."""
    dialog = MatchSettingsDialog(chosen, parent)
    if dialog.exec() != QDialog.DialogCode.Accepted:
        return None
    return dialog.chosen()
