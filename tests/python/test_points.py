"""Typing points as AutoCAD reads them, and ORTHO (ADR 0016 D6, D5)."""

import pytest

from arqstudiocad.points import PointSyntaxError, ortho, parse_point, snap_key


@pytest.mark.parametrize(
    ("text", "expected"),
    [
        ("3,4", (3.0, 4.0)),
        (" -1.5 , 2e3 ", (-1.5, 2000.0)),
        ("3,4,7", (3.0, 4.0)),  # Z is dropped: this is the plan
        ("@2,-1", (12.0, 4.0)),
        ("@", (10.0, 5.0)),
        ("@5<90", (10.0, 10.0)),
        ("@2<180", (8.0, 5.0)),
        ("10<0", (10.0, 0.0)),
        ("2<90", (0.0, 2.0)),
    ],
)
def test_absolute_relative_and_polar_points(text: str, expected: tuple[float, float]) -> None:
    assert parse_point(text, last=(10.0, 5.0)) == pytest.approx(expected)


def test_a_lone_distance_goes_toward_the_cursor() -> None:
    assert parse_point("5", last=(0, 0), toward=(3, 4)) == pytest.approx((3.0, 4.0))
    assert parse_point("10", last=(1, 1), toward=(1, -100)) == pytest.approx((1.0, -9.0))


@pytest.mark.parametrize(
    ("text", "last", "toward", "message"),
    [
        ("", None, None, "Enter a point."),
        ("@1,1", None, None, "There is no last point."),
        ("@abc", (0, 0), None, "After @, type dx,dy or distance<angle."),
        ("a,b", None, None, "a is not a number."),
        ("1,2,3,4", None, None, "A point is x,y."),
        ("nan,1", None, None, "nan is not a number."),
        ("@1e308<0", (1e308, 0), None, "That point is out of range."),
        ("5", None, (1, 1), "A distance alone needs a last point."),
        ("5", (0, 0), None, "Move the cursor to show the direction."),
        ("5", (0, 0), (0, 0), "Move the cursor to show the direction."),
    ],
)
def test_what_is_not_a_point_says_why(
    text: str,
    last: tuple[float, float] | None,
    toward: tuple[float, float] | None,
    message: str | None,
) -> None:
    with pytest.raises(PointSyntaxError) as raised:
        parse_point(text, last=last, toward=toward)
    if message is not None:
        assert str(raised.value) == message


def test_ortho_keeps_the_point_horizontal_or_vertical() -> None:
    assert ortho((0, 0), (10, 3)) == (10, 0)
    assert ortho((0, 0), (2, -9)) == (0, -9)
    assert ortho((5, 5), (8, 8)) == (8, 5), "a tie goes horizontal"


def test_snap_keys_ignore_case() -> None:
    assert snap_key(" end ") == "END"
    assert snap_key("Int") == "INT"
    assert snap_key("ENDP") is None
