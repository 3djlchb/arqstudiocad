"""The Edit Block Definition dialog (ADR 0023 D3, maintainer 2026-10-25):
BEDIT and a double click on a block choose the block here, seeing its
preview and description; a dynamic block is shown but cannot be chosen."""

from types import SimpleNamespace
from typing import cast

from PySide6.QtTest import QTest
from PySide6.QtWidgets import QDialog
from pytestqt.qtbot import QtBot
from test_block_editor import LOOKUP
from test_commands import regression_drawing

from arqstudiocad.core import CadApplication, CadDocument
from arqstudiocad.i18n import set_language
from arqstudiocad.ui.edit_block_dialog import EDIT_PREVIEW, EditBlockDialog


def drawing(*names: str) -> CadDocument:
    document = CadApplication().create_document("Plano")
    for index, name in enumerate(names):
        line, _ = document.add_line((0, 0), (index + 1, 2))
        reference, _ = document.create_block(name, (0, 0), [line])
        document.delete_entities([reference])
    return document


def dialog(qtbot: QtBot, document: CadDocument, chosen: str | None = None) -> EditBlockDialog:
    shown = EditBlockDialog(document, chosen)
    qtbot.addWidget(shown)
    return shown


def test_the_block_double_clicked_is_chosen_with_its_preview(qtbot: QtBot) -> None:
    document = drawing("Mesa", "Silla")
    shown = dialog(qtbot, document, "Silla")
    assert shown.windowTitle() == "Edit Block Definition"
    assert [shown.blocks.item(i).text() for i in range(shown.blocks.count())] == ["Mesa", "Silla"]
    assert shown.chosen() == "Silla" and shown.name.text() == "Silla"
    pixmap = shown.preview.pixmap()
    assert not pixmap.isNull() and pixmap.width() == round(EDIT_PREVIEW * pixmap.devicePixelRatio())
    assert shown.ok_button.isEnabled() and not shown.problem_label.isVisibleTo(shown)
    shown.blocks.setCurrentRow(0)
    assert shown.chosen() == "Mesa" and shown.name.text() == "Mesa", "the list fills the name"


def test_without_a_block_the_first_is_chosen(qtbot: QtBot) -> None:
    shown = dialog(qtbot, drawing("Mesa", "Silla"))
    assert shown.chosen() == "Mesa"


def test_a_name_typed_chooses_its_block_ignoring_case(qtbot: QtBot) -> None:
    shown = dialog(qtbot, drawing("Mesa", "Silla"), "Mesa")
    shown.name.clear()
    QTest.keyClicks(shown.name, "silla")
    assert shown.chosen() == "Silla" and shown.ok_button.isEnabled()
    QTest.keyClicks(shown.name, "s")
    assert shown.chosen() is None and not shown.ok_button.isEnabled()
    assert shown.problem_label.text() == "There is no block named sillas."
    assert shown.preview.pixmap().isNull(), "nothing to show"
    shown.name.clear()
    shown._typed("")
    assert not shown.ok_button.isEnabled() and shown.problem_label.text() == ""


class Described:
    """A drawing whose blocks all have a description: none of the samples'
    has one."""

    def __init__(self, document: CadDocument) -> None:
        self._document = document

    def __getattr__(self, name: str) -> object:
        return getattr(self._document, name)

    def block_info(self, name: str) -> object:
        return SimpleNamespace(description=f"The {name}, for the dining room.")


def test_the_chosen_blocks_description_is_shown(qtbot: QtBot) -> None:
    shown = dialog(qtbot, cast(CadDocument, Described(drawing("Mesa", "Silla"))), "Mesa")
    assert shown.description.text() == "The Mesa, for the dining room."
    shown.blocks.setCurrentRow(1)
    assert shown.description.text() == "The Silla, for the dining room."


def test_ok_and_a_double_click_in_the_list_choose(qtbot: QtBot) -> None:
    shown = dialog(qtbot, drawing("Mesa", "Silla"), "Mesa")
    shown.ok_button.click()
    assert shown.result() == QDialog.DialogCode.Accepted
    again = dialog(qtbot, drawing("Mesa", "Silla"), "Mesa")
    item = again.blocks.item(1)
    again.blocks.setCurrentItem(item)
    again.blocks.itemDoubleClicked.emit(item)
    assert again.result() == QDialog.DialogCode.Accepted and again.chosen() == "Silla"


def test_a_dynamic_block_is_shown_but_cannot_be_chosen(qtbot: QtBot) -> None:
    document = CadApplication().open_document(regression_drawing(LOOKUP))
    shown = dialog(qtbot, document, "My_Look_Block")
    assert shown.chosen() == "My_Look_Block" and not shown.preview.pixmap().isNull()
    assert not shown.ok_button.isEnabled()
    assert (
        shown.problem_label.text() == "My_Look_Block is a dynamic block: it cannot be edited yet."
    )
    shown.blocks.itemDoubleClicked.emit(shown.blocks.currentItem())
    assert shown.result() != QDialog.DialogCode.Accepted, "a double click cannot choose it either"


def test_the_dialog_in_spanish(qtbot: QtBot) -> None:
    set_language("es")
    shown = dialog(qtbot, drawing("Mesa"), "Mesa")
    assert shown.windowTitle() == "Editar definición de bloque"
    assert shown.ok_button.text() == "Aceptar"
