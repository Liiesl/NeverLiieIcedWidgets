//! A slider widget that displays a tooltip with the current value.
//!
//! This module provides [`SliderTooltip`], a wrapper around [`Slider`] that
//! shows a floating tooltip above (or below) the handle while the user hovers
//! or drags.
//!
//! # Overview
//!
//! - [`SliderTooltip`] — wraps a slider and adds a value tooltip
//! - [`TooltipPosition`] — controls whether the tooltip appears above or below
//!
//! # Example
//!
//! ```no_run
//! use iced::Element;
//! use neverlie_iced_widgets::slider_tooltip::{SliderTooltip, TooltipPosition};
//!
//! enum Message {
//!     ValueChanged(f64),
//! }
//!
//! fn view() -> Element<'_, Message> {
//!     SliderTooltip::new(0.0..=100.0, 50.0, Message::ValueChanged)
//!         .tooltip_position(TooltipPosition::Top)
//!         .tooltip_gap(12.0)
//!         .into()
//! }
//! ```
//!
//! [`SliderTooltip`]: struct.SliderTooltip
//! [`TooltipPosition`]: enum.TooltipPosition

use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::text::{self, Paragraph};
use iced::advanced::widget::{self, Operation, Tree};
use iced::advanced::{Clipboard, Shell};
use iced::border::Border;
use iced::mouse;
use iced::widget::container;
use iced::widget::slider::{self, Slider};
use iced::{
    Background, Color, Element, Event, Length, Pixels, Rectangle, Size,
    Vector,
};
use std::ops::RangeInclusive;
use std::rc::Rc;
use std::time::{Duration, Instant};

const DEFAULT_HANDLE_WIDTH: f32 = 14.0;

/// A slider with a tooltip that shows the current value during hover and drag.
///
/// Wraps an iced [`Slider`] and renders a floating tooltip above or below the
/// handle. The tooltip appears after a configurable delay and stays visible
/// while dragging. All of the underlying [`Slider`] builder methods are
/// forwarded for full configuration parity.
pub struct SliderTooltip<
    'a,
    T,
    Message,
    Theme = iced::Theme,
    Renderer = iced::Renderer,
> where
    Theme: slider::Catalog + container::Catalog,
{
    range: RangeInclusive<T>,
    value: T,
    on_change: Rc<dyn Fn(T) -> Message + 'a>,
    slider_element: Element<'a, Message, Theme, Renderer>,
    handle_width: f32,
    tooltip_position: TooltipPosition,
    tooltip_gap: f32,
    tooltip_delay: Duration,
    tooltip_format: Rc<dyn Fn(T) -> String + 'a>,
    tooltip_style: Option<Rc<dyn Fn(&Theme) -> container::Style + 'a>>,
    slider_width: Length,
    slider_height: f32,
    slider_step: T,
    slider_shift_step: Option<T>,
    slider_default: Option<T>,
    slider_on_release: Option<Message>,
    slider_style_fn:
        Option<Rc<dyn Fn(&Theme, slider::Status) -> slider::Style + 'a>>,
}

impl<'a, T, Message, Theme, Renderer>
    SliderTooltip<'a, T, Message, Theme, Renderer>
