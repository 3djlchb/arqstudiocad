"""The Block Definition dialog (ADR 0023 D2a): what it shows, when OK can
go on, how it closes to pick on screen, and what the Qt services keep."""

from pathlib import Path

import pytest
from PySide6.QtCore import QSettings
from PySide6.QtWidgets import QDialog, QMessageBox, QWidget
from pytestqt.qtbot import QtBot
from test_block_editor import LOOKUP
from test_commands import regression_drawing

from arqstudiocad.commands import BlockDefinitionState
from arqstudiocad.core import CadApplication, CadDocument
from arqstudiocad.i18n import set_language
from arqstudiocad.ui import block_definition_dialog
from arqstudiocad.ui.block_definition_dialog import BlockDefinitionDialog
from arqstudiocad.ui.qt_services import BLOCK_OBJECTS_SETTING, QtUiServices


def drawing() -> CadDocument:
    document = CadApplication().create_document("Plano")
    line, _ = document.add_line((0, 0), (1, 1))
    reference, _ = document.create_block("MESA", (0, 0), [line])
    document.delete_entities([reference])
    return document


def dialog(
    qtbot: QtBot, document: CadDocument, state: BlockDefinitionState, objects: str = "convert"
) -> BlockDefinitionDialog:
    shown = BlockDefinitionDialog(document, state, objects)  # type: ignore[arg-type]
    qtbot.addWidget(shown)
    return shown


def test_the_dialog_shows_the_state_and_gives_it_back(qtbot: QtBot) -> None:
    state = BlockDefinitionState(
        name="Silla",
        description="A chair",
        base=(10.5, -3.0),
        ids=(7, 8),
        objects="delete",
        scale_uniformly=True,
        explodable=False,
        units=4,
        open_editor=True,
    )
    shown = dialog(qtbot, drawing(), state)
    assert shown.windowTitle() == "Block Definition"
    assert [shown.name.itemText(i) for i in range(shown.name.count())] == ["MESA"]
    assert shown.name.currentText() == "Silla" and shown.description.text() == "A chair"
    assert (shown.base_x.value(), shown.base_y.value()) == (10.5, -3.0)
    assert shown.delete.isChecked() and shown.count.text() == "2 objects selected"
    assert not shown.annotative.isEnabled() and not shown.annotative.isChecked()
    assert shown.units.currentText() == "Millimeters"
    assert shown.ok_button.isEnabled()
    assert shown.state() == state


def test_the_remembered_choice_shows_when_the_state_leaves_it(qtbot: QtBot) -> None:
    shown = dialog(qtbot, drawing(), BlockDefinitionState(name="A", ids=(1,)), "retain")
    assert shown.retain.isChecked() and shown.state().objects == "retain"


def test_ok_waits_for_a_valid_name_and_objects(qtbot: QtBot) -> None:
    document = drawing()
    shown = dialog(qtbot, document, BlockDefinitionState(ids=(1,)))
    assert (
        shown.problem_label.text() == "Type the block's name." and not shown.ok_button.isEnabled()
    )
    shown.name.setCurrentText("A/B")
    assert shown.problem_label.text() == "A/B is not a valid block name."
    shown.name.setCurrentText("Nuevo")
    assert shown.problem() is None and shown.ok_button.isEnabled()
    shown.ids = ()
    shown.objects_on_screen.setChecked(True)
    assert shown.problem() is None, "objects selected after OK"
    assert not shown.select_objects.isEnabled()
    assert shown.count.text() == "Objects are selected after OK."
    shown.objects_on_screen.setChecked(False)
    assert shown.problem_label.text() == "No objects are selected."
    assert shown.count.text().endswith("No objects are selected.")
    shown.base_on_screen.setChecked(True)
    assert not shown.base_x.isEnabled() and not shown.pick_point.isEnabled()
    shown.buttons.accepted.emit()
    assert shown.result() != QDialog.DialogCode.Accepted, "OK refused while a problem stays"


def test_a_dynamic_block_cannot_be_redefined(qtbot: QtBot) -> None:
    document = CadApplication().open_document(regression_drawing(LOOKUP))
    shown = dialog(qtbot, document, BlockDefinitionState(name="my_look_block", ids=(1,)))
    assert shown.problem_label.text() == (
        "My_Look_Block is a dynamic block: it cannot be redefined yet."
    )
    assert not shown.ok_button.isEnabled()


@pytest.mark.parametrize(
    ("button", "action"), [("pick_point", "pick_point"), ("select_objects", "select_objects")]
)
def test_the_pick_buttons_close_the_dialog_for_the_command(
    qtbot: QtBot, button: str, action: str
) -> None:
    shown = dialog(qtbot, drawing(), BlockDefinitionState(name="Nuevo"))
    getattr(shown, button).click()
    assert shown.result() == QDialog.DialogCode.Accepted and shown.action == action
    assert shown.state().name == "Nuevo", "kept for when it opens again"


def test_the_dialog_in_spanish(qtbot: QtBot) -> None:
    set_language("es")
    shown = dialog(qtbot, drawing(), BlockDefinitionState(name="X", ids=(1,)))
    assert shown.windowTitle() == "Definición de bloque"
    assert (shown.retain.text(), shown.convert.text(), shown.delete.text()) == (
        "Retener entidades",
        "Convertir en bloque",
        "Suprimir entidades",
    )
    assert shown.select_objects.text() == "Designar entidades"
    assert shown.pick_point.text() == "Designar punto"


def test_the_services_remember_the_objects_choice(
    qtbot: QtBot, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    parent = QWidget()
    qtbot.addWidget(parent)
    settings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    services = QtUiServices(parent, settings=settings)
    shown: list[str] = []

    def answer(
        document: CadDocument, state: BlockDefinitionState, objects: str, parent: object
    ) -> tuple[str, BlockDefinitionState]:
        shown.append(objects)
        return "ok", BlockDefinitionState(name="A", objects="delete")

    monkeypatch.setattr(block_definition_dialog, "define_block", answer)
    services.define_block(drawing(), BlockDefinitionState())
    services.define_block(drawing(), BlockDefinitionState())
    assert shown == ["convert", "delete"], "the first time Convert, then the last choice"
    assert settings.value(BLOCK_OBJECTS_SETTING) == "delete"


def test_the_services_ask_before_redefining(qtbot: QtBot, monkeypatch: pytest.MonkeyPatch) -> None:
    parent = QWidget()
    qtbot.addWidget(parent)
    services = QtUiServices(parent)
    seen: list[str] = []

    def answer(box: QMessageBox) -> int:
        seen.append(box.text())
        return QMessageBox.StandardButton.Yes.value

    monkeypatch.setattr(QMessageBox, "exec", answer)
    assert services.ask_redefine_block("MESA", 3)
    assert seen == [
        "A block named MESA exists in the drawing. Redefine it with the objects selected? "
        "3 references in the drawing will change."
    ]
