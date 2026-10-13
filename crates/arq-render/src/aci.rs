//! AutoCAD Color Index (ACI) to RGB.
//!
//! Indices 1–9 and the grays 250–255 have fixed values; 10–249 follow the
//! ACI layout: 24 hues 15° apart, each in five shades, each shade full and
//! pastel. The shades are computed from that layout rather than copied from
//! a table, so they are close to, not identical with, every AutoCAD version.

/// RGB of an ACI index; index 7 (white/black) depends on the background,
/// see [`aci_rgb_on`]. Index 0 (ByBlock) and out-of-range values give 7.
pub fn aci_rgb(index: u8) -> [u8; 3] {
    match index {
        1 => [255, 0, 0],
        2 => [255, 255, 0],
        3 => [0, 255, 0],
        4 => [0, 255, 255],
        5 => [0, 0, 255],
        6 => [255, 0, 255],
        8 => [128, 128, 128],
        9 => [192, 192, 192],
        10..=249 => shaded_hue(index),
        250 => [51, 51, 51],
        251 => [91, 91, 91],
        252 => [132, 132, 132],
        253 => [173, 173, 173],
        254 => [214, 214, 214],
        _ => [255, 255, 255], // 7, 255 and anything else
    }
}

/// RGB of an ACI index drawn on `background`: index 7 is white on a dark
/// background and black on a light one, as in AutoCAD.
pub fn aci_rgb_on(index: u8, background: [u8; 3]) -> [u8; 3] {
    if index == 7 || index == 0 {
        if luminance(background) > 0.5 {
            [0, 0, 0]
        } else {
            [255, 255, 255]
        }
    } else {
        aci_rgb(index)
    }
}

/// Relative luminance of an sRGB color, 0 (black) to 1 (white).
pub fn luminance(c: [u8; 3]) -> f64 {
    (0.2126 * f64::from(c[0]) + 0.7152 * f64::from(c[1]) + 0.0722 * f64::from(c[2])) / 255.0
}

/// Indices 10–249: hue from the tens, shade from the units (even = full
/// color, odd = pastel; value falls from 1.0 to 0.3 every two units).
fn shaded_hue(index: u8) -> [u8; 3] {
    let hue = f64::from(index / 10 - 1) * 15.0;
    let unit = index % 10;
    let value = [1.0, 0.8, 0.6, 0.5, 0.3][usize::from(unit / 2)];
    let pastel = unit % 2 == 1;
    let [r, g, b] = hsv(hue, 1.0, value);
    let mix = |c: f64| if pastel { c + (value - c) * 0.5 } else { c };
    [mix(r), mix(g), mix(b)].map(|c| (c * 255.0).round() as u8)
}

/// HSV (hue in degrees) to RGB in 0..=1.
fn hsv(hue: f64, saturation: f64, value: f64) -> [f64; 3] {
    let c = value * saturation;
    let h = (hue % 360.0) / 60.0;
    let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = value - c;
    [r + m, g + m, b + m]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_colors() {
        assert_eq!(aci_rgb(1), [255, 0, 0]);
        assert_eq!(aci_rgb(5), [0, 0, 255]);
        assert_eq!(aci_rgb(255), [255, 255, 255]);
        let grays: Vec<u8> = (250..=255).map(|i| aci_rgb(i)[0]).collect();
        assert!(grays.windows(2).all(|w| w[0] < w[1]), "{grays:?}");
    }

    #[test]
    fn the_hue_layout() {
        // Hue = (index / 10 - 1) * 15 degrees.
        assert_eq!(aci_rgb(10), [255, 0, 0], "0°, full");
        assert_eq!(aci_rgb(50), [255, 255, 0], "60°, yellow");
        assert_eq!(aci_rgb(130), [0, 255, 255], "180°, cyan");
        assert_eq!(
            aci_rgb(11),
            [255, 128, 128],
            "odd: pastel, halfway to white"
        );
        assert_eq!(aci_rgb(18), [77, 0, 0], "units 8: value 0.3");
    }

    #[test]
    fn white_follows_the_background() {
        assert_eq!(aci_rgb_on(7, [33, 40, 48]), [255, 255, 255]);
        assert_eq!(aci_rgb_on(7, [250, 250, 250]), [0, 0, 0]);
        assert_eq!(aci_rgb_on(1, [250, 250, 250]), [255, 0, 0]);
    }
}
