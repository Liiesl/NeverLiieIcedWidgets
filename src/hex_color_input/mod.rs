//! Compact hex color input with alpha percentage and floating picker.
//!
//! [`HexColorInput`] renders as a dark pill like the reference:
//! `[■ FFFFFF | 100 %]`. The swatch rectangle opens the floating
//! [`ColorPicker`](crate::color_picker) dialog; the hex field edits `RRGGBB`
//! and the percentage field edits alpha (optional via
//! [`HexColorInput::show_alpha`]).
//!
//! When the picked value is a gradient, the hex field becomes the static text
//! `"gradient"`, alpha is hidden, and an angle field (with [`ANGLE_SVG`]
//! icon) appears: `[■ gradient | ∠ 90]`.
//!
//! # Example
//! ```no_run
//! use iced::Element;
//! use neverliie_iced_widgets::hex_color_input::{HexColorInput, HexColorValue};
//!
//! #[derive(Clone)]
//! enum Message {
//!     ColorChanged(HexColorValue),
//!     Open,
//!     Close,
//! }
//!
//! fn view(value: HexColorValue, show_picker: bool) -> Element<'_, Message> {
//!     HexColorInput::new(value, Message::ColorChanged, show_picker, Message::Open, Message::Close).into()
//! }
//! ```

use iced::advanced::layout::{self, Layout};
use iced::advanced::mouse;
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::text;
use iced::advanced::Renderer as _;
use iced::advanced::widget::{self, Operation, Tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::widget::{button, text as text_widget, text_input};
use iced::widget::svg as svg_widget;
use iced::widget::Renderer as ConcreteRenderer;
use iced::{
    border, Background, Border, Color, Element, Event, Length, Padding, Pixels,
    Point, Rectangle, Shadow, Size, Vector,
};

use std::cell::RefCell;
use std::rc::Rc;

use crate::color_picker::{DropperBuffer, Gradient, PickedValue};
use crate::color_picker::overlay as picker_overlay;
use crate::color_picker::style as picker_style;
use crate::overlay::Position;

/// Angle icon shown before the gradient angle field.
pub const ANGLE_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#D1D5DB" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 19h16"/><path d="M4 19 16 6"/><path d="M11 19a7 7 0 0 0-2.05-4.95"/></svg>"##;

/// Child index of the swatch button.
const SWATCH: usize = 0;
/// Child index of the hex field (`FFFFFF` or disabled `"gradient"`).
const HEX: usize = 1;
/// Child index of the alpha percentage field.
const ALPHA: usize = 2;
/// Child index of the `"%"` suffix label.
const ALPHA_SUFFIX: usize = 3;
/// Child index of the angle icon.
const ANGLE_ICON: usize = 4;
/// Child index of the angle field.
const ANGLE: usize = 5;
/// Child index of the dialog button trees (never laid out, overlay only).
const OVERLAY_BUTTONS: usize = 6;
/// Number of visible pill children (everything except overlay buttons).
const PILL_COUNT: usize = 6;

const SWATCH_W: f32 = 30.0;
const HEX_W: f32 = 68.0;
const ALPHA_W: f32 = 36.0;
const SUFFIX_W: f32 = 12.0;
const ICON_W: f32 = 20.0;
const ANGLE_W: f32 = 50.0;
const GAP: f32 = 4.0;
const PAD_X: f32 = 6.0;
/// Minimum hex field width when squeezed below intrinsic: keeps the `%`
/// suffix inside the pill in narrow `FillPortion` columns.
const HEX_MIN_W: f32 = 48.0;
/// Minimum alpha field width when squeezed below intrinsic.
const ALPHA_MIN_W: f32 = 28.0;

const DEFAULT_RADIUS: f32 = 8.0;
const FALLBACK_TEXT_SIZE: f32 = 13.0;

/// Compact color value: a solid or a two-stop gradient with an angle.
#[derive(Clone, Debug, PartialEq)]
pub enum HexColorValue {
    /// A solid color (alpha edited via the `%` field).
    Solid(Color),
    /// A gradient with a direction angle in degrees (`0..360`).
    Gradient { gradient: Gradient, angle: f32 },
}

impl HexColorValue {
    /// Whether this is a solid color.
    #[must_use]
    pub fn is_solid(&self) -> bool {
        matches!(self, Self::Solid(_))
    }

    /// Whether this is a gradient.
    #[must_use]
    pub fn is_gradient(&self) -> bool {
        matches!(self, Self::Gradient { .. })
    }

    /// The solid color, or the first gradient stop.
    #[must_use]
    pub fn solid_color(&self) -> Color {
        match self {
            Self::Solid(c) => *c,
            Self::Gradient { gradient, .. } => {
                gradient.stop(0).map_or(Color::BLACK, |s| s.color)
            }
        }
    }

    /// The gradient, or two identical stops for a solid.
    #[must_use]
    pub fn gradient_clone(&self) -> Gradient {
        match self {
            Self::Solid(c) => Gradient::two(*c, *c),
            Self::Gradient { gradient, .. } => gradient.clone(),
        }
    }

    /// The gradient angle in degrees (`0.0` for solids).
    #[must_use]
    pub fn angle(&self) -> f32 {
        match self {
            Self::Solid(_) => 0.0,
            Self::Gradient { angle, .. } => normalize_angle(*angle),
        }
    }

    /// Returns a copy with the gradient angle replaced.
    ///
    /// This is the entry point for external UI (e.g. a slider) driving the
    /// angle: `value = value.with_angle(new_angle)`. Solids have no angle
    /// and are returned unchanged — switch them to a gradient first.
    #[must_use]
    pub fn with_angle(&self, angle: f32) -> Self {
        match self {
            Self::Solid(_) => self.clone(),
            Self::Gradient { gradient, .. } => Self::Gradient {
                gradient: gradient.clone(),
                angle: normalize_angle(angle),
            },
        }
    }

    /// Uppercase `RRGGBB` without `#` and without alpha.
    #[must_use]
    pub fn hex_text(&self) -> String {
        let [r, g, b, _] = self.solid_color().into_rgba8();
        format!("{r:02X}{g:02X}{b:02X}")
    }

    /// Alpha as `0..=100`.
    #[must_use]
    pub fn alpha_pct(&self) -> u8 {
        (self.solid_color().a * 100.0).round().clamp(0.0, 100.0) as u8
    }

    /// Angle as an integer string.
    #[must_use]
    pub fn angle_text(&self) -> String {
        format!("{}", normalize_angle(self.angle()).round() as i32)
    }

    /// Preview background for the swatch (angle-aware for gradients).
    #[must_use]
    pub fn to_background(&self) -> Background {
        match self {
            Self::Solid(c) => Background::Color(*c),
            Self::Gradient { gradient, angle } => {
                let mut linear = iced::gradient::Linear::new(iced::Radians(
                    normalize_angle(*angle).to_radians(),
                ));
                for stop in &gradient.stops {
                    linear = linear.add_stop(stop.offset, stop.color);
                }
                Background::Gradient(iced::gradient::Gradient::Linear(linear))
            }
        }
    }

    /// Builds a value from a picker [`PickedValue`], keeping `angle`.
    #[must_use]
    pub fn from_picked(picked: PickedValue, angle: f32) -> Self {
        match picked {
            PickedValue::Solid(c) => Self::Solid(c),
            PickedValue::Gradient(g) => Self::Gradient {
                gradient: g,
                angle: normalize_angle(angle),
            },
        }
    }
}

impl From<Color> for HexColorValue {
    fn from(color: Color) -> Self {
        Self::Solid(color)
    }
}

/// Normalizes degrees to `0..360`.
#[must_use]
pub fn normalize_angle(angle: f32) -> f32 {
    angle.rem_euclid(360.0)
}

/// The visual status of a [`HexColorInput`] pill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Idle.
    Active,
    /// Mouse over.
    Hovered,
    /// A field focused.
    Focused,
}

