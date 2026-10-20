"""The drawing and selected-entity properties panel (ADR 0016, D10)."""

import math
from collections.abc import Sequence

from PySide6.QtCore import QLocale
from PySide6.QtWidgets import QDockWidget, QWidget

from arqstudiocad.core import (
    CadArc,
    CadCircle,
    CadColor,
    CadDocument,
    CadEntityInfo,
    CadError,
    CadLine,
    CadText,
)
from arqstudiocad.events import CommandMessage, DocumentChanged, Event, SelectionChanged
from arqstudiocad.i18n import tr
from arqstudiocad.session import Session
from arqstudiocad.ui import text_dialog
from arqstudiocad.ui.color_picker import (
    ColorPickerDialog,
    color_name,
    recent_colors,
    remember_color,
)
from arqstudiocad.ui.event_bridge import QtEventBridge
from arqstudiocad.ui.property_grid import EDIT_IN_DIALOG, Choice, Property, PropertyGrid, Section

_VARIES = "__varies__"
_PICK_COLOR = "__pick_color__"

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


def selection_properties(
    document: CadDocument, entity_ids: Sequence[int]
) -> tuple[str, list[Section]]:
    """Builds editable properties for the selected entities."""
    entities = document.get_entities(entity_ids)
    if not entities:
        return tr("Drawing"), drawing_properties(document)

    kinds = {entity.kind for entity in entities}
    count = len(entities)
    if len(kinds) == 1:
        kind = _kind_name(entities[0].kind)
        caption = kind if count == 1 else tr("{kind} ({count})", kind=kind, count=count)
    else:
        caption = tr("*Varies* ({count})", count=count)

    layers = document.get_layers()
    layer_names = [entity.layer for entity in entities]
    layer_value = _common(layer_names)
    selected_layer = next((layer.id for layer in layers if layer.name == layer_value), _VARIES)
    selected_layer_info = next((layer for layer in layers if layer.id == selected_layer), None)
    layer_choices = [Choice(tr("*Varies*"), _VARIES)] if selected_layer == _VARIES else []
    layer_choices.extend(Choice(layer.name, layer.id) for layer in layers)

    colors = [entity.color for entity in entities]
    selected_color = _common(colors)
    color_choices = _color_choices(
        selected_color,
        None if selected_layer_info is None else selected_layer_info.color,
    )
    color_label = (
        _color_label(selected_color) if isinstance(selected_color, CadColor) else tr("*Varies*")
    )
    general: list[tuple[str, str] | Property] = [
        (tr("Kind"), _kind_name(entities[0].kind) if len(kinds) == 1 else tr("*Varies*")),
        (tr("Linetype"), _metadata_text(_common([entity.linetype for entity in entities]))),
        (tr("Lineweight"), _metadata_text(_common([entity.lineweight for entity in entities]))),
        Property(
            "layer",
            tr("Layer"),
            str(layer_value) if layer_value != _VARIES else tr("*Varies*"),
            choices=layer_choices,
            choice_value=selected_layer,
        ),
        Property(
            "color",
            tr("Color"),
            color_label,
            choices=color_choices,
            choice_value=selected_color,
            accent=True,
            searchable=True,
        ),
    ]
    detail_maps = [dict(entity.details) for entity in entities]
    detail_names = list(dict.fromkeys(name for details in detail_maps for name in details))
    detail_labels = {
        "Block name": tr("Block name"),
        "Insertion point": tr("Insertion point"),
        "Scale": tr("Scale"),
        "Rotation": tr("Rotation"),
        "Attributes": tr("Attributes"),
        "Columns": tr("Columns"),
        "Rows": tr("Rows"),
        "Vertices": tr("Vertices"),
        "Closed": tr("Closed"),
        "Constant width": tr("Constant width"),
        "Thickness": tr("Thickness"),
    }
    for name in detail_names:
        value = _common([details.get(name, _VARIES) for details in detail_maps])
        general.append((detail_labels.get(name, name), _metadata_text(value)))

    sections = [Section("general", tr("General"), general)]
    texts = [entity.text for entity in entities]
    if all(text is not None for text in texts):
        sections.append(_text_section([text for text in texts if text is not None]))
    geometries = [entity.geometry for entity in entities]
    geometry_type = type(geometries[0])
    if (
        len(kinds) == 1
        and all(type(geometry) is geometry_type for geometry in geometries)
        and isinstance(geometries[0], (CadLine, CadCircle, CadArc))
    ):
        sections.append(_geometry_section(entities, geometries))
    else:
        sections.append(_extent_section(entities))
    return caption, sections


