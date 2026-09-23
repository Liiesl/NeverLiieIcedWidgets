//! A horizontal numeric input with increment/decrement buttons.
//!
//! [`NumberInput`] renders as a single rounded pill with the layout
//! `(-|input|+)`, or `(-|{prefix} input|+)` when an optional prefix
//! widget is set: a decrement button on the left, an optional prefix
//! widget plus a text field sharing the middle zone (no divider between
//! them), and an increment button on the right. This is intentionally
//! horizontal, not the Qt-style vertical spinbox.
//!
//! The widget is composed from iced's own primitives instead of
//! re-implementing text editing:
//!
//! - the middle field is a real [`text_input`](iced::widget::text_input),
//!   so selection, clipboard, IME and friends keep working;
//! - both `-` and `+` are real [`button`](iced::widget::button)s, so they
//!   get native hover / pressed / disabled styling and behavior.
//!
//! On top of that the wrapper adds:
//!
//! - Up/Down arrow keys step the value when the pill is hovered or the
//!   field is focused (Shift uses `shift_step` when set);
//! - mouse wheel over the pill steps the value (no modifier required)
//!   and is captured so an outer scrollable does not scroll;
//! - typed text is filtered live (invalid characters never reach the
//!   field) and parsed values are clamped to `min..=max`;
//! - in-progress text such as `"-"` or `"5."` is kept while typing so
//!   negative and fractional numbers can be entered digit by digit.
//!
//! # Example
//! ```no_run
//! use iced::Element;
//! use neverliie_iced_widgets::number_input::NumberInput;
//!
//! #[derive(Clone)]
//! enum Message {
//!     CountChanged(i32),
//! }
//!
//! fn view(value: i32) -> Element<'_, Message> {
//!     NumberInput::new(0..=100, value, Message::CountChanged)
//!         .step(1)
//!         .border_radius(999.0)
//!         .into()
//! }
//! ```

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text::{self, Paragraph as _};
use iced::advanced::widget::{self, Operation, Tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::widget::{button, text_input, text as text_widget};
use iced::{
    border, keyboard, mouse, touch, Background, Border, Color, Element,
    Event, Length, Padding, Pixels, Point, Rectangle, Shadow, Size,
};
use std::cell::RefCell;
use std::ops::{Add, RangeInclusive, Sub};
use std::rc::Rc;
use std::str::FromStr;

/// Child index of the decrement button (always first).
const DEC: usize = 0;
/// Child index of the optional prefix widget when present.
const PREFIX: usize = 1;
/// Fallback width estimate for the prefix before its first layout.
const FALLBACK_PREFIX_WIDTH: f32 = 28.0;
/// Breathing room inside the middle zone: separator-to-prefix inset and
/// prefix-to-input gap. Without it the prefix sits flush on the `|`
/// separator and the input starts flush on the prefix.
const PREFIX_GAP: f32 = 6.0;

/// Default outer pill corner radius (matches [`default`]).
const DEFAULT_RADIUS: f32 = 12.0;

/// Text-size estimate used before the first layout (no renderer yet).
const FALLBACK_TEXT_SIZE: f32 = 16.0;

/// Stash shared between the wrapper and its inner `text_input`.
///
/// `text_input` reports every edit through `on_input`, which cannot touch
/// the wrapper's tree state. The closure stashes the raw string here and
/// the wrapper drains it right after forwarding the event, so
/// in-progress text (`"-"`, `"5."`, …) survives even though the parent
/// only ever holds the numeric value.
type InputStash = Rc<RefCell<Option<String>>>;

/// A horizontal numeric input `(-|input|+)` with a rounded outer pill.
///
/// With [`NumberInput::prefix`] an optional widget can be placed between
/// the `-` button and the text field, sharing the middle zone:
/// `(-|{prefix} input|+)`. The prefix can be any [`Element`](iced::Element)
/// — a unit label, an icon, an image — like `context_menu` / `advanced_dropdown`
/// icons. There is no divider between prefix and input.
///
/// Generic over any numeric type that can be displayed, parsed from a
/// string and stepped with `+`/`-`.
pub struct NumberInput<
    'a,
    T,
    Message,
    Theme = iced::Theme,
    Renderer = iced::Renderer,
> where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    range: RangeInclusive<T>,
    value: T,
    on_change: Rc<dyn Fn(T) -> Message + 'a>,
    step: T,
    shift_step: Option<T>,
    placeholder: String,
    width: Length,
    input_padding: Padding,
    text_size: Option<Pixels>,
    font: Option<Renderer::Font>,
    border_radius: Option<border::Radius>,
    button_width: Option<f32>,
    prefix_width: Option<f32>,
    class: <Theme as Catalog>::Class<'a>,
    button_style:
        Option<Rc<dyn Fn(&Theme, button::Status) -> button::Style + 'a>>,
    input_style:
        Option<Rc<dyn Fn(&Theme, text_input::Status) -> text_input::Style + 'a>>,
    input_id: Option<widget::Id>,
    stash: InputStash,
    /// Combined children: `[dec, input, inc]` or
    /// `[dec, prefix, input, inc]` when a prefix is set.
    children: Vec<Element<'a, Message, Theme, Renderer>>,
}