/// Appearance of a [`HexColorInput`] pill.
#[derive(Debug, Clone)]
pub struct Style {
    /// Pill background.
    pub background: Background,
    /// Pill border.
    pub border: Border,
    /// Pill shadow.
    pub shadow: Shadow,
}

/// Theme catalog for the [`HexColorInput`] pill.
pub trait Catalog {
    /// Style class.
    type Class<'a>;
    /// Default class.
    fn default<'a>() -> <Self as Catalog>::Class<'a>;
    /// Style for a status.
    fn style(&self, class: &<Self as Catalog>::Class<'_>, status: Status) -> Style;
}

/// Styling closure for the pill.
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, Status) -> Style + 'a>;

impl Catalog for iced::Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> StyleFn<'a, Self> {
        Box::new(default)
    }

    fn style(&self, class: &StyleFn<'_, Self>, status: Status) -> Style {
        class(self, status)
    }
}

/// Default pill style: dark, matching the reference chip.
pub fn default(theme: &iced::Theme, status: Status) -> Style {
    let palette = theme.extended_palette();
    let active = Style {
        background: Background::Color(Color::from_rgb(0.07, 0.07, 0.08)),
        border: Border {
            radius: DEFAULT_RADIUS.into(),
            width: 1.0,
            color: palette.background.strong.color,
        },
        shadow: Shadow::default(),
    };
    match status {
        Status::Active => active,
        Status::Hovered => Style {
            border: Border {
                color: palette.background.base.text,
                ..active.border
            },
            ..active
        },
        Status::Focused => Style {
            border: Border {
                color: palette.primary.strong.color,
                ..active.border
            },
            ..active
        },
    }
}

/// Tree state: compact field buffers plus the floating dialog state.
struct HexState {
    hex_buffer: Option<String>,
    alpha_buffer: Option<String>,
    angle_buffer: Option<String>,
    last_canonical: String,
    hovered: bool,
    modifiers: iced::keyboard::Modifiers,
    old_show_picker: bool,
    last_cursor: Point,
    picker: picker_overlay::State,
}

/// A compact hex color input with a floating picker.
pub struct HexColorInput<'a, Message, Theme = iced::Theme>
where
    Theme: Catalog + picker_style::Catalog,
{
    value: HexColorValue,
    on_change: Rc<dyn Fn(HexColorValue) -> Message + 'a>,
    on_submit: Option<Rc<dyn Fn(HexColorValue) -> Message + 'a>>,
    show_picker: bool,
    on_open: Message,
    on_cancel: Message,
    show_alpha: bool,
    position: Option<Position>,
    dropper_buffer: Option<DropperBuffer>,
    on_dropper_capture: Option<Rc<dyn Fn() -> Message + 'a>>,
    width: Length,
    text_size: Option<Pixels>,
    input_padding: Padding,
    border_radius: Option<border::Radius>,
    class: <Theme as Catalog>::Class<'a>,
    picker_class: <Theme as picker_style::Catalog>::Class<'a>,
    hex_stash: Rc<RefCell<Option<String>>>,
    alpha_stash: Rc<RefCell<Option<String>>>,
    angle_stash: Rc<RefCell<Option<String>>>,
    picker_submit: Box<dyn Fn(Color) -> Message + 'a>,
    picker_color_change: Box<dyn Fn(Color) -> Message + 'a>,
    picker_gradient_submit: Box<dyn Fn(Gradient) -> Message + 'a>,
    picker_gradient_change: Box<dyn Fn(Gradient) -> Message + 'a>,
    picker_pick: Box<dyn Fn(PickedValue) -> Message + 'a>,
    picker_pick_submit: Box<dyn Fn(PickedValue) -> Message + 'a>,
    children: Vec<Element<'a, Message, Theme, ConcreteRenderer>>,
}

