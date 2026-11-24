"""An object picked on a locked layer, in the commands that edit one picked
object: said as the other commands say it, in the user's language, never
as the core's own error, and nothing changed."""

from collections.abc import Callable

import pytest
from test_offset import Harness

from arqstudiocad.commands.modify import locked_message
from arqstudiocad.i18n import set_language

LOCKED = "1 was on a locked layer."


def drawing() -> tuple[Harness, int, int, int]:
    """Two lines meeting at (10,0), the first on a locked layer, and an
    open polyline on it too."""
    h = Harness()
    a, _ = h.document.add_line((0.0, 0.0), (10.0, 0.0))
    b, _ = h.document.add_line((10.0, 0.0), (10.0, 10.0))
    p, _ = h.document.add_polyline([((0.0, 5.0), 0.0), ((4.0, 5.0), 0.0), ((4.0, 8.0), 0.0)])
    layer, _ = h.document.create_layer("LOCKED")
    h.document.set_entity_layer([a, p], layer)
    h.document.set_layer_locked(layer, True)
    return h, a, b, p


def offset(h: Harness, a: int, b: int, p: int) -> None:
    h.run("OFFSET", "1")
    h.runner.answer_entity(a, (5.0, 0.0))


def fillet_first(h: Harness, a: int, b: int, p: int) -> None:
    h.run("FILLET")
    h.runner.answer_entity(a, (5.0, 0.0))


def fillet_second(h: Harness, a: int, b: int, p: int) -> None:
    h.run("FILLET")
    h.runner.answer_entity(b, (10.0, 5.0))
    h.runner.answer_entity(a, (5.0, 0.0))


def fillet_polyline(h: Harness, a: int, b: int, p: int) -> None:
    h.run("FILLET", "P")
    h.runner.answer_entity(p, (2.0, 5.0))


def chamfer_first(h: Harness, a: int, b: int, p: int) -> None:
    h.run("CHAMFER")
    h.runner.answer_entity(a, (5.0, 0.0))


def chamfer_polyline(h: Harness, a: int, b: int, p: int) -> None:
    h.run("CHAMFER", "P")
    h.runner.answer_entity(p, (2.0, 5.0))


def join_source(h: Harness, a: int, b: int, p: int) -> None:
    h.run("JOIN")
    h.session.select(h.document, [a])
    h.run("")


def break_it(h: Harness, a: int, b: int, p: int) -> None:
    h.run("BREAK")
    h.runner.answer_entity(a, (3.0, 0.0))


Picking = Callable[[Harness, int, int, int], None]


@pytest.mark.parametrize(
    "pick",
    [
        offset,
        fillet_first,
        fillet_second,
        fillet_polyline,
        chamfer_first,
        chamfer_polyline,
        join_source,
        break_it,
    ],
)
def test_an_object_picked_on_a_locked_layer_is_said_so_and_left(pick: Picking) -> None:
    h, a, b, p = drawing()
    before = [(e.id, e.geometry) for e in h.document.get_entities()]
    history = h.document.undo_history
    pick(h, a, b, p)
    assert LOCKED in h.messages()
    assert not any(m.startswith("Error:") for m in h.messages()), h.messages()
    if h.runner.active is not None:
        h.runner.cancel()
    assert [(e.id, e.geometry) for e in h.document.get_entities()] == before
    assert h.document.undo_history == history


def test_the_count_reads_one_way_for_one_and_another_for_more() -> None:
    assert locked_message(1) == "1 was on a locked layer."
    assert locked_message(3) == "3 were on a locked layer."
    try:
        assert set_language("es")
        assert locked_message(1) == "1 estaba en una capa bloqueada."
        assert locked_message(3) == "3 estaban en una capa bloqueada."
    finally:
        set_language("en")