where
    T: Copy + From<u8> + PartialOrd + Into<f64> + num_traits::FromPrimitive + 'a,
    Message: Clone + 'a,
    Theme: slider::Catalog + container::Catalog + 'a,
    <Theme as slider::Catalog>::Class<'a>:
        From<slider::StyleFn<'a, Theme>>,
    Renderer: renderer::Renderer + text::Renderer + 'a,
{
    const DEFAULT_HEIGHT: f32 = 16.0;

    /// Creates a new [`SliderTooltip`].
    pub fn new<F>(range: RangeInclusive<T>, value: T, on_change: F) -> Self
    where
        F: Fn(T) -> Message + 'a,
    {
        let on_change: Rc<dyn Fn(T) -> Message + 'a> = Rc::new(on_change);
        let step = Self::infer_step(&range);
        let format = Self::make_format_rc(&range);
        let slider_element = Self::build_slider(
            &range,
            value,
            &on_change,
            step,
            &None,
            &None,
            Length::Fill,
            Self::DEFAULT_HEIGHT,
            &None,
            &None,
        );
        Self {
            range,
            value,
            on_change,
            slider_element,
            handle_width: DEFAULT_HANDLE_WIDTH,
            tooltip_position: TooltipPosition::Top,
            tooltip_gap: 8.0,
            tooltip_delay: Duration::ZERO,
            tooltip_format: format,
            tooltip_style: None,
            slider_width: Length::Fill,
            slider_height: Self::DEFAULT_HEIGHT,
            slider_step: step,
            slider_shift_step: None,
            slider_default: None,
            slider_on_release: None,
            slider_style_fn: None,
        }
    }

    fn infer_step(range: &RangeInclusive<T>) -> T {
        let start: f64 = (*range.start()).into();
        let end: f64 = (*range.end()).into();
        let range_len = end - start;
        if range_len < 1.0 {
            T::from_f64(0.01).unwrap_or(T::from(1u8))
        } else if range_len < 10.0 {
            T::from_f64(0.1).unwrap_or(T::from(1u8))
        } else {
            T::from(1u8)
        }
    }

    fn make_format_rc(
        range: &RangeInclusive<T>,
    ) -> Rc<dyn Fn(T) -> String + 'a> {
        let start: f64 = (*range.start()).into();
        let end: f64 = (*range.end()).into();
        let range_len = end - start;
        Rc::new(move |v: T| {
            let val: f64 = v.into();
            if range_len < 1.0 {
                format!("{:.2}", val)
            } else if range_len < 10.0 {
                format!("{:.1}", val)
            } else {
                format!("{:.0}", val)
            }
        })
    }

    fn rebuild_slider(&mut self) {
        self.slider_element = Self::build_slider(
            &self.range,
            self.value,
            &self.on_change,
            self.slider_step,
            &self.slider_shift_step,
            &self.slider_default,
            self.slider_width,
            self.slider_height,
            &self.slider_on_release,
            &self.slider_style_fn,
        );
    }

    fn build_slider(
        range: &RangeInclusive<T>,
        value: T,
        on_change: &Rc<dyn Fn(T) -> Message + 'a>,
        step: T,
        shift_step: &Option<T>,
        default: &Option<T>,
        width: Length,
        height: f32,
        on_release: &Option<Message>,
        style_fn: &Option<
            Rc<dyn Fn(&Theme, slider::Status) -> slider::Style + 'a>,
        >,
    ) -> Element<'a, Message, Theme, Renderer> {
        let oc = Rc::clone(on_change);
        let mut slider =
            Slider::new(range.clone(), value, move |v| oc(v))
                .step(step)
                .width(width)
                .height(height);

        if let Some(ss) = shift_step {
            slider = slider.shift_step(*ss);
        }
        if let Some(d) = default {
            slider = slider.default(*d);
        }
        if let Some(r) = on_release {
            slider = slider.on_release(r.clone());
        }
        if let Some(s) = style_fn {
            let s = Rc::clone(s);
            slider = slider.style(move |theme, status| s(theme, status));
        }

        Element::new(slider)
    }

    // -- Tooltip builder methods --

    /// Sets the position of the tooltip relative to the slider handle.
    #[must_use]
    pub fn tooltip_position(mut self, position: TooltipPosition) -> Self {
        self.tooltip_position = position;
        self
    }

    /// Sets the gap between the tooltip and the slider handle.
    #[must_use]
    pub fn tooltip_gap(mut self, gap: impl Into<Pixels>) -> Self {
        self.tooltip_gap = gap.into().0;
        self
    }

    /// Sets the delay before the tooltip appears on hover.
    #[must_use]
    pub fn tooltip_delay(mut self, delay: Duration) -> Self {
        self.tooltip_delay = delay;
        self
    }

    /// Sets a custom formatter for the tooltip text.
    #[must_use]
    pub fn tooltip_format(
        mut self,
        format: impl Fn(T) -> String + 'a,
    ) -> Self {
        self.tooltip_format = Rc::new(format);
        self
    }

    /// Sets a custom styling function for the tooltip container.
    #[must_use]
    pub fn tooltip_style(
        mut self,
        style: impl Fn(&Theme) -> container::Style + 'a,
    ) -> Self {
        self.tooltip_style = Some(Rc::new(style));
        self
    }

    // -- Slider forwarding methods --

    /// Sets the width of the slider handle in pixels.
    #[must_use]
    pub fn handle_width(mut self, width: f32) -> Self {
        self.handle_width = width;
        self
    }

    /// Sets the width of the slider.
    #[must_use]
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.slider_width = width.into();
        self.rebuild_slider();
        self
    }

    /// Sets the height of the slider.
    #[must_use]
    pub fn height(mut self, height: impl Into<Pixels>) -> Self {
        self.slider_height = height.into().0;
        self.rebuild_slider();
        self
    }

    /// Sets the step size of the slider.
    #[must_use]
    pub fn step(mut self, step: impl Into<T>) -> Self {
        self.slider_step = step.into();
        self.rebuild_slider();
        self
    }

    /// Sets the optional "shift" step for the slider.
    ///
    /// If set, this value is used as the step while the shift key is pressed.
    #[must_use]
    pub fn shift_step(mut self, shift_step: impl Into<T>) -> Self {
        self.slider_shift_step = Some(shift_step.into());
        self.rebuild_slider();
        self
    }

    /// Sets the optional default value for the slider.
    ///
    /// If set, the slider will reset to this value when ctrl-clicked or
    /// command-clicked.
    #[must_use]
    pub fn default_value(mut self, default: impl Into<T>) -> Self {
        self.slider_default = Some(default.into());
        self.rebuild_slider();
        self
    }

    /// Sets the release message of the slider.
    ///
    /// This is called when the mouse is released from the slider.
    /// Typically, the user's interaction with the slider is finished when this
    /// message is produced. This is useful if you need to spawn a long-running
    /// task from the slider's result, where the default on_change message
    /// could create too many events.
    #[must_use]
    pub fn on_release(mut self, on_release: Message) -> Self {
        self.slider_on_release = Some(on_release);
        self.rebuild_slider();
        self
    }

    /// Sets the style of the slider.
    #[must_use]
    pub fn style(
        mut self,
        style: impl Fn(&Theme, slider::Status) -> slider::Style + 'a,
    ) -> Self {
        self.slider_style_fn = Some(Rc::new(style));
        self.rebuild_slider();
        self
    }

    fn handle_center_x(&self, track_width: f32) -> f32 {
        let value: f64 = self.value.into();
        let range_start: f64 = (*self.range.start()).into();
        let range_end: f64 = (*self.range.end()).into();

        if range_start >= range_end {
            return 0.0;
        }

        let usable_width = track_width - self.handle_width;
        let percent = (value - range_start) / (range_end - range_start);
        usable_width * percent as f32 + self.handle_width / 2.0
    }
}

