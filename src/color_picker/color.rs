//! Helper functions and structs for picking colors.
//!
//! Ported from `iced_aw`'s `core::color` module.

use iced::Color;

/// A color in the HSV color space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsv {
    /// HSV hue.
    pub(crate) hue: u16,
    /// HSV Saturation.
    pub(crate) saturation: f32,
    /// HSV value.
    pub(crate) value: f32,
}

impl Hsv {
    /// Creates a [`Hsv`] from its HSV components.
    #[must_use]
    pub const fn from_hsv(hue: u16, saturation: f32, value: f32) -> Self {
        Self {
            hue,
            saturation,
            value,
        }
    }
}

/// Creates a string of hexadecimal characters.
pub trait HexString {
    /// Turns self into a string of hexadecimal characters.
    fn as_hex_string(&self) -> String;
}

impl HexString for Color {
    fn as_hex_string(&self) -> String {
        format!(
            "#{:02X?}{:02X?}{:02X?}{:02X?}",
            (self.r * 255.0) as u8,
            (self.g * 255.0) as u8,
            (self.b * 255.0) as u8,
            (self.a * 255.0) as u8,
        )
    }
}

impl From<Color> for Hsv {
    // https://de.wikipedia.org/wiki/HSV-Farbraum#Umrechnung_RGB_in_HSV/HSL
    fn from(color: Color) -> Self {
        let max = color.r.max(color.g.max(color.b));
        let min = color.r.min(color.g.min(color.b));

        let hue = if (max - min).abs() < f32::EPSILON {
            0.0
        } else if (max - color.r).abs() < f32::EPSILON {
            60.0 * (0.0 + (color.g - color.b) / (max - min))
        } else if (max - color.g).abs() < f32::EPSILON {
            60.0 * (2.0 + (color.b - color.r) / (max - min))
        } else {
            60.0 * (4.0 + (color.r - color.g) / (max - min))
        };

        let hue = if hue < 0.0 { hue + 360.0 } else { hue } as u16 % 360;

        let saturation = if max == 0.0 { 0.0 } else { (max - min) / max };

        let value = max;

        Self {
            hue,
            saturation,
            value,
        }
    }
}

impl From<Hsv> for Color {
    fn from(hsv: Hsv) -> Self {
        // https://de.wikipedia.org/wiki/HSV-Farbraum#Umrechnung_HSV_in_RGB
        let h_i = (f32::from(hsv.hue) / 60.0).floor();
        let f = (f32::from(hsv.hue) / 60.0) - h_i;

        let p = hsv.value * (1.0 - hsv.saturation);
        let q = hsv.value * (1.0 - hsv.saturation * f);
        let t = hsv.value * (1.0 - hsv.saturation * (1.0 - f));

        let h_i = h_i as u8;
        let (red, green, blue) = match h_i {
            1 => (q, hsv.value, p),
            2 => (p, hsv.value, t),
            3 => (p, q, hsv.value),
            4 => (t, p, hsv.value),
            5 => (hsv.value, p, q),
            _ => (hsv.value, t, p),
        };

        Self::from_rgb(red, green, blue)
    }
}

/// Normalizes an angle in degrees to `0..360`.
///
/// Mirrors the Python reference: `int(angle_deg + 360) % 360` (truncation
/// toward zero, wrap-around).
#[must_use]
pub fn hue_from_angle(angle_deg: f32) -> u16 {
    ((angle_deg + 360.0) as i32).rem_euclid(360) as u16
}