impl<'a, Message, Theme> HexColorInput<'a, Message, Theme>
where
    Message: Clone + 'a,
    Theme: Catalog
        + button::Catalog
        + text_widget::Catalog
        + text_input::Catalog
        + picker_style::Catalog
        + svg_widget::Catalog
        + 'a,
    <Theme as button::Catalog>::Class<'a>: From<button::StyleFn<'a, Theme>>,
    for<'c> <Theme as text_input::Catalog>::Class<'c>:
        From<text_input::StyleFn<'c, Theme>>,
{
    /// Creates a new [`HexColorInput`].
    pub fn new(
        value: HexColorValue,
        on_change: impl Fn(HexColorValue) -> Message + 'a,
        show_picker: bool,
        on_open: Message,
        on_cancel: Message,
    ) -> Self {
        let on_change: Rc<dyn Fn(HexColorValue) -> Message + 'a> =
            Rc::new(on_change);
        let angle = value.angle();
        let hex_stash = Rc::new(RefCell::new(None));
        let alpha_stash = Rc::new(RefCell::new(None));
        let angle_stash = Rc::new(RefCell::new(None));
        let mut this = Self {
            value,
            on_change,
            on_submit: None,
            show_picker,
            on_open,
            on_cancel,
            show_alpha: true,
            position: None,
            dropper_buffer: None,
            on_dropper_capture: None,
            width: Length::Shrink,
            text_size: None,
            input_padding: Padding::new(4.0),
            border_radius: None,
            class: <Theme as Catalog>::default(),
            picker_class: <Theme as picker_style::Catalog>::default(),
            hex_stash,
            alpha_stash,
            angle_stash,
            picker_submit: Box::new(|_| unreachable!("rebuilt in new")),
            picker_color_change: Box::new(|_| unreachable!("rebuilt in new")),
            picker_gradient_submit: Box::new(|_| {
                unreachable!("rebuilt in new")
            }),
            picker_gradient_change: Box::new(|_| {
                unreachable!("rebuilt in new")
            }),
            picker_pick: Box::new(|_| unreachable!("rebuilt in new")),
            picker_pick_submit: Box::new(|_| unreachable!("rebuilt in new")),
            children: Vec::new(),
        };
        this.rebuild_picker_callbacks();
        let hex = this.value.hex_text();
        let alpha = format!("{}", this.value.alpha_pct());
        let ang = this.value.angle_text();
        this.children = vec![
            this.make_swatch(),
            this.make_hex(&hex, !this.value.is_gradient()),
            this.make_alpha(&alpha, this.value.is_solid()),
            this.make_alpha_suffix(),
            this.make_angle_icon(),
            this.make_angle(&ang, this.value.is_gradient()),
            picker_overlay::ColorPickerOverlayButtons::default().into(),
        ];
        // Silence unused angle warning when solid; angle is still used by callbacks.
        let _ = angle;
        this
    }

    /// Sets the submit callback for the picker OK button (defaults to `on_change`).
    #[must_use]
    pub fn on_submit(
        mut self,
        on_submit: impl Fn(HexColorValue) -> Message + 'a,
    ) -> Self {
        self.on_submit = Some(Rc::new(on_submit));
        self.rebuild_picker_callbacks();
        self
    }

    /// Shows or hides the alpha `%` field for solids (always hidden for gradients).
    #[must_use]
    pub fn show_alpha(mut self, show: bool) -> Self {
        self.show_alpha = show;
        self
    }

    /// Sets the initial floating position of the picker window.
    #[must_use]
    pub fn position(mut self, position: Position) -> Self {
        self.position = Some(position);
        self
    }

    /// Enables the eye dropper with a shared buffer.
    #[must_use]
    pub fn dropper_buffer(mut self, buffer: DropperBuffer) -> Self {
        self.dropper_buffer = Some(buffer);
        self
    }

    /// Sets the capture request message for the eye dropper.
    #[must_use]
    pub fn on_dropper_capture(
        mut self,
        on_capture: impl Fn() -> Message + 'a,
    ) -> Self {
        self.on_dropper_capture = Some(Rc::new(on_capture));
        self
    }

    /// Sets the width of the pill.
    #[must_use]
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the text size.
    #[must_use]
    pub fn text_size(mut self, size: impl Into<Pixels>) -> Self {
        self.text_size = Some(size.into());
        self
    }

    /// Sets the padding of the inner hex/alpha/angle fields, so the pill
    /// height (`text + padding + frame`) can match `NumberInput`.
    #[must_use]
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.input_padding = padding.into();
        self
    }

    /// Sets the pill corner radius.
    #[must_use]
    pub fn border_radius(
        mut self,
        radius: impl Into<border::Radius>,
    ) -> Self {
        self.border_radius = Some(radius.into());
        self
    }

    /// Sets the pill style.
    #[must_use]
    pub fn style(
        mut self,
        style: impl Fn(&Theme, Status) -> Style + 'a,
    ) -> Self
    where
        <Theme as Catalog>::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the pill style class.
    #[must_use]
    pub fn class(
        mut self,
        class: impl Into<<Theme as Catalog>::Class<'a>>,
    ) -> Self {
        self.class = class.into();
        self
    }

    /// Sets the floating dialog style class.
    #[must_use]
    pub fn picker_class(
        mut self,
        class: impl Into<<Theme as picker_style::Catalog>::Class<'a>>,
    ) -> Self {
        self.picker_class = class.into();
        self
    }

    fn resolved_text_size(&self) -> Pixels {
        self.text_size.unwrap_or(Pixels(FALLBACK_TEXT_SIZE))
    }

    fn canonical_key(&self) -> String {
        Self::key_of(&self.value)
    }

    fn key_of(value: &HexColorValue) -> String {
        format!(
            "{}|{}|{}|{}",
            value.hex_text(),
            value.alpha_pct(),
            value.angle_text(),
            value.is_gradient()
        )
    }

    /// Alpha stepped by `delta` percent points, clamped to `0..=100`.
    fn stepped_alpha(&self, down: bool, large: bool) -> Option<HexColorValue> {
        if !self.value.is_solid() || !self.show_alpha {
            return None;
        }
        let pct = self.value.alpha_pct() as i32;
        let step = if large { 10 } else { 1 };
        let next = (pct + if down { -step } else { step }).clamp(0, 100);
        if next == pct {
            return None;
        }
        let mut c = self.value.solid_color();
        c.a = next as f32 / 100.0;
        Some(HexColorValue::Solid(c))
    }

    /// Angle stepped by `delta` degrees, wrapped to `0..360`.
    fn stepped_angle(&self, down: bool, large: bool) -> Option<HexColorValue> {
        let HexColorValue::Gradient { gradient, angle } = &self.value else {
            return None;
        };
        let step = if large { 15.0 } else { 1.0 };
        let next = normalize_angle(angle + if down { -step } else { step });
        // Avoid no-op publishes when the rounded display would not change
        // and the float barely moved (e.g. at rest).
        if (next - *angle).abs() < f32::EPSILON {
            return None;
        }
        Some(HexColorValue::Gradient {
            gradient: gradient.clone(),
            angle: next,
        })
    }

    fn solid_seed(&self) -> Color {
        self.value.solid_color()
    }

    fn gradient_seed(&self) -> Gradient {
        self.value.gradient_clone()
    }

    fn rebuild_picker_callbacks(&mut self) {
        let on_change = Rc::clone(&self.on_change);
        let on_submit = self.on_submit.clone();
        let angle = self.value.angle();
        let submit_for = move |v: HexColorValue| match &on_submit {
            Some(f) => f(v),
            None => on_change(v),
        };
        let on_change_c = Rc::clone(&self.on_change);
        self.picker_submit = Box::new(move |c: Color| {
            submit_for(HexColorValue::Solid(c))
        });
        let on_change_c2 = Rc::clone(&self.on_change);
        self.picker_color_change =
            Box::new(move |c: Color| on_change_c2(HexColorValue::Solid(c)));
        let on_change_g = Rc::clone(&self.on_change);
        let submit_g = self.on_submit.clone();
        self.picker_gradient_submit = Box::new(move |g: Gradient| {
            let v = HexColorValue::Gradient {
                gradient: g,
                angle,
            };
            match &submit_g {
                Some(f) => f(v),
                None => on_change_g(v),
            }
        });
        let on_change_g2 = Rc::clone(&self.on_change);
        self.picker_gradient_change = Box::new(move |g: Gradient| {
            on_change_g2(HexColorValue::Gradient {
                gradient: g,
                angle,
            })
        });
        let on_change_p = Rc::clone(&self.on_change);
        self.picker_pick = Box::new(move |p: PickedValue| {
            on_change_p(HexColorValue::from_picked(p, angle))
        });
        let on_change_ps = Rc::clone(&self.on_change);
        let submit_ps = self.on_submit.clone();
        self.picker_pick_submit = Box::new(move |p: PickedValue| {
            let v = HexColorValue::from_picked(p, angle);
            match &submit_ps {
                Some(f) => f(v),
                None => on_change_ps(v),
            }
        });
        let _ = on_change_c;
    }

    fn make_swatch(&self) -> Element<'a, Message, Theme, ConcreteRenderer> {
        let preview = self.value.to_background();
        let inner: Element<'a, Message, Theme, ConcreteRenderer> = text_widget("")
            .width(Length::Fixed(14.0))
            .height(Length::Fixed(14.0))
            .into();
        button(inner)
            .on_press(self.on_open.clone())
            .padding(2.0)
            .style(move |theme: &Theme, status: button::Status| {
                let mut s = <Theme as button::Catalog>::style(
                    theme,
                    &<Theme as button::Catalog>::default(),
                    status,
                );
                s.background = Some(preview.clone());
                s.border = Border {
                    width: 1.0,
                    color: Color::from_rgb(0.75, 0.75, 0.75),
                    radius: 3.0.into(),
                };
                s.shadow = Shadow::default();
                s
            })
            .into()
    }

