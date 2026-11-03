"""Measuring solids through the Python API (ADR 0024 step 3): a drawing's
solids, alone and together, inside block references, and the entities
refused, each with why."""

import math
from pathlib import Path

import pytest
from test_commands import regression_drawing

from arqstudiocad.core import CadApplication, CadDocument, EntityNotFoundError

CORPUS = Path(__file__).resolve().parents[2] / "target/corpus/libredwg"
ATMOS = CORPUS / "2007/ATMOS-DC22S.dwg"
EXAMPLE = CORPUS / "example_2000.dwg"
#: ED: a tube of radii 19 and 16, 70 long, along x, around (828, 485, 930).
TUBE = 0xED


def opened(path: Path) -> CadDocument:
    return CadApplication().open_document(regression_drawing(path))


def test_one_solid_measured() -> None:
    doc = opened(ATMOS)
    measures = doc.measure_solids([TUBE])
    assert (measures.solids, measures.refused) == (1, [])
    total = measures.total
    assert total is not None
    assert total.volume == pytest.approx(math.pi * 7350, rel=1e-10)
    # Its outer and inner sides, and its two ring-shaped ends.
    ends = 2 * math.pi * (19**2 - 16**2)
    assert total.area == pytest.approx(2 * math.pi * (19 + 16) * 70 + ends, rel=1e-10)
    assert total.centroid == pytest.approx((828.0, 485.0, 930.0), abs=1e-6)
    low, high = total.bounds
    assert low == pytest.approx((793.0, 466.0, 911.0))
    assert high == pytest.approx((863.0, 504.0, 949.0))


def test_several_solids_together() -> None:
    doc = opened(ATMOS)
    ids = [TUBE, 0xFC]  # FC: the same tube, lower down
    each = [doc.measure_solids([i]).total for i in ids]
    together = doc.measure_solids([*ids, TUBE]).total
    assert all(each) and together is not None
    assert doc.measure_solids(ids).solids == 2
    volumes = [m.volume for m in each if m]
    assert together.volume == pytest.approx(sum(volumes))
    centroids = [m.centroid[2] for m in each if m]
    weighted = sum(v * z for v, z in zip(volumes, centroids, strict=True)) / sum(volumes)
    assert together.centroid[2] == pytest.approx(weighted)


def test_a_block_reference_measures_what_it_holds() -> None:
    doc = opened(ATMOS)
    doc.create_block("TUBO", (828.0, 485.0), [TUBE])
    placed, _ = doc.insert_block("TUBO", (0.0, 0.0), scale=2.0, rotation=math.pi / 2)
    total = doc.measure_solids([placed]).total
    assert total is not None
    assert total.volume == pytest.approx(8 * math.pi * 7350, rel=1e-10)
    # Turned a quarter about z: the tube now runs along y.
    low, high = total.bounds
    assert high[1] - low[1] == pytest.approx(140.0)
    assert high[0] - low[0] == pytest.approx(76.0)


def test_what_is_not_measured_says_why() -> None:
    doc = opened(EXAMPLE)
    regions = [e.id for e in doc.get_entities() if e.kind == "REGION"]
    line, _ = doc.add_line((0, 0), (1, 0))
    measures = doc.measure_solids([*regions, line])
    assert measures.total is None and measures.solids == 0
    assert [(i, why) for i, why, _ in measures.refused] == [
        *((r, "not_closed") for r in regions),
        (line, "not_a_solid"),
    ]
    assert all(detail for _, _, detail in measures.refused)
    with pytest.raises(EntityNotFoundError):
        doc.measure_solids([999_999_999])
