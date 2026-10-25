"""Copying a block from another drawing through the Python API (ADR 0025
D6): the operation the Blocks palette's Template source inserts with."""

import pytest

from arqstudiocad.core import CadApplication, CadDocument, CadError
from arqstudiocad.paths import sample_template


def chair_drawing(app: CadApplication) -> CadDocument:
    """A drawing whose own WC block is a single line."""
    plan = app.create_document("Plano1")
    line, _ = plan.add_line((0, 0), (2, 0))
    reference, _ = plan.create_block("wc", (0, 0), [line])
    plan.delete_entities([reference])
    return plan


def test_a_template_block_is_copied_with_its_layers_in_one_step() -> None:
    app = CadApplication()
    template = app.open_document(sample_template())
    plan = app.create_document("Plano1")
    assert plan.block_clashes("door_090", template) == []
    name, changes = plan.copy_block("door_090", template)
    assert name == "Door_090"
    assert changes.blocks_created == ("Door_090",)
    assert plan.block_names == ["Door_090"]
    layers = {layer.name for layer in plan.get_layers()}
    assert "A-DOOR" in layers, "the layer it is drawn on came along"
    assert plan.can_undo
    plan.undo()
    assert plan.block_names == []
    assert "A-DOOR" not in {layer.name for layer in plan.get_layers()}
    assert template.block_names == sorted(template.block_names, key=str.lower)
    assert not template.is_modified, "the template is only read"


def test_a_clash_needs_a_choice_and_each_choice_does_its_part() -> None:
    app = CadApplication()
    template = app.open_document(sample_template())
    plan = chair_drawing(app)
    assert plan.block_clashes("WC", template) == ["WC"]
    with pytest.raises(CadError, match="WC"):
        plan.copy_block("WC", template)
    assert plan.block_names == ["wc"]

    name, _ = plan.copy_block("WC", template, keep=["WC"])
    assert (name, plan.block_names) == ("wc", ["wc"])

    name, _ = plan.copy_block("WC", template, rename={"WC": "WC_2"})
    assert name == "WC_2"
    assert plan.block_names == ["wc", "WC_2"]

    _, changes = plan.copy_block("WC", template, redefine=["wc"])
    assert changes.blocks_modified == ("wc",)
    assert plan.block_clashes("WC", template) == [], "now the same"


def test_copying_from_itself_or_a_missing_block_is_refused() -> None:
    app = CadApplication()
    template = app.open_document(sample_template())
    plan = app.create_document("Plano1")
    with pytest.raises(CadError):
        plan.copy_block("Door_090", plan)
    with pytest.raises(CadError):
        plan.copy_block("Sofa", template)
    assert plan.block_names == [] and not plan.can_undo