struct TooltipTree;

struct TooltipState {
    is_hovering: bool,
    is_dragging: bool,
    hover_started_at: Option<Instant>,
    cursor_position: mouse::Cursor,
}

impl Default for TooltipState {
    fn default() -> Self {
        Self {
            is_hovering: false,
            is_dragging: false,
            hover_started_at: None,
            cursor_position: mouse::Cursor::Unavailable,
        }
    }
}

impl<'a, T, Message, Theme, Renderer> widget::Widget<Message, Theme, Renderer>
    for SliderTooltip<'a, T, Message, Theme, Renderer>
where
    T: Copy + From<u8> + PartialOrd + Into<f64> + num_traits::FromPrimitive + 'a,
    Message: Clone + 'a,
    Theme: slider::Catalog + container::Catalog + 'a,
    <Theme as slider::Catalog>::Class<'a>:
        From<slider::StyleFn<'a, Theme>>,
    Renderer: renderer::Renderer + text::Renderer + 'a,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<TooltipTree>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(TooltipState::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.slider_element)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.slider_element]);
    }

    fn size(&self) -> Size<Length> {
        self.slider_element.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.slider_element.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.slider_element.as_widget_mut().layout(
            &mut tree.children[0],
            renderer,
            limits,
        )
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
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let now = Instant::now();
        let state = tree.state.downcast_mut::<TooltipState>();
        state.cursor_position = cursor;
        let cursor_over = cursor.is_over(bounds);

        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if cursor_over && !state.is_hovering {
                    state.is_hovering = true;
                    state.hover_started_at = Some(now);
                    shell.request_redraw();
                } else if !cursor_over && state.is_hovering && !state.is_dragging {
                    state.is_hovering = false;
                    state.hover_started_at = None;
                    shell.request_redraw();
                }
                if state.is_hovering
                    && state.hover_started_at.is_some()
                    && !state.is_dragging
                {
                    shell.request_redraw_at(now + self.tooltip_delay);
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(iced::touch::Event::FingerPressed { .. }) => {
                if cursor.is_over(bounds) {
                    state.is_dragging = true;
                    state.hover_started_at = None;
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(
                mouse::Button::Left,
            ))
            | Event::Touch(iced::touch::Event::FingerLifted { .. })
            | Event::Touch(iced::touch::Event::FingerLost { .. }) => {
                if state.is_dragging {
                    state.is_dragging = false;
                    shell.request_redraw();
                }
            }
            Event::Window(iced::window::Event::RedrawRequested(_)) => {
                if state.is_hovering && !state.is_dragging {
                    if let Some(started) = state.hover_started_at {
                        if now.duration_since(started) >= self.tooltip_delay {
                            state.hover_started_at = None;
                            shell.request_redraw();
                        }
                    }
                }
            }
            _ => {}
        }

        self.slider_element.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
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
        self.slider_element.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.slider_element.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        _translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let state = tree.state.downcast_ref::<TooltipState>();
        let should_show = state.is_dragging
            || (state.is_hovering && state.hover_started_at.is_none());
        let cursor_over = state.cursor_position.is_over(layout.bounds());
        let bounds = layout.bounds();
        let visible = should_show && (cursor_over || state.is_dragging);

        let tooltip_text = if visible {
            (self.tooltip_format)(self.value)
        } else {
            String::new()
        };

        let handle_x = self.handle_center_x(bounds.width);

        let padding_x = 8.0_f32;
        let padding_y = 4.0_f32;

        let tooltip_bounds = if visible {
            let text_paragraph =
                <Renderer::Paragraph as text::Paragraph>::with_text(
                    text::Text {
                        content: &tooltip_text,
                        bounds: Size::new(f32::INFINITY, f32::INFINITY),
                        size: Pixels(13.0),
                        line_height: text::LineHeight::Absolute(Pixels(
                            16.0,
                        )),
                        font: renderer.default_font(),
                        align_x: text::Alignment::Default,
                        align_y: iced::alignment::Vertical::Top,
                        shaping: text::Shaping::Basic,
                        wrapping: text::Wrapping::None,
                    },
                );

            let tooltip_w = text_paragraph.min_width() + padding_x * 2.0;
            let tooltip_h = text_paragraph.min_height() + padding_y * 2.0;

            let tooltip_x = bounds.x + handle_x - tooltip_w / 2.0;
            let tooltip_y = match self.tooltip_position {
                TooltipPosition::Top => {
                    bounds.y - tooltip_h - self.tooltip_gap
                }
                TooltipPosition::Bottom => {
                    bounds.y + bounds.height + self.tooltip_gap
                }
            };

            let mut b = Rectangle {
                x: tooltip_x,
                y: tooltip_y,
                width: tooltip_w,
                height: tooltip_h,
            };

            if b.x < viewport.x {
                b.x = viewport.x;
            } else if viewport.x + viewport.width < b.x + b.width {
                b.x = viewport.x + viewport.width - b.width;
            }

            if b.y < viewport.y {
                b.y = viewport.y;
            } else if viewport.y + viewport.height < b.y + b.height {
                b.y = viewport.y + viewport.height - b.height;
            }

            b
        } else {
            Rectangle {
                x: bounds.center().x,
                y: bounds.center().y,
                width: 0.0,
                height: 0.0,
            }
        };

        let style_ref: Option<&dyn Fn(&Theme) -> container::Style> =
            self.tooltip_style.as_deref();

        Some(overlay::Element::new(Box::new(TooltipOverlay {
            visible,
            text: tooltip_text,
            bounds: tooltip_bounds,
            padding_x,
            padding_y,
            style: style_ref,
        })))
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
            self.slider_element.as_widget_mut().operate(
                &mut tree.children[0],
                layout,
                renderer,
                operation,
            );
        });
    }
}