impl<'a, T, Message, Theme, Renderer> NumberInput<'a, T, Message, Theme, Renderer>
where
    T: Copy
        + PartialOrd
        + PartialEq
        + std::fmt::Display
        + FromStr
        + Add<Output = T>
        + Sub<Output = T>
        + From<u8>
        + num_traits::FromPrimitive
        + Into<f64>
        + 'a,
    Message: Clone + 'a,
    Theme: Catalog
        + button::Catalog
        + text_input::Catalog
        + iced::widget::text::Catalog
        + 'a,
    Renderer: text::Renderer + 'a,
    <Theme as button::Catalog>::Class<'a>:
        From<button::StyleFn<'a, Theme>>,
    <Theme as text_input::Catalog>::Class<'a>:
        From<text_input::StyleFn<'a, Theme>>,
{
    /// Creates a new [`NumberInput`] with the given range, current value,
    /// and change handler.
    pub fn new(
        range: RangeInclusive<T>,
        value: T,
        on_change: impl Fn(T) -> Message + 'a,
    ) -> Self {
        let step = Self::infer_step(&range);
        let on_change: Rc<dyn Fn(T) -> Message + 'a> =
            Rc::new(on_change);
        let stash: InputStash = Rc::new(RefCell::new(None));
        let placeholder = String::new();
        let input_padding = Padding::new(5.0);
        let text_size = Pixels(FALLBACK_TEXT_SIZE);
        let button_w =
            button_width_for(None, input_padding, text_size);
        let canonical = canonical_of(value);
        let mut this = Self {
            range,
            value,
            on_change,
            step,
            shift_step: None,
            placeholder,
            width: Length::Shrink,
            input_padding,
            text_size: None,
            font: None,
            border_radius: None,
            button_width: None,
            prefix_width: None,
            class: <Theme as Catalog>::default(),
            button_style: None,
            input_style: None,
            input_id: None,
            stash,
            children: vec![
                Element::new(text_widget("")),
                Element::new(text_widget("")),
                Element::new(text_widget("")),
            ],
        };
        let outer = this.cap_radius(this.estimated_height());
        this.children = vec![
            this.make_button(false, text_size, button_w, outer),
            this.make_input(&canonical, text_size),
            this.make_button(true, text_size, button_w, outer),
        ];
        this
    }

    fn infer_step(range: &RangeInclusive<T>) -> T {
        let start: f64 = (*range.start()).into();
        let end: f64 = (*range.end()).into();
        let len = (end - start).abs();
        if len < 1.0 {
            T::from_f64(0.01).unwrap_or_else(|| T::from(1u8))
        } else if len < 10.0 {
            T::from_f64(0.1).unwrap_or_else(|| T::from(1u8))
        } else {
            T::from(1u8)
        }
    }

    /// Sets the step used by buttons, arrow keys and the mouse wheel.
    #[must_use]
    pub fn step(mut self, step: impl Into<T>) -> Self {
        self.step = step.into();
        self.rebuild_buttons_estimated();
        self
    }

    /// Sets the step used while Shift is held.
    #[must_use]
    pub fn shift_step(mut self, step: impl Into<T>) -> Self {
        self.shift_step = Some(step.into());
        self.rebuild_buttons_estimated();
        self
    }

    /// Sets the placeholder shown when the field is empty and unfocused.
    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self.rebuild_input(
            &canonical_of(self.value),
            self.resolved_text_size_estimated(),
        );
        self
    }

    /// Sets the width of the whole pill.
    #[must_use]
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the padding of the inner text field.
    #[must_use]
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.input_padding = padding.into();
        self.rebuild_input(
            &canonical_of(self.value),
            self.resolved_text_size_estimated(),
        );
        self.rebuild_buttons_estimated();
        self
    }

    /// Sets the text size (field glyphs and button glyphs).
    #[must_use]
    pub fn text_size(mut self, size: impl Into<Pixels>) -> Self {
        self.text_size = Some(size.into());
        self.rebuild_input(
            &canonical_of(self.value),
            self.resolved_text_size_estimated(),
        );
        self.rebuild_buttons_estimated();
        self
    }

    /// Sets the font.
    #[must_use]
    pub fn font(mut self, font: impl Into<Renderer::Font>) -> Self {
        self.font = Some(font.into());
        self.rebuild_input(
            &canonical_of(self.value),
            self.resolved_text_size_estimated(),
        );
        self.rebuild_buttons_estimated();
        self
    }

    /// Sets the outer pill corner radius. Use `999.0` for a full pill.
    #[must_use]
    pub fn border_radius(
        mut self,
        radius: impl Into<border::Radius>,
    ) -> Self {
        self.border_radius = Some(radius.into());
        self
    }

    /// Overrides the width of each `-`/`+` button zone.
    #[must_use]
    pub fn button_width(mut self, width: f32) -> Self {
        self.button_width = Some(width);
        self.rebuild_buttons_estimated();
        self
    }

    /// Sets an optional widget placed between the `-` button and the
    /// text field: `(-|{prefix} input|+)`.
    ///
    /// The prefix can be any [`Element`](iced::Element) — a unit label
    /// ([`text`](iced::widget::text)), an icon, an image, or any other
    /// widget — like `context_menu` / `advanced_dropdown` icons. It
    /// shares the middle zone with the input and has no divider between
    /// them. It is fully interactive (events are forwarded).
    #[must_use]
    pub fn prefix(
        mut self,
        prefix: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        let prefix = prefix.into();
        if self.has_prefix() {
            self.children[PREFIX] = prefix;
        } else {
            self.children.insert(PREFIX, prefix);
        }
        self
    }

    /// Overrides the width reserved for the prefix widget.
    ///
    /// By default the prefix is auto-sized (measured with `Shrink`
    /// limits, remainder goes to the input). Set a fixed width to force
    /// an exact middle-zone split instead.
    #[must_use]
    pub fn prefix_width(mut self, width: f32) -> Self {
        self.prefix_width = Some(width);
        self
    }

    /// Sets the id forwarded to the inner text field, enabling
    /// programmatic focus via iced's focus operations.
    #[must_use]
    pub fn id(mut self, id: impl Into<widget::Id>) -> Self {
        self.input_id = Some(id.into());
        self.rebuild_input(
            &canonical_of(self.value),
            self.resolved_text_size_estimated(),
        );
        self
    }

    /// Sets the style of the outer pill.
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

    /// Sets the style class of the outer pill.
    #[must_use]
    pub fn class(
        mut self,
        class: impl Into<<Theme as Catalog>::Class<'a>>,
    ) -> Self {
        self.class = class.into();
        self
    }

    /// Overrides the style of the inner `-`/`+` buttons.
    ///
    /// By default the buttons use the theme's native button colors (so
    /// hover / pressed / disabled states work out of the box) with
    /// segmented corners, forming the flush `(` / `)` ends of the pill.
    #[must_use]
    pub fn button_style(
        mut self,
        style: impl Fn(&Theme, button::Status) -> button::Style + 'a,
    ) -> Self {
        self.button_style = Some(Rc::new(style));
        self.rebuild_buttons_estimated();
        self
    }

    /// Overrides the style of the inner text field.
    ///
    /// By default the field uses the theme's native text-input style
    /// with the background made transparent and the border removed, so
    /// only the outer pill draws a frame.
    #[must_use]
    pub fn input_style(
        mut self,
        style: impl Fn(&Theme, text_input::Status) -> text_input::Style + 'a,
    ) -> Self {
        self.input_style = Some(Rc::new(style));
        self.rebuild_input(
            &canonical_of(self.value),
            self.resolved_text_size_estimated(),
        );
        self
    }

    fn resolved_text_size_estimated(&self) -> Pixels {
        self.text_size.unwrap_or(Pixels(FALLBACK_TEXT_SIZE))
    }

    fn button_w_estimated(&self) -> f32 {
        button_width_for(
            self.button_width,
            self.input_padding,
            self.resolved_text_size_estimated(),
        )
    }

    /// Whether a prefix widget is set (`[dec, prefix, input, inc]`).
    fn has_prefix(&self) -> bool {
        self.children.len() == 4
    }

    /// Child index of the text field (1 without prefix, 2 with prefix).
    fn input_index(&self) -> usize {
        if self.has_prefix() { PREFIX + 1 } else { PREFIX }
    }

    /// Child index of the increment button (last child).
    fn inc_index(&self) -> usize {
        self.children.len() - 1
    }

    /// Width estimate for the prefix before its first layout.
    fn prefix_w_estimated(&self) -> f32 {
        self.prefix_width.unwrap_or(FALLBACK_PREFIX_WIDTH)
    }

    fn rebuild_buttons_estimated(&mut self) {
        let text_size = self.resolved_text_size_estimated();
        let button_w = self.button_w_estimated();
        let outer = self.cap_radius(self.estimated_height());
        let inc = self.inc_index();
        self.children[DEC] =
            self.make_button(false, text_size, button_w, outer);
        self.children[inc] =
            self.make_button(true, text_size, button_w, outer);
    }

    /// Estimated pill height before the first layout (mirrors the
    /// intrinsic height formula in [`layout`](Widget::layout)).
    fn estimated_height(&self) -> f32 {
        self.resolved_text_size_estimated().0 + self.input_padding.y() + 14.0
    }

    /// Outer corner radius for the `-`/`+` caps given the pill height.
    ///
    /// Clamped to half the height so a large pill radius (e.g. `999.0`
    /// for a full pill) renders as a true semicircle that coincides
    /// with the pill's outer ring, instead of an eclipsed curve that
    /// looks smaller than the border.
    fn cap_radius(&self, height: f32) -> f32 {
        let pill = self.pill_radius();
        // Outer corners share the same radius on top/bottom for the
        // radii we produce (`f32` uniform or default); take the most
        // conservative so the cap never overshoots the ring.
        let outer = pill
            .top_left
            .min(pill.bottom_left)
            .min(pill.top_right)
            .min(pill.bottom_right);
        outer.min((height / 2.0).max(0.0)).max(0.0)
    }

    fn rebuild_input(&mut self, display: &str, text_size: Pixels) {
        let input = self.input_index();
        self.children[input] = self.make_input(display, text_size);
    }

    /// Effective outer pill radius (user override or default).
    fn pill_radius(&self) -> border::Radius {
        self.border_radius.unwrap_or(DEFAULT_RADIUS.into())
    }

    fn step_for(&self, large: bool) -> T {
        if large {
            self.shift_step.unwrap_or(self.step)
        } else {
            self.step
        }
    }

    fn stepped(&self, down: bool, large: bool) -> T {
        clamp_in(
            &self.range,
            if down {
                self.value - self.step_for(large)
            } else {
                self.value + self.step_for(large)
            },
        )
    }

    /// Builds the `-` (`increment == false`) or `+` button.
    ///
    /// Real iced buttons: native hover / pressed / disabled behavior.
    /// Disabled (no `on_press`) when stepping would not change the value.
    ///
    /// `outer` is the already-clamped outer corner radius (see
    /// [`NumberInput::cap_radius`]): the outer corners use it and the
    /// inner corners are square, so `-` / `+` form the flush `(` / `)`
    /// ends of the pill instead of floating boxes. The button is laid
    /// out to fill its whole end zone edge-to-edge, so the cap curve
    /// coincides with the pill's outer ring.
    fn make_button(
        &self,
        increment: bool,
        text_size: Pixels,
        button_w: f32,
        outer: f32,
    ) -> Element<'a, Message, Theme, Renderer> {
        let next = self.stepped(!increment, false);
        let enabled = next != self.value;
        let glyph = if increment { "+" } else { "-" };
        let mut btn = button(
            text_widget(glyph)
                .size(text_size)
                .width(Length::Fill)
                .center(),
        )
        .width(Length::Fixed(button_w))
        .height(Length::Fill)
        .padding(Padding::new(2.0))
        .on_press_maybe(enabled.then(|| (self.on_change)(next)));
        match &self.button_style {
            Some(style) => {
                let style = Rc::clone(style);
                btn = btn.style(move |theme, status| style(theme, status));
            }
            None => {
                // Segmented cap: native theme colors (hover / pressed /
                // disabled all keep working).
                let cap = if increment {
                    border::Radius {
                        top_right: outer,
                        bottom_right: outer,
                        top_left: 0.0,
                        bottom_left: 0.0,
                    }
                } else {
                    border::Radius {
                        top_left: outer,
                        bottom_left: outer,
                        top_right: 0.0,
                        bottom_right: 0.0,
                    }
                };
                btn = btn.style(
                    move |theme: &Theme, status: button::Status| {
                        let mut style =
                            <Theme as button::Catalog>::style(
                                theme,
                                &<Theme as button::Catalog>::default(),
                                status,
                            );
                        style.border.radius = cap;
                        style.border.width = 0.0;
                        style.shadow = Shadow::default();
                        style
                    },
                );
            }
        }
        btn.into()
    }

    /// Builds the inner text field showing `display`.
    ///
    /// A real iced `text_input`: selection, clipboard, IME and friends
    /// keep working. Every edit is reported through `on_input`, stashed
    /// for the wrapper, and mapped to `on_change` (valid text publishes
    /// the clamped value, anything else re-publishes the current value
    /// as a harmless no-op).
    fn make_input(
        &self,
        display: &str,
        text_size: Pixels,
    ) -> Element<'a, Message, Theme, Renderer> {
        let stash = Rc::clone(&self.stash);
        let on_change = Rc::clone(&self.on_change);
        let current = self.value;
        let range = self.range.clone();
        let mut field = text_input(&self.placeholder, display)
            .on_input(move |s: String| {
                *stash.borrow_mut() = Some(s.clone());
                if is_intermediate_text::<T>(&s) {
                    on_change(current)
                } else {
                    match s.parse::<T>() {
                        Ok(v) => on_change(clamp_in(&range, v)),
                        Err(_) => on_change(current),
                    }
                }
            })
            .width(Length::Fill)
            .padding(self.input_padding)
            .size(text_size);
        if let Some(font) = self.font {
            field = field.font(font);
        }
        if let Some(id) = &self.input_id {
            field = field.id(id.clone());
        }
        match &self.input_style {
            Some(style) => {
                let style = Rc::clone(style);
                field = field.style(move |theme, status| {
                    style(theme, status)
                });
            }
            None => {
                field = field.style(stripped_input_style::<Theme>);
            }
        }
        field.into()
    }

    /// Is the inner text field currently focused?
    fn is_input_focused(&self, tree: &Tree) -> bool {
        tree.children
            .get(self.input_index())
            .map(|child| {
                child
                    .state
                    .downcast_ref::<text_input::State<Renderer::Paragraph>>()
                    .is_focused()
            })
            .unwrap_or(false)
    }

    /// Reverts an unfinished edit back to the canonical value.
    /// Returns true when something was reverted.
    fn revert_if_dirty(
        &mut self,
        tree: &mut Tree,
        text_size: Pixels,
    ) -> bool {
        let canonical = canonical_of(self.value);
        let dirty = {
            let state = tree.state.downcast_ref::<PillState>();
            state.buffer.is_some()
        };
        if dirty {
            let state = tree.state.downcast_mut::<PillState>();
            state.buffer = None;
            self.rebuild_input(&canonical, text_size);
            true
        } else {
            false
        }
    }
}