def _geometry_section(entities: Sequence[CadEntityInfo], geometries: Sequence[object]) -> Section:
    first = geometries[0]
    rows: list[tuple[str, str] | Property] = []

    def numeric(key: str, label: str, values: Sequence[float], *, editable: bool) -> None:
        value = _common(values)
        rows.append(
            Property(
                key,
                tr(label),
                _number_text(value),
                editable=editable,
            )
        )

    if isinstance(first, CadLine):
        lines = [geometry for geometry in geometries if isinstance(geometry, CadLine)]
        for name, index in (("Start X", 0), ("Start Y", 1), ("End X", 0), ("End Y", 1)):
            end = "start" if name.startswith("Start") else "end"
            numeric(
                f"geometry.{end}.{index}",
                name,
                [getattr(line, end)[index] for line in lines],
                editable=True,
            )
        numeric("length", "Length", [line.length for line in lines], editable=False)
    elif isinstance(first, CadCircle):
        circles = [geometry for geometry in geometries if isinstance(geometry, CadCircle)]
        for name, index in (("Center X", 0), ("Center Y", 1)):
            numeric(
                f"geometry.center.{index}",
                name,
                [circle.center[index] for circle in circles],
                editable=True,
            )
        numeric("geometry.radius", "Radius", [circle.radius for circle in circles], editable=True)
        numeric(
            "circumference",
            "Circumference",
            [circle.length for circle in circles],
            editable=False,
        )
        numeric("area", "Area", [math.pi * circle.radius**2 for circle in circles], editable=False)
    elif isinstance(first, CadArc):
        arcs = [geometry for geometry in geometries if isinstance(geometry, CadArc)]
        for name, index in (("Center X", 0), ("Center Y", 1)):
            numeric(
                f"geometry.center.{index}",
                name,
                [arc.center[index] for arc in arcs],
                editable=True,
            )
        numeric("geometry.radius", "Radius", [arc.radius for arc in arcs], editable=True)
        for name in ("start_angle", "end_angle"):
            numeric(
                f"geometry.{name}",
                "Start angle" if name == "start_angle" else "End angle",
                [math.degrees(getattr(arc, name)) for arc in arcs],
                editable=True,
            )
        numeric("length", "Length", [arc.length for arc in arcs], editable=False)
        numeric(
            "delta",
            "Delta angle",
            [math.degrees(_arc_sweep(arc)) for arc in arcs],
            editable=False,
        )
        numeric(
            "area",
            "Area",
            [0.5 * arc.radius**2 * _arc_sweep(arc) for arc in arcs],
            editable=False,
        )
    elevations = [entity.elevation for entity in entities]
    if elevations and all(elevation is not None for elevation in elevations):
        elevation = _common(elevations)
        rows.append(
            (
                tr("Elevation"),
                _number_text(elevation),
            )
        )
    return Section("geometry", tr("Geometry"), rows)


def _text_section(texts: Sequence[CadText]) -> Section:
    """Content, height and rotation, editable where the core allows it
    (ADR 0022); the rest is shown."""
    sizes = all(text.sizes_editable for text in texts)
    content = _common([text.content for text in texts])
    editable = all(text.content_editable for text in texts)
    multiline = any(text.is_multiline for text in texts)
    shown = content if isinstance(content, str) and content != _VARIES else tr("*Varies*")
    rows: list[tuple[str, str] | Property] = [
        Property(
            "text.content",
            tr("Contents"),
            # Paragraphs on one row; a double click edits them in a window.
            shown.replace("\n", " ¶ "),
            editable=editable and not multiline,
            dialog=editable and multiline,
        ),
        Property(
            "text.height",
            tr("Height"),
            _number_text(_common([text.height for text in texts])),
            editable=sizes,
        ),
        Property(
            "text.rotation",
            tr("Rotation"),
            _number_text(_common([math.degrees(text.rotation) for text in texts])),
            editable=sizes,
        ),
        (tr("Style"), _metadata_text(_common([text.style for text in texts]))),
        (
            tr("Justification"),
            _metadata_text(_common([justification_name(text.justification) for text in texts])),
        ),
        (tr("Width factor"), _number_text(_common([text.width_factor for text in texts]))),
        *(
            [(tr("Width"), _number_text(_common([text.width for text in texts])))]
            if multiline
            else []
        ),
        (
            tr("Oblique angle"),
            _number_text(_common([math.degrees(text.oblique) for text in texts])),
        ),
    ]
    return Section("text", tr("Text"), rows)


