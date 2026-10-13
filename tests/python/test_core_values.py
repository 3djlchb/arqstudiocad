"""The core's value types and exceptions as Python sees them (ADR 0012, step 4.1)."""

import math

import numpy as np
import pytest

from arqstudiocad import core
from arqstudiocad.core import (
    CadArc,
    CadBounds2,
    CadCircle,
    CadColor,
    CadError,
    CadLine,
    InvalidGeometryError,
    InvalidOperationError,
)

# Exceptions


def test_every_error_is_a_cad_error_with_a_stable_code() -> None:
    codes = {
        core.CadError: "cad_error",
        core.InvalidGeometryError: "invalid_geometry",
        core.EntityNotFoundError: "entity_not_found",
        core.LayerNotFoundError: "layer_not_found",
        core.DocumentNotFoundError: "document_not_found",
        core.InvalidOperationError: "invalid_operation",
        core.CadIoError: "io_error",
        core.UnsupportedFormatError: "unsupported_format",
    }
    for cls, code in codes.items():
        assert issubclass(cls, CadError)
        assert issubclass(cls, Exception)
        assert cls.code == code
        assert cls.__module__ == "arqstudiocad._core"
    assert len(set(codes.values())) == len(codes), "codes are unique"


def test_core_errors_reach_python_as_their_class() -> None:
    with pytest.raises(InvalidGeometryError, match="two different points"):
        CadLine((1.0, 1.0), (1.0, 1.0))
    with pytest.raises(InvalidGeometryError) as caught:
        CadCircle((0.0, 0.0), -2.0)
    assert caught.value.code == "invalid_geometry"
    assert str(caught.value), "an English technical message"


# Points


@pytest.mark.parametrize(
    "point",
    [(3.0, 4.0), [3.0, 4.0], (3, 4), range(3, 5)],
    ids=["tuple", "list", "ints", "any-sequence"],
)
def test_any_two_numbers_are_a_point(point: object) -> None:
    line = CadLine((0.0, 0.0), point)  # type: ignore[arg-type]
    assert line.end == (3.0, 4.0)
    assert isinstance(line.end, tuple)
    assert line.length == 5.0


def test_numpy_arrays_are_points() -> None:
    line = CadLine(np.array([0.0, 0.0]), np.array([3, 4], dtype=np.int32))
    assert line.end == (3.0, 4.0)
    with pytest.raises(TypeError, match="two numbers"):
        CadLine((0.0, 0.0), np.zeros(3))


@pytest.mark.parametrize(
    "bad", [(1.0,), (1.0, 2.0, 3.0), "ab", b"ab", bytearray(b"ab"), 5, None, ("a", "b")]
)
def test_a_point_is_exactly_two_numbers(bad: object) -> None:
    with pytest.raises(TypeError, match="two numbers"):
        CadLine((0.0, 0.0), bad)  # type: ignore[arg-type]


@pytest.mark.parametrize("value", [math.nan, math.inf, -math.inf])
def test_non_finite_coordinates_are_invalid_geometry(value: float) -> None:
    with pytest.raises(InvalidGeometryError):
        CadLine((0.0, 0.0), (value, 1.0))


# Colors


def test_colors() -> None:
    assert CadColor.by_layer().kind == "bylayer"
    assert CadColor.by_block().kind == "byblock"
    red = CadColor.from_index(1)
    assert (red.kind, red.index, red.rgb) == ("index", 1, None)
    wood = CadColor.from_rgb(200, 120, 40)
    assert (wood.kind, wood.index, wood.rgb) == ("rgb", None, (200, 120, 40))
    assert CadColor.from_index(7) != CadColor.from_rgb(7, 7, 7)
    assert CadColor.from_index(7) == CadColor.from_index(7)
    assert len({CadColor.from_index(7), CadColor.from_index(7), CadColor.by_layer()}) == 2
    assert repr(wood) == "CadColor.from_rgb(200, 120, 40)"
    assert repr(CadColor.from_index(255)) == "CadColor.from_index(255)"


def test_display_rgb_is_the_color_on_screen() -> None:
    assert CadColor.from_index(1).display_rgb() == (255, 0, 0)
    assert CadColor.from_rgb(200, 120, 40).display_rgb() == (200, 120, 40)
    # Index 7 is white on the dark default background, black on white paper.
    assert CadColor.from_index(7).display_rgb() == (255, 255, 255)
    assert CadColor.from_index(7).display_rgb((255, 255, 255)) == (0, 0, 0)
    # BYLAYER and BYBLOCK have no color of their own: shown as index 7.
    assert CadColor.by_layer().display_rgb() == (255, 255, 255)


@pytest.mark.parametrize("index", [0, 256, -1])
def test_color_index_is_1_to_255(index: int) -> None:
    with pytest.raises(InvalidOperationError, match="1 to 255"):
        CadColor.from_index(index)


def test_rgb_components_are_bytes() -> None:
    with pytest.raises(OverflowError):
        CadColor.from_rgb(256, 0, 0)


# Geometry and bounds


def test_curves_are_immutable_values() -> None:
    line = CadLine((0.0, 0.0), (10.0, 0.0))
    with pytest.raises(AttributeError):
        line.start = (1.0, 1.0)  # type: ignore[misc]
    assert line == CadLine([0, 0], [10, 0])
    assert hash(line) == hash(CadLine((0.0, 0.0), (10.0, 0.0)))
    assert hash(CadLine((-0.0, 0.0), (1.0, 0.0))) == hash(CadLine((0.0, 0.0), (1.0, 0.0)))
    assert repr(line) == "CadLine(start=(0.0, 0.0), end=(10.0, 0.0))"


def test_arc_is_counterclockwise_in_radians() -> None:
    arc = CadArc((0.0, 0.0), 2.0, 0.0, math.pi / 2)
    assert arc.start_point == pytest.approx((2.0, 0.0))
    assert arc.end_point == pytest.approx((0.0, 2.0))
    assert arc.length == pytest.approx(math.pi)
    # Angles are normalized to [0, 2π).
    assert CadArc((0.0, 0.0), 1.0, -math.pi / 2, 0.0).start_angle == pytest.approx(3 * math.pi / 2)


def test_curves_support_match() -> None:
    def describe(shape: CadLine | CadCircle | CadArc) -> str:
        match shape:
            case CadLine(start, end):
                return f"line {start}->{end}"
            case CadCircle(center=c, radius=r):
                return f"circle {c} r={r}"
            case CadArc(_, radius, start_angle, end_angle):
                return f"arc r={radius} {start_angle:.1f}-{end_angle:.1f}"

    assert describe(CadLine((0, 0), (1, 1))) == "line (0.0, 0.0)->(1.0, 1.0)"
    assert describe(CadCircle((2, 3), 4)) == "circle (2.0, 3.0) r=4.0"
    assert describe(CadArc((0, 0), 1, 0.5, 1.5)) == "arc r=1.0 0.5-1.5"


def test_bounds_order_their_corners() -> None:
    box = CadBounds2((10.0, -2.0), (4.0, 6.0))
    assert (box.min, box.max) == ((4.0, -2.0), (10.0, 6.0))
    assert (box.width, box.height, box.center) == (6.0, 8.0, (7.0, 2.0))
    assert box == CadBounds2((4.0, 6.0), (10.0, -2.0))
    match box:
        case CadBounds2(low, high):
            assert (low, high) == ((4.0, -2.0), (10.0, 6.0))
