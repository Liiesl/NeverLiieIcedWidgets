use iced::widget::{column, container, rule, scrollable, text};
use iced::{Element, Length, Task, Theme};

use neverliie_iced_widgets::context_menu::{ContextMenu, Menu};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .theme(App::theme)
        .run()
}

struct App {
    log: Vec<String>,
    counters: [i32; 4],
    stress_labels: Vec<String>,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (Self::default(), Task::none())
    }

    fn theme(&self) -> Theme {
        Theme::Dracula
    }
}

#[derive(Debug, Clone)]
enum Message {
    // Free area actions
    Copy,
    Paste,
    Cut,
    SelectAll,
    // Button actions
    ButtonAction(usize, &'static str),
    // Edit actions
    Undo,
    // File actions
    Open,
    Save,
    SaveAs,
    Exit,
    // Encoding submenu
    EncodingUtf8,
    EncodingUtf16Le,
    EncodingUtf16Be,
    ConvertToUtf8,
    ConvertToLf,
    // Line Endings submenu
    LineEndingLf,
    LineEndingCrlf,
    LineEndingCr,
    // Tab Width submenu
    TabWidth1,
    TabWidth2,
    TabWidth4,
    TabWidth8,
    // Indentation submenu
    IndentIncrease,
    IndentDecrease,
    // Stress test
    Stress(usize),
    // Misc
    Duplicate,
    Delete,
    Dismiss,
}

impl Default for App {
    fn default() -> Self {
        Self {
            log: vec!["Right-click anywhere to open context menus.".into()],
            counters: [0; 4],
            stress_labels: (0..20)
                .map(|i| format!("Stress Item {i:02}"))
                .collect(),
        }
    }
}

impl App {
    fn update(&mut self, message: Message) {
        let entry = match message {
            Message::Copy => "Free area: Copy".into(),
            Message::Paste => "Free area: Paste".into(),
            Message::Cut => "Free area: Cut".into(),
            Message::SelectAll => "Free area: Select All".into(),
            Message::Undo => "Free area: Undo".into(),
            Message::Duplicate => "Free area: Duplicate".into(),
            Message::Delete => "Free area: Delete".into(),
            Message::Open => "Free area: Open".into(),
            Message::Save => "Free area: Save".into(),
            Message::SaveAs => "Free area: Save As".into(),
            Message::Exit => "Free area: Exit".into(),
            Message::EncodingUtf8 => "Encoding: UTF-8".into(),
            Message::EncodingUtf16Le => "Encoding: UTF-16 LE".into(),
            Message::EncodingUtf16Be => "Encoding: UTF-16 BE".into(),
            Message::ConvertToUtf8 => "Encoding: Convert to UTF-8".into(),
            Message::ConvertToLf => "Encoding: Convert to LF".into(),
            Message::LineEndingLf => "Line Endings: LF (Unix)".into(),
            Message::LineEndingCrlf => "Line Endings: CRLF (Windows)".into(),
            Message::LineEndingCr => "Line Endings: CR (Old Mac)".into(),
            Message::TabWidth1 => "Tab Width: 1".into(),
            Message::TabWidth2 => "Tab Width: 2".into(),
            Message::TabWidth4 => "Tab Width: 4".into(),
            Message::TabWidth8 => "Tab Width: 8".into(),
            Message::IndentIncrease => "Indentation: Increase Indent".into(),
            Message::IndentDecrease => "Indentation: Decrease Indent".into(),
            Message::Stress(i) => format!(
                "Stress: {}",
                self.stress_labels.get(i).map_or("?", String::as_str)
            ),
            Message::ButtonAction(idx, action) => {
                self.counters[idx] += 1;
                format!("Button {}: {} (clicked {}x)", idx + 1, action, self.counters[idx])
            }
            Message::Dismiss => "Menu dismissed".into(),
        };
        self.log.push(entry);
        if self.log.len() > 30 {
            self.log.remove(0);
        }
    }

    fn view(&self) -> Element<'_, Message> {
        // === Left panel: free area with context menu ===
        let free_area_content = container(
            column![
                text("Free Area").size(18),
                rule::horizontal(1),
                text("Right-click anywhere in this box.").size(13),
                text("Every submenu has 20 extra stress items appended.").size(11),
                text("File > Encoding, File > Line Endings, Edit > Indentation > Tab Width.").size(11),
                text("").height(Length::Fill),
            ]
            .spacing(8)
            .padding(20),
        )
        .width(Length::Fill)
        .height(Length::Fill);

        let free_area_menu = self.free_area_menu();
        let free_area = ContextMenu::new(free_area_content, free_area_menu)
            .on_dismiss(Message::Dismiss);

