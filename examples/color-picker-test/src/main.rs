use iced::widget::{button, column, container, rule, scrollable, space, text};
use iced::{Background, Border, Color, Element, Length, Shadow, Task, Theme, Vector};

use neverliie_iced_widgets::color_picker::{color_picker_with_change, ColorPicker};
use neverliie_iced_widgets::overlay::{Anchor, Position};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .theme(App::theme)
        .run()
}

struct App {
    log: Vec<String>,
    builder_color: Color,
    builder_live_color: Color,
    helper_color: Color,
    helper_live_color: Color,
    position_color: Color,
    position_live_color: Color,
    show_builder_picker: bool,
    show_helper_picker: bool,
    show_position_picker: bool,
    position_choice: PositionChoice,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                log: vec!["Pick a color to change each panel's preview swatch.".into()],
                builder_color: Color::from_rgb(1.0, 0.55, 0.0),
                builder_live_color: Color::from_rgb(1.0, 0.55, 0.0),
                helper_color: Color::from_rgb(0.3, 0.6, 0.9),
                helper_live_color: Color::from_rgb(0.3, 0.6, 0.9),
                position_color: Color::from_rgb(0.25, 0.8, 0.35),
                position_live_color: Color::from_rgb(0.25, 0.8, 0.35),
                show_builder_picker: false,
                show_helper_picker: false,
                show_position_picker: false,
                position_choice: PositionChoice::BottomRight,
            },
            Task::none(),
        )
    }

    fn theme(&self) -> Theme {
        Theme::Dracula
    }
}

/// The position strategies demonstrated by the "Position API" panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PositionChoice {
    BottomRight,
    BottomLeft,
    ViewportTopRight,
    Absolute,
    FollowCursor,
}

impl PositionChoice {
    const ALL: [PositionChoice; 5] = [
        PositionChoice::BottomRight,
        PositionChoice::BottomLeft,
        PositionChoice::ViewportTopRight,
        PositionChoice::Absolute,
        PositionChoice::FollowCursor,
    ];