/// Transparent single-line field style so hex/alpha/angle read as part of
    /// the dark pill instead of nested boxes (same idea as `NumberInput`'s
    /// stripped input: theme text/selection colors, no background or border).
    fn field_style(
        theme: &Theme,
        status: text_input::Status,
    ) -> text_input::Style {
        let base = <Theme as text_input::Catalog>::style(
            theme,
            &<Theme as text_input::Catalog>::default(),
            status,
        );
        text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border {
                width: 0.0,
                color: Color::TRANSPARENT,
                radius: 0.0.into(),
            },
            icon: base.icon,
            placeholder: Color::from_rgba(1.0, 1.0, 1.0, 0.35),
            value: Color::from_rgb(0.96, 0.96, 0.97),
            selection: base.selection,
        }
    }

    fn make_hex(
        &self,
        display: &str,
        enabled: bool,
    ) -> Element<'a, Message, Theme, ConcreteRenderer> {
        let text_size = self.resolved_text_size();
        // Always a text_input (disabled showing "gradient" in gradient mode)
        // so the child tree state type stays stable and focus downcasts
        // never hit a text widget.
        let shown = if enabled { display } else { "gradient" };
        let stash = Rc::clone(&self.hex_stash);
        let on_change = Rc::clone(&self.on_change);
        let current = self.value.clone();
        let current_alpha = current.solid_color().a;
        let mut field = text_input("", shown)
            .width(Length::Fixed(HEX_W))
            .size(text_size)
            .padding(self.input_padding)
            .style(Self::field_style);
        if enabled {
            field = field.on_input(move |s: String| {
                *stash.borrow_mut() = Some(s.clone());
                match parse_hex_input(&s, current_alpha) {
                    Some(c) => on_change(HexColorValue::Solid(c)),
                    None => on_change(current.clone()),
                }
            });
        }
        field.into()
    }

    fn make_alpha(
        &self,
        display: &str,
        enabled: bool,
    ) -> Element<'a, Message, Theme, ConcreteRenderer> {
        let text_size = self.resolved_text_size();
        if !enabled {
            return text_input("", "")
                .width(Length::Fixed(0.0))
                .size(text_size)
                .style(Self::field_style)
                .into();
        }
        let stash = Rc::clone(&self.alpha_stash);
        let on_change = Rc::clone(&self.on_change);
        let current = self.value.clone();
        text_input("", display)
            .width(Length::Fixed(ALPHA_W))
            .size(text_size)
            .padding(self.input_padding)
            .style(Self::field_style)
            .on_input(move |s: String| {
                *stash.borrow_mut() = Some(s.clone());
                match parse_alpha_input(&s) {
                    Some(pct) => {
                        let mut c = current.solid_color();
                        c.a = pct as f32 / 100.0;
                        on_change(HexColorValue::Solid(c))
                    }
                    None => on_change(current.clone()),
                }
            })
            .into()
    }

    fn make_alpha_suffix(&self) -> Element<'a, Message, Theme, ConcreteRenderer> {
        if !self.value.is_solid() || !self.show_alpha {
            return text_widget("")
                .width(Length::Fixed(0.0))
                .into();
        }
        text_widget("%")
            .size(self.resolved_text_size())
            .width(Length::Fixed(SUFFIX_W))
            .into()
    }

    fn make_angle_icon(&self) -> Element<'a, Message, Theme, ConcreteRenderer> {
        if !self.value.is_gradient() {
            return text_widget("")
                .width(Length::Fixed(0.0))
                .into();
        }
        svg_widget::Svg::new(svg_widget::Handle::from_memory(ANGLE_SVG))
            .width(Length::Fixed(16.0))
            .height(Length::Fixed(16.0))
            .into()
    }

    fn make_angle(
        &self,
        display: &str,
        enabled: bool,
    ) -> Element<'a, Message, Theme, ConcreteRenderer> {
        let text_size = self.resolved_text_size();
        if !enabled {
            return text_input("", "")
                .width(Length::Fixed(0.0))
                .size(text_size)
                .style(Self::field_style)
                .into();
        }
        let stash = Rc::clone(&self.angle_stash);
        let on_change = Rc::clone(&self.on_change);
        let current = self.value.clone();
        let current_gradient = current.gradient_clone();
        text_input("", display)
            .width(Length::Fixed(ANGLE_W))
            .size(text_size)
            .padding(self.input_padding)
            .style(Self::field_style)
            .on_input(move |s: String| {
                *stash.borrow_mut() = Some(s.clone());
                match parse_angle_input(&s) {
                    Some(a) => on_change(HexColorValue::Gradient {
                        gradient: current_gradient.clone(),
                        angle: a,
                    }),
                    None => on_change(current.clone()),
                }
            })
            .into()
    }

    fn rebuild_pill(&mut self, hex: &str, alpha: &str, angle: &str) {
        let is_gradient = self.value.is_gradient();
        let alpha_enabled = self.value.is_solid() && self.show_alpha;
        self.children[SWATCH] = self.make_swatch();
        self.children[HEX] = self.make_hex(hex, !is_gradient);
        self.children[ALPHA] = self.make_alpha(alpha, alpha_enabled);
        self.children[ALPHA_SUFFIX] = self.make_alpha_suffix();
        self.children[ANGLE_ICON] = self.make_angle_icon();
        self.children[ANGLE] = self.make_angle(angle, is_gradient);
    }

    fn is_text_focused(
        &self,
        tree: &Tree,
        index: usize,
    ) -> bool {
        tree.children.get(index).map_or(false, |child| {
            child
                .state
                .downcast_ref::<text_input::State<<ConcreteRenderer as text::Renderer>::Paragraph>>()
                .is_focused()
        })
    }

    fn pill_widths(&self) -> (f32, f32, f32, f32, f32, f32) {
        if self.value.is_gradient() {
            (SWATCH_W, HEX_W, 0.0, 0.0, ICON_W, ANGLE_W)
        } else if self.show_alpha {
            (SWATCH_W, HEX_W, ALPHA_W, SUFFIX_W, 0.0, 0.0)
        } else {
            (SWATCH_W, HEX_W, 0.0, 0.0, 0.0, 0.0)
        }
    }
}

