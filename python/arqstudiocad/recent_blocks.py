"""The blocks used last, for the Blocks palette's Recent source (ADR 0025
D1): created with BLOCK or inserted with INSERT in this session and the
previous one, the newest first, without repeats, at most :data:`LIMIT`.
Each keeps the file its definition came from. No Qt: the palette stores
the list in its settings."""

import json
import time
from collections.abc import Iterable
from dataclasses import dataclass, replace
from pathlib import Path

from arqstudiocad.i18n import tr
from arqstudiocad.recent_files import same_file

#: How many blocks the list keeps (maintainer, 2026-10-25).
LIMIT = 30
#: How a block was used: made with BLOCK, inserted from a drawing, or
#: inserted from the palette's template.
CREATED, INSERTED, FROM_TEMPLATE = "created", "inserted", "template"
HOWS = (CREATED, INSERTED, FROM_TEMPLATE)


@dataclass(frozen=True, slots=True)
class RecentBlock:
    """Block ``name`` of the file at ``path`` (None: a drawing never
    saved), used as ``how`` says at ``when`` (seconds since the epoch) in
    session number ``session``. ``document_id`` is the open document it
    came from, in this session only."""

    name: str
    path: Path | None
    how: str
    when: float
    session: int
    document_id: int | None = None

    def same(self, other: "RecentBlock") -> bool:
        """The same block of the same file (of the same unsaved drawing),
        names compared ignoring case as the drawing does."""
        if self.name.casefold() != other.name.casefold():
            return False
        if self.path is not None and other.path is not None:
            return same_file(self.path, other.path)
        return (
            self.path is None
            and other.path is None
            and self.document_id is not None
            and self.document_id == other.document_id
        )


def used_text(when: float, now: float) -> str:
    """How long ago ``when`` was, for the Recent source's Used column."""
    seconds = max(0.0, now - when)
    if seconds < 60:
        return tr("Just now")
    if seconds < 3600:
        return tr("{count} min ago", count=int(seconds // 60))
    if seconds < 86400:
        return tr("{count} h ago", count=int(seconds // 3600))
    days = int(seconds // 86400)
    return tr("1 day ago") if days == 1 else tr("{count} days ago", count=days)


class RecentBlocks:
    """The recent blocks of session number ``session``, newest first: those
    ``stored`` from earlier sessions keep only the previous one's."""

    def __init__(self, stored: str = "", session: int = 1, limit: int = LIMIT) -> None:
        self.session = session
        self.limit = limit
        self._blocks: list[RecentBlock] = []
        for block in _parsed(stored):
            if block.session >= session - 1 and not any(block.same(b) for b in self._blocks):
                self._blocks.append(block)
        del self._blocks[limit:]

    @property
    def blocks(self) -> list[RecentBlock]:
        return list(self._blocks)

    def add(
        self,
        name: str,
        path: Path | None,
        how: str,
        document_id: int | None = None,
        when: float | None = None,
    ) -> RecentBlock:
        """Block ``name`` was used: it goes first."""
        block = RecentBlock(
            name,
            None if path is None else Path(path),
            how,
            time.time() if when is None else when,
            self.session,
            document_id,
        )
        self._blocks = [block] + [b for b in self._blocks if not b.same(block)]
        del self._blocks[self.limit :]
        return block

    def saved(self, document_id: int, path: Path) -> None:
        """Document ``document_id`` was saved to ``path``: its blocks come
        from that file now."""
        moved = [
            replace(b, path=Path(path)) if b.document_id == document_id else b for b in self._blocks
        ]
        self._blocks = []
        for block in moved:
            if not any(block.same(b) for b in self._blocks):
                self._blocks.append(block)

    def renamed(self, document_id: int, old: str, new: str) -> None:
        """Block ``old`` of document ``document_id`` is called ``new`` now."""
        self._blocks = [
            replace(b, name=new)
            if b.document_id == document_id and b.name.casefold() == old.casefold()
            else b
            for b in self._blocks
        ]

    def closed(self, document_id: int) -> None:
        """Document ``document_id`` was closed: the blocks of a drawing
        never saved leave the list, since no file holds them (maintainer,
        2026-10-25); the others are found by their file from now on."""
        self._blocks = [
            replace(b, document_id=None) if b.document_id == document_id else b
            for b in self._blocks
            if b.document_id != document_id or b.path is not None
        ]

    def to_json(self) -> str:
        """The list as the settings store it: only blocks a file holds."""
        return json.dumps(
            [
                {
                    "name": b.name,
                    "path": str(b.path),
                    "how": b.how,
                    "when": b.when,
                    "session": b.session,
                }
                for b in self._blocks
                if b.path is not None
            ]
        )


def _parsed(stored: str) -> Iterable[RecentBlock]:
    """The blocks ``stored`` holds; what cannot be read is left out."""
    try:
        items = json.loads(stored) if stored else []
    except ValueError:
        return
    if not isinstance(items, list):
        return
    for item in items:
        if not isinstance(item, dict):
            continue
        name, path, how = item.get("name"), item.get("path"), item.get("how")
        when, session = item.get("when"), item.get("session")
        if (
            isinstance(name, str)
            and name
            and isinstance(path, str)
            and path
            and how in HOWS
            and isinstance(when, int | float)
            and isinstance(session, int)
        ):
            yield RecentBlock(name, Path(path), how, float(when), session)
