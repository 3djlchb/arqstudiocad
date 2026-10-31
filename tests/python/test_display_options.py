"""Options > Display (ADR 0029 step 4b): the crosshair's size with its live
preview, its type and color, the snap markers' and command line's colors,
the block editor's background; how they are kept, and the window applying
them to every viewport and the command line."""

from dataclasses import replace
from pathlib import Path

import pytest
from PySide6.QtCore import QPoint, QSettings
from PySide6.QtGui import QColor
from PySide6.QtWidgets import QColorDialog, QGroupBox
from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.events import OptionsChanged
from arqstudiocad.i18n import set_language
from arqstudiocad.ui.crosshair import ON_DARK, Crosshair
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.options_dialog import (
    DEFAULTS,
    LABEL_WIDTH,
    STORED,
    Options,
    OptionsDialog,
    changed_keys,
    save_options,
    saved_options,
)
from arqstudiocad.ui.options_widgets import INVALID, ColorField, CrosshairPreview, color_value
from arqstudiocad.ui.viewport import (
    BLOCK_EDITOR_BACKGROUND,
    MARKER,
    MODEL_SPACE_BACKGROUND,
    Viewport,
)

ORANGE = "#ff8800"


def test_a_color_is_hex_or_empty() -> None:
    assert color_value("") == "" and color_value("   ") == ""
    assert color_value("#FF8800") == ORANGE and color_value(" #ff8800 ") == ORANGE
    for wrong in ("#f80", "orange", "#gg8800", "ff8800", "#ff88001"):
        assert color_value(wrong) is None, wrong


def test_the_settings_are_kept_and_read_back(tmp_path: Path) -> None:
    settings = QSettings(str(tmp_path / "o.ini"), QSettings.Format.IniFormat)
    assert saved_options(settings) == DEFAULTS
    assert DEFAULTS.block_editor_background == "#606060", "today's grey"
    values = Options(
        crosshair_size=40,
        crosshair_type="small",
        crosshair_color=ORANGE,
        marker_color="#00ff00",
        command_line_color="#123456",
        block_editor_background="#336699",
    )
    save_options(settings, values, changed_keys(DEFAULTS, values))
    settings.sync()
    again = QSettings(str(tmp_path / "o.ini"), QSettings.Format.IniFormat)
    assert saved_options(again) == values, "across restarts (the INI keeps text)"
    assert STORED["crosshair_size"] == "options/crosshair_size"
    assert "language" not in STORED and "theme" not in STORED, "keys of their own"


@pytest.mark.parametrize(
    ("name", "saved"),
    [
        ("crosshair_size", "0"),
        ("crosshair_size", "101"),
        ("crosshair_size", "big"),
        ("crosshair_type", "circle"),
        ("crosshair_color", "orange"),
        ("marker_color", "#12345"),
        ("block_editor_background", ""),
        ("block_editor_background", "grey"),
        ("right_click", "later"),
    ],
)
def test_a_value_this_version_does_not_take_reads_as_the_default(
    tmp_path: Path, name: str, saved: str
) -> None:
    settings = QSettings(str(tmp_path / "o.ini"), QSettings.Format.IniFormat)
    settings.setValue(STORED[name], saved)
    assert getattr(saved_options(settings), name) == getattr(DEFAULTS, name)


def test_save_keeps_only_the_keys_given(tmp_path: Path) -> None:
    settings = QSettings(str(tmp_path / "o.ini"), QSettings.Format.IniFormat)
    save_options(settings, Options(crosshair_size=30, marker_color=ORANGE), ("crosshair_size",))
    assert settings.value(STORED["crosshair_size"]) == 30
    assert settings.value(STORED["marker_color"]) is None


# The color field


def field(qtbot: QtBot, required: bool = False) -> tuple[ColorField, list[str]]:
    f = ColorField(lambda: QColor(MARKER), required)
    qtbot.addWidget(f)
    seen: list[str] = []
    f.changed.connect(lambda: seen.append(f.value()))
    return f, seen


def test_typing_a_color(qtbot: QtBot) -> None:
    f, seen = field(qtbot)
    assert f.edit.placeholderText() == "#RRGGBB or empty"
    assert f.shown() == QColor(MARKER), "empty: the color used instead"
    assert MARKER.name() in f.swatch.styleSheet()
    f.edit.textEdited.emit("#FF8800")
    assert f.value() == ORANGE and seen == [ORANGE] and not f.invalid
    assert ORANGE in f.swatch.styleSheet()
    f.edit.textEdited.emit("#FF88")
    assert f.invalid and f.value() == ORANGE, "the last valid value kept"
    assert INVALID in f.edit.styleSheet()
    assert f.edit.toolTip() == "Enter a color as #RRGGBB, or leave it empty"
    assert seen == [ORANGE], "nothing changed"
    f.edit.textEdited.emit("")
    assert f.value() == "" and not f.invalid and f.edit.styleSheet() == ""
    assert seen == [ORANGE, ""]


