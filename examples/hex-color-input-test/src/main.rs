use iced::widget::{column, container, row, rule, scrollable, slider, text};
use iced::window::screenshot::Screenshot;
use iced::{Color, Element, Length, Task, Theme, window};

use neverliie_iced_widgets::color_picker::{DropperBuffer, Gradient};
use neverliie_iced_widgets::hex_color_input::{HexColorInput, HexColorValue};
use neverliie_iced_widgets::overlay::Position;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .theme(App::theme)
        .run()
}

#[derive(Debug, Clone)]
enum Message {
    SolidChanged(HexColorValue),
    SolidSubmit(HexColorValue),
    SolidOpen,
    SolidCancel,
    NoAlphaChanged(HexColorValue),
    NoAlphaSubmit(HexColorValue),
    NoAlphaOpen,
    NoAlphaCancel,
    GradientChanged(HexColorValue),
    GradientSubmit(HexColorValue),
    GradientOpen,
    GradientCancel,
    GradientAngleChanged(f32),
    DropperCapture,
    DropperShot(Screenshot),
}

struct App {
    solid: HexColorValue,
    no_alpha: HexColorValue,
    gradient: HexColorValue,
    show_solid: bool,
    show_no_alpha: bool,
    show_gradient: bool,
    dropper_buffer: DropperBuffer,
    log: Vec<String>,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                solid: HexColorValue::Solid(Color::from_rgba8(
                    255, 255, 255, 1.0,
                )),
                no_alpha: HexColorValue::Solid(Color::from_rgb(
                    0.2, 0.6, 1.0,
                )),
                gradient: HexColorValue::Gradient {
                    gradient: Gradient::two(
                        Color::from_rgb(0.9, 0.3, 0.6),
                        Color::from_rgb(0.3, 0.6, 0.9),
                    ),
                    angle: 90.0,
                },
                show_solid: false,
                show_no_alpha: false,
                show_gradient: false,
                dropper_buffer: DropperBuffer::new(),
                log: vec![
                    "Click a swatch to open the floating picker.".into(),
                ],
            },
            Task::none(),
        )
    }

    fn theme(&self) -> Theme {
        Theme::Dracula
    }

    fn log(&mut self, entry: impl Into<String>) {
        self.log.push(entry.into());
        if self.log.len() > 30 {
            self.log.remove(0);
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SolidChanged(v) => {
                self.solid = v;
            }
            Message::SolidSubmit(v) => {
                self.solid = v.clone();
                self.show_solid = false;
                self.log(format!("Solid submitted: {}", describe(&v)));
            }
            Message::SolidOpen => {
                self.show_solid = true;
            }
            Message::SolidCancel => {
                self.show_solid = false;
            }
            Message::NoAlphaChanged(v) => {
                self.no_alpha = v;
            }
            Message::NoAlphaSubmit(v) => {
                self.no_alpha = v.clone();
                self.show_no_alpha = false;
                self.log(format!("No-alpha submitted: {}", describe(&v)));
            }
            Message::NoAlphaOpen => {
                self.show_no_alpha = true;
            }
            Message::NoAlphaCancel => {
                self.show_no_alpha = false;
            }
            Message::GradientChanged(v) => {
                self.gradient = v;
            }
            Message::GradientSubmit(v) => {
                self.gradient = v.clone();
                self.show_gradient = false;
                self.log(format!("Gradient submitted: {}", describe(&v)));
            }
            Message::GradientOpen => {
                self.show_gradient = true;
            }
            Message::GradientCancel => {
                self.show_gradient = false;
            }
            Message::GradientAngleChanged(angle) => {
                self.gradient = self.gradient.with_angle(angle);
            }
            Message::DropperCapture => {
                self.log("Eye dropper capturing window...".to_string());
                return window::latest()
                    .and_then(window::screenshot)
                    .map(Message::DropperShot);
            }
            Message::DropperShot(screenshot) => {
                self.dropper_buffer.store(&screenshot);
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let solid_demo = column![
            text("Solid + alpha (■ FFFFFF | 100 %)").size(18),
            rule::horizontal(1),
            text("Hex edits RGB, % edits alpha. Swatch opens the picker.")
                .size(12),
            HexColorInput::new(
                self.solid.clone(),
                Message::SolidChanged,
                self.show_solid,
                Message::SolidOpen,
                Message::SolidCancel,
            )
            .on_submit(Message::SolidSubmit)
            .position(Position::BottomLeft)
            .dropper_buffer(self.dropper_buffer.clone())
            .on_dropper_capture(|| Message::DropperCapture),
            text(format!("Current: {}", describe(&self.solid))).size(12),
        ]
        .spacing(8)
        .padding(16);

        let no_alpha_demo = column![
            text("Solid, alpha hidden").size(18),
            rule::horizontal(1),
            text("show_alpha(false): just [■ FFFFFF].").size(12),
            HexColorInput::new(
                self.no_alpha.clone(),
                Message::NoAlphaChanged,
                self.show_no_alpha,
                Message::NoAlphaOpen,
                Message::NoAlphaCancel,
            )
            .show_alpha(false)
            .on_submit(Message::NoAlphaSubmit)
            .position(Position::BottomLeft)
            .dropper_buffer(self.dropper_buffer.clone())
            .on_dropper_capture(|| Message::DropperCapture),
            text(format!("Current: {}", describe(&self.no_alpha))).size(12),
        ]
        .spacing(8)
        .padding(16);

        let gradient_demo = column![
            text("Gradient (■ gradient | ∠ angle)").size(18),
            rule::horizontal(1),
            text("Hex becomes \"gradient\", alpha hidden, angle appears.")
                .size(12),
            HexColorInput::new(
                self.gradient.clone(),
                Message::GradientChanged,
                self.show_gradient,
                Message::GradientOpen,
                Message::GradientCancel,
            )
            .on_submit(Message::GradientSubmit)
            .position(Position::BottomLeft)
            .dropper_buffer(self.dropper_buffer.clone())
            .on_dropper_capture(|| Message::DropperCapture),
            text("Angle (external slider drives the widget):").size(12),
            row![
                slider(0.0..=360.0, self.gradient.angle(), Message::GradientAngleChanged),
                text(format!("{:.0}°", self.gradient.angle())).size(12),
            ]
            .spacing(8),
            text(format!("Current: {}", describe(&self.gradient))).size(12),
        ]
        .spacing(8)
        .padding(16);

        let log_entries = self.log.iter().enumerate().fold(
            column![].spacing(2),
            |col, (i, entry)| {
                col.push(text(format!("{}: {}", i + 1, entry)).size(11))
            },
        );
        let log_panel = column![
            text("Event Log").size(14),
            rule::horizontal(1),
            scrollable(log_entries),
        ]
        .spacing(4)
        .padding(12);

        container(
            row![
                container(
                    column![solid_demo, no_alpha_demo, gradient_demo]
                        .spacing(8)
                )
                .width(380)
                .height(Length::Fill),
                container(log_panel)
                    .width(Length::Fill)
                    .height(Length::Fill),
            ]
            .spacing(8)
            .padding(8)
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

fn describe(value: &HexColorValue) -> String {
    match value {
        HexColorValue::Solid(c) => {
            let [r, g, b, a] = c.into_rgba8();
            format!(
                "#{:02X}{:02X}{:02X} @ {}%",
                r,
                g,
                b,
                (a as f32 / 255.0 * 100.0).round() as u8
            )
        }
        HexColorValue::Gradient { gradient, angle } => {
            let parts = gradient
                .stops
                .iter()
                .map(|stop| {
                    let [r, g, b, _] = stop.color.into_rgba8();
                    format!("#{r:02X}{g:02X}{b:02X}@{:.2}", stop.offset)
                })
                .collect::<Vec<_>>()
                .join(" -> ");
            format!("gradient {parts} @ {angle:.0}°")
        }
    }
}
