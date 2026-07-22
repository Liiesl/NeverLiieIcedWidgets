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
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

static SLIDER_ID_COUNTER: AtomicU32 = AtomicU32::new(0);

const DEFAULT_HANDLE_WIDTH: f32 = 14.0;

/// A slider with a tooltip that shows the current value during hover and drag.
pub struct SliderTooltip<
    'a,
    T,
    Message,
    Theme = iced::Theme,
    Renderer = iced::Renderer,
> where
    Theme: slider::Catalog,
{
    id: u32,
    slider: Element<'a, Message, Theme, Renderer>,
    value: T,
    range: RangeInclusive<T>,
    handle_width: f32,
    tooltip_position: TooltipPosition,
    tooltip_gap: f32,
    tooltip_delay: Duration,
    tooltip_format: Box<dyn Fn(T) -> String + 'a>,
    tooltip_style: Option<Box<dyn Fn(&Theme) -> container::Style + 'a>>,
}

impl<'a, T, Message, Theme, Renderer>
    SliderTooltip<'a, T, Message, Theme, Renderer>
where
    T: Copy + From<u8> + PartialOrd + Into<f64> + num_traits::FromPrimitive + 'a,
    Message: Clone + 'a,
    Theme: slider::Catalog + container::Catalog + 'a,
    Renderer: renderer::Renderer + text::Renderer + 'a,
{
    /// Creates a new [`SliderTooltip`].
    pub fn new<F>(range: RangeInclusive<T>, value: T, on_change: F) -> Self
    where
        F: 'a + Fn(T) -> Message,
    {
        let format = Self::make_format(range.clone());
        let id = SLIDER_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        Self {
            id,
            slider: Element::new(Slider::new(range.clone(), value, on_change)),
            value,
            range,
            handle_width: DEFAULT_HANDLE_WIDTH,
            tooltip_position: TooltipPosition::Top,
            tooltip_gap: 8.0,
            tooltip_delay: Duration::ZERO,
            tooltip_format: Box::new(format),
            tooltip_style: None,
        }
    }

    fn make_format(range: RangeInclusive<T>) -> impl Fn(T) -> String {
        move |v: T| {
            let start: f64 = (*range.start()).into();
            let end: f64 = (*range.end()).into();
            let val: f64 = v.into();
            let range_len = end - start;
            if range_len < 1.0 {
                format!("{:.2}", val)
            } else if range_len < 10.0 {
                format!("{:.1}", val)
            } else {
                format!("{:.0}", val)
            }
        }
    }

    #[must_use]
    pub fn tooltip_position(mut self, position: TooltipPosition) -> Self {
        self.tooltip_position = position;
        self
    }

    #[must_use]
    pub fn tooltip_gap(mut self, gap: impl Into<Pixels>) -> Self {
        self.tooltip_gap = gap.into().0;
        self
    }

    #[must_use]
    pub fn tooltip_delay(mut self, delay: Duration) -> Self {
        self.tooltip_delay = delay;
        self
    }

    #[must_use]
    pub fn tooltip_format(
        mut self,
        format: impl Fn(T) -> String + 'a,
    ) -> Self {
        self.tooltip_format = Box::new(format);
        self
    }

    #[must_use]
    pub fn tooltip_style(
        mut self,
        style: impl Fn(&Theme) -> container::Style + 'a,
    ) -> Self {
        self.tooltip_style = Some(Box::new(style));
        self
    }

    #[must_use]
    pub fn handle_width(mut self, width: f32) -> Self {
        self.handle_width = width;
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
    T: Copy + From<u8> + PartialOrd + Into<f64> + num_traits::FromPrimitive,
    Message: Clone + 'a,
    Theme: slider::Catalog + container::Catalog + 'a,
    Renderer: renderer::Renderer + text::Renderer + 'a,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<TooltipTree>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(TooltipState::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.slider)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.slider]);
    }

    fn size(&self) -> Size<Length> {
        self.slider.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.slider.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.slider.as_widget_mut().layout(
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
        let id = self.id;

        let _was_hovering = state.is_hovering;

        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if cursor_over && !state.is_hovering {
                    state.is_hovering = true;
                    state.hover_started_at = Some(now);
                    shell.request_redraw();
                    eprintln!("[update] id={} >>> HOVER ENTER | bounds={:?} cursor={:?}", id, bounds, cursor);
                } else if !cursor_over && state.is_hovering {
                    state.is_hovering = false;
                    state.hover_started_at = None;
                    shell.request_redraw();
                    eprintln!("[update] id={} <<< HOVER EXIT  | bounds={:?} cursor={:?}", id, bounds, cursor);
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
                    eprintln!("[update] id={} >>> DRAG START  | bounds={:?}", id, bounds);
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
                    eprintln!("[update] id={} <<< DRAG END    | bounds={:?}", id, bounds);
                }
            }
            Event::Window(iced::window::Event::RedrawRequested(_)) => {
                if state.is_hovering && !state.is_dragging {
                    if let Some(started) = state.hover_started_at {
                        if now.duration_since(started) >= self.tooltip_delay {
                            state.hover_started_at = None;
                            shell.request_redraw();
                            eprintln!("[update] id={} *** DELAY ELAPSED, TOOLTIP NOW ACTIVE", id);
                        }
                    }
                }
            }
            _ => {}
        }

        self.slider.as_widget_mut().update(
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
        let state = tree.state.downcast_ref::<TooltipState>();
        let bounds = layout.bounds();
        let should_show = state.is_dragging
            || (state.is_hovering && state.hover_started_at.is_none());
        eprintln!(
            "[draw] id={} | bounds={:?} cursor={:?} cursor_over={} | is_hovering={} hover_started_at={:?} is_dragging={} should_show={}",
            self.id, bounds, cursor, cursor.is_over(bounds),
            state.is_hovering, state.hover_started_at, state.is_dragging, should_show
        );

        self.slider.as_widget().draw(
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
        self.slider.as_widget().mouse_interaction(
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
        let visible = should_show && cursor_over;

        eprintln!(
            "[overlay] id={} | bounds={:?} cursor={:?} cursor_over={} | is_hovering={} hover_started_at={:?} is_dragging={} should_show={} -> {}",
            self.id, bounds, state.cursor_position, cursor_over, state.is_hovering, state.hover_started_at, state.is_dragging, should_show,
            if visible { "VISIBLE" } else { "hidden" }
        );

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

        let style_ref: Option<&(dyn Fn(&Theme) -> container::Style + 'a)> =
            self.tooltip_style.as_deref();

        Some(overlay::Element::new(Box::new(TooltipOverlay {
            id: self.id,
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
            self.slider.as_widget_mut().operate(
                &mut tree.children[0],
                layout,
                renderer,
                operation,
            );
        });
    }
}

struct TooltipOverlay<'a, 'b, Theme> {
    id: u32,
    visible: bool,
    text: String,
    bounds: Rectangle,
    padding_x: f32,
    padding_y: f32,
    style: Option<&'b (dyn Fn(&Theme) -> container::Style + 'a)>,
}

impl<'a, 'b, Message, Theme, Renderer>
    overlay::Overlay<Message, Theme, Renderer>
    for TooltipOverlay<'a, 'b, Theme>
where
    Message: Clone,
    Theme: container::Catalog + 'a,
    Renderer: renderer::Renderer + text::Renderer + 'a,
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

        eprintln!("[overlay::draw] id={} RENDERING tooltip at {:?}", self.id, self.bounds);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooltipPosition {
    Top,
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
    Renderer: renderer::Renderer + text::Renderer + 'a,
{
    fn from(slider: SliderTooltip<'a, T, Message, Theme, Renderer>) -> Self {
        Element::new(slider)
    }
}
