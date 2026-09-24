//! Gradient model for the [`ColorPicker`](crate::color_picker::ColorPicker).
//!
//! A v1 linear gradient with exactly two stops. Offsets are in `0..=1` and
//! kept sorted with a small minimum gap so the two handles can never cross.

use iced::{gradient, Color, Radians};

/// Minimum gap between the two stop offsets.
pub const MIN_STOP_GAP: f32 = 0.02;

/// A single gradient stop: a color at a position along the bar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientStop {
    /// Position along the bar in `0..=1`.
    pub offset: f32,
    /// The color of the stop.
    pub color: Color,
}

impl GradientStop {
    /// Creates a stop, clamping `offset` to `0..=1`.
    #[must_use]
    pub fn new(offset: f32, color: Color) -> Self {
        Self {
            offset: offset.clamp(0.0, 1.0),
            color,
        }
    }
}

/// A picked value: either a solid color or a two-stop gradient.
///
/// Treating the gradient as another type of color lets previews, swatches,
/// recent colors and submit/change callbacks share one code path: a solid
/// is just a gradient with two identical stops.
#[derive(Clone, Debug, PartialEq)]
pub enum PickedValue {
    /// A solid color.
    Solid(Color),
    /// A two-stop linear gradient.
    Gradient(Gradient),
}

impl PickedValue {
    /// Returns the value as a gradient: solids become two identical stops.
    #[must_use]
    pub fn as_gradient(&self) -> Gradient {
        match self {
            Self::Solid(color) => Gradient::two(*color, *color),
            Self::Gradient(gradient) => gradient.clone(),
        }
    }

    /// Converts to an iced [`Background`](iced::Background): solids become
    /// `Background::Color`, gradients become a horizontal linear gradient.
    #[must_use]
    pub fn to_background(&self) -> iced::Background {
        match self {
            Self::Solid(color) => iced::Background::Color(*color),
            Self::Gradient(gradient) => iced::Background::Gradient(
                iced::gradient::Gradient::Linear(gradient.to_linear()),
            ),
        }
    }

    /// Returns the solid color, if any.
    #[must_use]
    pub fn as_solid(&self) -> Option<Color> {
        match self {
            Self::Solid(color) => Some(*color),
            Self::Gradient(_) => None,
        }
    }

    /// Returns the gradient, if any.
    #[must_use]
    pub fn as_gradient_ref(&self) -> Option<&Gradient> {
        match self {
            Self::Solid(_) => None,
            Self::Gradient(gradient) => Some(gradient),
        }
    }

    /// Whether this is a solid color.
    #[must_use]
    pub fn is_solid(&self) -> bool {
        matches!(self, Self::Solid(_))
    }

    /// Whether this is a gradient.
    #[must_use]
    pub fn is_gradient(&self) -> bool {
        matches!(self, Self::Gradient(_))
    }
}

impl From<Color> for PickedValue {
    fn from(color: Color) -> Self {
        Self::Solid(color)
    }
}

impl From<Gradient> for PickedValue {
    fn from(gradient: Gradient) -> Self {
        Self::Gradient(gradient)
    }
}

/// A two-stop linear gradient.
#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    /// The stops, always length 2 and sorted by offset.
    pub stops: Vec<GradientStop>,
}

impl Gradient {
    /// Creates a gradient from two colors at offsets 0 and 1.
    #[must_use]
    pub fn two(a: Color, b: Color) -> Self {
        Self {
            stops: vec![GradientStop::new(0.0, a), GradientStop::new(1.0, b)],
        }
    }

    /// Creates a gradient from two explicit stops, clamping and sorting them.
    #[must_use]
    pub fn from_stops(a: GradientStop, b: GradientStop) -> Self {
        let mut stops = vec![a, b];
        for stop in &mut stops {
            stop.offset = stop.offset.clamp(0.0, 1.0);
        }
        stops.sort_by(|x, y| x.offset.total_cmp(&y.offset));
        Self::enforce_gap(&mut stops);
        Self { stops }
    }

    /// Returns the stop at `index`, if any.
    #[must_use]
    pub fn stop(&self, index: usize) -> Option<GradientStop> {
        self.stops.get(index).copied()
    }

    /// Sets a stop's color.
    pub fn set_stop_color(&mut self, index: usize, color: Color) {
        if let Some(stop) = self.stops.get_mut(index) {
            stop.color = color;
        }
    }

    /// Sets a stop's offset, clamped to `0..=1` with anti-crossing.
    pub fn set_stop_offset(&mut self, index: usize, offset: f32) {
        if self.stops.len() != 2 || index > 1 {
            return;
        }
        let offset = offset.clamp(0.0, 1.0);
        match index {
            0 => {
                self.stops[0].offset = offset.min(self.stops[1].offset - MIN_STOP_GAP).max(0.0);
            }
            _ => {
                self.stops[1].offset = offset.max(self.stops[0].offset + MIN_STOP_GAP).min(1.0);
            }
        }
    }