        // === Center panel: button list, each with its own context menu ===
        let button_items = column![
            text("Button List").size(18),
            rule::horizontal(1),
            text("Right-click each button for its own menu.").size(13),
            self.button_context(0, "Rename File", "View Details", "Pin to Top"),
            self.button_context(1, "Rename Image", "View Details", "Share"),
            self.button_context(2, "Rename Document", "Export as PDF", "Print"),
            self.button_context(3, "Rename Archive", "Extract Here", "Compress"),
        ]
        .spacing(6)
        .padding(16);

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

        iced::widget::row![
            free_area,
            button_items.width(280).height(Length::Fill),
            log_panel,
        ]
        .spacing(8)
        .padding(8)
        .height(Length::Fill)
        .into()
    }
}

impl App {
    /// Appends the 20 stress items to the given menu.
    fn stress_pad<'a>(
        &'a self,
        menu: Menu<'a, Message>,
    ) -> Menu<'a, Message> {
        let mut menu = menu;
        for (i, label) in self.stress_labels.iter().enumerate() {
            menu = menu.item(label.as_str(), Message::Stress(i));
        }
        menu
    }

    fn free_area_menu(&self) -> Menu<'_, Message> {
        let encoding_menu = self.stress_pad(
            Menu::new()
                .item("UTF-8", Message::EncodingUtf8)
                .item("UTF-16 LE", Message::EncodingUtf16Le)
                .item("UTF-16 BE", Message::EncodingUtf16Be)
                .separator()
                .item("Convert to UTF-8", Message::ConvertToUtf8)
                .item("Convert to LF", Message::ConvertToLf),
        );

        let line_endings_menu = self.stress_pad(
            Menu::new()
                .item("LF (Unix)", Message::LineEndingLf)
                .item("CRLF (Windows)", Message::LineEndingCrlf)
                .item("CR (Old Mac)", Message::LineEndingCr),
        );

        let file_menu = self.stress_pad(
            Menu::new()
                .item("Open", Message::Open)
                .shortcut("Ctrl+O")
                .separator()
                .item("Save", Message::Save)
                .shortcut("Ctrl+S")
                .item("Save As", Message::SaveAs)
                .shortcut("Ctrl+Shift+S")
                .separator()
                .submenu("Encoding", encoding_menu)
                .submenu("Line Endings", line_endings_menu)
                .separator()
                .item("Exit", Message::Exit)
                .shortcut("Alt+F4"),
        );

        let tab_width_menu = self.stress_pad(
            Menu::new()
                .item("1", Message::TabWidth1)
                .item("2", Message::TabWidth2)
                .item("4", Message::TabWidth4)
                .item("8", Message::TabWidth8),
        );

        let indent_menu = self.stress_pad(
            Menu::new()
                .item("Increase Indent", Message::IndentIncrease)
                .shortcut("Tab")
                .item("Decrease Indent", Message::IndentDecrease)
                .shortcut("Shift+Tab")
                .separator()
                .submenu("Tab Width", tab_width_menu),
        );

        let editor_menu = self.stress_pad(
            Menu::new()
                .item("Undo", Message::Undo)
                .shortcut("Ctrl+Z")
                .item_disabled("Redo")
                .shortcut("Ctrl+Y")
                .separator()
                .item("Cut", Message::Cut)
                .shortcut("Ctrl+X")
                .item("Copy", Message::Copy)
                .shortcut("Ctrl+C")
                .item("Paste", Message::Paste)
                .shortcut("Ctrl+V")
                .item_disabled("Paste Special")
                .shortcut("Ctrl+Shift+V")
                .separator()
                .item("Duplicate", Message::Duplicate)
                .shortcut("Ctrl+Shift+D")
                .item("Delete", Message::Delete)
                .shortcut("Del")
                .separator()
                .submenu("Indentation", indent_menu)
                .separator()
                .item("Select All", Message::SelectAll)
                .shortcut("Ctrl+A"),
        );

        Menu::new()
            .submenu("File", file_menu)
            .submenu("Edit", editor_menu)
    }

    fn button_context(
        &self,
        idx: usize,
        rename: &'static str,
        details: &'static str,
        extra: &'static str,
    ) -> Element<'_, Message> {
        let menu = Menu::new()
            .item(rename, Message::ButtonAction(idx, rename))
            .item("Duplicate", Message::ButtonAction(idx, "Duplicate"))
            .separator()
            .item(details, Message::ButtonAction(idx, details))
            .item(extra, Message::ButtonAction(idx, extra))
            .separator()
            .item_disabled("Move to Trash")
            .shortcut("Del");

        let content = container(
            text(format!("Item {}", idx + 1)).size(14),
        )
        .width(Length::Fill)
        .padding([10, 14]);

        ContextMenu::new(content, menu)
            .on_dismiss(Message::Dismiss)
            .into()
    }
}
