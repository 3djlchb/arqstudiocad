"""The clash dialog (ADR 0025 D6): both previews, 96 pixels, the template's
on the left and the drawing's on the right (maintainer, 2026-10-25), the
three choices, and the names a copy may take."""

from collections.abc import Iterator

import pytest
from PySide6.QtCore import QSize
from PySide6.QtGui import QImage
from pytestqt.qtbot import QtBot

from arqstudiocad.commands import BlockClash, ClashChoice
from arqstudiocad.commands.blocks import block_clashes
from arqstudiocad.core import CadApplication, CadDocument
from arqstudiocad.i18n import set_language
from arqstudiocad.paths import sample_template
from arqstudiocad.ui.block_clash_dialog import CLASH_PREVIEW, BlockClashDialog
from arqstudiocad.ui.blocks_panel import preview_pixmap


def own_block(document: CadDocument, name: str, x: float) -> None:
    line, _ = document.add_line((0, 0), (x, 1))
    reference, _ = document.create_block(name, (0, 0), [line])
    document.delete_entities([reference])


@pytest.fixture
def clashes() -> Iterator[list[BlockClash]]:
    app = CadApplication()
    template = app.open_document(sample_template())
    drawing = app.create_document("Plano1")
    own_block(drawing, "wc", 1)
    own_block(drawing, "Tree", 2)
    found = block_clashes(drawing, template, "WC") + block_clashes(drawing, template, "Tree")
    yield found


def image(label_pixmap: object) -> QImage:
    from PySide6.QtGui import QPixmap

    assert isinstance(label_pixmap, QPixmap) and not label_pixmap.isNull()
    return label_pixmap.toImage()


def test_each_clash_shows_the_templates_block_left_and_the_drawings_right(
    qtbot: QtBot, clashes: list[BlockClash]
) -> None:
    dialog = BlockClashDialog(clashes)
    qtbot.addWidget(dialog)
    assert [row.title() for row in dialog.rows] == ["wc", "Tree"]
    row = dialog.rows[0]
    ratio = row.devicePixelRatioF()
    for label in (row.template_preview, row.drawing_preview):
        pixmap = label.pixmap()
        assert pixmap.deviceIndependentSize().toSize() == QSize(CLASH_PREVIEW, CLASH_PREVIEW)
        assert CLASH_PREVIEW == 96
    left = preview_pixmap(row.clash.template, "WC", 96, row.palette(), ratio)
    right = preview_pixmap(row.clash.drawing, "wc", 96, row.palette(), ratio)
    assert left is not None and right is not None
    assert image(row.template_preview.pixmap()) == left.toImage(), "the template's on the left"
    assert image(row.drawing_preview.pixmap()) == right.toImage(), "the drawing's on the right"
    assert left.toImage() != right.toImage()
    assert row.template_caption.text() == "In the template: WC"
    assert row.drawing_caption.text() == "In the drawing: wc"


def test_the_drawings_block_is_the_default_and_choices_come_back(
    qtbot: QtBot, clashes: list[BlockClash]
) -> None:
    dialog = BlockClashDialog(clashes)
    qtbot.addWidget(dialog)
    assert dialog.choices() == {"WC": ClashChoice("keep"), "Tree": ClashChoice("keep")}
    wc, tree = dialog.rows
    assert not wc.new_name.isEnabled() and wc.new_name.text() == "WC_2"
    wc.rename.setChecked(True)
    assert wc.new_name.isEnabled()
    tree.redefine.setChecked(True)
    assert dialog.choices() == {
        "WC": ClashChoice("rename", "WC_2"),
        "Tree": ClashChoice("redefine"),
    }
    assert dialog.insert_button.isEnabled() and dialog.insert_button.text() == "Insert"


def test_a_new_name_must_be_valid_free_and_used_once(
    qtbot: QtBot, clashes: list[BlockClash]
) -> None:
    dialog = BlockClashDialog(clashes)
    qtbot.addWidget(dialog)
    wc, tree = dialog.rows
    wc.rename.setChecked(True)
    for bad, why in (
        ("", " is not a valid block name."),
        ("a*b", "a*b is not a valid block name."),
        ("TREE", "A block named TREE exists in the drawing."),
    ):
        wc.new_name.setText(bad)
        assert not dialog.insert_button.isEnabled(), bad
        assert dialog.problem_label.text().endswith(why) and not dialog.problem_label.isHidden()
    wc.new_name.setText("Inodoro")
    tree.rename.setChecked(True)
    tree.new_name.setText("inodoro")
    assert not dialog.insert_button.isEnabled(), "two copies cannot share a name"
    tree.new_name.setText("Arbol")
    assert dialog.insert_button.isEnabled() and dialog.problem_label.isHidden()


def test_a_dynamic_block_can_only_use_the_drawings(qtbot: QtBot, clashes: list[BlockClash]) -> None:
    clash = clashes[0]
    dynamic = BlockClash(
        clash.name, clash.template, clash.drawing, clash.drawing_name, True, clash.suggested
    )
    dialog = BlockClashDialog([dynamic])
    qtbot.addWidget(dialog)
    (row,) = dialog.rows
    assert row.keep.isChecked() and row.keep.isEnabled()
    assert not row.redefine.isEnabled() and not row.rename.isEnabled()
    reason = "wc is a dynamic block: until the parametric engine, only the drawing's can be used."
    assert row.note.text() == reason and row.redefine.toolTip() == reason


def test_the_dialog_speaks_spanish(qtbot: QtBot, clashes: list[BlockClash]) -> None:
    set_language("es")
    try:
        dialog = BlockClashDialog(clashes)
        qtbot.addWidget(dialog)
        row = dialog.rows[0]
        assert dialog.windowTitle() == "Bloques con el mismo nombre"
        assert row.keep.text() == "Usar el del dibujo"
        assert row.redefine.text() == "Redefinir desde la plantilla"
        assert row.rename.text() == "Copiar con otro nombre"
        assert row.template_caption.text() == "En la plantilla: WC"
        assert dialog.insert_button.text() == "Insertar"
    finally:
        set_language("en")
