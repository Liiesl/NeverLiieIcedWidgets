use iced::widget::{button, column, container, row, rule, scrollable, text, text_input};
use iced::{Element, Length, Task, Theme};

use neverliie_iced_widgets::advanced_dropdown::{advanced_dropdown, Item, MenuItem};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .theme(App::theme)
        .run()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Fruit {
    Apple,
    Banana,
    Cherry,
    Grape,
    Kiwi,
    Mango,
    Orange,
    Peach,
    Pear,
    Plum,
    Raspberry,
    Strawberry,
    Watermelon,
    Custom(String),
}

impl ToString for Fruit {
    fn to_string(&self) -> String {
        match self {
            Fruit::Apple => "Apple",
            Fruit::Banana => "Banana",
            Fruit::Cherry => "Cherry",
            Fruit::Grape => "Grape",
            Fruit::Kiwi => "Kiwi",
            Fruit::Mango => "Mango",
            Fruit::Orange => "Orange",
            Fruit::Peach => "Peach",
            Fruit::Pear => "Pear",
            Fruit::Plum => "Plum",
            Fruit::Raspberry => "Raspberry",
            Fruit::Strawberry => "Strawberry",
            Fruit::Watermelon => "Watermelon",
            Fruit::Custom(name) => name.as_str(),
        }
        .to_string()
    }
}

