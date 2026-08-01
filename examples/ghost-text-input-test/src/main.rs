use iced::widget::{column, text};
use iced::{Element, Length};

use neverliie_iced_widgets::ghost_text_input::GhostTrailTextInput;

fn main() -> iced::Result {
    iced::run(App::update, App::view)
}

#[derive(Default)]
struct App {
    basic_value: String,
    secure_value: String,
    styled_value: String,
}

#[derive(Debug, Clone)]
enum Message {
    BasicChanged(String),
    SecureChanged(String),
    StyledChanged(String),
}

impl App {
    fn update(&mut self, message: Message) {
        match message {
            Message::BasicChanged(v) => self.basic_value = v,
            Message::SecureChanged(v) => self.secure_value = v,
            Message::StyledChanged(v) => self.styled_value = v,
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let basic_input = GhostTrailTextInput::new("Type something...", &self.basic_value)
            .on_input(Message::BasicChanged)
            .width(300);

        let secure_input = GhostTrailTextInput::new("Enter password...", &self.secure_value)
            .on_input(Message::SecureChanged)
            .secure(true)
            .width(300);

        let styled_input = GhostTrailTextInput::new("Custom styled...", &self.styled_value)
            .on_input(Message::StyledChanged)
            .width(300)
            .cursor_color(iced::Color::from_rgb(0.2, 0.8, 0.4))
            .text_color(iced::Color::from_rgb(0.9, 0.9, 0.9))
            .placeholder_color(iced::Color::from_rgb(0.5, 0.5, 0.5));

        column![
            text("Ghost Trail Text Input Demo").size(24),
            text("Basic input:"),
            basic_input,
            text(format!("Value: {}", self.basic_value)),
            text(""),
            text("Secure (password) input:"),
            secure_input,
            text(format!("Value: {}", self.secure_value)),
            text(""),
            text("Custom styled input:"),
            styled_input,
            text(format!("Value: {}", self.styled_value)),
        ]
        .spacing(10)
        .padding(20)
        .width(Length::Fill)
        .into()
    }
}
