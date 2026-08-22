//! Theme-aware styling for the custom frame.
//!
//! Everything here is derived from [`iced::Theme`]'s built-in palette, so the
//! frame follows whatever theme the application selects.

use iced::widget::container;
use iced::{Border, Color, Shadow, Theme, Vector};

use super::action::CaptionControl;
use super::config::NativeFrameConfig;

/// The visual state of a caption control.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CaptionState {
    pub(crate) hot: bool,
    pub(crate) pressed: bool,
    pub(crate) active: bool,
}

/// The background of the title bar.
pub(crate) fn title_bar(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style::default()
        .background(palette.background.weakest.color)
        .color(palette.background.weakest.text)
}

/// The window surface: background, border and optional client-side shadow.
///
/// `border_color` is resolved by the caller, which lets the active window
/// pick up the platform accent (see [`platform::border_color`]) while the
/// rest of the styling stays theme-derived.
pub(crate) fn surface(
    theme: &Theme,
    config: NativeFrameConfig,
    border_color: Color,
) -> container::Style {
    let palette = theme.extended_palette();

    let shadow = if config.client_shadow && config.outer_padding > 0.0 {
        Shadow {
            color: Color {
                a: 0.35,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 4.0),
            blur_radius: config.outer_padding,
        }
    } else {
        Shadow::default()
    };

    let border = Border {
        color: border_color,
        width: if config.frame_border { 1.0 } else { 0.0 },
        radius: config.corner_radius.into(),
    };

    container::Style::default()
        .background(palette.background.base.color)
        .color(palette.background.base.text)
        .border(border)
        .shadow(shadow)
}

/// The background of a caption control.
pub(crate) fn caption_button(
    theme: &Theme,
    control: CaptionControl,
    state: CaptionState,
) -> container::Style {
    let palette = theme.extended_palette();

    let background = match (control, state.hot, state.pressed) {
        // The close control keeps the platform-conventional red highlight
        // rather than a palette color, because it communicates danger.
        (CaptionControl::Close, true, true) => Color::from_rgb8(150, 20, 20),
        (CaptionControl::Close, true, false) => Color::from_rgb8(196, 43, 28),
        (_, _, true) => palette.background.strong.color,
        (_, true, false) => palette.background.weak.color,
        _ => Color::TRANSPARENT,
    };

    container::Style::default().background(background)
}

/// The color of the glyph inside a caption control.
pub(crate) fn caption_icon(theme: &Theme, control: CaptionControl, state: CaptionState) -> Color {
    let palette = theme.extended_palette();

    if control == CaptionControl::Close && state.hot {
        Color::WHITE
    } else if state.active {
        palette.background.strong.text
    } else {
        palette.background.weak.text
    }
}
