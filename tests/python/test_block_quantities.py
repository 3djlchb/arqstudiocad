"""The budget's quantities (ADR 0028 D3, D5): each block's placements in
model space, nested references multiplied by the placements of the block
holding them, for the whole drawing or for the selected references."""

import pytest

from arqstudiocad.core import CadApplication, CadDocument, EntityNotFoundError


def furnish(document: CadDocument) -> tuple[int, int, int]:
    """JUEGO holds four SILLA and is placed three times, a line beside them.
    Returns a set's id, another set's id and the line's."""
    a, _ = document.add_line((0, 0), (1, 0))
    chair, _ = document.create_block("SILLA", (0, 0), [a])
    chairs = [chair] + [document.insert_block("SILLA", (x, 0))[0] for x in (2, 4, 6)]
    first, _ = document.create_block("JUEGO", (0, 0), chairs)
    second, _ = document.insert_block("JUEGO", (0, 10))
    document.insert_block("juego", (0, 20))
    line, _ = document.add_line((0, 30), (5, 30))
    return first, second, line


def test_nested_references_count_for_each_placement_of_their_holder() -> None:
    document = CadApplication().create_document("a")
    furnish(document)
    assert document.block_quantities == {"JUEGO": 3, "SILLA": 12}
    assert document.block_references == {"JUEGO": 3, "SILLA": 4}, (
        "the palette counts references, not placements"
    )


def test_selection_only_counts_the_chosen_references() -> None:
    document = CadApplication().create_document("a")
    first, second, line = furnish(document)
    assert document.block_quantities_of([first, line]) == {"JUEGO": 1, "SILLA": 4}
    assert document.block_quantities_of([first, second, first]) == {
        "JUEGO": 2,
        "SILLA": 8,
    }
    assert document.block_quantities_of([line]) == {}
    with pytest.raises(EntityNotFoundError):
        document.block_quantities_of([first, 999_999])


def test_an_empty_drawing_has_no_quantities() -> None:
    document = CadApplication().create_document("a")
    assert document.block_quantities == {}
    assert document.block_quantities_of([]) == {}
