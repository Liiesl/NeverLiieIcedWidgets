//! Text input with an animated ghost trail cursor effect.
//!
//! This module provides [`GhostTrailTextInput`], a fully custom text input
//! widget (not a wrapper around `iced::widget::text_input`) whose cursor
//! smoothly slides between positions with a fading gradient trail instead of
//! blinking in place.
//!
//! # Overview
//!
//! - [`GhostTrailTextInput`] — the text input widget
//! - [`Value`] — Unicode-aware text storage with grapheme support
//! - [`Cursor`] — cursor position and selection tracking
//! - [`Status`] — widget state used by the style function
//! - [`Style`] — visual styling: background, border, colors
//!
//! # Example
//!
//! ```no_run
//! use iced::Element;
//! use neverliie_iced_widgets::ghost_text_input::GhostTrailTextInput;
//!
//! #[derive(Clone)]
//! enum Message {
//!     InputChanged(String),
//! }
//!
//! fn view() -> Element<'_, Message> {
//!     GhostTrailTextInput::new("Type something...", "")
//!         .on_input(Message::InputChanged)
//!         .into()
//! }
//! ```
//!
//! # Features
//!
//! - **Animated cursor trail**: when the cursor moves, it smoothly slides from
//!   the old position to the new one, leaving a fading gradient trail
//! - **Blinking cursor**: standard 500ms blink interval when focused and idle
//! - **Secure mode**: optionally masks input with dot characters for passwords
//! - **Icon support**: display an optional icon on the left or right side
//! - **Full keyboard shortcuts**: Ctrl/Cmd+C/X/V/A, Home/End, arrow keys with
//!   Shift and Ctrl/Alt
//! - **IME support**: handles preedit and commit events for international
//!   keyboard input
//! - **Unicode support**: full grapheme-aware text editing via
//!   `unicode-segmentation`
//!
//! [`GhostTrailTextInput`]: struct.GhostTrailTextInput
//! [`Value`]: struct.Value
//! [`Cursor`]: struct.Cursor
//! [`Status`]: enum.Status
//! [`Style`]: struct.Style

const CURSOR_DURATION_MS: u64 = 150;
const CURSOR_SNAP_THRESHOLD: f32 = 0.5;
const GRADIENT_TAIL_RATIO: f32 = 0.9;

/// Easing curve used by the cursor ghost animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EasingType {
    /// Linear interpolation with no acceleration.
    Linear,
    /// Eases in — starts slow and accelerates.
    EaseIn,
    /// Eases out — starts fast and decelerates. The default.
    EaseOut,
    /// Eases both in and out — slow start and slow end.
    EaseInOut,
}

const CURSOR_EASING: EasingType = EasingType::EaseOut;

fn cubic_bezier(p1x: f32, p1y: f32, p2x: f32, p2y: f32, time: f32) -> f32 {
    let mut t = time;
    for _ in 0..8 {
        let x = bezier_x(p1x, p2x, t) - time;
        let dx = bezier_dx(p1x, p2x, t);
        if dx.abs() < 0.0001 {
            break;
        }
        t = (t - x / dx).clamp(0.0, 1.0);
    }
    bezier_y(p1y, p2y, t)
}

fn bezier_x(p1x: f32, p2x: f32, t: f32) -> f32 {
    let o = 1.0 - t;
    3.0 * o * o * t * p1x + 3.0 * o * t * t * p2x + t * t * t
}

fn bezier_dx(p1x: f32, p2x: f32, t: f32) -> f32 {
    let o = 1.0 - t;
    3.0 * o * (1.0 - 3.0 * t) * p1x + 3.0 * t * (2.0 - 3.0 * t) * p2x + 3.0 * t * t
}

fn bezier_y(p1y: f32, p2y: f32, t: f32) -> f32 {
    let o = 1.0 - t;
    3.0 * o * o * t * p1y + 3.0 * o * t * t * p2y + t * t * t
}

mod editor;
mod value;

pub mod cursor;

pub use cursor::Cursor;
pub use value::Value;

use editor::Editor;

use iced::alignment;
use iced::advanced::clipboard::{self, Clipboard};
use iced::advanced::input_method;
use iced::gradient::Linear;
use iced::keyboard;
use iced::keyboard::key;
use iced::advanced::layout;
use iced::advanced::mouse::{self, click};
use iced::advanced::renderer;
use iced::advanced::text::paragraph::{self, Paragraph as _};
use iced::advanced::text::{self, Text};
use iced::touch;
use iced::advanced::widget;
use iced::advanced::widget::operation::{self, Operation};
use iced::advanced::widget::tree::{self, Tag, Tree};
use iced::window;
use iced::advanced::{InputMethod, Shell, Widget};
use iced::{
    Alignment, Background, Border, Color, Element, Event, Length,
    Padding, Pixels, Point, Radians, Rectangle, Size, Theme, Vector,
};
use iced::advanced::Layout;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct AnimatedCursorState {
    current_x: f32,
    target_x: f32,
    target_width: f32,
    start_x: f32,
    start_time: Option<Instant>,
    is_animating: bool,
    last_cursor_index: usize,
    last_text: String,
}

impl Default for AnimatedCursorState {
    fn default() -> Self {
        Self {
            current_x: 0.0,
            target_x: 0.0,
            target_width: 2.0,
            start_x: 0.0,
            start_time: None,
            is_animating: false,
            last_cursor_index: 0,
            last_text: String::new(),
        }
    }
}

impl AnimatedCursorState {
    fn update_physics(&mut self, target_x: f32, target_width: f32, now: Instant, duration: Duration, easing: EasingType) {
        let target_changed =
            (target_x - self.target_x).abs() > f32::EPSILON
                || (target_width - self.target_width).abs() > f32::EPSILON;

        if target_changed {
            self.start_x = self.current_x;
            self.start_time = Some(now);
            self.target_x = target_x;
            self.target_width = target_width;
        }

        if let Some(start_time) = self.start_time {
            let elapsed = now.duration_since(start_time);

            if elapsed >= duration {
                self.current_x = self.target_x;
                self.is_animating = false;
                self.start_time = None;
            } else {
                let t = elapsed.as_secs_f32() / duration.as_secs_f32();
                let eased = match easing {
                    EasingType::Linear => cubic_bezier(0.0, 0.0, 1.0, 1.0, t),
                    EasingType::EaseIn => cubic_bezier(0.42, 0.0, 1.0, 1.0, t),
                    EasingType::EaseOut => cubic_bezier(0.0, 0.0, 0.58, 1.0, t),
                    EasingType::EaseInOut => cubic_bezier(0.42, 0.0, 0.58, 1.0, t),
                };
                self.current_x =
                    self.start_x + (self.target_x - self.start_x) * eased;
                self.is_animating = true;
            }
        } else {
            self.current_x = target_x;
            self.is_animating = false;
        }
    }
}