fn parse_hex_input(s: &str, current_alpha: f32) -> Option<Color> {
    let t = s.trim().strip_prefix('#').unwrap_or(s.trim());
    if t.len() != 3 && t.len() != 6 {
        return None;
    }
    if !t.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let nibble = |c: char| c.to_digit(16).unwrap_or(0) as u8;
    let byte = |hi: char, lo: char| (nibble(hi) << 4) | nibble(lo);
    let chars: Vec<char> = t.chars().collect();
    let (r, g, b) = if chars.len() == 3 {
        (
            byte(chars[0], chars[0]),
            byte(chars[1], chars[1]),
            byte(chars[2], chars[2]),
        )
    } else {
        (
            byte(chars[0], chars[1]),
            byte(chars[2], chars[3]),
            byte(chars[4], chars[5]),
        )
    };
    Some(Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: current_alpha,
    })
}

fn is_hex_intermediate(s: &str) -> bool {
    let t = s.trim().strip_prefix('#').unwrap_or(s.trim());
    if s.trim().is_empty() {
        return true;
    }
    t.len() <= 6 && t.chars().all(|c| c.is_ascii_hexdigit())
}

fn parse_alpha_input(s: &str) -> Option<u8> {
    let t = s.trim().strip_suffix('%').unwrap_or(s.trim()).trim();
    if t.is_empty() {
        return None;
    }
    if !t.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let v: i32 = t.parse().ok()?;
    if !(0..=100).contains(&v) {
        return None;
    }
    Some(v as u8)
}

fn is_alpha_intermediate(s: &str) -> bool {
    let t = s.trim();
    t.is_empty()
        || (t.len() <= 3 && t.chars().all(|c| c.is_ascii_digit()))
}

fn parse_angle_input(s: &str) -> Option<f32> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let v: f32 = t.parse().ok()?;
    if !v.is_finite() {
        return None;
    }
    Some(normalize_angle(v))
}