/// Native text-input style with background and border stripped, so only
/// the outer pill draws a frame. Value / placeholder / selection colors
/// still come from the theme.
fn stripped_input_style<Theme>(
    theme: &Theme,
    status: text_input::Status,
) -> text_input::Style
where
    Theme: text_input::Catalog,
{
    let mut style = theme.style(
        &<Theme as text_input::Catalog>::default(),
        status,
    );
    style.background = Background::Color(Color::TRANSPARENT);
    style.border = Border {
        width: 0.0,
        ..style.border
    };
    style
}

fn canonical_of<T: std::fmt::Display>(value: T) -> String {
    value.to_string()
}

fn clamp_in<T: Copy + PartialOrd>(
    range: &RangeInclusive<T>,
    v: T,
) -> T {
    let (min, max) = (*range.start(), *range.end());
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

fn button_width_for(
    override_width: Option<f32>,
    padding: Padding,
    text_size: Pixels,
) -> f32 {
    override_width
        .unwrap_or_else(|| (text_size.0 + padding.x()).max(30.0))
}

fn split_zones(
    bounds: Rectangle,
    button_w: f32,
) -> (Rectangle, Rectangle, Rectangle) {
    let bw = button_w.min(bounds.width / 3.0).max(0.0);
    let dec = Rectangle {
        width: bw,
        ..bounds
    };
    let inc = Rectangle {
        x: bounds.x + bounds.width - bw,
        width: bw,
        ..bounds
    };
    let input = Rectangle {
        x: bounds.x + bw,
        width: (bounds.width - bw * 2.0).max(0.0),
        ..bounds
    };
    (dec, input, inc)
}

fn measured_text_width<Renderer>(
    content: &str,
    font: Renderer::Font,
    size: Pixels,
) -> f32
where
    Renderer: text::Renderer,
{
    if content.is_empty() {
        return 0.0;
    }
    let paragraph =
        <Renderer::Paragraph as text::Paragraph>::with_text(text::Text {
            content,
            bounds: Size::new(f32::INFINITY, f32::INFINITY),
            size,
            line_height: text::LineHeight::default(),
            font,
            align_x: text::Alignment::Default,
            align_y: iced::alignment::Vertical::Top,
            shaping: text::Shaping::Basic,
            wrapping: text::Wrapping::None,
        });
    paragraph.min_width()
}

fn is_float_type<T: FromStr>() -> bool {
    "0.5".parse::<T>().is_ok()
}

fn is_trailing_dot<T: FromStr>(s: &str) -> bool {
    if !is_float_type::<T>() {
        return false;
    }
    if !s.ends_with('.') {
        return false;
    }
    s[..s.len().saturating_sub(1)].parse::<T>().is_ok()
}

/// Text that is not a complete number yet but is a valid prefix of one
/// the user may still be typing (`"-"`, `"5."`, `"1e"`, …).
fn is_intermediate_text<T: FromStr>(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    if s == "-" || s == "+" {
        return true;
    }
    if !is_float_type::<T>() {
        return false;
    }
    if s == "." || s == "-." || s == "+." {
        return true;
    }
    if is_trailing_dot::<T>(s) {
        return true;
    }
    let lower = s.to_ascii_lowercase();
    if lower.ends_with('e')
        || lower.ends_with("e-")
        || lower.ends_with("e+")
    {
        return true;
    }
    false
}

fn is_allowed_char<T: FromStr>(c: char) -> bool {
    if c.is_ascii_digit() {
        return true;
    }
    match c {
        '-' | '+' => true,
        '.' | 'e' | 'E' => is_float_type::<T>(),
        _ => false,
    }
}

/// Wrapper tree state: the in-progress text buffer plus hover/modifier
/// tracking. The actual editing state (cursor, selection, focus) lives
/// in the inner `text_input`'s own tree.
#[derive(Debug, Clone, Default)]
struct PillState {
    buffer: Option<String>,
    last_canonical: String,
    modifiers: keyboard::Modifiers,
    hovered: bool,
}

/// The visual status of a [`NumberInput`]'s outer pill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Idle, unfocused.
    Active,
    /// Mouse over.
    Hovered,
    /// Text field focused.
    Focused,
}