#[derive(Debug, Clone)]
struct CombinedState<P: text::Paragraph> {
    text_input: State<P>,
    animated_cursor: AnimatedCursorState,
}

impl<P: text::Paragraph> Default for CombinedState<P> {
    fn default() -> Self {
        Self {
            text_input: State::default(),
            animated_cursor: AnimatedCursorState::default(),
        }
    }
}

fn combined_state<Renderer: text::Renderer>(
    tree: &mut Tree,
) -> &mut CombinedState<Renderer::Paragraph> {
    tree.state.downcast_mut::<CombinedState<Renderer::Paragraph>>()
}

fn state<Renderer: text::Renderer>(
    tree: &mut Tree,
) -> &mut State<Renderer::Paragraph> {
    &mut combined_state::<Renderer>(tree).text_input
}

fn animated_cursor_state<Renderer: text::Renderer>(
    tree: &mut Tree,
) -> &mut AnimatedCursorState {
    &mut combined_state::<Renderer>(tree).animated_cursor
}

/// A text input field with an animated "ghost trail" cursor.
///
/// Behaves like `iced::widget::text_input`, but when the cursor moves it
/// slides from the old position to the new one, leaving a fading gradient
/// trail. Supports secure (password) mode, icons, IME input, and full
/// keyboard shortcuts.
pub struct GhostTrailTextInput<
    'a,
    Message,
    Theme = iced::Theme,
    Renderer = iced::Renderer,
> where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    id: Option<widget::Id>,
    placeholder: String,
    value: Value,
    is_secure: bool,
    font: Option<Renderer::Font>,
    width: Length,
    padding: Padding,
    size: Option<Pixels>,
    line_height: text::LineHeight,
    alignment: alignment::Horizontal,
    on_input: Option<Box<dyn Fn(String) -> Message + 'a>>,
    on_paste: Option<Box<dyn Fn(String) -> Message + 'a>>,
    on_submit: Option<Message>,
    icon: Option<Icon<Renderer::Font>>,
    class: Theme::Class<'a>,
    last_status: Option<Status>,
    cursor_color: Color,
    text_color: Color,
    placeholder_color: Color,
    cursor_width: f32,
    ghost_duration: Duration,
    ghost_easing: EasingType,
    colored_spans: Vec<(std::ops::Range<usize>, Color)>,
}

/// The default [`Padding`] of a [`GhostTrailTextInput`].
pub const DEFAULT_PADDING: Padding = Padding::new(5.0);