    /// Samples the gradient at `t` in `0..=1` (linear RGBA lerp).
    #[must_use]
    pub fn sample(&self, t: f32) -> Color {
        if self.stops.len() != 2 {
            return Color::BLACK;
        }
        let (a, b) = (self.stops[0], self.stops[1]);
        let span = (b.offset - a.offset).max(f32::EPSILON);
        let t = ((t - a.offset) / span).clamp(0.0, 1.0);
        lerp_color(a.color, b.color, t)
    }

    /// Converts to an iced horizontal linear gradient for use as a
    /// [`Background`](iced::Background) (e.g. preview swatches in examples).
    #[must_use]
    pub fn to_linear(&self) -> gradient::Linear {
        let mut linear =
            gradient::Linear::new(Radians(std::f32::consts::FRAC_PI_2));
        for stop in &self.stops {
            linear = linear.add_stop(stop.offset, stop.color);
        }
        linear
    }

    fn enforce_gap(stops: &mut [GradientStop]) {
        if stops.len() != 2 {
            return;
        }
        if stops[1].offset - stops[0].offset < MIN_STOP_GAP {
            if stops[0].offset + MIN_STOP_GAP <= 1.0 {
                stops[1].offset = stops[0].offset + MIN_STOP_GAP;
            } else {
                stops[0].offset = stops[1].offset - MIN_STOP_GAP;
            }
        }
    }
}

/// Linearly interpolates between two colors.
#[must_use]
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

/// Returns true if two picked values have identical RGBA bytes (solids) or
/// identical stop offsets + stop RGBA bytes (gradients).
#[must_use]
pub fn same_picked(a: &PickedValue, b: &PickedValue) -> bool {
    match (a, b) {
        (PickedValue::Solid(x), PickedValue::Solid(y)) => same_rgba_bytes(*x, *y),
        (PickedValue::Gradient(x), PickedValue::Gradient(y)) => {
            x.stops.len() == y.stops.len()
                && x.stops.iter().zip(y.stops.iter()).all(|(s, o)| {
                    (s.offset - o.offset).abs() < f32::EPSILON
                        && same_rgba_bytes(s.color, o.color)
                })
        }
        _ => false,
    }
}

#[must_use]
fn same_rgba_bytes(a: Color, b: Color) -> bool {
    (a.r * 255.0) as u8 == (b.r * 255.0) as u8
        && (a.g * 255.0) as u8 == (b.g * 255.0) as u8
        && (a.b * 255.0) as u8 == (b.b * 255.0) as u8
        && (a.a * 255.0) as u8 == (b.a * 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_stop_endpoints() {
        let g = Gradient::two(Color::BLACK, Color::WHITE);
        assert_eq!(g.sample(0.0), Color::BLACK);
        assert_eq!(g.sample(1.0), Color::WHITE);
    }

    #[test]
    fn sample_midpoint() {
        let g = Gradient::two(Color::BLACK, Color::WHITE);
        let mid = g.sample(0.5);
        assert!((mid.r - 0.5).abs() < 0.01);
    }

    #[test]
    fn offsets_clamped_and_sorted() {
        let g = Gradient::from_stops(
            GradientStop::new(0.9, Color::BLACK),
            GradientStop::new(0.1, Color::WHITE),
        );
        assert!(g.stops[0].offset <= g.stops[1].offset);
    }

    #[test]
    fn stops_cannot_cross() {
        let mut g = Gradient::two(Color::BLACK, Color::WHITE);
        g.set_stop_offset(0, 1.0);
        assert!(g.stops[0].offset <= g.stops[1].offset - MIN_STOP_GAP + f32::EPSILON);
        g.set_stop_offset(1, 0.0);
        assert!(g.stops[1].offset >= g.stops[0].offset + MIN_STOP_GAP - f32::EPSILON);
    }

    #[test]
    fn picked_solid_as_gradient_is_solid() {
        let solid = PickedValue::Solid(Color::from_rgb(0.2, 0.4, 0.6));
        let gradient = solid.as_gradient();
        assert_eq!(gradient.sample(0.0), Color::from_rgb(0.2, 0.4, 0.6));
        assert_eq!(gradient.sample(1.0), Color::from_rgb(0.2, 0.4, 0.6));
    }

    #[test]
    fn same_picked_distinguishes_types() {
        let solid = PickedValue::Solid(Color::BLACK);
        let gradient = PickedValue::Gradient(Gradient::two(Color::BLACK, Color::BLACK));
        assert!(!same_picked(&solid, &gradient));
        assert!(same_picked(&solid, &PickedValue::Solid(Color::BLACK)));
    }
}