fn is_angle_intermediate(s: &str) -> bool {
    let t = s.trim();
    t.is_empty()
        || t == "-"
        || t == "+"
        || t == "."
        || (t.len() <= 6
            && t.chars()
                .all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
}

impl<'a, Message, Theme> Widget<Message, Theme, ConcreteRenderer>
    for HexColorInput<'a, Message, Theme>
where
    Message: 'static + Clone,
    Theme: Catalog
        + button::Catalog
        + text_widget::Catalog
        + text_input::Catalog
        + picker_style::Catalog
        + svg_widget::Catalog
        + 'a,
    <Theme as button::Catalog>::Class<'a>: From<button::StyleFn<'a, Theme>>,
    for<'c> <Theme as text_input::Catalog>::Class<'c>:
        From<text_input::StyleFn<'c, Theme>>,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<HexState>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(HexState {
            hex_buffer: None,
            alpha_buffer: None,
            angle_buffer: None,
            last_canonical: self.canonical_key(),
            hovered: false,
            modifiers: iced::keyboard::Modifiers::default(),
            old_show_picker: false,
            last_cursor: Point::ORIGIN,
            picker: picker_overlay::State::new(Color::BLACK),
        })
    }

    fn children(&self) -> Vec<Tree> {
        self.children.iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        {
            let state = tree.state.downcast_mut::<HexState>();
            if self.show_picker && !state.old_show_picker {
                state.picker.force_synchronize(self.solid_seed());
                state
                    .picker
                    .force_synchronize_gradient(self.gradient_seed());
            }
            state.old_show_picker = self.show_picker;
        }
        tree.diff_children(&self.children);
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: Length::Shrink,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &ConcreteRenderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let text_size = self.resolved_text_size();
        let canonical = self.canonical_key();
        let hex_c = self.value.hex_text();
        let alpha_c = format!("{}", self.value.alpha_pct());
        let angle_c = self.value.angle_text();
        let (hex_disp, alpha_disp, angle_disp) = {
            let state = tree.state.downcast_mut::<HexState>();
            if state.last_canonical != canonical {
                state.last_canonical = canonical.clone();
                state.hex_buffer = None;
                state.alpha_buffer = None;
                state.angle_buffer = None;
            }
            let hex_focused = tree.children.get(HEX).map_or(false, |child| {
                child
                    .state
                    .downcast_ref::<text_input::State<<ConcreteRenderer as text::Renderer>::Paragraph>>()
                    .is_focused()
            });
            let alpha_focused = tree.children.get(ALPHA).map_or(false, |child| {
                child
                    .state
                    .downcast_ref::<text_input::State<<ConcreteRenderer as text::Renderer>::Paragraph>>()
                    .is_focused()
            });
            let angle_focused = tree.children.get(ANGLE).map_or(false, |child| {
                child
                    .state
                    .downcast_ref::<text_input::State<<ConcreteRenderer as text::Renderer>::Paragraph>>()
                    .is_focused()
            });
            let hex_disp = if hex_focused {
                state.hex_buffer.clone().unwrap_or(hex_c.clone())
            } else {
                state.hex_buffer = None;
                hex_c.clone()
            };
            let alpha_disp = if alpha_focused {
                state.alpha_buffer.clone().unwrap_or(alpha_c.clone())
            } else {
                state.alpha_buffer = None;
                alpha_c.clone()
            };
            let angle_disp = if angle_focused {
                state.angle_buffer.clone().unwrap_or(angle_c.clone())
            } else {
                state.angle_buffer = None;
                angle_c.clone()
            };
            (hex_disp, alpha_disp, angle_disp)
        };
        self.rebuild_pill(&hex_disp, &alpha_disp, &angle_disp);

        let (sw, hw, aw, suf_w, icon_w, ang_w) = self.pill_widths();
        let mut widths = [sw, hw, aw, suf_w, icon_w, ang_w];
        let visible: Vec<f32> =
            widths.iter().copied().filter(|w| *w > 0.0).collect();
        let gaps = GAP * visible.len().saturating_sub(1) as f32;
        let content_w: f32 = visible.iter().sum::<f32>() + gaps;
        let intrinsic = Size::new(
            content_w + PAD_X * 2.0,
            text_size.0 + self.input_padding.y() + 14.0,
        );
        let size = limits
            .width(self.width)
            .resolve(self.width, Length::Shrink, intrinsic);
        // Flex the hex field so Fill pills keep the `%` suffix inside
        // (extra goes to hex; squeeze shrinks hex, then alpha).
        if widths[HEX] > 0.0 {
            let mut extra = size.width - intrinsic.width;
            if extra > 0.0 {
                widths[HEX] += extra;
            } else if extra < 0.0 {
                let take_hex = (-extra).min((widths[HEX] - HEX_MIN_W).max(0.0));
                widths[HEX] -= take_hex;
                extra += take_hex;
                if extra < 0.0 && widths[ALPHA] > 0.0 {
                    let take_alpha =
                        (-extra).min((widths[ALPHA] - ALPHA_MIN_W).max(0.0));
                    widths[ALPHA] -= take_alpha;
                }
            }
        }

        let mut nodes = Vec::with_capacity(PILL_COUNT);
        let mut x = PAD_X;
        for (index, w) in widths.iter().enumerate() {
            // Always run the child layout (even at 0 width) so widgets
            // like text_input produce their internal child nodes; a
            // hand-made empty node makes their draw() unwrap on None.
            let child_limits = layout::Limits::new(
                Size::ZERO,
                Size::new(*w, size.height),
            );
            let mut node = self.children[index].as_widget_mut().layout(
                &mut tree.children[index],
                renderer,
                &child_limits,
            );
            if *w <= 0.0 {
                node.move_to_mut(Point::new(x, 0.0));
                nodes.push(node);
                continue;
            }
            let offset_y =
                (size.height - node.size().height).max(0.0) / 2.0;
            node.move_to_mut(Point::new(x, offset_y));
            nodes.push(node);
            x += w + GAP;
        }
        layout::Node::with_children(size, nodes)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &ConcreteRenderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            self.children
                .iter_mut()
                .zip(&mut tree.children)
                .zip(layout.children())
                .take(PILL_COUNT)
                .for_each(|((child, state), layout)| {
                    child
                        .as_widget_mut()
                        .operate(state, layout, renderer, operation);
                });
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &ConcreteRenderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let hovered = cursor.is_over(bounds);
        let text_size = self.resolved_text_size();
        {
            let state = tree.state.downcast_mut::<HexState>();
            if state.hovered != hovered {
                state.hovered = hovered;
                shell.request_redraw();
            }
            state.last_cursor =
                cursor.position().unwrap_or(state.last_cursor);
            let canonical = self.canonical_key();
            if state.last_canonical != canonical {
                state.last_canonical = canonical;
                state.hex_buffer = None;
                state.alpha_buffer = None;
                state.angle_buffer = None;
                let hex = self.value.hex_text();
                let alpha = format!("{}", self.value.alpha_pct());
                let angle = self.value.angle_text();
                self.rebuild_pill(&hex, &alpha, &angle);
                shell.invalidate_layout();
            }
        }

        if let Event::Mouse(mouse::Event::CursorMoved { position }) = event {
            tree.state.downcast_mut::<HexState>().last_cursor = *position;
        }

        // Child bounds for per-field hover hit-testing (wheel stepping).
        let child_layouts: Vec<Layout<'_>> = layout.children().collect();

        match event {
            Event::Keyboard(iced::keyboard::Event::ModifiersChanged(m)) => {
                tree.state.downcast_mut::<HexState>().modifiers = *m;
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let y = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y,
                };
                if y != 0.0 {
                    let down = y < 0.0;
                    let large = tree
                        .state
                        .downcast_ref::<HexState>()
                        .modifiers
                        .shift();
                    let alpha_hit = child_layouts
                        .get(ALPHA)
                        .map_or(false, |l| cursor.is_over(l.bounds()));
                    let angle_hit = child_layouts
                        .get(ANGLE)
                        .map_or(false, |l| cursor.is_over(l.bounds()));
                    // Hidden fields have zero bounds and never hit, so
                    // gradient mode (alpha hidden) only steps the angle.
                    let stepped = if alpha_hit {
                        self.stepped_alpha(down, large)
                    } else if angle_hit {
                        self.stepped_angle(down, large)
                    } else {
                        None
                    };
                    if let Some(next) = stepped {
                        shell.publish((self.on_change)(next.clone()));
                        let key = Self::key_of(&next);
                        let hex = next.hex_text();
                        let alpha = format!("{}", next.alpha_pct());
                        let angle = next.angle_text();
                        {
                            let state =
                                tree.state.downcast_mut::<HexState>();
                            state.hex_buffer = None;
                            state.alpha_buffer = None;
                            state.angle_buffer = None;
                            state.last_canonical = key;
                        }
                        self.rebuild_pill(&hex, &alpha, &angle);
                        shell.capture_event();
                        shell.invalidate_layout();
                        shell.request_redraw();
                        return;
                    }
                    // At min/max `stepped_*` returns None (no value change).
                    // Still capture so the outer styling-panel scrollable
                    // does not scroll while the cursor is over a stepper.
                    if alpha_hit || angle_hit {
                        shell.capture_event();
                        return;
                    }
                }
            }
            Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key,
                modifiers,
                text: key_text,
                ..
            }) => {
                let hex_focused = self.is_text_focused(tree, HEX);
                let alpha_focused = self.is_text_focused(tree, ALPHA);
                let angle_focused = self.is_text_focused(tree, ANGLE);
                // Up/Down steps the focused numeric field.
                let arrow = match key.as_ref() {
                    iced::keyboard::Key::Named(
                        iced::keyboard::key::Named::ArrowUp,
                    ) => Some(false),
                    iced::keyboard::Key::Named(
                        iced::keyboard::key::Named::ArrowDown,
                    ) => Some(true),
                    _ => None,
                };
                if let Some(down) = arrow {
                    if alpha_focused || angle_focused {
                        let large = modifiers.shift();
                        let stepped = if alpha_focused {
                            self.stepped_alpha(down, large)
                        } else {
                            self.stepped_angle(down, large)
                        };
                        if let Some(next) = stepped {
                            shell.publish((self.on_change)(next.clone()));
                            let key = Self::key_of(&next);
                            let hex = next.hex_text();
                            let alpha =
                                format!("{}", next.alpha_pct());
                            let angle = next.angle_text();
                            {
                                let state =
                                    tree.state.downcast_mut::<HexState>();
                                state.hex_buffer = None;
                                state.alpha_buffer = None;
                                state.angle_buffer = None;
                                state.last_canonical = key;
                            }
                            self.rebuild_pill(&hex, &alpha, &angle);
                            shell.capture_event();
                            shell.invalidate_layout();
                            shell.request_redraw();
                            return;
                        }
                    }
                }
                if (hex_focused || alpha_focused || angle_focused)
                    && !modifiers.command()
                {
                    if let Some(pressed) = key_text {
                        if let Some(c) = pressed
                            .chars()
                            .next()
                            .filter(|c| !c.is_control())
                        {
                            let allowed = if hex_focused {
                                c.is_ascii_hexdigit() || c == '#'
                            } else if alpha_focused {
                                c.is_ascii_digit()
                            } else {
                                c.is_ascii_digit()
                                    || c == '.'
                                    || c == '-'
                                    || c == '+'
                            };
                            if !allowed {
                                shell.capture_event();
                                return;
                            }
                        }
                    }
                }
                if matches!(
                    key.as_ref(),
                    iced::keyboard::Key::Named(
                        iced::keyboard::key::Named::Escape,
                    ) | iced::keyboard::Key::Named(
                        iced::keyboard::key::Named::Enter,
                    )
                ) {
                    let dirty = {
                        let state = tree.state.downcast_ref::<HexState>();
                        state.hex_buffer.is_some()
                            || state.alpha_buffer.is_some()
                            || state.angle_buffer.is_some()
                    };
                    if dirty {
                        let hex = self.value.hex_text();
                        let alpha = format!("{}", self.value.alpha_pct());
                        let angle = self.value.angle_text();
                        {
                            let state =
                                tree.state.downcast_mut::<HexState>();
                            state.hex_buffer = None;
                            state.alpha_buffer = None;
                            state.angle_buffer = None;
                        }
                        self.rebuild_pill(&hex, &alpha, &angle);
                        shell.capture_event();
                        shell.invalidate_layout();
                        shell.request_redraw();
                    }
                }
            }
            _ => {}
        }

        for ((child, state), child_layout) in self
            .children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(child_layouts.iter())
            .take(PILL_COUNT)
        {
            child.as_widget_mut().update(
                state,
                event,
                *child_layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }

        let hex_raw = self.hex_stash.borrow_mut().take();
        let alpha_raw = self.alpha_stash.borrow_mut().take();
        let angle_raw = self.angle_stash.borrow_mut().take();
        let mut relayout = false;
        if let Some(raw) = hex_raw {
            if self.is_text_focused(tree, HEX) && !self.value.is_gradient() {
                if parse_hex_input(&raw, self.value.solid_color().a).is_some()
                {
                    let state = tree.state.downcast_mut::<HexState>();
                    state.hex_buffer = None;
                } else if is_hex_intermediate(&raw) {
                    let state = tree.state.downcast_mut::<HexState>();
                    state.hex_buffer = Some(raw.clone());
                    self.children[HEX] = self.make_hex(&raw, true);
                    relayout = true;
                } else {
                    let state = tree.state.downcast_mut::<HexState>();
                    state.hex_buffer = None;
                    let hex = self.value.hex_text();
                    let alpha = format!("{}", self.value.alpha_pct());
                    let angle = self.value.angle_text();
                    self.rebuild_pill(&hex, &alpha, &angle);
                    relayout = true;
                }
                shell.request_redraw();
            }
        }
        if let Some(raw) = alpha_raw {
            if self.is_text_focused(tree, ALPHA) && self.value.is_solid() {
                if parse_alpha_input(&raw).is_some() {
                    let state = tree.state.downcast_mut::<HexState>();
                    state.alpha_buffer = None;
                } else if is_alpha_intermediate(&raw) {
                    let state = tree.state.downcast_mut::<HexState>();
                    state.alpha_buffer = Some(raw.clone());
                    self.children[ALPHA] = self.make_alpha(&raw, true);
                    relayout = true;
                } else {
                    let state = tree.state.downcast_mut::<HexState>();
                    state.alpha_buffer = None;
                }
                shell.request_redraw();
            }
        }
        if let Some(raw) = angle_raw {
            if self.is_text_focused(tree, ANGLE) && self.value.is_gradient() {
                if parse_angle_input(&raw).is_some() {
                    let state = tree.state.downcast_mut::<HexState>();
                    state.angle_buffer = None;
                } else if is_angle_intermediate(&raw) {
                    let state = tree.state.downcast_mut::<HexState>();
                    state.angle_buffer = Some(raw.clone());
                    self.children[ANGLE] = self.make_angle(&raw, true);
                    relayout = true;
                } else {
                    let state = tree.state.downcast_mut::<HexState>();
                    state.angle_buffer = None;
                }
                shell.request_redraw();
            }
        }
        if relayout {
            shell.invalidate_layout();
        }

        if !self.is_text_focused(tree, HEX)
            && !self.is_text_focused(tree, ALPHA)
            && !self.is_text_focused(tree, ANGLE)
        {
            let revert = {
                let state = tree.state.downcast_ref::<HexState>();
                state.hex_buffer.is_some()
                    || state.alpha_buffer.is_some()
                    || state.angle_buffer.is_some()
            };
            if revert {
                {
                    let state = tree.state.downcast_mut::<HexState>();
                    state.hex_buffer = None;
                    state.alpha_buffer = None;
                    state.angle_buffer = None;
                }
                let hex = self.value.hex_text();
                let alpha = format!("{}", self.value.alpha_pct());
                let angle = self.value.angle_text();
                self.rebuild_pill(&hex, &alpha, &angle);
                shell.invalidate_layout();
                shell.request_redraw();
            }
        }

        let _ = text_size;
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &ConcreteRenderer,
    ) -> mouse::Interaction {
        self.children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .take(PILL_COUNT)
            .map(|((child, tree), layout)| {
                child.as_widget().mouse_interaction(
                    tree, layout, cursor, viewport, renderer,
                )
            })
            .max()
            .unwrap_or_default()
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut ConcreteRenderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let status = if self.is_text_focused(tree, HEX)
            || self.is_text_focused(tree, ALPHA)
            || self.is_text_focused(tree, ANGLE)
        {
            Status::Focused
        } else if cursor.is_over(bounds) {
            Status::Hovered
        } else {
            Status::Active
        };
        let mut appearance = Catalog::style(theme, &self.class, status);
        if let Some(radius) = self.border_radius {
            appearance.border.radius = radius;
        }
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border {
                    width: 0.0,
                    ..appearance.border
                },
                shadow: appearance.shadow,
                ..renderer::Quad::default()
            },
            appearance.background,
        );
        for ((child, state), child_layout) in self
            .children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .take(PILL_COUNT)
        {
            child.as_widget().draw(
                state,
                renderer,
                theme,
                style,
                child_layout,
                cursor,
                viewport,
            );
        }
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: appearance.border,
                ..renderer::Quad::default()
            },
            Background::Color(Color::TRANSPARENT),
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        _renderer: &ConcreteRenderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, ConcreteRenderer>> {
        if !self.show_picker {
            return None;
        }
        self.rebuild_picker_callbacks();
        let bounds = layout.bounds();
        let fallback_center = Point::new(bounds.center_x(), bounds.center_y());
        let parent_bounds = bounds + translation;
        let cursor_position = tree
            .state
            .downcast_ref::<HexState>()
            .last_cursor;
        let state: &mut HexState = tree.state.downcast_mut::<HexState>();
        let picker_state = &mut state.picker;
        let button_tree = &mut tree.children[OVERLAY_BUTTONS];
        let dropper = self.dropper_buffer.as_ref();
        let capture = self.on_dropper_capture.as_deref();
        Some(
            picker_overlay::ColorPickerWindow::new(
                picker_state,
                self.on_cancel.clone(),
                &self.picker_submit,
                Some(&self.picker_color_change),
                Some(&self.picker_gradient_submit),
                Some(&self.picker_gradient_change),
                Some(&self.picker_pick),
                Some(&self.picker_pick_submit),
                None,
                dropper,
                capture,
                self.position,
                parent_bounds,
                fallback_center,
                cursor_position,
                &self.picker_class,
                button_tree,
                *viewport,
            )
            .overlay(),
        )
    }
}