impl<'a, Message, Theme, Renderer> GhostTrailTextInput<'a, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: Catalog,
    Renderer: text::Renderer,
{
    /// Creates a new [`GhostTrailTextInput`] with the given placeholder and value.
    pub fn new(placeholder: &str, value: &str) -> Self {
        GhostTrailTextInput {
            id: None,
            placeholder: String::from(placeholder),
            value: Value::new(value),
            is_secure: false,
            font: None,
            width: Length::Fill,
            padding: DEFAULT_PADDING,
            size: None,
            line_height: text::LineHeight::default(),
            alignment: alignment::Horizontal::Left,
            on_input: None,
            on_paste: None,
            on_submit: None,
            icon: None,
            class: Theme::default(),
            last_status: None,
            cursor_color: Color::from_rgb(0.4, 0.6, 0.9),
            text_color: Color::from_rgb(0.75, 0.75, 0.75),
            placeholder_color: Color::from_rgb(0.5, 0.5, 0.55),
            cursor_width: 2.0,
            ghost_duration: Duration::from_millis(CURSOR_DURATION_MS),
            ghost_easing: CURSOR_EASING,
            colored_spans: Vec::new(),
        }
    }

    /// Sets the [`widget::Id`] of this input, enabling focus and text
    /// operations via `iced::advanced::widget::operation`.
    #[must_use]
    pub fn id(mut self, id: impl Into<widget::Id>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets whether the input masks its value with dot characters
    /// (password mode).
    #[must_use]
    pub fn secure(mut self, is_secure: bool) -> Self {
        self.is_secure = is_secure;
        self
    }

    /// Sets the message handler called whenever the value changes.
    #[must_use]
    pub fn on_input(
        mut self,
        on_input: impl Fn(String) -> Message + 'a,
    ) -> Self {
        self.on_input = Some(Box::new(on_input));
        self
    }

    /// Sets the optional message handler called whenever the value changes.
    #[must_use]
    pub fn on_input_maybe(
        mut self,
        on_input: Option<impl Fn(String) -> Message + 'a>,
    ) -> Self {
        self.on_input = on_input.map(|f| Box::new(f) as _);
        self
    }

    /// Sets the message to publish when the Enter key is pressed.
    #[must_use]
    pub fn on_submit(mut self, message: Message) -> Self {
        self.on_submit = Some(message);
        self
    }

    /// Sets the optional message to publish when the Enter key is pressed.
    #[must_use]
    pub fn on_submit_maybe(mut self, on_submit: Option<Message>) -> Self {
        self.on_submit = on_submit;
        self
    }

    /// Sets the message handler called whenever content is pasted.
    #[must_use]
    pub fn on_paste(
        mut self,
        on_paste: impl Fn(String) -> Message + 'a,
    ) -> Self {
        self.on_paste = Some(Box::new(on_paste));
        self
    }

    /// Sets the optional message handler called whenever content is pasted.
    #[must_use]
    pub fn on_paste_maybe(
        mut self,
        on_paste: Option<impl Fn(String) -> Message + 'a>,
    ) -> Self {
        self.on_paste = on_paste.map(|f| Box::new(f) as _);
        self
    }

    /// Sets the font used to render the input text.
    #[must_use]
    pub fn font(mut self, font: Renderer::Font) -> Self {
        self.font = Some(font);
        self
    }

    /// Displays an [`Icon`] on the left or right side of the input.
    #[must_use]
    pub fn icon(mut self, icon: Icon<Renderer::Font>) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Sets the width of the input.
    #[must_use]
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the padding of the input.
    #[must_use]
    pub fn padding<P: Into<Padding>>(mut self, padding: P) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the text size of the input.
    #[must_use]
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = Some(size.into());
        self
    }

    /// Sets the line height of the input.
    #[must_use]
    pub fn line_height(
        mut self,
        line_height: impl Into<text::LineHeight>,
    ) -> Self {
        self.line_height = line_height.into();
        self
    }

    /// Sets the horizontal alignment of the input text.
    #[must_use]
    pub fn align_x(
        mut self,
        alignment: impl Into<alignment::Horizontal>,
    ) -> Self {
        self.alignment = alignment.into();
        self
    }

    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    /// Overrides the color of the cursor (and its ghost trail).
    #[must_use]
    pub fn cursor_color(mut self, color: Color) -> Self {
        self.cursor_color = color;
        self
    }

    /// Sets the width of the cursor in pixels.
    #[must_use]
    pub fn cursor_width(mut self, width: f32) -> Self {
        self.cursor_width = width;
        self
    }

    /// Overrides the color of the input text.
    #[must_use]
    pub fn text_color(mut self, color: Color) -> Self {
        self.text_color = color;
        self
    }

    /// Overrides the color of the placeholder text.
    #[must_use]
    pub fn placeholder_color(mut self, color: Color) -> Self {
        self.placeholder_color = color;
        self
    }

    /// Sets the duration of the cursor ghost animation.
    #[must_use]
    pub fn ghost_duration(mut self, duration: Duration) -> Self {
        self.ghost_duration = duration;
        self
    }

    /// Sets the easing curve used by the cursor ghost animation.
    #[must_use]
    pub fn ghost_easing(mut self, easing: EasingType) -> Self {
        self.ghost_easing = easing;
        self
    }

    /// Computes the layout of the input, laying out the value, placeholder,
    /// and icon text.
    ///
    /// This is an internal helper shared by the widget's `layout` and
    /// `draw` passes; use the widget itself in regular application code.
    pub fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
        value: Option<&Value>,
    ) -> layout::Node {
        let state = &mut combined_state::<Renderer>(tree).text_input;
        let value = value.unwrap_or(&self.value);

        let font = self.font.unwrap_or_else(|| renderer.default_font());
        let text_size = self.size.unwrap_or_else(|| renderer.default_size());
        let padding = self.padding.fit(Size::ZERO, limits.max());
        let height = self.line_height.to_absolute(text_size);

        let limits = limits.width(self.width).shrink(padding);
        let text_bounds = limits.resolve(self.width, height, Size::ZERO);

        let placeholder_text = Text {
            font,
            line_height: self.line_height,
            content: self.placeholder.as_str(),
            bounds: Size::new(f32::INFINITY, text_bounds.height),
            size: text_size,
            align_x: text::Alignment::Default,
            align_y: alignment::Vertical::Center,
            shaping: text::Shaping::Advanced,
            wrapping: text::Wrapping::default(),
        };

        let _ = state.placeholder.update(placeholder_text);

        let secure_value = self.is_secure.then(|| value.secure());
        let value = secure_value.as_ref().unwrap_or(value);

        let _ = state.value.update(Text {
            content: &value.to_string(),
            ..placeholder_text
        });

        if let Some(icon) = &self.icon {
            let mut content = [0; 4];

            let icon_text = Text {
                line_height: self.line_height,
                content: icon.code_point.encode_utf8(&mut content) as &_,
                font: icon.font,
                size: icon.size.unwrap_or_else(|| renderer.default_size()),
                bounds: Size::new(f32::INFINITY, text_bounds.height),
                align_x: text::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text::Shaping::Advanced,
                wrapping: text::Wrapping::default(),
            };

            let _ = state.icon.update(icon_text);

            let icon_width = state.icon.min_width();

            let (text_position, icon_position) = match icon.side {
                Side::Left => (
                    Point::new(
                        padding.left + icon_width + icon.spacing,
                        padding.top,
                    ),
                    Point::new(padding.left, padding.top),
                ),
                Side::Right => (
                    Point::new(padding.left, padding.top),
                    Point::new(
                        padding.left + text_bounds.width - icon_width,
                        padding.top,
                    ),
                ),
            };

            let text_node = layout::Node::new(
                text_bounds - Size::new(icon_width + icon.spacing, 0.0),
            )
            .move_to(text_position);

            let icon_node =
                layout::Node::new(Size::new(icon_width, text_bounds.height))
                    .move_to(icon_position);

            layout::Node::with_children(
                text_bounds.expand(padding),
                vec![text_node, icon_node],
            )
        } else {
            let text = layout::Node::new(text_bounds)
                .move_to(Point::new(padding.left, padding.top));

            layout::Node::with_children(text_bounds.expand(padding), vec![text])
        }
    }

    fn input_method<'b>(
        &self,
        state: &'b State<Renderer::Paragraph>,
        layout: Layout<'_>,
        value: &Value,
    ) -> InputMethod<&'b str> {
        let Some(Focus {
            is_window_focused: true,
            ..
        }) = &state.is_focused
        else {
            return InputMethod::Disabled;
        };

        let secure_value = self.is_secure.then(|| value.secure());
        let value = secure_value.as_ref().unwrap_or(value);

        let text_bounds = layout.children().next().unwrap().bounds();

        let caret_index = match state.cursor.state(value) {
            cursor::State::Index(position) => position,
            cursor::State::Selection { start, end } => start.min(end),
        };

        let text = state.value.raw();
        let (cursor_x, scroll_offset) =
            measure_cursor_and_scroll_offset(text, text_bounds, caret_index);

        let alignment_offset = alignment_offset(
            text_bounds.width,
            text.min_width(),
            self.alignment,
        );

        let x = (text_bounds.x + cursor_x).floor() - scroll_offset
            + alignment_offset;

        InputMethod::Enabled {
            cursor: Rectangle::new(
                Point::new(x, text_bounds.y),
                Size::new(1.0, text_bounds.height),
            ),
            purpose: if self.is_secure {
                input_method::Purpose::Secure
            } else {
                input_method::Purpose::Normal
            },
            preedit: state.preedit.as_ref().map(input_method::Preedit::as_ref),
        }
    }

    fn make_paragraph(
        &self,
        renderer: &Renderer,
        content: &str,
        height: f32,
    ) -> paragraph::Plain<Renderer::Paragraph> {
        let font = self.font.unwrap_or_else(|| renderer.default_font());
        let text_size = self.size.unwrap_or_else(|| renderer.default_size());
        paragraph::Plain::new(Text {
            font,
            line_height: self.line_height,
            content: content.to_string(),
            bounds: Size::new(f32::INFINITY, height),
            size: text_size,
            align_x: text::Alignment::Default,
            align_y: alignment::Vertical::Center,
            shaping: text::Shaping::Advanced,
            wrapping: text::Wrapping::default(),
        })
    }

    pub fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        value: Option<&Value>,
        viewport: &Rectangle,
    ) {
        let combined = tree.state.downcast_ref::<CombinedState<Renderer::Paragraph>>();
        let state = &combined.text_input;
        let anim_cursor = &combined.animated_cursor;
        let value = value.unwrap_or(&self.value);
        let is_disabled = self.on_input.is_none();

        let secure_value = self.is_secure.then(|| value.secure());
        let value = secure_value.as_ref().unwrap_or(value);

        let mut children_layout = layout.children();
        let text_bounds = children_layout.next().unwrap().bounds();

        if self.icon.is_some() {
            let icon_layout = children_layout.next().unwrap();
            let icon = state.icon.raw();

            renderer.fill_paragraph(
                icon,
                icon_layout.bounds().anchor(
                    icon.min_bounds(),
                    Alignment::Center,
                    Alignment::Center,
                ),
                self.placeholder_color,
                *viewport,
            );
        }

        let text = value.to_string();

        let (sel_quad, offset, is_selecting) = if let Some(focus) = state
            .is_focused
            .as_ref()
            .filter(|focus| focus.is_window_focused)
        {
            match state.cursor.state(value) {
                cursor::State::Index(position) => {
                    let (_, offset) = measure_cursor_and_scroll_offset(
                        state.value.raw(),
                        text_bounds,
                        position,
                    );

                    let is_cursor_visible = !is_disabled
                        && ((focus.now - focus.updated_at).as_millis()
                            / CURSOR_BLINK_INTERVAL_MILLIS)
                            .is_multiple_of(2);

                    (
                        if is_cursor_visible { Some(renderer::Quad::default()) } else { None },
                        offset,
                        false,
                    )
                }
                cursor::State::Selection { start, end } => {
                    let left = start.min(end);
                    let right = end.max(start);

                    let (left_position, left_offset) =
                        measure_cursor_and_scroll_offset(
                            state.value.raw(),
                            text_bounds,
                            left,
                        );

                    let (right_position, right_offset) =
                        measure_cursor_and_scroll_offset(
                            state.value.raw(),
                            text_bounds,
                            right,
                        );

                    let width = right_position - left_position;

                    (
                        Some(renderer::Quad {
                            bounds: Rectangle {
                                x: left_position,
                                y: 0.0,
                                width,
                                height: text_bounds.height,
                            },
                            ..renderer::Quad::default()
                        }),
                        if end == right { right_offset } else { left_offset },
                        true,
                    )
                }
            }
        } else {
            (None, 0.0, false)
        };

        let draw = |renderer: &mut Renderer, viewport| {
            let paragraph = if text.is_empty()
                && state
                    .preedit
                    .as_ref()
                    .map(|preedit| preedit.content.is_empty())
                    .unwrap_or(true)
            {
                state.placeholder.raw()
            } else {
                state.value.raw()
            };

            let alignment_off = alignment_offset(
                text_bounds.width,
                paragraph.min_width(),
                self.alignment,
            );

            // Draw text selection
            if is_selecting {
                if let Some(mut quad) = sel_quad {
                    quad.bounds.x += text_bounds.x;
                    quad.bounds.y = text_bounds.y;
                    renderer.with_translation(
                        Vector::new(alignment_off - offset, 0.0),
                        |renderer| {
                            renderer.fill_quad(
                                quad,
                                Color {
                                    a: 0.3,
                                    ..self.cursor_color
                                },
                            );
                        },
                    );
                }
            } else if state.is_focused.as_ref().is_some_and(|f| f.is_window_focused) {
                // Smooth animated cursor logic
                let diff = anim_cursor.target_x - anim_cursor.current_x;
                let is_moving = diff.abs() > CURSOR_SNAP_THRESHOLD;

                let is_cursor_visible = sel_quad.is_some() || is_moving || anim_cursor.is_animating;

                if is_cursor_visible {
                    let (quad_x, quad_width, gradient) = if is_moving {
                        let base_color = self.cursor_color;
                        let angle = Radians(std::f32::consts::FRAC_PI_2);

                        if diff > 0.0 {
                            // Moving Right: Trail fades on Left
                            let start_x = anim_cursor.current_x;
                            let width = (anim_cursor.target_x + self.cursor_width - anim_cursor.current_x)
                                .max(self.cursor_width);
                            let grad = Linear::new(angle)
                                .add_stop(0.0, Color { a: 0.0, ..base_color })
                                .add_stop(GRADIENT_TAIL_RATIO, base_color)
                                .add_stop(1.0, base_color);
                            (start_x, width, Some(grad))
                        } else {
                            // Moving Left: Trail fades on Right
                            let start_x = anim_cursor.target_x;
                            let width = (anim_cursor.current_x + self.cursor_width - anim_cursor.target_x)
                                .max(self.cursor_width);
                            let grad = Linear::new(angle)
                                .add_stop(0.0, base_color)
                                .add_stop(1.0 - GRADIENT_TAIL_RATIO, base_color)
                                .add_stop(1.0, Color { a: 0.0, ..base_color });
                            (start_x, width, Some(grad))
                        }
                    } else {
                        // Static cursor: thin line cursor (2.0px)
                        (
                            anim_cursor.target_x,
                            self.cursor_width,
                            None,
                        )
                    };
                    let cursor_bounds = Rectangle {
                        x: text_bounds.x + alignment_off - offset + quad_x,
                        y: text_bounds.y,
                        width: quad_width,
                        height: text_bounds.height,
                    };

                    if let Some(grad) = gradient {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: cursor_bounds,
                                ..Default::default()
                            },
                            grad,
                        );
                    } else {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: cursor_bounds,
                                ..Default::default()
                            },
                            self.cursor_color,
                        );
                    }
                }
            }

            // Draw text
            renderer.fill_paragraph(
                paragraph,
                text_bounds.anchor(
                    paragraph.min_bounds(),
                    Alignment::Start,
                    Alignment::Center,
                ) + Vector::new(alignment_off - offset, 0.0),
                if text.is_empty() {
                    self.placeholder_color
                } else {
                    self.text_color
                },
                viewport,
            );
        };

        if is_selecting {
            renderer
                .with_layer(text_bounds, |renderer| draw(renderer, *viewport));
        } else {
            draw(renderer, text_bounds);
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for GhostTrailTextInput<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: Catalog,
    Renderer: text::Renderer,
{
    fn tag(&self) -> tree::Tag {
        Tag::of::<CombinedState<Renderer::Paragraph>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(CombinedState::<Renderer::Paragraph>::default())
    }

    fn diff(&self, tree: &mut Tree) {
        let combined = tree.state.downcast_mut::<CombinedState<Renderer::Paragraph>>();
        let state = &mut combined.text_input;
        if self.on_input.is_none() {
            state.is_pasting = None;
        }
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
        self.layout(tree, renderer, limits, None)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = &mut combined_state::<Renderer>(tree).text_input;

        operation.text_input(self.id.as_ref(), layout.bounds(), state);
        operation.focusable(self.id.as_ref(), layout.bounds(), state);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let update_cache = |state, value| {
            replace_paragraph(
                renderer,
                state,
                layout,
                value,
                self.font,
                self.size,
                self.line_height,
            );
        };

        match &event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                let state = state::<Renderer>(tree);
                let cursor_before = state.cursor;

                let click_position = cursor.position_over(layout.bounds());

                state.is_focused = if click_position.is_some() {
                    let now = Instant::now();

                    Some(Focus {
                        updated_at: now,
                        now,
                        is_window_focused: true,
                    })
                } else {
                    None
                };

                if let Some(cursor_position) = click_position {
                    let text_layout = layout.children().next().unwrap();

                    let target = {
                        let text_bounds = text_layout.bounds();

                        let alignment_offset = alignment_offset(
                            text_bounds.width,
                            state.value.raw().min_width(),
                            self.alignment,
                        );

                        cursor_position.x - text_bounds.x - alignment_offset
                    };

                    let click = mouse::Click::new(
                        cursor_position,
                        mouse::Button::Left,
                        state.last_click,
                    );

                    match click.kind() {
                        click::Kind::Single => {
                            let position = if target > 0.0 {
                                let value = if self.is_secure {
                                    self.value.secure()
                                } else {
                                    self.value.clone()
                                };

                                find_cursor_position(
                                    text_layout.bounds(),
                                    &value,
                                    state,
                                    target,
                                )
                            } else {
                                None
                            }
                            .unwrap_or(0);

                            if state.keyboard_modifiers.shift() {
                                state.cursor.select_range(
                                    state.cursor.start(&self.value),
                                    position,
                                );
                            } else {
                                state.cursor.move_to(position);
                            }

                            state.is_dragging = Some(Drag::Select);
                        }
                        click::Kind::Double => {
                            if self.is_secure {
                                state.cursor.select_all(&self.value);

                                state.is_dragging = None;
                            } else {
                                let position = find_cursor_position(
                                    text_layout.bounds(),
                                    &self.value,
                                    state,
                                    target,
                                )
                                .unwrap_or(0);

                                state.cursor.select_range(
                                    self.value.previous_start_of_word(position),
                                    self.value.next_end_of_word(position),
                                );

                                state.is_dragging = Some(Drag::SelectWords {
                                    anchor: position,
                                });
                            }
                        }
                        click::Kind::Triple => {
                            state.cursor.select_all(&self.value);
                            state.is_dragging = None;
                        }
                    }

                    state.last_click = Some(click);

                    if cursor_before != state.cursor {
                        shell.request_redraw();
                    }

                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. })
            | Event::Touch(touch::Event::FingerLost { .. }) => {
                state::<Renderer>(tree).is_dragging = None;
            }
            Event::Mouse(mouse::Event::CursorMoved { position })
            | Event::Touch(touch::Event::FingerMoved { position, .. }) => {
                let state = state::<Renderer>(tree);

                if let Some(is_dragging) = &state.is_dragging {
                    let text_layout = layout.children().next().unwrap();

                    let target = {
                        let text_bounds = text_layout.bounds();

                        let alignment_offset = alignment_offset(
                            text_bounds.width,
                            state.value.raw().min_width(),
                            self.alignment,
                        );

                        position.x - text_bounds.x - alignment_offset
                    };

                    let value = if self.is_secure {
                        self.value.secure()
                    } else {
                        self.value.clone()
                    };

                    let position = find_cursor_position(
                        text_layout.bounds(),
                        &value,
                        state,
                        target,
                    )
                    .unwrap_or(0);

                    let selection_before = state.cursor.selection(&value);

                    match is_dragging {
                        Drag::Select => {
                            state.cursor.select_range(
                                state.cursor.start(&value),
                                position,
                            );
                        }
                        Drag::SelectWords { anchor } => {
                            if position < *anchor {
                                state.cursor.select_range(
                                    self.value.previous_start_of_word(position),
                                    self.value.next_end_of_word(*anchor),
                                );
                            } else {
                                state.cursor.select_range(
                                    self.value.previous_start_of_word(*anchor),
                                    self.value.next_end_of_word(position),
                                );
                            }
                        }
                    }

                    if let Some(focus) = &mut state.is_focused {
                        focus.updated_at = Instant::now();
                    }

                    if selection_before != state.cursor.selection(&value) {
                        shell.request_redraw();
                    }

                    shell.capture_event();
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                text,
                modified_key,
                physical_key,
                ..
            }) => {
                let state = state::<Renderer>(tree);

                if let Some(focus) = &mut state.is_focused {
                    let modifiers = state.keyboard_modifiers;

                    match key.to_latin(*physical_key) {
                        Some('c')
                            if state.keyboard_modifiers.command()
                                && !self.is_secure =>
                        {
                            if let Some((start, end)) =
                                state.cursor.selection(&self.value)
                            {
                                clipboard.write(
                                    clipboard::Kind::Standard,
                                    self.value.select(start, end).to_string(),
                                );
                            }

                            shell.capture_event();
                            return;
                        }
                        Some('x')
                            if state.keyboard_modifiers.command()
                                && !self.is_secure =>
                        {
                            let Some(on_input) = &self.on_input else {
                                return;
                            };

                            if let Some((start, end)) =
                                state.cursor.selection(&self.value)
                            {
                                clipboard.write(
                                    clipboard::Kind::Standard,
                                    self.value.select(start, end).to_string(),
                                );
                            }

                            let mut editor =
                                Editor::new(&mut self.value, &mut state.cursor);
                            editor.delete();

                            let message = (on_input)(editor.contents());
                            shell.publish(message);
                            shell.capture_event();

                            focus.updated_at = Instant::now();
                            update_cache(state, &self.value);
                            return;
                        }
                        Some('v')
                            if state.keyboard_modifiers.command()
                                && !state.keyboard_modifiers.alt() =>
                        {
                            let Some(on_input) = &self.on_input else {
                                return;
                            };

                            let content = match state.is_pasting.take() {
                                Some(content) => content,
                                None => {
                                    let content: String = clipboard
                                        .read(clipboard::Kind::Standard)
                                        .unwrap_or_default()
                                        .chars()
                                        .filter(|c| !c.is_control())
                                        .collect();

                                    Value::new(&content)
                                }
                            };

                            let mut editor =
                                Editor::new(&mut self.value, &mut state.cursor);
                            editor.paste(content.clone());

                            let message = if let Some(paste) = &self.on_paste {
                                (paste)(editor.contents())
                            } else {
                                (on_input)(editor.contents())
                            };
                            shell.publish(message);
                            shell.capture_event();

                            state.is_pasting = Some(content);
                            focus.updated_at = Instant::now();
                            update_cache(state, &self.value);
                            return;
                        }
                        Some('a') if state.keyboard_modifiers.command() => {
                            let cursor_before = state.cursor;

                            state.cursor.select_all(&self.value);

                            if cursor_before != state.cursor {
                                focus.updated_at = Instant::now();

                                shell.request_redraw();
                            }

                            shell.capture_event();
                            return;
                        }
                        _ => {}
                    }

                    if let Some(text) = text {
                        let Some(on_input) = &self.on_input else {
                            return;
                        };

                        state.is_pasting = None;

                        if let Some(c) =
                            text.chars().next().filter(|c| !c.is_control())
                        {
                            let mut editor =
                                Editor::new(&mut self.value, &mut state.cursor);

                            editor.insert(c);

                            let message = (on_input)(editor.contents());
                            shell.publish(message);
                            shell.capture_event();

                            focus.updated_at = Instant::now();
                            update_cache(state, &self.value);
                            return;
                        }
                    }

                    #[cfg(target_os = "macos")]
                    let macos_shortcut =
                        crate::text_editor::convert_macos_shortcut(
                            key, modifiers,
                        );

                    #[cfg(target_os = "macos")]
                    let modified_key =
                        macos_shortcut.as_ref().unwrap_or(modified_key);

                    match modified_key.as_ref() {
                        keyboard::Key::Named(key::Named::Enter) => {
                            if let Some(on_submit) = self.on_submit.clone() {
                                shell.publish(on_submit);
                                shell.capture_event();
                            }
                        }
                        keyboard::Key::Named(key::Named::Backspace) => {
                            let Some(on_input) = &self.on_input else {
                                return;
                            };

                            if state.cursor.selection(&self.value).is_none() {
                                if (self.is_secure && modifiers.jump())
                                    || modifiers.macos_command()
                                {
                                    state.cursor.select_range(
                                        state.cursor.start(&self.value),
                                        0,
                                    );
                                } else if modifiers.jump() {
                                    state
                                        .cursor
                                        .select_left_by_words(&self.value);
                                }
                            }

                            let mut editor =
                                Editor::new(&mut self.value, &mut state.cursor);
                            editor.backspace();

                            let message = (on_input)(editor.contents());
                            shell.publish(message);
                            shell.capture_event();

                            focus.updated_at = Instant::now();
                            update_cache(state, &self.value);
                        }
                        keyboard::Key::Named(key::Named::Delete) => {
                            let Some(on_input) = &self.on_input else {
                                return;
                            };

                            if state.cursor.selection(&self.value).is_none() {
                                if (self.is_secure && modifiers.jump())
                                    || modifiers.macos_command()
                                {
                                    state.cursor.select_range(
                                        state.cursor.start(&self.value),
                                        self.value.len(),
                                    );
                                } else if modifiers.jump() {
                                    state
                                        .cursor
                                        .select_right_by_words(&self.value);
                                }
                            }

                            let mut editor =
                                Editor::new(&mut self.value, &mut state.cursor);
                            editor.delete();

                            let message = (on_input)(editor.contents());
                            shell.publish(message);
                            shell.capture_event();

                            focus.updated_at = Instant::now();
                            update_cache(state, &self.value);
                        }
                        keyboard::Key::Named(key::Named::Home) => {
                            let cursor_before = state.cursor;

                            if modifiers.shift() {
                                state.cursor.select_range(
                                    state.cursor.start(&self.value),
                                    0,
                                );
                            } else {
                                state.cursor.move_to(0);
                            }

                            if cursor_before != state.cursor {
                                focus.updated_at = Instant::now();

                                shell.request_redraw();
                            }

                            shell.capture_event();
                        }
                        keyboard::Key::Named(key::Named::End) => {
                            let cursor_before = state.cursor;

                            if modifiers.shift() {
                                state.cursor.select_range(
                                    state.cursor.start(&self.value),
                                    self.value.len(),
                                );
                            } else {
                                state.cursor.move_to(self.value.len());
                            }

                            if cursor_before != state.cursor {
                                focus.updated_at = Instant::now();

                                shell.request_redraw();
                            }

                            shell.capture_event();
                        }
                        keyboard::Key::Named(key::Named::ArrowLeft) => {
                            let cursor_before = state.cursor;

                            if (self.is_secure && modifiers.jump())
                                || modifiers.macos_command()
                            {
                                if modifiers.shift() {
                                    state.cursor.select_range(
                                        state.cursor.start(&self.value),
                                        0,
                                    );
                                } else {
                                    state.cursor.move_to(0);
                                }
                            } else if modifiers.jump() {
                                if modifiers.shift() {
                                    state
                                        .cursor
                                        .select_left_by_words(&self.value);
                                } else {
                                    state
                                        .cursor
                                        .move_left_by_words(&self.value);
                                }
                            } else if modifiers.shift() {
                                state.cursor.select_left(&self.value);
                            } else {
                                state.cursor.move_left(&self.value);
                            }

                            if cursor_before != state.cursor {
                                focus.updated_at = Instant::now();

                                shell.request_redraw();
                            }

                            shell.capture_event();
                        }
                        keyboard::Key::Named(key::Named::ArrowRight) => {
                            let cursor_before = state.cursor;

                            if (self.is_secure && modifiers.jump())
                                || modifiers.macos_command()
                            {
                                if modifiers.shift() {
                                    state.cursor.select_range(
                                        state.cursor.start(&self.value),
                                        self.value.len(),
                                    );
                                } else {
                                    state.cursor.move_to(self.value.len());
                                }
                            } else if modifiers.jump() {
                                if modifiers.shift() {
                                    state
                                        .cursor
                                        .select_right_by_words(&self.value);
                                } else {
                                    state
                                        .cursor
                                        .move_right_by_words(&self.value);
                                }
                            } else if modifiers.shift() {
                                state.cursor.select_right(&self.value);
                            } else {
                                state.cursor.move_right(&self.value);
                            }

                            if cursor_before != state.cursor {
                                focus.updated_at = Instant::now();

                                shell.request_redraw();
                            }

                            shell.capture_event();
                        }
                        keyboard::Key::Named(key::Named::Escape) => {
                            state.is_focused = None;
                            state.is_dragging = None;
                            state.is_pasting = None;

                            state.keyboard_modifiers =
                                keyboard::Modifiers::default();

                            shell.capture_event();
                        }
                        _ => {}
                    }
                }
            }
            Event::Keyboard(keyboard::Event::KeyReleased { key, .. }) => {
                let state = state::<Renderer>(tree);

                if state.is_focused.is_some()
                    && let keyboard::Key::Character("v") = key.as_ref()
                {
                    state.is_pasting = None;

                    shell.capture_event();
                }

                state.is_pasting = None;
            }
            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                let state = state::<Renderer>(tree);

                state.keyboard_modifiers = *modifiers;
            }
            Event::InputMethod(event) => match event {
                input_method::Event::Opened | input_method::Event::Closed => {
                    let state = state::<Renderer>(tree);

                    state.preedit =
                        matches!(event, input_method::Event::Opened)
                            .then(input_method::Preedit::new);

                    shell.request_redraw();
                }
                input_method::Event::Preedit(content, selection) => {
                    let state = state::<Renderer>(tree);

                    if state.is_focused.is_some() {
                        state.preedit = Some(input_method::Preedit {
                            content: content.to_owned(),
                            selection: selection.clone(),
                            text_size: self.size,
                        });

                        shell.request_redraw();
                    }
                }
                input_method::Event::Commit(text) => {
                    let state = state::<Renderer>(tree);

                    if let Some(focus) = &mut state.is_focused {
                        let Some(on_input) = &self.on_input else {
                            return;
                        };

                        let mut editor =
                            Editor::new(&mut self.value, &mut state.cursor);
                        editor.paste(Value::new(text));

                        focus.updated_at = Instant::now();
                        state.is_pasting = None;

                        let message = (on_input)(editor.contents());
                        shell.publish(message);
                        shell.capture_event();

                        update_cache(state, &self.value);
                    }
                }
            },
            Event::Window(window::Event::Unfocused) => {
                let state = state::<Renderer>(tree);

                if let Some(focus) = &mut state.is_focused {
                    focus.is_window_focused = false;
                }
            }
            Event::Window(window::Event::Focused) => {
                let state = state::<Renderer>(tree);

                if let Some(focus) = &mut state.is_focused {
                    focus.is_window_focused = true;
                    focus.updated_at = Instant::now();

                    shell.request_redraw();
                }
            }
            Event::Window(window::Event::RedrawRequested(now)) => {
                {
                    let ti_state = state::<Renderer>(tree);

                    if let Some(focus) = &mut ti_state.is_focused
                        && focus.is_window_focused
                    {
                        if matches!(
                            ti_state.cursor.state(&self.value),
                            cursor::State::Index(_)
                        ) {
                            focus.now = *now;

                            let millis_until_redraw = CURSOR_BLINK_INTERVAL_MILLIS
                                - (*now - focus.updated_at).as_millis()
                                    % CURSOR_BLINK_INTERVAL_MILLIS;

                            shell.request_redraw_at(
                                *now + Duration::from_millis(
                                    millis_until_redraw as u64,
                                ),
                            );
                        }

                        shell.request_input_method(&self.input_method(
                            ti_state,
                            layout,
                            &self.value,
                        ));
                    }
                }

                // Smooth cursor animation physics
                {
                    let text = self.value.to_string();
                    let combined = combined_state::<Renderer>(tree);

                    let cursor_idx = match combined.text_input.cursor.state(&self.value) {
                        cursor::State::Index(i) => i,
                        cursor::State::Selection { start, end } => start.min(end),
                    };

                    let text_layout = layout.children().next().unwrap();
                    let text_bounds = text_layout.bounds();

                    let text_changed = text != combined.animated_cursor.last_text
                        || cursor_idx != combined.animated_cursor.last_cursor_index;

                    let paragraph = self.make_paragraph(renderer, &text, text_bounds.height);

                    let target_x = paragraph
                        .raw()
                        .grapheme_position(0, cursor_idx)
                        .map(|p| p.x)
                        .unwrap_or(0.0);

                    let target_width = if cursor_idx < text.len() {
                        let next_x = paragraph
                            .raw()
                            .grapheme_position(0, cursor_idx + 1)
                            .map(|p| p.x)
                            .unwrap_or(target_x + self.cursor_width);
                        (next_x - target_x).max(self.cursor_width)
                    } else {
                        self.cursor_width
                    };

                    combined.animated_cursor.update_physics(target_x, target_width, *now, self.ghost_duration, self.ghost_easing);

                    if text_changed {
                        combined.animated_cursor.last_text = text;
                        combined.animated_cursor.last_cursor_index = cursor_idx;
                    }

                    if combined.animated_cursor.is_animating {
                        shell.request_redraw();
                    }
                }
            }
            _ => {}
        }

        let state = state::<Renderer>(tree);
        let is_disabled = self.on_input.is_none();

        let status = if is_disabled {
            Status::Disabled
        } else if state.is_focused() {
            Status::Focused {
                is_hovered: cursor.is_over(layout.bounds()),
            }
        } else if cursor.is_over(layout.bounds()) {
            Status::Hovered
        } else {
            Status::Active
        };

        if let Event::Window(window::Event::RedrawRequested(_now)) = event {
            self.last_status = Some(status);
        } else if self
            .last_status
            .is_some_and(|last_status| status != last_status)
        {
            shell.request_redraw();
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.draw(tree, renderer, theme, layout, cursor, None, viewport);
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            if self.on_input.is_none() {
                mouse::Interaction::Idle
            } else {
                mouse::Interaction::Text
            }
        } else {
            mouse::Interaction::default()
        }
    }
}