def test_a_required_color_cannot_be_empty(qtbot: QtBot) -> None:
    f, seen = field(qtbot, required=True)
    f.set_value("#606060")
    assert f.edit.text() == "#606060" and f.edit.placeholderText() == "#RRGGBB"
    f.edit.textEdited.emit("")
    assert f.invalid and f.value() == "#606060" and seen == []
    assert f.edit.toolTip() == "Enter a color as #RRGGBB"


def test_the_swatch_opens_qts_color_dialog(qtbot: QtBot, monkeypatch: pytest.MonkeyPatch) -> None:
    f, seen = field(qtbot)
    asked: list[QColor] = []

    def pick(initial: QColor, parent: object, title: str) -> QColor:
        asked.append(initial)
        assert title == "Choose a color"
        return QColor(ORANGE)

    monkeypatch.setattr(QColorDialog, "getColor", staticmethod(pick))
    f.swatch.click()
    assert asked == [QColor(MARKER)], "it starts from the color shown"
    assert f.value() == ORANGE and f.edit.text() == "#FF8800" and seen == [ORANGE]
    monkeypatch.setattr(QColorDialog, "getColor", staticmethod(lambda *_: QColor()))
    f.swatch.click()
    assert f.value() == ORANGE and seen == [ORANGE], "cancelled: unchanged"


# The preview


def test_the_preview_paints_the_crosshair_live(qtbot: QtBot) -> None:
    preview = CrosshairPreview(QColor(*MODEL_SPACE_BACKGROUND))
    qtbot.addWidget(preview)
    assert (preview.width(), preview.height()) == (150, 100)
    preview.set_crosshair(Crosshair(size=40, color=ORANGE))
    assert preview.accessibleName() == "Crosshair preview at 40 %"
    image = preview.grab().toImage()
    # 40 % of the half-width 75: the arm reaches 30 px left of the center.
    assert image.pixelColor(75 - 25, 50) == QColor(ORANGE)
    assert image.pixelColor(75 - 35, 50) == QColor(*MODEL_SPACE_BACKGROUND)
    assert image.pixelColor(10, 10) == QColor(*MODEL_SPACE_BACKGROUND)
    preview.set_crosshair(Crosshair(type="arrow"))
    arrow = preview.grab().toImage()
    assert arrow.pixelColor(75 - 25, 50) == QColor(*MODEL_SPACE_BACKGROUND), "no crosshair"
    assert arrow.pixelColor(76, 50) == ON_DARK, "the system arrow, drawn"


# The dialog


def dialog(qtbot: QtBot, values: Options = DEFAULTS) -> tuple[OptionsDialog, list[Options]]:
    applied: list[Options] = []
    d = OptionsDialog(values, applied.append)
    qtbot.addWidget(d)
    d.sidebar.setCurrentRow(1)
    return d, applied


def test_display_holds_the_crosshair_and_the_block_editor(qtbot: QtBot) -> None:
    d, _ = dialog(qtbot)
    assert d.section() == "display"
    page = d.pages.currentWidget()
    assert page is not None
    assert [g.title() for g in page.findChildren(QGroupBox)] == ["Crosshair", "Block editor"]
    assert (d.size_slider.value(), d.size_spin.value()) == (5, 5)
    assert (d.size_spin.minimum(), d.size_spin.maximum()) == (1, 100)
    assert d.size_help.text().startswith("Percent of the drawing area.")
    assert d.preview_caption.text() == "Preview"
    types = [d.crosshair_type.itemText(i) for i in range(d.crosshair_type.count())]
    assert types == ["Crosshair", "Arrow", "Small cross"]
    assert d.crosshair_color.value() == "" and d.crosshair_color.shown() == ON_DARK
    assert d.marker_color.shown() == QColor(MARKER)
    assert d.command_line_color.shown() == d.palette().text().color()
    assert d.block_editor_background.value() == "#606060"
    assert d.block_editor_background.required and not d.crosshair_color.required
    assert d.colors_help.text().startswith("Leave the color empty")
    assert d.preview.background == QColor(*MODEL_SPACE_BACKGROUND)
    muted = d.size_label.styleSheet()
    assert muted and d.size_help.styleSheet() == muted == d.colors_help.styleSheet()
    assert d.size_help.font().pointSizeF() < d.size_label.font().pointSizeF()
    assert d.colors_help.font().pointSizeF() < d.size_label.font().pointSizeF()


