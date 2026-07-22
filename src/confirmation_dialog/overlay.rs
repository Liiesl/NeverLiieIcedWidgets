use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::text::{self, Paragraph, Text};
use iced::advanced::widget::Operation;
use iced::advanced::Shell;
use iced::mouse;
use iced::{Background, Event, Pixels, Point, Rectangle, Size};

use super::{ButtonStyle, Catalog, DialogButton, DIALOG_MAX_WIDTH, MIN_BUTTON_WIDTH};

/// State tracking for the dialog overlay.
pub(crate) struct DialogState {
    pub hovered_button: Option<usize>,
    pub dismissed: bool,
    pub pulse_active: bool,
    pub pulse_counter: u32,
}

impl DialogState {
    pub fn new() -> Self {
        Self {
            hovered_button: None,
            dismissed: false,
            pulse_active: false,
            pulse_counter: 0,
        }
    }
}

// Layout constants (independent of theme)
const DIALOG_PADDING: f32 = 24.0;
const DIALOG_SPACING: f32 = 16.0;
const TITLE_SIZE: f32 = 18.0;
const MESSAGE_SIZE: f32 = 14.0;
const BUTTON_HEIGHT: f32 = 36.0;
const BUTTON_PADDING_X: f32 = 20.0;
const BUTTON_SPACING: f32 = 10.0;

pub(crate) struct DialogOverlay<'a, 'b, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    title: &'a str,
    message: &'a str,
    buttons: &'a [DialogButton<'b, Message>],
    state: &'a mut DialogState,
    viewport: Rectangle,
    class: &'a <Theme as Catalog>::Class<'b>,
    font: Renderer::Font,
    blocking: bool,
}

impl<'a, 'b, Message, Theme, Renderer> DialogOverlay<'a, 'b, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    pub fn new(
        title: &'a str,
        message: &'a str,
        buttons: &'a [DialogButton<'b, Message>],
        state: &'a mut DialogState,
        viewport: Rectangle,
        class: &'a <Theme as Catalog>::Class<'b>,
        font: Renderer::Font,
        blocking: bool,
    ) -> Self {
        Self {
            title,
            message,
            buttons,
            state,
            viewport,
            class,
            font,
            blocking,
        }
    }

    fn measure_text_width(&self, content: &str, size: f32) -> f32 {
        Renderer::Paragraph::with_text(Text {
            content,
            bounds: Size::new(f32::INFINITY, size * 1.4),
            size: Pixels(size),
            line_height: text::LineHeight::Absolute(Pixels(size * 1.4)),
            font: self.font.clone(),
            align_x: text::Alignment::Left,
            align_y: iced::alignment::Vertical::Top,
            shaping: text::Shaping::Basic,
            wrapping: text::Wrapping::None,
        })
        .min_width()
    }

    fn measure_wrapped_height(&self, content: &str, size: f32, max_width: f32) -> f32 {
        let p = Renderer::Paragraph::with_text(Text {
            content,
            bounds: Size::new(max_width, f32::INFINITY),
            size: Pixels(size),
            line_height: text::LineHeight::Absolute(Pixels(size * 1.4)),
            font: self.font.clone(),
            align_x: text::Alignment::Left,
            align_y: iced::alignment::Vertical::Top,
            shaping: text::Shaping::Basic,
            wrapping: text::Wrapping::Word,
        });
        let h = p.min_height();
        h
    }

    fn measure_button_width(&self, label: &str) -> f32 {
        let text_width = self.measure_text_width(label, MESSAGE_SIZE);
        text_width + BUTTON_PADDING_X * 2.0
    }

    fn dialog_width(&self) -> f32 {
        let title_w = self.measure_text_width(self.title, TITLE_SIZE);
        let message_w = self.measure_text_width(self.message, MESSAGE_SIZE);

        let buttons_width: f32 = self.buttons.iter().enumerate().map(|(i, btn)| {
            let w = self.measure_button_width(btn.label);
            if i > 0 { w + BUTTON_SPACING } else { w }
        }).sum();

        let content_width = title_w.max(message_w).max(buttons_width);
        let dialog_w = content_width + DIALOG_PADDING * 2.0;
        dialog_w.clamp(0.0, DIALOG_MAX_WIDTH)
    }

    fn dialog_height(&self) -> f32 {
        let text_area_w = self.dialog_width() - DIALOG_PADDING * 2.0;
        let title_h = self.measure_wrapped_height(self.title, TITLE_SIZE, text_area_w);
        let message_h = self.measure_wrapped_height(self.message, MESSAGE_SIZE, text_area_w);
        title_h + message_h + BUTTON_HEIGHT + DIALOG_SPACING * 2.0 + DIALOG_PADDING * 2.0
    }

    fn dialog_bounds(&self) -> Rectangle {
        let w = self.dialog_width();
        let h = self.dialog_height();
        let x = (self.viewport.width - w) / 2.0;
        let y = (self.viewport.height - h) / 2.0;
        Rectangle::new(Point::new(x, y), Size::new(w, h))
    }

    fn button_bounds(&self, dialog: Rectangle, button_index: usize) -> Rectangle {
        let mut total_width = 0.0f32;
        let widths: Vec<f32> = self.buttons.iter()
            .map(|btn| self.measure_button_width(btn.label).max(MIN_BUTTON_WIDTH))
            .collect();

        for (i, w) in widths.iter().enumerate() {
            if i > 0 { total_width += BUTTON_SPACING; }
            total_width += w;
        }

        let buttons_y = dialog.y + dialog.height - DIALOG_PADDING - BUTTON_HEIGHT;
        let mut x = dialog.x + (dialog.width - total_width) / 2.0;

        for (i, w) in widths.iter().enumerate() {
            if i == button_index {
                return Rectangle::new(Point::new(x, buttons_y), Size::new(*w, BUTTON_HEIGHT));
            }
            x += w + BUTTON_SPACING;
        }
        Rectangle::default()
    }

    fn button_at(&self, position: Point) -> Option<usize> {
        let dialog = self.dialog_bounds();
        for i in 0..self.buttons.len() {
            if self.button_bounds(dialog, i).contains(position) {
                return Some(i);
            }
        }
        None
    }
}