impl<'a, Message, Theme, Renderer> From<GhostTrailTextInput<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: text::Renderer + 'a,
{
    fn from(
        text_input: GhostTrailTextInput<'a, Message, Theme, Renderer>,
    ) -> Element<'a, Message, Theme, Renderer> {
        Element::new(text_input)
    }
}

/// An icon displayed inside the input, on the left or right side.
///
/// Create one and pass it to [`GhostTrailTextInput::icon`].
#[derive(Debug, Clone)]
pub struct Icon<Font> {
    /// The font that contains the icon glyph.
    pub font: Font,
    /// The icon glyph's code point.
    pub code_point: char,
    /// The icon size in pixels. Defaults to the input's text size when `None`.
    pub size: Option<Pixels>,
    /// The spacing between the icon and the text.
    pub spacing: f32,
    /// Which side of the input the icon is rendered on.
    pub side: Side,
}

/// Which side of the input an [`Icon`] is rendered on.
#[derive(Debug, Clone)]
pub enum Side {
    /// Left side of the input.
    Left,
    /// Right side of the input.
    Right,
}

/// The internal state of a [`GhostTrailTextInput`].
///
/// Exposed to support focus and text operations, e.g. through
/// `iced::advanced::widget::operation::focus` or
/// `operation::text_input` (see the `operation` module).
#[derive(Debug, Default, Clone)]
pub struct State<P: text::Paragraph> {
    value: paragraph::Plain<P>,
    placeholder: paragraph::Plain<P>,
    icon: paragraph::Plain<P>,
    is_focused: Option<Focus>,
    is_dragging: Option<Drag>,
    is_pasting: Option<Value>,
    preedit: Option<input_method::Preedit>,
    last_click: Option<mouse::Click>,
    cursor: Cursor,
    keyboard_modifiers: keyboard::Modifiers,
}