struct TooltipOverlay<'a, Theme> {
    visible: bool,
    text: String,
    bounds: Rectangle,
    padding_x: f32,
    padding_y: f32,
    style: Option<&'a dyn Fn(&Theme) -> container::Style>,
}

impl<'a, Message, Theme, Renderer>
    overlay::Overlay<Message, Theme, Renderer>
    for TooltipOverlay<'a, Theme>
where
    Message: Clone,
    Theme: container::Catalog,
    Renderer: renderer::Renderer + text::Renderer,
{
    fn layout(&mut self, _renderer: &Renderer, _bounds: Size) -> layout::Node {
        if self.visible {
            layout::Node::new(self.bounds.size())
                .translate(Vector::new(self.bounds.x, self.bounds.y))
        } else {
            layout::Node::new(Size::ZERO)
        }
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
    ) {
        if !self.visible {
            return;
        }

        let style = self
            .style
            .map(|f| f(theme))
            .unwrap_or_else(|| container::Style {
                background: Some(Background::Color(
                    Color::from_rgba(0.1, 0.1, 0.1, 0.9),
                )),
                border: Border {
                    radius: 4.0.into(),
                    width: 1.0,
                    color: Color::from_rgba(0.3, 0.3, 0.3, 1.0),
                },
                text_color: Some(Color::from_rgb(0.9, 0.9, 0.9)),
                ..container::Style::default()
            });

        let bounds = layout.bounds();

        if let Some(background) = style.background {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: style.border,
                    ..renderer::Quad::default()
                },
                background,
            );
        }

        let text_color =
            style.text_color.unwrap_or(Color::from_rgb(0.0, 0.0, 0.0));

        let text_bounds = Rectangle {
            x: bounds.x + self.padding_x,
            y: bounds.y + self.padding_y,
            width: bounds.width - self.padding_x * 2.0,
            height: bounds.height - self.padding_y * 2.0,
        };

        renderer.fill_text(
            text::Text {
                content: self.text.clone(),
                bounds: Size::new(text_bounds.width, text_bounds.height),
                size: Pixels(13.0),
                line_height: text::LineHeight::Absolute(Pixels(16.0)),
                font: renderer.default_font(),
                align_x: text::Alignment::Center,
                align_y: iced::alignment::Vertical::Center,
                shaping: text::Shaping::Basic,
                wrapping: text::Wrapping::None,
            },
            text_bounds.center(),
            text_color,
            text_bounds,
        );
    }

    fn update(
        &mut self,
        _event: &Event,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        _shell: &mut Shell<'_, Message>,
    ) {
    }

    fn mouse_interaction(
        &self,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        mouse::Interaction::default()
    }
}

/// Controls where the tooltip appears relative to the slider handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooltipPosition {
    /// Tooltip appears above the slider handle.
    Top,
    /// Tooltip appears below the slider handle.
    Bottom,
}

impl Default for TooltipPosition {
    fn default() -> Self {
        Self::Top
    }
}

impl<'a, T, Message, Theme, Renderer>
    From<SliderTooltip<'a, T, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    T: Copy + From<u8> + PartialOrd + Into<f64> + num_traits::FromPrimitive + 'a,
    Message: Clone + 'a,
    Theme: slider::Catalog + container::Catalog + 'a,
    <Theme as slider::Catalog>::Class<'a>:
        From<slider::StyleFn<'a, Theme>>,
    Renderer: renderer::Renderer + text::Renderer + 'a,
{
    fn from(slider: SliderTooltip<'a, T, Message, Theme, Renderer>) -> Self {
        Element::new(slider)
    }
}