impl<'a, Message, Theme> From<HexColorInput<'a, Message, Theme>>
    for Element<'a, Message, Theme, ConcreteRenderer>
where
    Message: 'static + Clone,
    Theme: Catalog
        + button::Catalog
        + text_widget::Catalog
        + text_input::Catalog
        + picker_style::Catalog
        + svg_widget::Catalog
        + 'a,
    <Theme as button::Catalog>::Class<'a>: From<button::StyleFn<'a, Theme>>,
    for<'c> <Theme as text_input::Catalog>::Class<'c>:
        From<text_input::StyleFn<'c, Theme>>,
{
    fn from(input: HexColorInput<'a, Message, Theme>) -> Self {
        Element::new(input)
    }
}

/// Shortcut helper to create a [`HexColorInput`].
pub fn hex_color_input<'a, Message, Theme>(
    value: HexColorValue,
    on_change: impl Fn(HexColorValue) -> Message + 'a,
    show_picker: bool,
    on_open: Message,
    on_cancel: Message,
) -> HexColorInput<'a, Message, Theme>
where
    Message: 'static + Clone,
    Theme: Catalog
        + button::Catalog
        + text_widget::Catalog
        + text_input::Catalog
        + picker_style::Catalog
        + svg_widget::Catalog
        + 'a,
    <Theme as button::Catalog>::Class<'a>: From<button::StyleFn<'a, Theme>>,
    for<'c> <Theme as text_input::Catalog>::Class<'c>:
        From<text_input::StyleFn<'c, Theme>>,
{
    HexColorInput::new(value, on_change, show_picker, on_open, on_cancel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angle_wraps() {
        assert!((normalize_angle(370.0) - 10.0).abs() < f32::EPSILON);
        assert!((normalize_angle(-10.0) - 350.0).abs() < f32::EPSILON);
    }

    #[test]
    fn hex_text_format() {
        let v = HexColorValue::Solid(Color::from_rgba8(255, 255, 255, 1.0));
        assert_eq!(v.hex_text(), "FFFFFF");
        assert_eq!(v.alpha_pct(), 100);
    }

    #[test]
    fn alpha_hidden_on_gradient_background() {
        let g = Gradient::two(Color::WHITE, Color::BLACK);
        let v = HexColorValue::Gradient {
            gradient: g,
            angle: 90.0,
        };
        assert!(v.is_gradient());
        assert!(matches!(
            v.to_background(),
            Background::Gradient(_)
        ));
    }

    #[test]
    fn parses_hex_and_alpha() {
        let c = parse_hex_input("FFFFFF", 1.0).expect("white");
        assert_eq!(c.r, 1.0);
        assert_eq!(parse_alpha_input("100"), Some(100));
        assert_eq!(parse_alpha_input("101"), None);
        assert_eq!(parse_angle_input("45"), Some(45.0));
    }
}