const CURSOR_BLINK_INTERVAL_MILLIS: u128 = 500;

#[derive(Debug, Clone)]
struct Focus {
    updated_at: Instant,
    now: Instant,
    is_window_focused: bool,
}

#[derive(Debug, Clone)]
enum Drag {
    Select,
    SelectWords { anchor: usize },
}

impl<P: text::Paragraph> State<P> {
    /// Creates a new [`State`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns whether the input is currently focused.
    pub fn is_focused(&self) -> bool {
        self.is_focused.is_some()
    }

    /// Returns the current [`Cursor`].
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Focuses the input, moving the cursor to the end of the value.
    pub fn focus(&mut self) {
        let now = Instant::now();

        self.is_focused = Some(Focus {
            updated_at: now,
            now,
            is_window_focused: true,
        });

        self.move_cursor_to_end();
    }

    /// Unfocuses the input.
    pub fn unfocus(&mut self) {
        self.is_focused = None;
    }

    /// Moves the cursor to the start of the value.
    pub fn move_cursor_to_front(&mut self) {
        self.cursor.move_to(0);
    }

    /// Moves the cursor to the end of the value.
    pub fn move_cursor_to_end(&mut self) {
        self.cursor.move_to(usize::MAX);
    }

    /// Moves the cursor to the given grapheme position.
    pub fn move_cursor_to(&mut self, position: usize) {
        self.cursor.move_to(position);
    }

