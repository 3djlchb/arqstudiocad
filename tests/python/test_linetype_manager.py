"""The LINETYPE dialog exposes the simple linetype workflow."""

from pathlib import Path

import pytest
from PySide6.QtCore import Qt
from pytestqt.qtbot import QtBot

from arqstudiocad.core import CadApplication
from arqstudiocad.ui.linetype_manager_dialog import LinetypeManagerDialog


def test_linetype_dialog_loads_selected_lin_definitions(
    qtbot: QtBot, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    document = CadApplication().create_document("Drawing")
    source = tmp_path / "custom.lin"
    source.write_text("*DASHED,Dashed\nA,0.5,-0.25\n*CENTER,Center\nA,1,-0.25,0,-0.25\n")
    dialog = LinetypeManagerDialog(document)
    qtbot.addWidget(dialog)
    monkeypatch.setattr(
        "arqstudiocad.ui.linetype_manager_dialog.QFileDialog.getOpenFileName",
        lambda *_args, **_kwargs: (str(source), ""),
    )

    dialog._choose_file()
    assert dialog._available.count() == 2
    dialog._available.item(0).setCheckState(Qt.CheckState.Unchecked)
    dialog._load()

    assert "CENTER" in document.get_linetypes()
    assert "DASHED" not in document.get_linetypes()
    assert dialog._loaded.count() == 2
