use iced::widget::{button, column, container, scrollable, text};
use iced::{Element, Length};

fn main() -> iced::Result {
    iced::run(App::update, App::view)
}

#[derive(Default)]
struct App;

#[derive(Debug, Clone)]
enum Message {
    RunDemo(&'static str),
}

impl App {
    fn update(&mut self, message: Message) {
        match message {
            Message::RunDemo(name) => {
                let _ = std::process::Command::new("cargo")
                    .args(["run", "-p", name])
                    .spawn();
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let demos = column![
            text("NeverLie Iced Widgets").size(24),
            text("Widget Demos").size(14),
            button(text("Overlay").width(Length::Fill))
                .on_press(Message::RunDemo("overlay-test")),
            button(text("Context Menu").width(Length::Fill))
                .on_press(Message::RunDemo("context-menu-test")),
            button(text("Confirmation Dialog").width(Length::Fill))
                .on_press(Message::RunDemo("confirmation-dialog-test")),
            button(text("Ghost Text Input").width(Length::Fill))
                .on_press(Message::RunDemo("ghost-text-input-test")),
            button(text("Slider Tooltip").width(Length::Fill))
                .on_press(Message::RunDemo("slider-tooltip")),
            button(text("Lazy Icon").width(Length::Fill))
                .on_press(Message::RunDemo("lazy-icon-test")),
            button(text("Ellipsis Text").width(Length::Fill))
                .on_press(Message::RunDemo("ellipsis-text-test")),
        ]
        .spacing(8)
        .padding(24);

        container(scrollable(demos))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