def test_the_size_slider_and_number_keep_in_step(qtbot: QtBot) -> None:
    d, _ = dialog(qtbot)
    d.size_slider.setValue(40)
    assert d.size_spin.value() == 40 and d.pending.crosshair_size == 40
    assert d.preview.crosshair.size == 40, "the preview, live"
    assert d.apply_button.isEnabled()
    d.size_spin.setValue(70)
    assert d.size_slider.value() == 70 and d.pending.crosshair_size == 70


def test_every_display_control_changes_what_is_pending(qtbot: QtBot) -> None:
    d, applied = dialog(qtbot)
    d.crosshair_type.setCurrentIndex(d.crosshair_type.findData("small"))
    d.crosshair_color.edit.textEdited.emit("#FF8800")
    d.marker_color.edit.textEdited.emit("#00FF00")
    d.command_line_color.edit.textEdited.emit("#123456")
    d.block_editor_background.edit.textEdited.emit("#336699")
    d.block_editor_background.edit.textEdited.emit("#33")  # not a color: kept
    assert d.preview.crosshair == Crosshair(5, "small", ORANGE)
    d.ok_button.click()
    assert applied == [
        Options(
            crosshair_type="small",
            crosshair_color=ORANGE,
            marker_color="#00ff00",
            command_line_color="#123456",
            block_editor_background="#336699",
        )
    ]


def test_the_dialog_shows_the_values_given(qtbot: QtBot) -> None:
    values = Options(crosshair_size=30, crosshair_type="arrow", marker_color=ORANGE)
    d, _ = dialog(qtbot, values)
    assert d.size_spin.value() == 30 and d.size_slider.value() == 30
    assert d.crosshair_type.currentData() == "arrow"
    assert d.marker_color.edit.text() == "#FF8800"
    assert d.preview.crosshair == Crosshair(30, "arrow")
    assert not d.apply_button.isEnabled()


def test_reset_section_resets_display_only(qtbot: QtBot) -> None:
    values = Options(language="es", right_click="repeat", crosshair_size=30, marker_color=ORANGE)
    d, _ = dialog(qtbot, values)
    d.reset_button.click()
    assert d.pending == Options(language="es", right_click="repeat")
    assert d.size_spin.value() == 5 and d.marker_color.value() == ""


def test_display_in_spanish(qtbot: QtBot) -> None:
    try:
        set_language("es")
        d, _ = dialog(qtbot)
        assert d.sidebar.item(1).text() == "Visualización"
        assert d.size_label.text() == "Tamaño" and d.type_label.text() == "Tipo"
        assert d.crosshair_type.itemText(2) == "Cruz pequeña"
        assert d.marker_color_label.text() == "Color del marcador de referencia"
        assert d.crosshair_color.edit.placeholderText() == "#RRGGBB o vacío"
        assert d.block_editor_label.text() == "Color de fondo"
        assert d.preview.accessibleName() == "Vista preliminar del cursor en cruz al 5 %"
    finally:
        set_language("en")


# The window


@pytest.fixture
def window(qtbot: QtBot, tmp_path: Path) -> MainWindow:
    settings = QSettings(str(tmp_path / "w.ini"), QSettings.Format.IniFormat)
    w = MainWindow(settings, ui=QuietUi())
    qtbot.addWidget(w)
    w.resize(900, 600)
    w.show()
    qtbot.waitExposed(w)
    return w


def test_the_window_applies_display_to_every_viewport(window: MainWindow) -> None:
    drawing = window.session.new_document()
    a, _ = drawing.add_line((0, 0), (40, 0))
    window.session.edit(drawing, lambda d: d.create_block("MARCO", (0, 0), [a])[1])
    editor = window.session.open_block_editor(drawing, "MARCO")
    plain, grey = window.viewports[drawing.id], window.viewports[editor.id]
    assert grey.view.background == BLOCK_EDITOR_BACKGROUND
    announced: list[OptionsChanged] = []
    window.bus.subscribe(OptionsChanged, announced.append)
    values = replace_display(window)
    window.apply_options(values)
    for viewport in (plain, grey):
        assert viewport.crosshair == Crosshair(40, "small", ORANGE)
        assert viewport.marker_color == QColor("#00ff00")
    assert grey.view.background == (0x33, 0x66, 0x99)
    assert plain.view.background == MODEL_SPACE_BACKGROUND, "the drawing keeps its own"
    for widget in (
        window.command_window.history,
        window.command_window.prompt,
        window.command_window.input,
    ):
        assert widget.styleSheet() == "color: #123456;"
    (event,) = announced
    assert set(event.keys) == {
        "crosshair_size",
        "crosshair_type",
        "crosshair_color",
        "marker_color",
        "command_line_color",
        "block_editor_background",
    }
    # Back to the defaults: AutoCAD's yellow, the theme's text, the grey.
    window.apply_options(replace_display(window, DEFAULTS))
    assert plain.marker_color == QColor(MARKER) and plain.crosshair == Crosshair()
    assert grey.view.background == BLOCK_EDITOR_BACKGROUND
    assert window.command_window.input.styleSheet() == ""