    /// Selects the entire value.
    pub fn select_all(&mut self) {
        self.cursor.select_range(0, usize::MAX);
    }

    /// Selects the given grapheme range.
    pub fn select_range(&mut self, start: usize, end: usize) {
        self.cursor.select_range(start, end);
    }
}

impl<P: text::Paragraph> operation::Focusable for State<P> {
    fn is_focused(&self) -> bool {
        State::is_focused(self)
    }

    fn focus(&mut self) {
        State::focus(self);
    }

    fn unfocus(&mut self) {
        State::unfocus(self);
    }
}

impl<P: text::Paragraph> operation::TextInput for State<P> {
    fn text(&self) -> &str {
        if self.value.content().is_empty() {
            self.placeholder.content()
        } else {
            self.value.content()
        }
    }

    fn move_cursor_to_front(&mut self) {
        State::move_cursor_to_front(self);
    }

    fn move_cursor_to_end(&mut self) {
        State::move_cursor_to_end(self);
    }

    fn move_cursor_to(&mut self, position: usize) {
        State::move_cursor_to(self, position);
    }

    fn select_all(&mut self) {
        State::select_all(self);
    }

    fn select_range(&mut self, start: usize, end: usize) {
        State::select_range(self, start, end);
    }
}

fn offset<P: text::Paragraph>(
    text_bounds: Rectangle,
    value: &Value,
    state: &State<P>,
) -> f32 {
    if state.is_focused() {
        let cursor = state.cursor();

        let focus_position = match cursor.state(value) {
            cursor::State::Index(i) => i,
            cursor::State::Selection { end, .. } => end,
        };

        let (_, offset) = measure_cursor_and_scroll_offset(
            state.value.raw(),
            text_bounds,
            focus_position,
        );

        offset
    } else {
        0.0
    }
}

