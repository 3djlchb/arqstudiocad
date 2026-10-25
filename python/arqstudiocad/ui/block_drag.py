"""Dragging a block from the Blocks palette onto a drawing (ADR 0025 D5):
what the drag carries, in an application-specific MIME type, and how the
viewport reads it back."""

import json

from PySide6.QtCore import QMimeData

#: The drag's MIME type: a block of a drawing, never mistaken for a file.
BLOCK_MIME = "application/x-arqstudiocad-block"


def block_mime(document_id: int, name: str) -> QMimeData:
    """The drag of block ``name`` of the drawing ``document_id``."""
    mime = QMimeData()
    mime.setData(BLOCK_MIME, json.dumps({"document": document_id, "name": name}).encode())
    return mime


def dragged_block(mime: QMimeData) -> tuple[int, str] | None:
    """The drawing and block a drag carries, or None if it carries none."""
    if not mime.hasFormat(BLOCK_MIME):
        return None
    try:
        data = json.loads(bytes(mime.data(BLOCK_MIME).data()).decode())
        document, name = data["document"], data["name"]
    except (ValueError, KeyError, TypeError):
        return None
    if not isinstance(document, int) or not isinstance(name, str) or not name:
        return None
    return document, name
