use iced::widget::{column, container, row, rule, scrollable, text};
use iced::{Element, Length, Task, Theme};

use neverliie_iced_widgets::split_button::{Item, MenuItem, danger, split_button};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .theme(App::theme)
        .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Save,
    SaveAs,
    Export,
    Print,
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Save => "Save",
            Self::SaveAs => "Save As",
            Self::Export => "Export",
            Self::Print => "Print",
        })
    }
}

#[derive(Debug, Clone)]
enum Message {
    ActionSelected(Action),
    ActionPressed(Action),
    SecondarySelected(Action),
    SecondaryPressed(Action),
    ClearLog,
}

struct App {
    selected: Option<Action>,
    secondary: Option<Action>,
    executions: u32,
    log: Vec<String>,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                selected: Some(Action::Save),
                secondary: None,
                executions: 0,
                log: vec![
                    "Main area executes the selection; arrow opens the menu.".into(),
                    "Menu picks only change the selection.".into(),
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
            Message::ActionSelected(action) => {
                self.selected = Some(action);
                self.log(format!("Selected: {action}"));
            }
            Message::ActionPressed(action) => {
                self.selected = Some(action);
                self.executions += 1;
                self.log(format!("Executed: {action} (#{})", self.executions));
            }
            Message::SecondarySelected(action) => {
                self.secondary = Some(action);
                self.log(format!("Secondary selected: {action}"));
            }
            Message::SecondaryPressed(action) => {
                self.secondary = Some(action);
                self.log(format!("Secondary executed: {action}"));
            }
            Message::ClearLog => self.log.clear(),
        }
    }

    fn options() -> [MenuItem<'static, Action, Message, Theme, iced::Renderer>; 5]
    {
        [
            MenuItem::Item(
                Item::new(Action::Save, "Save").icon(text("💾").size(14)),
            ),
            MenuItem::Item(
                Item::new(Action::SaveAs, "Save As").icon(text("📝").size(14)),
            ),
            MenuItem::Separator,
            MenuItem::Item(
                Item::new(Action::Export, "Export").icon(text("📤").size(14)),
            ),
            MenuItem::Item(
                Item::new(Action::Print, "Print").icon(text("🖨").size(14)),
            ),
        ]
    }

    fn view(&self) -> Element<'_, Message> {
        let main = column![
            text("SplitButton (preselected)").size(18),
            rule::horizontal(1),
            text("Click the label to execute; click ▼ for the menu.").size(12),
            split_button(Self::options(), self.selected, Message::ActionSelected)
                .placeholder("Choose an action...")
                .on_press(Message::ActionPressed)
                .width(220.0),
            text(format!(
                "Current: {} — executions: {}",
                self.selected
                    .map(|a| a.to_string())
                    .unwrap_or_else(|| "(none)".into()),
                self.executions
            ))
            .size(12),
        ]
        .spacing(8)
        .padding(16);

        let secondary = column![
            text("SplitButton (starts empty, danger)").size(18),
            rule::horizontal(1),
            text("Main click with no selection opens the menu.").size(12),
            split_button(
                Self::options(),
                self.secondary,
                Message::SecondarySelected
            )
            .placeholder("Choose an action...")
            .on_press(Message::SecondaryPressed)
            .style(danger)
            .width(220.0),
            text(format!(
                "Current: {}",
                self.secondary
                    .map(|a| a.to_string())
                    .unwrap_or_else(|| "(none)".into())
            ))
            .size(12),
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
            row![
                text("Event Log").size(14).width(Length::Fill),
                iced::widget::button(text("Clear").size(11))
                    .on_press(Message::ClearLog)
                    .padding([2, 8]),
            ],
            rule::horizontal(1),
            scrollable(log_entries),
        ]
        .spacing(4)
        .padding(12);

        iced::widget::row![
            container(main).width(300).height(Length::Fill),
            container(secondary).width(300).height(Length::Fill),
            container(log_panel).width(Length::Fill).height(Length::Fill),
        ]
        .spacing(8)
        .padding(8)
        .height(Length::Fill)
        .into()
    }
}
