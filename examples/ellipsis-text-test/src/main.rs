use iced::widget::{column, container, scrollable, text};
use iced::{Element, Length};

use neverliie_iced_widgets::ellipsis_text::EllipsisText;

fn main() -> iced::Result {
    iced::run(App::update, App::view)
}

#[derive(Default)]
struct App;

#[derive(Debug, Clone)]
enum Message {}

impl App {
    fn update(&mut self, _message: Message) {}

    fn view(&self) -> Element<'_, Message> {
        let long =
            "A very long file name that must be clamped to the available width of its container";

        let content = column![
            text("Ellipsis Text Demo").size(24),
            text("Single line, truncated:"),
            EllipsisText::new(long).max_lines(1),
            text("Two lines, truncated:"),
            EllipsisText::new(long).max_lines(2),
            text("Three lines, truncated:"),
            EllipsisText::new(long).max_lines(3),
            text("Fits — rendered as plain text:"),
            EllipsisText::new("Short label").max_lines(2),
            text("Custom styling:"),
            EllipsisText::new(long)
                .max_lines(1)
                .size(14)
                .color(iced::Color::from_rgb(0.5, 0.8, 1.0)),
        ]
        .spacing(10)
        .padding(20)
        .width(Length::Fill);

        container(scrollable(content))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