def justification_name(name: str) -> str:
    """A text justification as AutoCAD names it, translated."""
    names = {
        "Left": tr("Left"),
        "Center": tr("Center"),
        "Right": tr("Right"),
        "Aligned": tr("Aligned"),
        "Middle": tr("Middle"),
        "Fit": tr("Fit"),
        "TopLeft": tr("Top left"),
        "TopCenter": tr("Top center"),
        "TopRight": tr("Top right"),
        "MiddleLeft": tr("Middle left"),
        "MiddleCenter": tr("Middle center"),
        "MiddleRight": tr("Middle right"),
        "BottomLeft": tr("Bottom left"),
        "BottomCenter": tr("Bottom center"),
        "BottomRight": tr("Bottom right"),
    }
    return names.get(name, name)


def _extent_section(entities: Sequence[CadEntityInfo]) -> Section:
    bounds = [entity.bounds for entity in entities]
    if any(bound is None for bound in bounds):
        value = tr("*Varies*")
    else:
        sizes = [
            (_format_number(bound.width), _format_number(bound.height))
            for bound in bounds
            if bound is not None
        ]
        width = _common([size[0] for size in sizes])
        height = _common([size[1] for size in sizes])
        value = (
            tr("*Varies*")
            if width == _VARIES or height == _VARIES
            else tr("{width} x {height}", width=width, height=height)
        )
    elevation = _common([entity.elevation for entity in entities])
    rows: list[tuple[str, str]] = [(tr("Extents"), value)]
    if any(entity.elevation is not None for entity in entities):
        rows.append(
            (
                tr("Elevation"),
                _number_text(elevation),
            )
        )
    return Section("geometry", tr("Geometry"), rows)


def _color_choices(
    selected: CadColor | str, by_layer_color: CadColor | None = None
) -> list[Choice]:
    choices: list[Choice] = []
    if selected == _VARIES:
        choices.append(Choice(tr("*Varies*"), _VARIES))
    current = selected if isinstance(selected, CadColor) else None
    by_layer = CadColor.by_layer()
    by_block = CadColor.by_block()
    choices.append(
        Choice(
            _color_label(by_layer),
            by_layer,
            (by_layer_color or by_layer).display_rgb(),
        )
    )
    choices.append(Choice(_color_label(by_block), by_block, by_block.display_rgb()))

    recent = list(recent_colors())
    recent_values = [color for color in recent if color.index not in range(1, 10)]
    if current is not None and current.kind in {"index", "rgb"} and current not in recent_values:
        recent_values.insert(0, current)
    shown = [CadColor.from_index(index) for index in range(1, 10)]
    shown.extend(color for color in recent_values if color not in shown)
    choices.extend(Choice(_color_label(color), color, color.display_rgb()) for color in shown)
    choices.append(Choice(tr("More colors…"), _PICK_COLOR))
    return choices


def _color_label(color: CadColor) -> str:
    if color.kind == "bylayer":
        return tr("ByLayer")
    if color.kind == "byblock":
        return tr("ByBlock")
    if color.kind == "none":
        return tr("None")
    return color_name(color)


def _kind_name(kind: str) -> str:
    names = {
        "LINE": tr("Line"),
        "CIRCLE": tr("Circle"),
        "ARC": tr("Arc"),
        "TEXT": tr("Text"),
        "MTEXT": tr("Multiline text"),
    }
    return names.get(kind, kind)


def _metadata_text(value: str) -> str:
    if value == _VARIES:
        return tr("*Varies*")
    return {
        "ByLayer": tr("ByLayer"),
        "ByBlock": tr("ByBlock"),
        "Default": tr("Default"),
        "true": tr("Yes"),
        "false": tr("No"),
    }.get(value, value)


def _common[T](values: Sequence[T]) -> T | str:
    if not values:
        return _VARIES
    first = values[0]
    return first if all(value == first for value in values[1:]) else _VARIES


def _format_number(value: float) -> str:
    return QLocale().toString(float(value), "g", 12)


def _number_text(value: float | str | None) -> str:
    return _format_number(value) if isinstance(value, (int, float)) else tr("*Varies*")


def _arc_sweep(arc: CadArc) -> float:
    return (arc.end_angle - arc.start_angle) % (2.0 * math.pi)