/// Parses hex color digits into `(r, g, b, a)` bytes.
///
/// Accepts lengths 3 (RGB), 4 (RGBA), 6 (RRGGBB) and 8 (RRGGBBAA). Shorter
/// forms expand each nibble (e.g. `"f80"` → `0xFF 0x88 0x00`). Missing alpha
/// defaults to `255`. The leading `#` (if any) must already be stripped.
#[must_use]
pub fn parse_hex_digits(s: &str) -> Option<(u8, u8, u8, u8)> {
    let digits: Vec<char> = s.chars().collect();
    let expand = matches!(digits.len(), 3 | 4);

    if !matches!(digits.len(), 3 | 4 | 6 | 8) || !digits.iter().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let nibble = |c: char| c.to_digit(16).unwrap_or(0) as u8;
    let byte = |hi: char, lo: char| (nibble(hi) << 4) | nibble(lo);

    let (r, g, b) = if expand {
        (
            byte(digits[0], digits[0]),
            byte(digits[1], digits[1]),
            byte(digits[2], digits[2]),
        )
    } else {
        (
            byte(digits[0], digits[1]),
            byte(digits[2], digits[3]),
            byte(digits[4], digits[5]),
        )
    };

    let a = match digits.len() {
        3 | 6 => 255,
        4 => byte(digits[3], digits[3]),
        _ => byte(digits[6], digits[7]),
    };

    Some((r, g, b, a))
}

/// Returns true if `s` (with optional leading `#`) is a valid hex color:
/// `#RGB`, `#RGBA`, `#RRGGBB` or `#RRGGBBAA`.
#[must_use]
pub fn is_valid_hex(s: &str) -> bool {
    let s = s.strip_prefix('#').unwrap_or(s);
    parse_hex_digits(s).is_some()
}