def replace_display(window: MainWindow, source: Options | None = None) -> Options:
    """The window's values with the display settings of ``source``, or a
    set of changed ones."""
    from dataclasses import replace

    source = source or Options(
        crosshair_size=40,
        crosshair_type="small",
        crosshair_color=ORANGE,
        marker_color="#00ff00",
        command_line_color="#123456",
        block_editor_background="#336699",
    )
    current = window.options_values()
    return replace(
        current,
        **{
            name: getattr(source, name)
            for name in (
                "crosshair_size",
                "crosshair_type",
                "crosshair_color",
                "marker_color",
                "command_line_color",
                "block_editor_background",
            )
        },
    )


def test_a_new_viewport_and_a_restart_take_the_settings(window: MainWindow, qtbot: QtBot) -> None:
    settings = window._settings
    assert settings is not None
    window.apply_options(replace_display(window))
    later = window.session.new_document()
    assert window.viewports[later.id].crosshair == Crosshair(40, "small", ORANGE)
    assert window.viewports[later.id].marker_color == QColor("#00ff00")
    settings.sync()
    again = MainWindow(settings, ui=QuietUi())
    qtbot.addWidget(again)
    assert again.options_values().crosshair == Crosshair(40, "small", ORANGE)
    assert again.command_window.input.styleSheet() == "color: #123456;", "from the start"
    drawing = again.session.new_document()
    a, _ = drawing.add_line((0, 0), (40, 0))
    again.session.edit(drawing, lambda d: d.create_block("MARCO", (0, 0), [a])[1])
    editor = again.session.open_block_editor(drawing, "MARCO")
    assert again.viewports[editor.id].view.background == (0x33, 0x66, 0x99)


def test_long_labels_wrap_and_every_group_lines_up(qtbot: QtBot) -> None:
    try:
        set_language("es")
        d, _ = dialog(qtbot)
        d.show()
        qtbot.waitExposed(d)
        label = d.command_line_color_label
        assert label.text() == "Color del texto de la línea de comandos"
        assert label.wordWrap() and label.width() == LABEL_WIDTH
        assert label.height() > d.type_label.height(), "on two lines"
        first = d.crosshair_color.mapTo(d, QPoint(0, 0)).x()
        assert d.block_editor_background.mapTo(d, QPoint(0, 0)).x() == first
        assert d.crosshair_type.mapTo(d, QPoint(0, 0)).x() == first
    finally:
        set_language("en")


def marker_pixels(viewport: Viewport, color: QColor) -> int:
    """Pixels of ``color`` around the snap marker."""
    assert viewport.target is not None
    image = viewport.grab().toImage()
    sx, sy = viewport.view.to_screen(*viewport.target)
    return sum(
        image.pixelColor(round(sx) + dx, round(sy) + dy) == color
        for dx in range(-8, 9)
        for dy in range(-8, 9)
    )


def test_the_snap_marker_is_painted_in_the_chosen_color(window: MainWindow, tmp_path: Path) -> None:
    from PySide6.QtCore import QEvent
    from test_point_input import mouse, plan

    viewport = plan(window, tmp_path)
    window.runner.execute("ID")
    mouse(viewport, QEvent.Type.MouseMove, (60.5, 40.0))
    assert viewport.snap is not None
    assert marker_pixels(viewport, QColor(MARKER)) > 5, "AutoCAD's yellow"
    window.apply_options(replace(window.options_values(), marker_color="#00ff00"))
    assert marker_pixels(viewport, QColor("#00ff00")) > 5
    assert marker_pixels(viewport, QColor(MARKER)) == 0
    window.runner.cancel()


def test_a_new_background_is_drawn_at_once(window: MainWindow) -> None:
    viewport = window.viewports[window.session.new_document().id]
    viewport.grab()  # the drawing rendered once, on its own background
    viewport.set_background((0x33, 0x66, 0x99))
    assert viewport.grab().toImage().pixelColor(3, 3) == QColor("#336699")
