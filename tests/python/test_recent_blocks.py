"""The Blocks palette's recent blocks (ADR 0025 D1): newest first, one entry
per block of a file, this session and the previous one, at most 30."""

import json
from pathlib import Path

from arqstudiocad.i18n import set_language
from arqstudiocad.recent_blocks import (
    CREATED,
    FROM_TEMPLATE,
    INSERTED,
    LIMIT,
    RecentBlocks,
    used_text,
)


def names(recent: RecentBlocks) -> list[str]:
    return [b.name for b in recent.blocks]


def test_a_block_used_goes_first_once_per_file(tmp_path: Path) -> None:
    recent = RecentBlocks()
    a, b = tmp_path / "a.dwg", tmp_path / "b.dwg"
    recent.add("Mesa", a, CREATED, when=1.0)
    recent.add("Silla", a, INSERTED, when=2.0)
    recent.add("WC", b, FROM_TEMPLATE, when=3.0)
    assert names(recent) == ["WC", "Silla", "Mesa"], "newest first"
    recent.add("MESA", a, INSERTED, when=4.0)
    assert names(recent) == ["MESA", "WC", "Silla"], "the same block: moved, not repeated"
    recent.add("Mesa", b, INSERTED, when=5.0)
    assert names(recent) == ["Mesa", "MESA", "WC", "Silla"], "another file's Mesa is another"
    assert recent.blocks[0].how == INSERTED and recent.blocks[0].when == 5.0


def test_the_list_keeps_thirty_blocks(tmp_path: Path) -> None:
    recent = RecentBlocks()
    for number in range(LIMIT + 5):
        recent.add(f"B{number}", tmp_path / "a.dwg", CREATED)
    assert LIMIT == 30 and len(recent.blocks) == 30
    assert recent.blocks[0].name == "B34" and recent.blocks[-1].name == "B5"


def test_blocks_of_a_drawing_never_saved_are_told_apart_by_document(tmp_path: Path) -> None:
    recent = RecentBlocks()
    recent.add("Mesa", None, CREATED, document_id=1)
    recent.add("Mesa", None, CREATED, document_id=2)
    recent.add("mesa", None, INSERTED, document_id=1)
    assert [(b.name, b.document_id) for b in recent.blocks] == [("mesa", 1), ("Mesa", 2)]


def test_saving_gives_the_blocks_their_file_and_closing_unsaved_drops_them(
    tmp_path: Path,
) -> None:
    recent = RecentBlocks()
    saved = tmp_path / "planta.dwg"
    recent.add("Mesa", saved, INSERTED, document_id=3)
    recent.add("Mesa", None, CREATED, document_id=1)
    recent.add("Silla", None, CREATED, document_id=2)
    recent.saved(1, saved)
    assert [(b.name, b.path) for b in recent.blocks] == [("Silla", None), ("Mesa", saved)], (
        "the saved drawing's Mesa is the file's Mesa: one entry, the newest"
    )
    recent.closed(2)
    recent.closed(1)
    (left,) = recent.blocks
    assert left.name == "Mesa" and left.path == saved
    assert left.document_id is None, "found by its file from now on"


def test_the_settings_keep_this_session_and_the_previous_one(tmp_path: Path) -> None:
    older = RecentBlocks(session=3)
    older.add("Old", tmp_path / "a.dwg", CREATED, when=1.0)
    previous = RecentBlocks(older.to_json(), session=4)
    previous.add("Previous", tmp_path / "a.dwg", INSERTED, when=2.0)
    previous.add("Unsaved", None, CREATED, document_id=7)
    stored = previous.to_json()
    assert [item["name"] for item in json.loads(stored)] == ["Previous", "Old"], (
        "a block no file holds is not kept"
    )
    now = RecentBlocks(stored, session=5)
    (kept,) = now.blocks
    assert kept.name == "Previous" and kept.how == INSERTED and kept.when == 2.0
    assert kept.path == tmp_path / "a.dwg" and kept.session == 4 and kept.document_id is None


def test_what_cannot_be_read_from_the_settings_is_left_out(tmp_path: Path) -> None:
    assert RecentBlocks("not json", session=2).blocks == []
    assert RecentBlocks('{"name": "x"}', session=2).blocks == []
    good = {
        "name": "Mesa",
        "path": str(tmp_path / "a.dwg"),
        "how": CREATED,
        "when": 1,
        "session": 1,
    }
    items = [
        good,
        {**good, "how": "stolen"},
        {**good, "path": ""},
        {**good, "name": 5},
        {**good, "session": "1"},
        "Mesa",
    ]
    assert names(RecentBlocks(json.dumps(items), session=2)) == ["Mesa"]


def test_used_says_how_long_ago() -> None:
    now = 1_000_000.0
    assert used_text(now - 5, now) == "Just now"
    assert used_text(now - 125, now) == "2 min ago"
    assert used_text(now - 3 * 3600 - 10, now) == "3 h ago"
    assert used_text(now - 86400 - 10, now) == "1 day ago"
    assert used_text(now - 3 * 86400, now) == "3 days ago"
    assert used_text(now + 50, now) == "Just now", "a clock set back"
    set_language("es")
    assert used_text(now - 125, now) == "Hace 2 min"
    assert used_text(now - 3 * 86400, now) == "Hace 3 días"