/// Clamps an `i32` value into a `u8` byte range.
#[must_use]
pub fn clamp_u8(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

/// Clamps an `i32` hue value into `0..360` using Euclidean remainder.
#[must_use]
pub fn clamp_hue(v: i32) -> u16 {
    v.rem_euclid(360) as u16
}

/// Formats a [`Color`] like `QColor.name(HexArgb)`: `#RRGGBBAA`.
#[must_use]
pub fn color_to_hex_argb(color: Color) -> String {
    color.as_hex_string()
}

#[cfg(test)]
mod tests {
    use iced::Color;

    use super::{color_to_hex_argb, hue_from_angle, is_valid_hex, parse_hex_digits};
    use super::Hsv;

    #[test]
    fn hue_from_angle_table() {
        assert_eq!(hue_from_angle(0.0), 0);
        assert_eq!(hue_from_angle(90.0), 90);
        assert_eq!(hue_from_angle(180.0), 180);
        assert_eq!(hue_from_angle(270.0), 270);
        assert_eq!(hue_from_angle(-45.0), 315);
        assert_eq!(hue_from_angle(-1.0), 359);
        assert_eq!(hue_from_angle(360.0), 0);
        assert_eq!(hue_from_angle(405.0), 45);
        assert_eq!(hue_from_angle(-180.0), 180);
    }

    #[allow(clippy::cognitive_complexity)]
    #[test]
    fn rgb_to_hsv() {
        // https://de.wikipedia.org/wiki/HSV-Farbraum#Transformation_von_HSV/HSL_und_RGB
        let red_rgb = Color::from_rgb(1.0, 0.0, 0.0);
        let red_hsv = Hsv::from_hsv(0, 1.0, 1.0);
        assert_eq!(red_hsv, red_rgb.into());

        let orange_rgb = Color::from_rgb(1.0, 0.5, 0.0);
        let orange_hsv = Hsv::from_hsv(30, 1.0, 1.0);
        assert_eq!(orange_hsv, orange_rgb.into());

        let yellow_rgb = Color::from_rgb(1.0, 1.0, 0.0);
        let yellow_hsv = Hsv::from_hsv(60, 1.0, 1.0);
        assert_eq!(yellow_hsv, yellow_rgb.into());

        let dark_green_rgb = Color::from_rgb(0.0, 0.5, 0.0);
        let dark_green_hsv = Hsv::from_hsv(120, 1.0, 0.5);
        assert_eq!(dark_green_hsv, dark_green_rgb.into());

        let violett_rgb = Color::from_rgb(0.5, 0.0, 1.0);
        let violett_hsv = Hsv::from_hsv(270, 1.0, 1.0);
        assert_eq!(violett_hsv, violett_rgb.into());

        let black_rgb = Color::from_rgb(0.0, 0.0, 0.0);
        let black_hsv = Hsv::from_hsv(0, 0.0, 0.0);
        assert_eq!(black_hsv, black_rgb.into());

        let blue_rgb = Color::from_rgb(0.0, 0.0, 1.0);
        let blue_hsv = Hsv::from_hsv(240, 1.0, 1.0);
        assert_eq!(blue_hsv, blue_rgb.into());

        let brown_rgb = Color::from_rgb(0.36, 0.18, 0.09);
        let brown_hsv = Hsv::from_hsv(20, 0.75, 0.36);
        assert_eq!(brown_hsv, brown_rgb.into());

        let white_rgb = Color::from_rgb(1.0, 1.0, 1.0);
        let white_hsv = Hsv::from_hsv(0, 0.0, 1.0);
        assert_eq!(white_hsv, white_rgb.into());

        let green_rgb = Color::from_rgb(0.0, 1.0, 0.0);
        let green_hsv = Hsv::from_hsv(120, 1.0, 1.0);
        assert_eq!(green_hsv, green_rgb.into());

        let cyan_rgb = Color::from_rgb(0.0, 1.0, 1.0);
        let cyan_hsv = Hsv::from_hsv(180, 1.0, 1.0);
        assert_eq!(cyan_hsv, cyan_rgb.into());

        let magenta_rgb = Color::from_rgb(1.0, 0.0, 1.0);
        let magenta_hsv = Hsv::from_hsv(300, 1.0, 1.0);
        assert_eq!(magenta_hsv, magenta_rgb.into());

        let blue_green_rgb = Color::from_rgb(0.0, 1.0, 0.5);
        let blue_green_hsv = Hsv::from_hsv(150, 1.0, 1.0);
        assert_eq!(blue_green_hsv, blue_green_rgb.into());

        let green_blue_rgb = Color::from_rgb(0.0, 0.5, 1.0);
        let green_blue_hsv = Hsv::from_hsv(210, 1.0, 1.0);
        assert_eq!(green_blue_hsv, green_blue_rgb.into());

        let green_yellow_rgb = Color::from_rgb(0.5, 1.0, 0.0);
        let green_yellow_hsv = Hsv::from_hsv(90, 1.0, 1.0);
        assert_eq!(green_yellow_hsv, green_yellow_rgb.into());

        let blue_red_rgb = Color::from_rgb(1.0, 0.0, 0.5);
        let blue_red_hsv = Hsv::from_hsv(330, 1.0, 1.0);
        assert_eq!(blue_red_hsv, blue_red_rgb.into());

        let zinnober_rgb = Color::from_rgb(1.0, 0.25, 0.0);
        let zinnober_hsv = Hsv::from_hsv(15, 1.0, 1.0);
        assert_eq!(zinnober_hsv, zinnober_rgb.into());

        let indigo_rgb = Color::from_rgb(0.25, 0.0, 1.0);
        let indigo_hsv = Hsv::from_hsv(255, 1.0, 1.0);
        assert_eq!(indigo_hsv, indigo_rgb.into());

        let light_blue_green_rgb = Color::from_rgb(0.0, 1.0, 0.25);
        let light_blue_green_hsv = Hsv::from_hsv(135, 1.0, 1.0);
        assert_eq!(light_blue_green_hsv, light_blue_green_rgb.into());

        let blue_cyan_rgb = Color::from_rgb(0.0, 0.75, 1.0);
        let blue_cyan_hsv = Hsv::from_hsv(195, 1.0, 1.0);
        assert_eq!(blue_cyan_hsv, blue_cyan_rgb.into());

        let light_green_yellow_rgb = Color::from_rgb(0.75, 1.0, 0.0);
        let light_green_yellow_hsv = Hsv::from_hsv(75, 1.0, 1.0);
        assert_eq!(light_green_yellow_hsv, light_green_yellow_rgb.into());

        let red_magenta_rgb = Color::from_rgb(1.0, 0.0, 0.75);
        let red_magenta_hsv = Hsv::from_hsv(315, 1.0, 1.0);
        assert_eq!(red_magenta_hsv, red_magenta_rgb.into());

        let safran_rgb = Color::from_rgb(1.0, 0.75, 0.0);
        let safran_hsv = Hsv::from_hsv(45, 1.0, 1.0);
        assert_eq!(safran_hsv, safran_rgb.into());

        let blue_magenta_rgb = Color::from_rgb(0.75, 0.0, 1.0);
        let blue_magenta_hsv = Hsv::from_hsv(285, 1.0, 1.0);
        assert_eq!(blue_magenta_hsv, blue_magenta_rgb.into());

        let green_cyan_rgb = Color::from_rgb(0.0, 1.0, 0.75);
        let green_cyan_hsv = Hsv::from_hsv(165, 1.0, 1.0);
        assert_eq!(green_cyan_hsv, green_cyan_rgb.into());

        let light_green_blue_rgb = Color::from_rgb(0.0, 0.25, 1.0);
        let light_green_blue_hsv = Hsv::from_hsv(225, 1.0, 1.0);
        assert_eq!(light_green_blue_hsv, light_green_blue_rgb.into());

        let lime_rgb = Color::from_rgb(0.25, 1.0, 0.0);
        let lime_hsv = Hsv::from_hsv(105, 1.0, 1.0);
        assert_eq!(lime_hsv, lime_rgb.into());

        let light_blue_red_rgb = Color::from_rgb(1.0, 0.0, 0.25);
        let light_blue_red_hsv = Hsv::from_hsv(345, 1.0, 1.0);
        assert_eq!(light_blue_red_hsv, light_blue_red_rgb.into());
    }

    #[allow(clippy::cognitive_complexity)]
    #[test]
    fn hsv_to_rgb() {
        // https://de.wikipedia.org/wiki/HSV-Farbraum#Transformation_von_HSV/HSL_und_RGB
        let red_hsv = Hsv::from_hsv(0, 1.0, 1.0);
        let red_rgb = Color::from_rgb(1.0, 0.0, 0.0);
        assert_eq!(red_rgb, red_hsv.into());

        let orange_hsv = Hsv::from_hsv(30, 1.0, 1.0);
        let orange_rgb = Color::from_rgb(1.0, 0.5, 0.0);
        assert_eq!(orange_rgb, orange_hsv.into());

        let yellow_hsv = Hsv::from_hsv(60, 1.0, 1.0);
        let yellow_rgb = Color::from_rgb(1.0, 1.0, 0.0);
        assert_eq!(yellow_rgb, yellow_hsv.into());

        let dark_green_hsv = Hsv::from_hsv(120, 1.0, 0.5);
        let dark_green_rgb = Color::from_rgb(0.0, 0.5, 0.0);
        assert_eq!(dark_green_rgb, dark_green_hsv.into());

        let violett_hsv = Hsv::from_hsv(270, 1.0, 1.0);
        let violett_rgb = Color::from_rgb(0.5, 0.0, 1.0);
        assert_eq!(violett_rgb, violett_hsv.into());

        let black_hsv = Hsv::from_hsv(0, 0.0, 0.0);
        let black_rgb = Color::from_rgb(0.0, 0.0, 0.0);
        assert_eq!(black_rgb, black_hsv.into());

        let blue_hsv = Hsv::from_hsv(240, 1.0, 1.0);
        let blue_rgb = Color::from_rgb(0.0, 0.0, 1.0);
        assert_eq!(blue_rgb, blue_hsv.into());

        let brown_hsv = Hsv::from_hsv(20, 0.75, 0.36);
        let brown_rgb = Color::from_rgb(0.36, 0.18, 0.09);
        assert_eq!(brown_rgb, brown_hsv.into());

        let white_hsv = Hsv::from_hsv(0, 0.0, 1.0);
        let white_rgb = Color::from_rgb(1.0, 1.0, 1.0);
        assert_eq!(white_rgb, white_hsv.into());

        let green_hsv = Hsv::from_hsv(120, 1.0, 1.0);
        let green_rgb = Color::from_rgb(0.0, 1.0, 0.0);
        assert_eq!(green_rgb, green_hsv.into());

        let cyan_hsv = Hsv::from_hsv(180, 1.0, 1.0);
        let cyan_rgb = Color::from_rgb(0.0, 1.0, 1.0);
        assert_eq!(cyan_rgb, cyan_hsv.into());

        let magenta_hsv = Hsv::from_hsv(300, 1.0, 1.0);
        let magenta_rgb = Color::from_rgb(1.0, 0.0, 1.0);
        assert_eq!(magenta_rgb, magenta_hsv.into());

        let blue_green_hsv = Hsv::from_hsv(150, 1.0, 1.0);
        let blue_green_rgb = Color::from_rgb(0.0, 1.0, 0.5);
        assert_eq!(blue_green_rgb, blue_green_hsv.into());

        let green_blue_hsv = Hsv::from_hsv(210, 1.0, 1.0);
        let green_blue_rgb = Color::from_rgb(0.0, 0.5, 1.0);
        assert_eq!(green_blue_rgb, green_blue_hsv.into());

        let green_yellow_hsv = Hsv::from_hsv(90, 1.0, 1.0);
        let green_yellow_rgb = Color::from_rgb(0.5, 1.0, 0.0);
        assert_eq!(green_yellow_rgb, green_yellow_hsv.into());

        let blue_red_hsv = Hsv::from_hsv(330, 1.0, 1.0);
        let blue_red_rgb = Color::from_rgb(1.0, 0.0, 0.5);
        assert_eq!(blue_red_rgb, blue_red_hsv.into());

        let zinnober_hsv = Hsv::from_hsv(15, 1.0, 1.0);
        let zinnober_rgb = Color::from_rgb(1.0, 0.25, 0.0);
        assert_eq!(zinnober_rgb, zinnober_hsv.into());

        let indigo_hsv = Hsv::from_hsv(255, 1.0, 1.0);
        let indigo_rgb = Color::from_rgb(0.25, 0.0, 1.0);
        assert_eq!(indigo_rgb, indigo_hsv.into());

        let light_blue_green_hsv = Hsv::from_hsv(135, 1.0, 1.0);
        let light_blue_green_rgb = Color::from_rgb(0.0, 1.0, 0.25);
        assert_eq!(light_blue_green_rgb, light_blue_green_hsv.into());

        let blue_cyan_hsv = Hsv::from_hsv(195, 1.0, 1.0);
        let blue_cyan_rgb = Color::from_rgb(0.0, 0.75, 1.0);
        assert_eq!(blue_cyan_rgb, blue_cyan_hsv.into());

        let light_green_yellow_hsv = Hsv::from_hsv(75, 1.0, 1.0);
        let light_green_yellow_rgb = Color::from_rgb(0.75, 1.0, 0.0);
        assert_eq!(light_green_yellow_rgb, light_green_yellow_hsv.into());

        let red_magenta_hsv = Hsv::from_hsv(315, 1.0, 1.0);
        let red_magenta_rgb = Color::from_rgb(1.0, 0.0, 0.75);
        assert_eq!(red_magenta_rgb, red_magenta_hsv.into());

        let safran_hsv = Hsv::from_hsv(45, 1.0, 1.0);
        let safran_rgb = Color::from_rgb(1.0, 0.75, 0.0);
        assert_eq!(safran_rgb, safran_hsv.into());

        let blue_magenta_hsv = Hsv::from_hsv(285, 1.0, 1.0);
        let blue_magenta_rgb = Color::from_rgb(0.75, 0.0, 1.0);
        assert_eq!(blue_magenta_rgb, blue_magenta_hsv.into());

        let green_cyan_hsv = Hsv::from_hsv(165, 1.0, 1.0);
        let green_cyan_rgb = Color::from_rgb(0.0, 1.0, 0.75);
        assert_eq!(green_cyan_rgb, green_cyan_hsv.into());

        let light_green_blue_hsv = Hsv::from_hsv(225, 1.0, 1.0);
        let light_green_blue_rgb = Color::from_rgb(0.0, 0.25, 1.0);
        assert_eq!(light_green_blue_rgb, light_green_blue_hsv.into());

        let lime_hsv = Hsv::from_hsv(105, 1.0, 1.0);
        let lime_rgb = Color::from_rgb(0.25, 1.0, 0.0);
        assert_eq!(lime_rgb, lime_hsv.into());

        let light_blue_red_hsv = Hsv::from_hsv(345, 1.0, 1.0);
        let light_blue_red_rgb = Color::from_rgb(1.0, 0.0, 0.25);
        assert_eq!(light_blue_red_rgb, light_blue_red_hsv.into());
    }

    #[test]
    fn parse_hex_various_lengths() {
        assert_eq!(parse_hex_digits("f80"), Some((0xFF, 0x88, 0x00, 0xFF)));
        assert_eq!(parse_hex_digits("f80a"), Some((0xFF, 0x88, 0x00, 0xAA)));
        assert_eq!(parse_hex_digits("ff8000"), Some((0xFF, 0x80, 0x00, 0xFF)));
        assert_eq!(
            parse_hex_digits("ff800080"),
            Some((0xFF, 0x80, 0x00, 0x80))
        );
        assert_eq!(parse_hex_digits("000000ff"), Some((0x00, 0x00, 0x00, 0xFF)));
        assert_eq!(parse_hex_digits("ABCDEFAB"), Some((0xAB, 0xCD, 0xEF, 0xAB)));
    }

    #[test]
    fn parse_hex_invalid() {
        assert_eq!(parse_hex_digits("FF8000Z"), None);
        assert_eq!(parse_hex_digits("12"), None);
        assert_eq!(parse_hex_digits("GGGGGG"), None);
        assert_eq!(parse_hex_digits(""), None);
        assert_eq!(parse_hex_digits("FF80000"), None);
        assert_eq!(parse_hex_digits("FFFFF"), None);
    }

    #[test]
    fn is_valid_hex_accepts_optional_hash() {
        assert!(is_valid_hex("#ff8000"));
        assert!(is_valid_hex("ff8000"));
        assert!(is_valid_hex("#FFF"));
        assert!(is_valid_hex("#F80A"));
        assert!(is_valid_hex("#ff800080"));
        assert!(!is_valid_hex("#FF8000Z"));
        assert!(!is_valid_hex("#12"));
        assert!(!is_valid_hex("GGGGGG"));
        assert!(!is_valid_hex(""));
        assert!(!is_valid_hex("#FF80000"));
    }

    #[test]
    fn clamps() {
        assert_eq!(super::clamp_u8(-5), 0);
        assert_eq!(super::clamp_u8(255), 255);
        assert_eq!(super::clamp_u8(300), 255);
        assert_eq!(super::clamp_hue(-1), 359);
        assert_eq!(super::clamp_hue(360), 0);
        assert_eq!(super::clamp_hue(-360), 0);
        assert_eq!(super::clamp_hue(405), 45);
    }

    #[test]
    fn color_to_hex_argb_round_trip() {
        for color in [
            Color::from_rgb(0.5, 0.25, 0.25),
            Color::from_rgba8(255, 128, 0, 0.5),
            Color::from_rgba8(0, 0, 0, 1.0),
            Color::from_rgba8(255, 255, 255, 0.0),
        ] {
            let hex = color_to_hex_argb(color);
            assert!(hex.starts_with('#'));
            assert_eq!(hex.len(), 9);
            let (r, g, b, a) = parse_hex_digits(&hex[1..]).expect("valid hex");
            assert_eq!(
                (r, g, b, a),
                (
                    (color.r * 255.0) as u8,
                    (color.g * 255.0) as u8,
                    (color.b * 255.0) as u8,
                    (color.a * 255.0) as u8,
                )
            );
        }
    }
}