impl Fruit {
    fn icon(&self) -> &'static str {
        match self {
            Fruit::Apple => "\u{1f34e}",
            Fruit::Banana => "\u{1f34c}",
            Fruit::Cherry => "\u{1f352}",
            Fruit::Grape => "\u{1f347}",
            Fruit::Kiwi => "\u{1f95d}",
            Fruit::Mango => "\u{1f96d}",
            Fruit::Orange => "\u{1f34a}",
            Fruit::Peach => "\u{1f351}",
            Fruit::Pear => "\u{1f350}",
            Fruit::Plum => "\u{1f355}",
            Fruit::Raspberry => "\u{1f353}",
            Fruit::Strawberry => "\u{1f353}",
            Fruit::Watermelon => "\u{1f349}",
            Fruit::Custom(_) => "\u{1f34f}",
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Selected(Fruit),
    ToggleSearch(bool),
    ClearLog,
    NewItemPressed,
    NewItemNameChanged(String),
    NewItemSubmitted,
}

struct App {
    selected: Option<Fruit>,
    search_enabled: bool,
    log: Vec<String>,
    creating: bool,
    new_item_name: String,
    custom_fruits: Vec<Fruit>,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                selected: Some(Fruit::Apple),
                search_enabled: true,
                log: vec!["Pick a fruit from the dropdowns.".to_string()],
                creating: false,
                new_item_name: String::new(),
                custom_fruits: Vec::new(),
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
            Message::Selected(fruit) => {
                self.selected = Some(fruit.clone());
                self.log(format!("Selected: {} {}", fruit.icon(), fruit.to_string()));
            }
            Message::ToggleSearch(enabled) => {
                self.search_enabled = enabled;
            }
            Message::ClearLog => {
                self.log.clear();
            }
            Message::NewItemPressed => {
                self.creating = true;
                self.new_item_name.clear();
                self.log("New item panel opened.");
            }
            Message::NewItemNameChanged(name) => {
                self.new_item_name = name;
            }
            Message::NewItemSubmitted => {
                let name = self.new_item_name.trim().to_string();

                if !name.is_empty() {
                    let fruit = Fruit::Custom(name);
                    self.custom_fruits.push(fruit.clone());
                    self.selected = Some(fruit.clone());
                    self.log(format!("Created: {} {}", fruit.icon(), fruit.to_string()));
                }

                self.creating = false;
            }
        }
    }

    fn entries(&self) -> Vec<MenuItem<'static, Fruit, Message, Theme, iced::Renderer>> {
        use iced::widget::text;

        let mut entries = vec![
            MenuItem::Label("Basic Fruits"),
            MenuItem::Item(
                Item::new(Fruit::Apple, "Apple").icon(text(Fruit::Apple.icon()).size(14)),
            ),
            MenuItem::Item(
                Item::new(Fruit::Banana, "Banana").icon(text(Fruit::Banana.icon()).size(14)),
            ),
            MenuItem::Item(
                Item::new(Fruit::Cherry, "Cherry").icon(text(Fruit::Cherry.icon()).size(14)),
            ),
            MenuItem::Separator,
            MenuItem::Label("Citrus"),
            MenuItem::Item(
                Item::new(Fruit::Orange, "Orange").icon(text(Fruit::Orange.icon()).size(14)),
            ),
            MenuItem::Item(
                Item::new(Fruit::Grape, "Grape").icon(text(Fruit::Grape.icon()).size(14)),
            ),
            MenuItem::Separator,
            MenuItem::Label("Berries"),
            MenuItem::Item(
                Item::new(Fruit::Raspberry, "Raspberry")
                    .icon(text(Fruit::Raspberry.icon()).size(14)),
            ),
            MenuItem::Item(
                Item::new(Fruit::Strawberry, "Strawberry")
                    .icon(text(Fruit::Strawberry.icon()).size(14)),
            ),
            MenuItem::Separator,
            MenuItem::Label("Tropical"),
            MenuItem::Item(
                Item::new(Fruit::Mango, "Mango").icon(text(Fruit::Mango.icon()).size(14)),
            ),
            MenuItem::Item(Item::new(Fruit::Kiwi, "Kiwi").icon(text(Fruit::Kiwi.icon()).size(14))),
            MenuItem::Item(
                Item::new(Fruit::Peach, "Peach").icon(text(Fruit::Peach.icon()).size(14)),
            ),
            MenuItem::Item(Item::new(Fruit::Plum, "Plum").icon(text(Fruit::Plum.icon()).size(14))),
            MenuItem::Item(Item::new(Fruit::Pear, "Pear").icon(text(Fruit::Pear.icon()).size(14))),
            MenuItem::Item(
                Item::new(Fruit::Watermelon, "Watermelon")
                    .icon(text(Fruit::Watermelon.icon()).size(14)),
            ),
        ];

        if !self.custom_fruits.is_empty() {
            entries.push(MenuItem::Separator);
            entries.push(MenuItem::Label("Custom"));

            for fruit in &self.custom_fruits {
                entries.push(MenuItem::Item(
                    Item::new(fruit.clone(), fruit.to_string()).icon(text(fruit.icon()).size(14)),
                ));
            }
        }

        entries
    }

    fn view(&self) -> Element<'_, Message> {
        let searchable =
            advanced_dropdown(self.entries(), self.selected.clone(), Message::Selected)
                .searchable(self.search_enabled)
                .placeholder("Pick a fruit...")
                .width(220)
                .on_new_item(Message::NewItemPressed)
                .new_item_label("+ New Item")
                .new_item_icon(text("➕").size(14));

        let plain = advanced_dropdown(self.entries(), self.selected.clone(), Message::Selected)
            .placeholder("No search here")
            .width(220);

        let selected_text = self
            .selected
            .as_ref()
            .map(|f| format!("{} {}", f.icon(), f.to_string()))
            .unwrap_or_else(|| "Nothing selected".to_string());

        let new_item_panel: Element<'_, Message> = if self.creating {
            container(
                column![
                    text("Create a new item").size(13),
                    row![
                        text_input("Fruit name...", &self.new_item_name)
                            .on_input(Message::NewItemNameChanged)
                            .width(Length::Fill),
                        button("Add").on_press(Message::NewItemSubmitted),
                    ]
                    .spacing(4),
                ]
                .spacing(4),
            )
            .padding(8)
            .style(container::bordered_box)
            .width(220)
            .into()
        } else {
            column![].into()
        };

        let controls = column![
            text("Advanced Dropdown").size(18),
            rule::horizontal(1),
            text("Searchable:").size(13),
            iced::widget::checkbox(self.search_enabled)
                .label("Enable search / filtering")
                .on_toggle(Message::ToggleSearch),
            text("").height(8),
            text("Searchable dropdown:").size(13),
            container(searchable).padding(4),
            text("").height(8),
            text("Plain dropdown (no search):").size(13),
            container(plain).padding(4),
            text("").height(8),
            new_item_panel,
            text("").height(8),
            text(format!("Current selection: {selected_text}")).size(13),
            button("Clear log").on_press(Message::ClearLog),
        ]
        .spacing(4)
        .padding(20)
        .width(300);

        let log_entries = self
            .log
            .iter()
            .enumerate()
            .fold(column![].spacing(2), |col, (i, entry)| {
                col.push(text(format!("{}: {}", i + 1, entry)).size(11))
            });

        let log_panel = container(
            column![
                text("Event Log").size(14),
                rule::horizontal(1),
                scrollable(log_entries),
            ]
            .spacing(4)
            .padding(12),
        )
        .width(300)
        .height(Length::Fill);

        row![controls, log_panel]
            .spacing(8)
            .padding(8)
            .height(Length::Fill)
            .into()
    }
}