/// The appearance of a [`NumberInput`]'s outer pill.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// Pill background.
    pub background: Background,
    /// Pill border (radius drives the rounded `()` look).
    pub border: Border,
    /// Pill shadow.
    pub shadow: Shadow,
}

/// Theme catalog for the [`NumberInput`] outer pill.
pub trait Catalog {
    /// Style class.
    type Class<'a>;
    /// Default class.
    fn default<'a>() -> <Self as Catalog>::Class<'a>;
    /// Style for a status.
    fn style(
        &self,
        class: &<Self as Catalog>::Class<'_>,
        status: Status,
    ) -> Style;
}

/// Styling closure for the outer pill.
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

/// Default pill style, mirroring `text_input` with a rounded border.
pub fn default(theme: &iced::Theme, status: Status) -> Style {
    let palette = theme.extended_palette();

    let active = Style {
        background: Background::Color(palette.background.base.color),
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

impl<'a, T, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for NumberInput<'a, T, Message, Theme, Renderer>
where
    T: Copy
        + PartialOrd
        + PartialEq
        + std::fmt::Display
        + FromStr
        + Add<Output = T>
        + Sub<Output = T>
        + From<u8>
        + num_traits::FromPrimitive
        + Into<f64>
        + 'a,
    Message: Clone + 'a,
    Theme: Catalog
        + button::Catalog
        + text_input::Catalog
        + iced::widget::text::Catalog
        + 'a,
    Renderer: renderer::Renderer + text::Renderer + 'a,
    <Theme as button::Catalog>::Class<'a>:
        From<button::StyleFn<'a, Theme>>,
    <Theme as text_input::Catalog>::Class<'a>:
        From<text_input::StyleFn<'a, Theme>>,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<PillState>()
    }

    fn state(&self) -> widget::tree::State {
        let canonical = canonical_of(self.value);
        widget::tree::State::new(PillState {
            buffer: None,
            last_canonical: canonical,
            modifiers: keyboard::Modifiers::default(),
            hovered: false,
        })
    }

    fn children(&self) -> Vec<Tree> {
        self.children.iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
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
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let text_size =
            self.text_size.unwrap_or_else(|| renderer.default_size());
        let font = self.font.unwrap_or_else(|| renderer.default_font());
        let button_w = button_width_for(
            self.button_width,
            self.input_padding,
            text_size,
        );

        // Sync with external value changes and re-apply the in-progress
        // buffer (if the field still has focus).
        let canonical = canonical_of(self.value);
        let focused = self.is_input_focused(tree);
        let display = {
            let state = tree.state.downcast_mut::<PillState>();
            if state.last_canonical != canonical {
                state.last_canonical = canonical.clone();
                state.buffer = None;
            }
            if focused {
                if let Some(buffer) = state.buffer.clone() {
                    buffer
                } else {
                    canonical.clone()
                }
            } else {
                state.buffer = None;
                canonical.clone()
            }
        };
        self.rebuild_input(&display, text_size);
        // Height-aware caps need the final height; build once with the
        // estimate now (correct enabled state from the start) and again
        // below once `size` is resolved.
        self.rebuild_buttons_estimated();

        let text_w =
            measured_text_width::<Renderer>(&canonical, font, text_size);
        let prefix_estimate = if self.has_prefix() {
            self.prefix_w_estimated() + PREFIX_GAP * 2.0
        } else {
            0.0
        };
        let intrinsic = Size::new(
            (button_w * 2.0
                + prefix_estimate
                + text_w
                + self.input_padding.x()
                + 32.0)
                .max(120.0),
            text_size.0 + self.input_padding.y() + 14.0,
        );
        let size = limits
            .width(self.width)
            .resolve(self.width, Length::Shrink, intrinsic);

        let zone_bounds = Rectangle {
            x: 0.0,
            y: 0.0,
            width: size.width,
            height: size.height,
        };
        // `split_zones` middle slot is the combined `[prefix]input` zone:
        // no divider is drawn between prefix and input.
        let (dec_zone, middle_zone, inc_zone) =
            split_zones(zone_bounds, button_w);

        // Rebuild caps with the real height so the outer radius is a
        // true semicircle (half height) when the pill radius is large.
        {
            let outer = self.cap_radius(size.height);
            let inc = self.inc_index();
            self.children[DEC] =
                self.make_button(false, text_size, button_w, outer);
            self.children[inc] =
                self.make_button(true, text_size, button_w, outer);
        }

        if !self.has_prefix() {
            let mut nodes = Vec::with_capacity(3);
            for (index, zone) in
                [dec_zone, middle_zone, inc_zone].into_iter().enumerate()
            {
                let is_button = index == DEC || index == self.inc_index();
                if is_button {
                    // End caps fill their zone edge-to-edge so the cap
                    // curve coincides with the pill's outer ring.
                    let zone_size = Size::new(zone.width, zone.height);
                    let child_limits =
                        layout::Limits::new(zone_size, zone_size);
                    let mut node = self.children[index]
                        .as_widget_mut()
                        .layout(
                            &mut tree.children[index],
                            renderer,
                            &child_limits,
                        );
                    node.move_to_mut(Point::new(zone.x, zone.y));
                    nodes.push(node);
                } else {
                    let child_limits = layout::Limits::new(
                        Size::ZERO,
                        Size::new(zone.width, zone.height),
                    );
                    let mut node = self.children[index]
                        .as_widget_mut()
                        .layout(
                            &mut tree.children[index],
                            renderer,
                            &child_limits,
                        );
                    // Center shorter content (the text field) vertically.
                    let offset_y =
                        (zone.height - node.size().height).max(0.0) / 2.0;
                    node.move_to_mut(Point::new(zone.x, zone.y + offset_y));
                    nodes.push(node);
                }
            }

            return layout::Node::with_children(size, nodes);
        }

        // With prefix: `(-| prefix input |+)`. Prefix is auto-sized
        // (Shrink, remainder goes to the input) unless `prefix_width`
        // forces an exact split.
        let mut nodes = Vec::with_capacity(4);

        // Decrement button: fills its end zone edge-to-edge.
        {
            let zone_size = Size::new(dec_zone.width, dec_zone.height);
            let child_limits =
                layout::Limits::new(zone_size, zone_size);
            let mut node = self.children[DEC].as_widget_mut().layout(
                &mut tree.children[DEC],
                renderer,
                &child_limits,
            );
            node.move_to_mut(Point::new(dec_zone.x, dec_zone.y));
            nodes.push(node);
        }

        // Inset the prefix off the `|` separator and leave a gap
        // before the input: `| gap prefix gap input |`.
        let middle_w = middle_zone.width;
        let prefix_x = middle_zone.x + PREFIX_GAP;
        let avail_w = (middle_w - PREFIX_GAP * 2.0).max(0.0);
        let prefix_w = if let Some(fixed) = self.prefix_width {
            fixed.min(avail_w).max(0.0)
        } else {
            // Auto: measure with loose limits so a Shrink prefix takes
            // only what it needs.
            let probe_limits = layout::Limits::new(
                Size::ZERO,
                Size::new(avail_w, middle_zone.height),
            );
            let probe = self.children[PREFIX].as_widget_mut().layout(
                &mut tree.children[PREFIX],
                renderer,
                &probe_limits,
            );
            let measured = probe.size().width;
            // Reuse the probe as the final node when it already fits.
            let offset_y = (middle_zone.height - probe.size().height)
                .max(0.0)
                / 2.0;
            let mut node = probe;
            node.move_to_mut(Point::new(
                prefix_x,
                middle_zone.y + offset_y,
            ));
            nodes.push(node);
            measured.min(avail_w).max(0.0)
        };

        if self.prefix_width.is_some() {
            let child_limits = layout::Limits::new(
                Size::ZERO,
                Size::new(prefix_w, middle_zone.height),
            );
            let mut node = self.children[PREFIX]
                .as_widget_mut()
                .layout(
                    &mut tree.children[PREFIX],
                    renderer,
                    &child_limits,
                );
            // Center a narrower prefix in its fixed column (like
            // dropdown menu icons in their fixed icon column) instead
            // of leaving all dead space on one side.
            let offset_x =
                (prefix_w - node.size().width).max(0.0) / 2.0;
            let offset_y =
                (middle_zone.height - node.size().height).max(0.0) / 2.0;
            node.move_to_mut(Point::new(
                prefix_x + offset_x,
                middle_zone.y + offset_y,
            ));
            nodes.push(node);
        }

        // Text field takes the remainder of the middle zone.
        {
            let input = self.input_index();
            let input_x = prefix_x + prefix_w + PREFIX_GAP;
            let input_zone = Rectangle {
                x: input_x,
                width: (middle_zone.x + middle_w - input_x).max(0.0),
                ..middle_zone
            };
            let child_limits = layout::Limits::new(
                Size::ZERO,
                Size::new(input_zone.width, input_zone.height),
            );
            let mut node = self.children[input].as_widget_mut().layout(
                &mut tree.children[input],
                renderer,
                &child_limits,
            );
            let offset_y =
                (input_zone.height - node.size().height).max(0.0) / 2.0;
            node.move_to_mut(Point::new(
                input_zone.x,
                input_zone.y + offset_y,
            ));
            nodes.push(node);
        }

        // Increment button: fills its end zone edge-to-edge.
        {
            let inc = self.inc_index();
            let zone_size = Size::new(inc_zone.width, inc_zone.height);
            let child_limits =
                layout::Limits::new(zone_size, zone_size);
            let mut node = self.children[inc].as_widget_mut().layout(
                &mut tree.children[inc],
                renderer,
                &child_limits,
            );
            node.move_to_mut(Point::new(inc_zone.x, inc_zone.y));
            nodes.push(node);
        }

        layout::Node::with_children(size, nodes)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            self.children
                .iter_mut()
                .zip(&mut tree.children)
                .zip(layout.children())
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
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let hovered = cursor.is_over(bounds);
        let canonical = canonical_of(self.value);
        let text_size =
            self.text_size.unwrap_or_else(|| renderer.default_size());

        // External value change wins over any in-progress edit.
        {
            let state = tree.state.downcast_mut::<PillState>();
            if state.last_canonical != canonical {
                state.last_canonical = canonical.clone();
                state.buffer = None;
                self.rebuild_input(&canonical, text_size);
                shell.invalidate_layout();
            }
        }

        match event {
            Event::Keyboard(keyboard::Event::ModifiersChanged(m)) => {
                let state = tree.state.downcast_mut::<PillState>();
                state.modifiers = *m;
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let state = tree.state.downcast_mut::<PillState>();
                if state.hovered != hovered {
                    state.hovered = hovered;
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if hovered {
                    let y = match delta {
                        mouse::ScrollDelta::Lines { y, .. } => *y,
                        mouse::ScrollDelta::Pixels { y, .. } => *y,
                    };
                    if y != 0.0 {
                        let large = tree
                            .state
                            .downcast_ref::<PillState>()
                            .modifiers
                            .shift();
                        let next = self.stepped(y < 0.0, large);
                        if next != self.value {
                            shell.publish((self.on_change)(next));
                        }
                        // Optimistic display until the parent round-trips.
                        {
                            let state =
                                tree.state.downcast_mut::<PillState>();
                            let text = next.to_string();
                            state.buffer = None;
                            state.last_canonical = text.clone();
                            self.rebuild_input(&text, text_size);
                        }
                        shell.capture_event();
                        shell.invalidate_layout();
                        shell.request_redraw();
                        return;
                    }
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modifiers,
                text: key_text,
                ..
            }) => {
                let focused = self.is_input_focused(tree);

                // Up/Down stepping works when hovered or focused.
                let arrow = match key.as_ref() {
                    keyboard::Key::Named(
                        keyboard::key::Named::ArrowUp,
                    ) => Some(false),
                    keyboard::Key::Named(
                        keyboard::key::Named::ArrowDown,
                    ) => Some(true),
                    _ => None,
                };
                if let Some(down) = arrow {
                    if focused || hovered {
                        let large = modifiers.shift();
                        let next = self.stepped(down, large);
                        if next != self.value {
                            shell.publish((self.on_change)(next));
                        }
                        {
                            let state =
                                tree.state.downcast_mut::<PillState>();
                            let text = next.to_string();
                            state.buffer = None;
                            state.last_canonical = text.clone();
                            self.rebuild_input(&text, text_size);
                        }
                        shell.capture_event();
                        shell.invalidate_layout();
                        shell.request_redraw();
                        return;
                    }
                }

                if focused {
                    // Live filter: invalid characters never reach the
                    // inner field (no flash, cursor undisturbed).
                    if !modifiers.command() {
                        if let Some(pressed) = key_text {
                            if let Some(c) = pressed
                                .chars()
                                .next()
                                .filter(|c| !c.is_control())
                            {
                                if !is_allowed_char::<T>(c) {
                                    shell.capture_event();
                                    return;
                                }
                            }
                        }
                    }

                    // Revert unfinished edits on Escape / Enter.
                    if matches!(
                        key.as_ref(),
                        keyboard::Key::Named(
                            keyboard::key::Named::Escape,
                        ) | keyboard::Key::Named(
                            keyboard::key::Named::Enter,
                        )
                    ) && self.revert_if_dirty(tree, text_size)
                    {
                        shell.capture_event();
                        shell.invalidate_layout();
                        shell.request_redraw();
                    }

                    // Note: clicks outside are forwarded so the field
                    // unfocuses itself; the buffer is reverted below
                    // once focus is gone.
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if !hovered && self.revert_if_dirty(tree, text_size) {
                    shell.invalidate_layout();
                    shell.request_redraw();
                }
            }
            _ => {}
        }

        // Forward to `-`, `[prefix]`, field, `+` with their own layouts.
        let child_layouts: Vec<Layout<'_>> =
            layout.children().collect();
        for ((child, state), child_layout) in self
            .children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(child_layouts.iter())
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

        // Drain what the inner field reported and decide what stays
        // visible: intermediate text is buffered, valid text is
        // committed (optimistically), invalid text reverts.
        let stashed = { self.stash.borrow_mut().take() };
        if let Some(raw) = stashed {
            if self.is_input_focused(tree) {
                if is_intermediate_text::<T>(&raw) {
                    let state = tree.state.downcast_mut::<PillState>();
                    state.buffer = Some(raw.clone());
                    self.rebuild_input(&raw, text_size);
                    shell.invalidate_layout();
                } else if let Ok(v) = raw.parse::<T>() {
                    let clamped = clamp_in(&self.range, v);
                    let text = clamped.to_string();
                    if clamped != self.value {
                        shell.publish((self.on_change)(clamped));
                    }
                    let state = tree.state.downcast_mut::<PillState>();
                    state.buffer = None;
                    state.last_canonical = text.clone();
                    self.rebuild_input(&text, text_size);
                    shell.invalidate_layout();
                } else {
                    let state = tree.state.downcast_mut::<PillState>();
                    state.buffer = None;
                    self.rebuild_input(&canonical, text_size);
                    shell.invalidate_layout();
                }
                shell.request_redraw();
            }
        }

        // Focus left (click outside, Escape) with a stale buffer:
        // fall back to the canonical value.
        if !self.is_input_focused(tree) {
            let revert = {
                let state = tree.state.downcast_ref::<PillState>();
                state.buffer.is_some()
            };
            if revert {
                self.revert_if_dirty(tree, text_size);
                shell.invalidate_layout();
                shell.request_redraw();
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
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
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let status = if self.is_input_focused(tree) {
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

        // Under-pass: pill background + shadow. No border here: the
        // `-` / `+` caps cover the pill edges edge-to-edge, so a border
        // drawn underneath would be hidden behind them.
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

        // Over-pass: crisp outer ring tracing the pill over the flush
        // caps, plus the `|` separators. Only two lines: at the end of
        // the `-` zone and at the start of the `+` zone. The middle
        // `[prefix]input` zone intentionally has no divider inside.
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: appearance.border,
                ..renderer::Quad::default()
            },
            Background::Color(Color::TRANSPARENT),
        );

        let separator = appearance.border.color.scale_alpha(0.35);
        let text_size =
            self.text_size.unwrap_or_else(|| renderer.default_size());
        let button_w = button_width_for(
            self.button_width,
            self.input_padding,
            text_size,
        );
        let (dec_zone, middle_zone, _) = split_zones(bounds, button_w);
        for x in [
            dec_zone.x + dec_zone.width,
            middle_zone.x + middle_zone.width,
        ] {
            let top = bounds.y + 6.0;
            let bottom = bounds.y + bounds.height - 6.0;
            if bottom > top {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle::new(
                            Point::new(x - 0.5, top),
                            Size::new(1.0, bottom - top),
                        ),
                        border: Border::default(),
                        ..renderer::Quad::default()
                    },
                    separator,
                );
            }
        }
    }
}

impl<'a, T, Message, Theme, Renderer>
    From<NumberInput<'a, T, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    T: Copy
        + PartialOrd
        + PartialEq
        + std::fmt::Display
        + FromStr
        + Add<Output = T>
        + Sub<Output = T>
        + From<u8>
        + num_traits::FromPrimitive
        + Into<f64>
        + 'a,
    Message: Clone + 'a,
    Theme: Catalog
        + button::Catalog
        + text_input::Catalog
        + iced::widget::text::Catalog
        + 'a,
    Renderer: renderer::Renderer + text::Renderer + 'a,
    <Theme as button::Catalog>::Class<'a>:
        From<button::StyleFn<'a, Theme>>,
    <Theme as text_input::Catalog>::Class<'a>:
        From<text_input::StyleFn<'a, Theme>>,
{
    fn from(input: NumberInput<'a, T, Message, Theme, Renderer>) -> Self {
        Element::new(input)
    }
}
