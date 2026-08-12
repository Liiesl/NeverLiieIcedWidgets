//! Styles for the [`ColorPicker`](crate::color_picker::ColorPicker).
//!
//! Ported from `iced_aw`'s `style::color_picker` and `style::status` modules.
//! The style has been extended to cover the richer dialog-like layout
//! (tabs, sliders, hex input, swatches, previews) inspired by the
//! `BetterColorDialog` reference implementation.

use iced::{Background, Color, Theme};

/// Linearly interpolates between two colors, clamping `t` to `0..=1`.
fn lerp(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

/// The Status of a widget event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// can be pressed.
    Active,
    /// can be pressed and it is being hovered.
    Hovered,
    /// is being pressed.
    Pressed,
    /// cannot be pressed.
    Disabled,
    /// is focused.
    Focused,
    /// is Selected.
    Selected,
}

/// The style function of widget.
pub type StyleFn<'a, Theme, Style> = Box<dyn Fn(&Theme, Status) -> Style + 'a>;

/// The appearance of a [`ColorPicker`](crate::color_picker::ColorPicker).
#[derive(Clone, Copy, Debug)]
pub struct Style {
    /// The background of the [`ColorPicker`](crate::color_picker::ColorPicker).
    pub background: Background,

    /// The border radius of the [`ColorPicker`](crate::color_picker::ColorPicker).
    pub border_radius: f32,

    /// The border with of the [`ColorPicker`](crate::color_picker::ColorPicker).
    pub border_width: f32,

    /// The border color of the [`ColorPicker`](crate::color_picker::ColorPicker).
    pub border_color: Color,

    /// The background of the panes/containers inside the dialog
    /// (controls area, hex container, swatch panel).
    pub panel_background: Background,

    /// The border radius of the panes/containers inside the dialog.
    pub panel_border_radius: f32,

    /// The border color of the panes/containers inside the dialog.
    pub panel_border_color: Color,

    /// The primary text color of the dialog (`#FFFFFF`).
    pub text_primary: Color,

    /// A muted text color used for labels and secondary information (`#BBBBBB`).
    pub text_secondary: Color,

    /// The background of an inactive tab of a tab bar.
    pub tab_background: Background,

    /// The background of the active tab of a tab bar.
    pub tab_selected_background: Background,

    /// The background of a hovered tab of a tab bar.
    pub tab_hover_background: Background,

    /// The border color of the tabs of a tab bar.
    pub tab_border_color: Color,

    /// The radius of the slider bars of the [`ColorPicker`](crate::color_picker::ColorPicker).
    pub bar_border_radius: f32,

    /// The width of the border of the slider bars of the [`ColorPicker`].
    pub bar_border_width: f32,

    /// The border color of the slider bars of the [`ColorPicker`].
    pub bar_border_color: Color,

    /// The border color of the slider grooves (`#3A3A3A`).
    pub slider_groove_border_color: Color,

    /// The color of the slider handles.
    pub slider_handle_background: Color,

    /// The color of the slider handles while hovered.
    pub slider_handle_hover_background: Color,

    /// The border color of the slider handles.
    pub slider_handle_border_color: Color,

    /// The radius of the S/V square indicator circle (outline only), in pixels.
    pub sv_square_indicator_radius: f32,

    /// The first (light) color of the checkerboard pattern.
    pub checker_color_1: Color,

    /// The second (dark) color of the checkerboard pattern.
    pub checker_color_2: Color,

    /// The first (light) color of the alpha groove checker pattern.
    pub checker_alpha_1: Color,

    /// The second (dark) color of the alpha groove checker pattern.
    pub checker_alpha_2: Color,

    /// The border color of the Original/New preview panels.
    pub preview_border_color: Color,

    /// The border color of the swatch buttons.
    pub swatch_border_color: Color,

    /// The border color of a hovered swatch button.
    pub swatch_hover_border_color: Color,

    /// The background of the Reset button.
    pub reset_background: Color,

    /// The background of the Reset button while hovered.
    pub reset_hover_background: Color,
}

/// The Catalog of a [`ColorPicker`](crate::color_picker::ColorPicker).
pub trait Catalog {
    ///Style for the trait to use.
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class with the given status.
    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style;
}

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self, Style>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(primary)
    }

    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style {
        class(self, status)
    }
}