fn measure_cursor_and_scroll_offset(
    paragraph: &impl text::Paragraph,
    text_bounds: Rectangle,
    cursor_index: usize,
) -> (f32, f32) {
    let grapheme_position = paragraph
        .grapheme_position(0, cursor_index)
        .unwrap_or(Point::ORIGIN);

    let offset = ((grapheme_position.x + 5.0) - text_bounds.width).max(0.0);

    (grapheme_position.x, offset)
}

fn find_cursor_position<P: text::Paragraph>(
    text_bounds: Rectangle,
    value: &Value,
    state: &State<P>,
    x: f32,
) -> Option<usize> {
    let offset = offset(text_bounds, value, state);
    let value = value.to_string();

    let char_offset = state
        .value
        .raw()
        .hit_test(Point::new(x + offset, text_bounds.height / 2.0))
        .map(text::Hit::cursor)?;

    Some(
        unicode_segmentation::UnicodeSegmentation::graphemes(
            &value[..char_offset.min(value.len())],
            true,
        )
        .count(),
    )
}

fn replace_paragraph<Renderer>(
    renderer: &Renderer,
    state: &mut State<Renderer::Paragraph>,
    layout: Layout<'_>,
    value: &Value,
    font: Option<Renderer::Font>,
    text_size: Option<Pixels>,
    line_height: text::LineHeight,
) where
    Renderer: text::Renderer,
{
    let font = font.unwrap_or_else(|| renderer.default_font());
    let text_size = text_size.unwrap_or_else(|| renderer.default_size());

    let mut children_layout = layout.children();
    let text_bounds = children_layout.next().unwrap().bounds();

    state.value = paragraph::Plain::new(Text {
        font,
        line_height,
        content: value.to_string(),
        bounds: Size::new(f32::INFINITY, text_bounds.height),
        size: text_size,
        align_x: text::Alignment::Default,
        align_y: alignment::Vertical::Center,
        shaping: text::Shaping::Advanced,
        wrapping: text::Wrapping::default(),
    });
}