impl<'a, 'b, Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for DialogOverlay<'a, 'b, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: Catalog + 'b,
    Renderer: renderer::Renderer + text::Renderer,
{
    fn layout(&mut self, _renderer: &Renderer, bounds: Size) -> layout::Node {
        layout::Node::new(bounds)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
    ) {
        let s = <Theme as Catalog>::style(theme, self.class);
        let viewport_bounds = layout.bounds();
        let dialog = self.dialog_bounds();

        // Backdrop
        renderer.fill_quad(
            renderer::Quad {
                bounds: viewport_bounds,
                border: iced::Border::default(),
                ..renderer::Quad::default()
            },
            s.backdrop_color,
        );

        // Dialog background
        renderer.fill_quad(
            renderer::Quad {
                bounds: dialog,
                border: s.border,
                shadow: s.shadow,
                ..renderer::Quad::default()
            },
            s.background,
        );

        // Pulsing border on blocked dismiss attempt
        if self.state.pulse_active {
            let t = self.state.pulse_counter as f32 * 0.4; // fast
            let pulse = (t.sin() + 1.0) * 0.5; // 0.0 .. 1.0
            let alpha = 0.3 + pulse * 0.7; // 0.3 .. 1.0

            let accent = match s.button_background {
                Background::Color(c) => c,
                _ => iced::Color::from_rgb(0.4, 0.5, 0.8),
            };

            let border_color = iced::Color { a: alpha, ..accent };

            renderer.fill_quad(
                renderer::Quad {
                    bounds: dialog,
                    border: iced::Border {
                        width: 2.0,
                        radius: s.border.radius,
                        color: border_color,
                    },
                    shadow: iced::Shadow::default(),
                    ..renderer::Quad::default()
                },
                iced::Background::Color(iced::Color::TRANSPARENT),
            );
        }

        // Title
        let title_y = dialog.y + DIALOG_PADDING;
        let text_area_w = dialog.width - DIALOG_PADDING * 2.0;
        let title_h = self.measure_wrapped_height(self.title, TITLE_SIZE, text_area_w);

        renderer.fill_text(
            Text {
                content: self.title.to_string(),
                bounds: Size::new(text_area_w, title_h),
                size: Pixels(TITLE_SIZE),
                line_height: text::LineHeight::Absolute(Pixels(TITLE_SIZE * 1.4)),
                font: self.font.clone(),
                align_x: text::Alignment::Default,
                align_y: iced::alignment::Vertical::Top,
                shaping: text::Shaping::Basic,
                wrapping: text::Wrapping::Word,
            },
            Point::new(dialog.x + DIALOG_PADDING, title_y),
            s.title_color,
            dialog,
        );

        // Message
        let message_y = title_y + title_h + DIALOG_SPACING;
        let msg_h = self.measure_wrapped_height(self.message, MESSAGE_SIZE, text_area_w);

        renderer.fill_text(
            Text {
                content: self.message.to_string(),
                bounds: Size::new(text_area_w, msg_h),
                size: Pixels(MESSAGE_SIZE),
                line_height: text::LineHeight::Absolute(Pixels(MESSAGE_SIZE * 1.4)),
                font: self.font.clone(),
                align_x: text::Alignment::Default,
                align_y: iced::alignment::Vertical::Top,
                shaping: text::Shaping::Basic,
                wrapping: text::Wrapping::Word,
            },
            Point::new(dialog.x + DIALOG_PADDING, message_y),
            s.message_color,
            dialog,
        );

        // Buttons
        let hovered = self.state.hovered_button;
        for (i, btn) in self.buttons.iter().enumerate() {
            let btn_bounds = self.button_bounds(dialog, i);
            let is_hovered = hovered == Some(i);

            let (bg, border, text_color) = match btn.style.unwrap_or(ButtonStyle::Secondary) {
                ButtonStyle::Secondary => (s.secondary_button_background.clone(), s.secondary_button_border, s.secondary_button_text_color),
                ButtonStyle::Default => (s.button_background.clone(), s.button_border, s.button_text_color),
                ButtonStyle::Danger => (s.danger_button_background.clone(), s.danger_button_border, s.danger_button_text_color),
            };

            // Hover: brighten background and text
            let (bg, text_color) = if is_hovered {
                match bg {
                    Background::Color(c) => {
                        let brightened = iced::Color {
                            r: (c.r + 0.08).min(1.0),
                            g: (c.g + 0.08).min(1.0),
                            b: (c.b + 0.08).min(1.0),
                            a: c.a,
                        };
                        (Background::Color(brightened), text_color)
                    }
                    other => (other, text_color),
                }
            } else {
                (bg, text_color)
            };

            renderer.fill_quad(
                renderer::Quad { bounds: btn_bounds, border, ..renderer::Quad::default() },
                bg,
            );

            renderer.fill_text(
                Text {
                    content: btn.label.to_string(),
                    bounds: Size::new(btn_bounds.width, btn_bounds.height),
                    size: Pixels(MESSAGE_SIZE),
                    line_height: text::LineHeight::Absolute(Pixels(MESSAGE_SIZE * 1.4)),
                    font: self.font.clone(),
                    align_x: text::Alignment::Center,
                    align_y: iced::alignment::Vertical::Center,
                    shaping: text::Shaping::Basic,
                    wrapping: text::Wrapping::None,
                },
                btn_bounds.center(),
                text_color,
                btn_bounds,
            );
        }
    }

    fn update(
        &mut self,
        event: &Event,
        _layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        match event {
            Event::Mouse(mouse::Event::CursorMoved { position: _ }) => {
                if let Some(pos) = cursor.position() {
                    let hovered = self.button_at(pos);
                    if self.state.hovered_button != hovered {
                        self.state.hovered_button = hovered;
                        shell.request_redraw();
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position() {
                    if let Some(idx) = self.button_at(pos) {
                        if let Some(btn) = self.buttons.get(idx) {
                            shell.publish(btn.action.clone());
                            self.state.dismissed = true;
                            shell.capture_event();
                        }
                    } else if !self.dialog_bounds().contains(pos) {
                        if self.blocking {
                            self.state.pulse_active = true;
                            self.state.pulse_counter = 0;
                            shell.capture_event();
                        } else {
                            self.state.dismissed = true;
                            shell.request_redraw();
                        }
                    }
                }
            }
            Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => {
                match key.as_ref() {
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) => {
                        if !self.blocking {
                            self.state.dismissed = true;
                            shell.request_redraw();
                            shell.capture_event();
                        } else {
                            shell.capture_event();
                        }
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Tab) => {
                        let next = match self.state.hovered_button {
                            Some(i) => Some((i + 1) % self.buttons.len()),
                            None => Some(0),
                        };
                        self.state.hovered_button = next;
                        shell.request_redraw();
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter) => {
                        if let Some(idx) = self.state.hovered_button {
                            if let Some(btn) = self.buttons.get(idx) {
                                shell.publish(btn.action.clone());
                                self.state.dismissed = true;
                                shell.capture_event();
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        // Pulse animation — runs briefly after a blocked dismiss attempt
        if self.state.pulse_active {
            self.state.pulse_counter += 1;
            if self.state.pulse_counter > 30 {
                self.state.pulse_active = false;
            }
            shell.request_redraw();
        }
    }

    fn mouse_interaction(
        &self,
        _layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if let Some(pos) = cursor.position() {
            if self.dialog_bounds().contains(pos) && self.button_at(pos).is_some() {
                mouse::Interaction::Pointer
            } else {
                mouse::Interaction::default()
            }
        } else {
            mouse::Interaction::default()
        }
    }

    fn operate(&mut self, _layout: Layout<'_>, _renderer: &Renderer, _operation: &mut dyn Operation) {}

    fn index(&self) -> f32 {
        200.0
    }
}