/// The primary theme of a [`ColorPicker`](crate::color_picker::ColorPicker).
///
/// The defaults follow the dark, panel-based look of the reference
/// `BetterColorDialog`, mapped onto the palette of the active [`Theme`].
#[must_use]
pub fn primary(theme: &Theme, status: Status) -> Style {
    let p = theme.extended_palette();

    // `#2D2D2D` dialog background.
    let background = p.background.weak.color;

    // `#333333` panels are slightly lifted from the dialog background.
    let panel = lerp(p.background.weak.color, p.background.base.color, 0.15);

    // `#FFFFFF` primary / `#BBBBBB` secondary text.
    let text_primary = p.background.base.text;
    let text_secondary = Color {
        a: 0.75,
        ..text_primary
    };

    // `#3A3A3A` neutral surfaces (tab background, groove border, panel border).
    let neutral = p.background.base.color;

    // `#4A4A4A` hovered surfaces.
    let neutral_hover = p.background.strong.color;

    // `#DDDDDD` slider handle.
    let handle = text_primary;

    // `#555555` handle / swatch / preview borders.
    let handle_border = lerp(handle, Color::BLACK, 0.45);

    // `#553333` / `#6F4040` reset button.
    let reset = p.danger.weak.color;
    let reset_hover = lerp(p.danger.weak.color, p.danger.base.color, 0.4);

    // Checkerboard tiles (`#C8C8C8` / `#E6E6E6`), semi-transparent so the
    // picked color shows through; alpha groove tiles are the same pair
    // darkened by 0.25.
    let checker_light = Color {
        a: 0.5,
        ..p.background.base.color
    };
    let checker_dark = Color {
        a: 0.5,
        ..p.background.weak.color
    };
    let checker_alpha_light = lerp(checker_light, Color::BLACK, 0.25);
    let checker_alpha_dark = lerp(checker_dark, Color::BLACK, 0.25);

    let base = Style {
        background: background.into(),
        border_radius: 10.0,
        border_width: 1.0,
        border_color: p.background.strong.color,
        panel_background: panel.into(),
        panel_border_radius: 5.0,
        panel_border_color: neutral,
        text_primary,
        text_secondary,
        tab_background: neutral.into(),
        tab_selected_background: neutral_hover.into(),
        tab_hover_background: neutral_hover.into(),
        tab_border_color: neutral,
        bar_border_radius: 4.0,
        bar_border_width: 1.0,
        bar_border_color: neutral,
        slider_groove_border_color: neutral,
        slider_handle_background: handle,
        slider_handle_hover_background: lerp(handle, Color::WHITE, 0.9),
        slider_handle_border_color: handle_border,
        sv_square_indicator_radius: 7.0,
        checker_color_1: checker_light,
        checker_color_2: checker_dark,
        checker_alpha_1: checker_alpha_light,
        checker_alpha_2: checker_alpha_dark,
        preview_border_color: handle_border,
        swatch_border_color: handle_border,
        swatch_hover_border_color: text_primary,
        reset_background: reset,
        reset_hover_background: reset_hover,
    };

    // Accent used by the active/selected tab and focus outlines.
    let accent: iced::theme::palette::Secondary = p.secondary;
    let prim: iced::theme::palette::Primary = p.primary;

    match status {
        Status::Focused => Style {
            border_color: prim.base.color,
            bar_border_color: prim.base.color,
            tab_background: prim.base.color.into(),
            ..base
        },
        Status::Selected => Style {
            tab_background: prim.base.color.into(),
            ..base
        },
        Status::Hovered => Style {
            bar_border_color: accent.base.color,
            ..base
        },
        _ => base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_all_variants() {
        let statuses = [
            Status::Active,
            Status::Hovered,
            Status::Pressed,
            Status::Disabled,
            Status::Focused,
            Status::Selected,
        ];

        assert_eq!(statuses.len(), 6);
    }

    #[test]
    fn primary_theme_active() {
        let theme = Theme::TokyoNight;
        let style = primary(&theme, Status::Active);

        assert!(matches!(style.background, Background::Color(_)));
        assert_eq!(style.border_radius, 10.0);
        assert_eq!(style.border_width, 1.0);
        assert_eq!(style.bar_border_radius, 4.0);
        assert_eq!(style.bar_border_width, 1.0);
    }

    #[test]
    fn primary_theme_focused() {
        let theme = Theme::TokyoNight;
        let style = primary(&theme, Status::Focused);

        assert!(matches!(style.background, Background::Color(_)));
        assert_eq!(style.border_radius, 10.0);
        assert_eq!(style.border_width, 1.0);
        assert_eq!(style.bar_border_radius, 4.0);
        assert_eq!(style.bar_border_width, 1.0);
    }

    #[test]
    fn catalog_default_class() {
        let _class = <Theme as Catalog>::default();
    }

    #[test]
    fn catalog_style() {
        let theme = Theme::TokyoNight;
        let class = <Theme as Catalog>::default();
        let style = theme.style(&class, Status::Active);

        assert!(matches!(style.background, Background::Color(_)));
        assert_eq!(style.border_radius, 10.0);
        assert_eq!(style.border_width, 1.0);
        assert_eq!(style.bar_border_radius, 4.0);
        assert_eq!(style.bar_border_width, 1.0);
    }

    #[test]
    fn focused_changes_border_colors() {
        let theme = Theme::TokyoNight;
        let base_style = primary(&theme, Status::Active);
        let focused_style = primary(&theme, Status::Focused);

        // Border colors should be different when focused
        assert_ne!(base_style.border_color, focused_style.border_color);

        // Other properties should remain the same
        assert_eq!(base_style.background, focused_style.background);
        assert_eq!(base_style.border_radius, focused_style.border_radius);
        assert_eq!(base_style.border_width, focused_style.border_width);
        assert_eq!(
            base_style.bar_border_radius,
            focused_style.bar_border_radius
        );
        assert_eq!(base_style.bar_border_width, focused_style.bar_border_width);
    }

    #[test]
    fn style_fn_compiles() {
        fn example_style_fn(_theme: &(), status: Status) -> u32 {
            match status {
                Status::Active => 1,
                Status::Hovered => 2,
                Status::Pressed => 3,
                Status::Disabled => 4,
                Status::Focused => 5,
                Status::Selected => 6,
            }
        }

        let style_fn: StyleFn<(), u32> = Box::new(example_style_fn);
        assert_eq!(style_fn(&(), Status::Active), 1);
        assert_eq!(style_fn(&(), Status::Focused), 5);
    }

    #[test]
    fn style_fields_populated_for_all_statuses() {
        let theme = Theme::TokyoNight;
        let statuses = [
            Status::Active,
            Status::Hovered,
            Status::Pressed,
            Status::Disabled,
            Status::Focused,
            Status::Selected,
        ];

        for status in statuses {
            let style = primary(&theme, status);

            assert_ne!(style.text_primary, Color::TRANSPARENT);
            assert_ne!(style.text_secondary, Color::TRANSPARENT);
            assert_ne!(style.slider_handle_background, Color::TRANSPARENT);
            assert_ne!(
                style.slider_handle_hover_background,
                Color::TRANSPARENT
            );
            assert_ne!(style.slider_handle_border_color, Color::TRANSPARENT);
            assert_eq!(style.sv_square_indicator_radius, 7.0);
            assert_ne!(style.slider_groove_border_color, Color::TRANSPARENT);
            assert_ne!(style.checker_color_1, Color::TRANSPARENT);
            assert_ne!(style.checker_color_2, Color::TRANSPARENT);
            assert_ne!(style.checker_alpha_1, Color::TRANSPARENT);
            assert_ne!(style.checker_alpha_2, Color::TRANSPARENT);
            assert_ne!(style.preview_border_color, Color::TRANSPARENT);
            assert_ne!(style.swatch_border_color, Color::TRANSPARENT);
            assert_ne!(style.swatch_hover_border_color, Color::TRANSPARENT);
            assert_ne!(style.reset_background, Color::TRANSPARENT);
            assert_ne!(style.reset_hover_background, Color::TRANSPARENT);
            assert_ne!(style.panel_border_color, Color::TRANSPARENT);
            assert_eq!(style.panel_border_radius, 5.0);
            assert_eq!(style.border_radius, 10.0);

            for field in [
                &style.background,
                &style.panel_background,
                &style.tab_background,
                &style.tab_selected_background,
                &style.tab_hover_background,
            ] {
                assert!(matches!(field, Background::Color(c) if c.a > 0.0));
            }
        }
    }

    #[test]
    fn lerp_interpolates() {
        let a = Color::from_rgb(0.0, 0.0, 0.0);
        let b = Color::from_rgb(1.0, 1.0, 1.0);

        assert_eq!(lerp(a, b, 0.0), a);
        assert_eq!(lerp(a, b, 1.0), b);
        assert_eq!(lerp(a, b, 0.5), Color::from_rgb(0.5, 0.5, 0.5));
        // Out-of-range factors are clamped.
        assert_eq!(lerp(a, b, 2.0), b);
        assert_eq!(lerp(a, b, -1.0), a);
    }
}