/// The status of a [`GhostTrailTextInput`], passed to the style function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The input is not focused and not hovered.
    Active,
    /// The mouse is over the input.
    Hovered,
    /// The input is focused, optionally hovered at the same time.
    Focused {
        /// Whether the mouse is also over the input.
        is_hovered: bool,
    },
    /// The input is disabled.
    Disabled,
}

/// The visual style of a [`GhostTrailTextInput`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The background of the input.
    pub background: Background,
    /// The border of the input.
    pub border: Border,
    /// The color of the optional icon.
    pub icon: Color,
    /// The color of the placeholder text.
    pub placeholder: Color,
    /// The color of the input text.
    pub value: Color,
    /// The color of the text selection highlight.
    pub selection: Color,
}

/// The theme catalog of a [`GhostTrailTextInput`].
///
/// Implemented for [`Theme`] by default, pulling colors from the extended
/// palette. Use [`GhostTrailTextInput::style`] for a custom style function
/// or [`GhostTrailTextInput::class`] for a theme class.
pub trait Catalog: Sized {
    /// The style class of this theme.
    type Class<'a>;

    /// Returns the default class of this theme.
    fn default<'a>() -> Self::Class<'a>;

    /// Resolves a class into a concrete [`Style`] for the given [`Status`].
    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style;
}

/// A styling function for a [`GhostTrailTextInput`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, Status) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style {
        class(self, status)
    }
}

/// Default style derived from the iced theme palette.
pub fn default(theme: &Theme, status: Status) -> Style {
    let palette = theme.extended_palette();

    let active = Style {
        background: Background::Color(palette.background.base.color),
        border: Border {
            radius: 2.0.into(),
            width: 1.0,
            color: palette.background.strong.color,
        },
        icon: palette.background.weak.text,
        placeholder: palette.secondary.base.color,
        value: palette.background.base.text,
        selection: palette.primary.weak.color,
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
        Status::Focused { .. } => Style {
            border: Border {
                color: palette.primary.strong.color,
                ..active.border
            },
            ..active
        },
        Status::Disabled => Style {
            background: Background::Color(palette.background.weak.color),
            value: active.placeholder,
            placeholder: palette.background.strongest.color,
            ..active
        },
    }
}

fn alignment_offset(
    text_bounds_width: f32,
    text_min_width: f32,
    alignment: alignment::Horizontal,
) -> f32 {
    if text_min_width > text_bounds_width {
        0.0
    } else {
        match alignment {
            alignment::Horizontal::Left => 0.0,
            alignment::Horizontal::Center => {
                (text_bounds_width - text_min_width) / 2.0
            }
            alignment::Horizontal::Right => text_bounds_width - text_min_width,
        }
    }
}