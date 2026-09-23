use iced::widget::{column, container, row, rule, scrollable, text};
use iced::{Element, Length, Task, Theme};

use neverliie_iced_widgets::number_input::NumberInput;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .theme(App::theme)
        .run()
}

#[derive(Debug, Clone)]
enum Message {
    CountChanged(i32),
    TempChanged(f64),
    OffsetChanged(i32),
}

struct App {
    count: i32,
    temp: f64,
    offset: i32,
    log: Vec<String>,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                count: 42,
                temp: 3.5,
                offset: 0,
                log: vec![
                    "Click -/+ or focus the field and type.".into(),
                    "Up/Down arrows step; Shift steps bigger.".into(),
                    "Mouse wheel over the pill steps (no modifier).".into(),
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

    fn update(&mut self, message: Message) {
        match message {
            Message::CountChanged(v) => {
                self.log(format!("Count: {} -> {v}", self.count));
                self.count = v;
            }
            Message::TempChanged(v) => {
                self.log(format!("Temp: {:.2} -> {v:.2}", self.temp));
                self.temp = v;
            }
            Message::OffsetChanged(v) => {
                self.log(format!("Offset: {} -> {v}", self.offset));
                self.offset = v;
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let int_demo = column![
            text("Integer 0..=100 (step 1, shift-step 10)").size(18),
            rule::horizontal(1),
            text("Buttons clamp at the ends; try typing, arrows, wheel.")
                .size(12),
            NumberInput::new(0..=100, self.count, Message::CountChanged)
                .step(1)
                .shift_step(10)
                .placeholder("0")
                .border_radius(999.0)
                .width(200.0),
            text(format!("Current: {}", self.count)).size(12),
        ]
        .spacing(8)
        .padding(16);

        let float_demo = column![
            text("Float 0.0..=10.0 (step 0.5)").size(18),
            rule::horizontal(1),
            text("Trailing-dot and exponent prefixes stay editable.").size(12),
            NumberInput::new(0.0..=10.0, self.temp, Message::TempChanged)
                .step(0.5)
                .shift_step(2.0)
                .placeholder("0.0")
                .border_radius(999.0)
                .width(200.0),
            text(format!("Current: {:.2}", self.temp)).size(12),
        ]
        .spacing(8)
        .padding(16);

        let negative_demo = column![
            text("Signed -50..=50 (type a leading '-')").size(18),
            rule::horizontal(1),
            text("Intermediate '-' is kept while typing.").size(12),
            NumberInput::new(-50..=50, self.offset, Message::OffsetChanged)
                .step(1)
                .placeholder("0")
                .border_radius(12.0)
                .width(200.0),
            text(format!("Current: {}", self.offset)).size(12),
        ]
        .spacing(8)
        .padding(16);

        let prefix_demo = column![
            text("With icon prefix (-| icon input |+)").size(18),
            rule::horizontal(1),
            text("Auto-sized icon like dropdown menu icons.")
                .size(12),
            NumberInput::new(0..=100, self.count, Message::CountChanged)
                .step(1)
                .shift_step(10)
                .placeholder("0")
                .prefix(text("★").size(14))
                .border_radius(999.0)
                .width(200.0),
            text(format!("Current: {}", self.count)).size(12),
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
                container(column![int_demo, float_demo, negative_demo, prefix_demo].spacing(8))
                    .width(340)
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
