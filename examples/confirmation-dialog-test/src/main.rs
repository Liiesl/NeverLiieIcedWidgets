use iced::widget::{button, column, container, rule, scrollable, space, text};
use iced::{Element, Length, Task, Theme};

use neverliie_iced_widgets::confirmation_dialog::{
    ConfirmationDialog, DialogButton,
};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .theme(App::theme)
        .run()
}

struct App {
    log: Vec<String>,
    show_simple_dialog: bool,
    show_custom_dialog: bool,
    show_danger_dialog: bool,
    delete_count: u32,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                log: vec!["Click buttons to open confirmation dialogs.".into()],
                show_simple_dialog: false,
                show_custom_dialog: false,
                show_danger_dialog: false,
                delete_count: 0,
            },
            Task::none(),
        )
    }

    fn theme(&self) -> Theme {
        Theme::Dracula
    }
}

#[derive(Debug, Clone)]
enum Message {
    // Dialog triggers
    ShowSimpleDialog,
    ShowCustomDialog,
    ShowDangerDialog,
    // Simple dialog actions
    SimpleConfirm,
    SimpleCancel,
    // Custom dialog actions
    CustomOk,
    CustomMaybe,
    CustomCancel,
    // Danger dialog actions
    DangerDelete,
    DangerCancel,
    // Dismiss
    DismissDialog,
}

impl App {
    fn update(&mut self, message: Message) {
        match message {
            Message::ShowSimpleDialog => {
                self.show_simple_dialog = true;
            }
            Message::ShowCustomDialog => {
                self.show_custom_dialog = true;
            }
            Message::ShowDangerDialog => {
                self.show_danger_dialog = true;
            }
            Message::SimpleConfirm => {
                self.show_simple_dialog = false;
                self.log_entry("Simple dialog: Confirmed");
            }
            Message::SimpleCancel => {
                self.show_simple_dialog = false;
                self.log_entry("Simple dialog: Cancelled");
            }
            Message::CustomOk => {
                self.show_custom_dialog = false;
                self.log_entry("Custom dialog: OK pressed");
            }
            Message::CustomMaybe => {
                self.show_custom_dialog = false;
                self.log_entry("Custom dialog: Maybe pressed");
            }
            Message::CustomCancel => {
                self.show_custom_dialog = false;
                self.log_entry("Custom dialog: Cancel pressed");
            }
            Message::DangerDelete => {
                self.show_danger_dialog = false;
                self.delete_count += 1;
                self.log_entry(format!(
                    "Danger dialog: Deleted (total: {})",
                    self.delete_count
                ));
            }
            Message::DangerCancel => {
                self.show_danger_dialog = false;
                self.log_entry("Danger dialog: Cancelled");
            }
            Message::DismissDialog => {
                self.show_simple_dialog = false;
                self.show_custom_dialog = false;
                self.show_danger_dialog = false;
                self.log_entry("Dialog dismissed (click outside / Escape)");
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
        // === Left panel: trigger buttons ===
        let trigger_panel = container(
            column![
                text("Confirmation Dialog Test").size(18),
                rule::horizontal(1),
                text("Click buttons to open dialogs.").size(13),
                space::vertical().height(8),
                button(text("Simple Dialog"))
                    .on_press(Message::ShowSimpleDialog)
                    .width(Length::Fill),
                button(text("Custom Buttons"))
                    .on_press(Message::ShowCustomDialog)
                    .width(Length::Fill),
                button(text("Danger Dialog (blocking)"))
                    .on_press(Message::ShowDangerDialog)
                    .width(Length::Fill),
                space::vertical().height(8),
                text(format!("Deletes: {}", self.delete_count)).size(12),
            ]
            .spacing(8)
            .padding(20),
        )
        .width(220)
        .height(Length::Fill);

        // === Center: main content area ===
        let main_content = container(
            column![
                text("Main Content Area").size(16),
                rule::horizontal(1),
                text("This content is visible underneath the dialog backdrop.").size(13),
                text("Click outside the dialog or press Escape to dismiss.").size(12),
            ]
            .spacing(8)
            .padding(20),
        )
        .width(Length::Fill)
        .height(Length::Fill);

        // Wrap main content with dialogs
        let content_with_dialogs = ConfirmationDialog::new(
            main_content,
            self.show_simple_dialog,
            "Confirm Action",
            "Do you want to proceed with this action?",
        )
        .on_confirm(Message::SimpleConfirm)
        .on_cancel(Message::SimpleCancel)
        .on_dismiss(Message::DismissDialog);

        let content_with_dialogs = ConfirmationDialog::new(
            content_with_dialogs,
            self.show_custom_dialog,
            "Choose an Option",
            "Select what you want to do next. You have three choices.",
        )
        .button(DialogButton::new("OK", Message::CustomOk))
        .button(DialogButton::new("Maybe", Message::CustomMaybe))
        .button(DialogButton::new("Cancel", Message::CustomCancel))
        .on_dismiss(Message::DismissDialog);

        let content_with_dialogs = ConfirmationDialog::new(
            content_with_dialogs,
            self.show_danger_dialog,
            "Delete Item?",
            "This action cannot be undone. The item will be permanently removed.",
        )
        .button(
            DialogButton::new("Delete", Message::DangerDelete)
                .style(
                    neverliie_iced_widgets::confirmation_dialog::ButtonStyle::Danger,
                ),
        )
        .on_cancel(Message::DangerCancel)
        .on_dismiss(Message::DismissDialog)
        .blocking();

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

        iced::widget::row![trigger_panel, content_with_dialogs, log_panel]
            .spacing(8)
            .padding(8)
            .height(Length::Fill)
            .into()
    }
}
