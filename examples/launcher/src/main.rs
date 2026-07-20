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
        ]
        .spacing(8)
        .padding(24);

        container(scrollable(demos))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