    fn label(self) -> &'static str {
        match self {
            PositionChoice::BottomRight => "BottomRight",
            PositionChoice::BottomLeft => "BottomLeft + offset",
            PositionChoice::ViewportTopRight => "ViewportTopRight",
            PositionChoice::Absolute => "absolute(100, 100)",
            PositionChoice::FollowCursor => "FollowCursor",
        }
    }

    fn position(self) -> Position {
        match self {
            PositionChoice::BottomRight => Position::BottomRight,
            PositionChoice::BottomLeft => Position::Parent {
                anchor: Anchor::BottomLeft,
                offset: Vector::new(0.0, 8.0),
            },
            PositionChoice::ViewportTopRight => Position::ViewportTopRight,
            PositionChoice::Absolute => Position::absolute(100.0, 100.0),
            PositionChoice::FollowCursor => Position::FollowCursor,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    OpenBuilder,
    OpenHelper,
    BuilderCancel,
    BuilderSubmit(Color),
    HelperCancel,
    HelperSubmit(Color),
    BuilderColorChanged(Color),
    HelperColorChanged(Color),
    SelectPosition(PositionChoice),
    OpenPosition,
    PositionCancel,
    PositionSubmit(Color),
    PositionColorChanged(Color),
}

impl App {
    fn update(&mut self, message: Message) {
        match message {
            Message::OpenBuilder => {
                self.show_builder_picker = true;
                self.log_entry("Builder picker opened");
            }
            Message::OpenHelper => {
                self.show_helper_picker = true;
                self.log_entry("Helper picker opened");
            }
            Message::BuilderCancel => {
                self.show_builder_picker = false;
                self.log_entry("Builder picker cancelled");
            }
            Message::BuilderSubmit(color) => {
                self.show_builder_picker = false;
                self.builder_color = color;
                self.builder_live_color = color;
                self.log_entry(format!("Builder picker submitted: {}", hex(color)));
            }
            Message::HelperCancel => {
                self.show_helper_picker = false;
                self.log_entry("Helper picker cancelled");
            }
            Message::HelperSubmit(color) => {
                self.show_helper_picker = false;
                self.helper_color = color;
                self.helper_live_color = color;
                self.log_entry(format!("Helper picker submitted: {}", hex(color)));
            }
            Message::BuilderColorChanged(color) => {
                self.builder_live_color = color;
            }
            Message::HelperColorChanged(color) => {
                self.helper_live_color = color;
            }
            Message::SelectPosition(choice) => {
                self.position_choice = choice;
                self.show_position_picker = true;
                self.log_entry(format!("Position picker opened at {}", choice.label()));
            }
            Message::OpenPosition => {
                self.show_position_picker = true;
                self.log_entry(format!(
                    "Position picker opened at {}",
                    self.position_choice.label()
                ));
            }
            Message::PositionCancel => {
                self.show_position_picker = false;
                self.log_entry("Position picker cancelled");
            }
            Message::PositionSubmit(color) => {
                self.show_position_picker = false;
                self.position_color = color;
                self.position_live_color = color;
                self.log_entry(format!("Position picker submitted: {}", hex(color)));
            }
            Message::PositionColorChanged(color) => {
                self.position_live_color = color;
            }
        }
    }

    fn log_entry(&mut self, msg: impl Into<String>) {
        self.log.push(msg.into());
        if self.log.len() > 30 {
            self.log.remove(0);
        }
    }

    fn view(&self) -> Element<'_, Message> {
        // === Position API panel ===
        let position_picker = ColorPicker::new(
            self.show_position_picker,
            self.position_live_color,
            self.pick_button(
                "Position API",
                "Open picker",
                Message::OpenPosition,
            ),
            Message::PositionCancel,
            Message::PositionSubmit,
        )
        .on_color_change(Message::PositionColorChanged)
        .position(self.position_choice.position());

        let choice_buttons = column![
            text("Current:").size(12),
            text(self.position_choice.label()).size(13),
            space::vertical().height(4),
        ]
        .spacing(2)
        .push(
            column(PositionChoice::ALL.map(|choice| {
                button(text(choice.label()))
                    .on_press(Message::SelectPosition(choice))
                    .width(Length::Fill)
                    .into()
            }))
            .spacing(4),
        );

        let position_panel = container(
            column![
                text("Position API").size(18),
                rule::horizontal(1),
                text("ColorPicker::new(...) and .position(Position::...).").size(12),
                space::vertical().height(8),
                self.swatch(self.position_live_color),
                text(hex(self.position_live_color)).size(13),
                space::vertical().height(8),
                position_picker,
                space::vertical().height(8),
                choice_buttons,
            ]
            .spacing(8)
            .padding(20),
        )
        .width(220)
        .height(Length::Fill);

        // === Builder panel: builder API (`ColorPicker::new`) ===
        let builder_picker = ColorPicker::new(
            self.show_builder_picker,
            self.builder_live_color,
            self.pick_button("Builder API", "Open builder picker", Message::OpenBuilder),
            Message::BuilderCancel,
            Message::BuilderSubmit,
        )
        .on_color_change(Message::BuilderColorChanged);

        let builder_panel = container(
            column![
                text("Builder API").size(18),
                rule::horizontal(1),
                text("ColorPicker::new(...) with on_color_change for live preview.").size(12),
                space::vertical().height(8),
                self.swatch(self.builder_live_color),
                text(hex(self.builder_live_color)).size(13),
                space::vertical().height(8),
                builder_picker,
            ]
            .spacing(8)
            .padding(20),
        )
        .width(220)
        .height(Length::Fill);

        // === Center panel: shortcut helper API ===
        let helper_picker = color_picker_with_change(
            self.show_helper_picker,
            self.helper_live_color,
            self.pick_button("Helper API", "Open helper picker", Message::OpenHelper),
            Message::HelperCancel,
            Message::HelperSubmit,
            Message::HelperColorChanged,
        );

        let helper_panel = container(
            column![
                text("Helper API").size(18),
                rule::horizontal(1),
                text("color_picker_with_change(...) shortcut, same live preview.").size(12),
                space::vertical().height(8),
                self.swatch(self.helper_live_color),
                text(hex(self.helper_live_color)).size(13),
                space::vertical().height(8),
                helper_picker,
            ]
            .spacing(8)
            .padding(20),
        )
        .width(220)
        .height(Length::Fill);

        // === Right panel: log ===
        let log_entries = self.log.iter().enumerate().fold(
            column![].spacing(2),
            |col, (i, entry)| {
                col.push(text(format!("{}: {}", i + 1, entry)).size(11))
            },
        );

        let log_panel = container(
            column![
                text("Event Log").size(14),
                rule::horizontal(1),
                scrollable(log_entries),
            ]
            .spacing(4)
            .padding(12),
        )
        .width(260)
        .height(Length::Fill);

        iced::widget::row![position_panel, builder_panel, helper_panel, log_panel]
            .spacing(8)
            .padding(8)
            .height(Length::Fill)
            .into()
    }

    fn pick_button(
        &self,
        _title: &'static str,
        label: &'static str,
        message: Message,
    ) -> iced::widget::Button<'_, Message, iced::Theme> {
        button(text(label))
            .on_press(message)
            .width(Length::Fill)
    }

    fn swatch(&self, color: Color) -> Element<'_, Message> {
        container(text(""))
            .width(Length::Fill)
            .height(40)
            .style(move |_theme: &Theme| container::Style {
                background: Some(Background::Color(color)),
                text_color: None,
                border: Border {
                    radius: 4.0.into(),
                    width: 1.0,
                    color: Color::from_rgb(0.3, 0.3, 0.3),
                },
                shadow: Shadow::default(),
                snap: true,
            })
            .into()
    }
}

fn hex(color: Color) -> String {
    let [r, g, b, _] = color.into_rgba8();
    format!("#{r:02X}{g:02X}{b:02X}")
}