def _replace_geometry_value(
    geometry: object, key: str, value: float
) -> CadLine | CadCircle | CadArc:
    parts = key.split(".")
    if isinstance(geometry, CadLine):
        start = list(geometry.start)
        end = list(geometry.end)
        point = start if parts[1] == "start" else end
        point[int(parts[2])] = value
        return CadLine(start, end)
    if isinstance(geometry, CadCircle):
        center = list(geometry.center)
        if parts[1] == "center":
            center[int(parts[2])] = value
            return CadCircle(center, geometry.radius)
        return CadCircle(center, value)
    if isinstance(geometry, CadArc):
        center = list(geometry.center)
        radius = geometry.radius
        start_angle = geometry.start_angle
        end_angle = geometry.end_angle
        if parts[1] == "center":
            center[int(parts[2])] = value
        elif parts[1] == "radius":
            radius = value
        elif parts[1] == "start_angle":
            start_angle = math.radians(value)
        elif parts[1] == "end_angle":
            end_angle = math.radians(value)
        return CadArc(center, radius, start_angle, end_angle)
    raise ValueError(str(value))


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
            events.selection_changed,
        ):
            signal.connect(self.refresh)
        self.grid.property_edited.connect(self._edit_property)
        self.retranslate()

    def retranslate(self) -> None:
        self.setWindowTitle(tr("Properties"))
        self.refresh()

    def refresh(self, event: Event | None = None) -> None:
        document = self._session.active
        if document is None or not document.is_open:
            self.grid.show_sections(tr("No drawing open."), [])
        elif isinstance(event, SelectionChanged) and event.document_id != document.id:
            return
        else:
            selection = self._session.selection(document)
            if isinstance(event, DocumentChanged) and event.document_id == document.id:
                deleted = set(event.changes.deleted)
                selection = tuple(entity_id for entity_id in selection if entity_id not in deleted)
            if selection:
                caption, sections = selection_properties(document, selection)
                self.grid.show_sections(caption, sections)
            else:
                self.grid.show_sections(tr("Drawing"), drawing_properties(document))

    def _edit_property(self, key: str, value: object) -> None:
        document = self._session.active
        if document is None:
            return
        ids = self._session.selection(document)
        try:
            if key == "layer":
                if isinstance(value, int):
                    layer_id = int(value)
                    self._session.edit(document, lambda doc: doc.set_entity_layer(ids, layer_id))
            elif key == "color":
                if value == _PICK_COLOR:
                    entities = document.get_entities(ids)
                    current = entities[0].color if entities else None
                    picker = ColorPickerDialog(current, parent=self)
                    if picker.exec() != ColorPickerDialog.DialogCode.Accepted:
                        self.refresh()
                        return
                    value = picker.selected_color()
                if isinstance(value, CadColor):
                    color = value
                    self._session.edit(document, lambda doc: doc.set_entity_color(ids, color))
                    remember_color(color)
            elif key.startswith("geometry."):
                self._edit_geometry(document, ids, key, str(value))
            elif key.startswith("text."):
                self._edit_text(document, ids, key, str(value))
        except (CadError, ValueError, OverflowError) as error:
            message = (
                str(error)
                if isinstance(error, CadError)
                else tr("{text} is not a number.", text=value)
            )
            self._session.bus.publish(CommandMessage(tr("Error: {message}", message=message)))
            self.refresh()

    def _edit_geometry(
        self, document: CadDocument, ids: Sequence[int], key: str, text: str
    ) -> None:
        value, valid = QLocale().toDouble(text)
        if not valid or not math.isfinite(value):
            raise ValueError(text)
        with document.group("PROPERTIES") as group:
            for entity in document.get_entities(ids):
                geometry = _replace_geometry_value(entity.geometry, key, value)
                document.set_entity_geometry(entity.id, geometry)
        if group.changes is not None:
            self._session.report(document, group.changes)

    def _edit_text(self, document: CadDocument, ids: Sequence[int], key: str, text: str) -> None:
        """One text property for every selected text, one undo step."""
        if key == "text.content":
            if text == EDIT_IN_DIALOG:
                entities = document.get_entities(ids)
                first = entities[0].text if entities else None
                typed = text_dialog.ask_text(
                    self, tr("Multiline text"), "" if first is None else first.content
                )
                if typed is None:
                    return
                text = typed
            content = text
            self._session.edit(document, lambda doc: doc.set_text(ids, content=content))
            return
        value, valid = QLocale().toDouble(text)
        if not valid or not math.isfinite(value):
            raise ValueError(text)
        if key == "text.height":
            self._session.edit(document, lambda doc: doc.set_text(ids, height=value))
        else:
            angle = math.radians(value)
            self._session.edit(document, lambda doc: doc.set_text(ids, rotation=angle))

    def rows(self) -> list[tuple[str, str]]:
        """Every (name, value) the panel shows, for tests."""
        return [row for _title, rows in self.grid.sections() for row in rows]
