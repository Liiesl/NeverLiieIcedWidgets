use iced::widget::{button, column, container, rule, scrollable, space, text};
use iced::window::{self, screenshot::Screenshot};
use iced::{Border, Color, Element, Length, Shadow, Task, Theme, Vector};

use neverliie_iced_widgets::color_picker::{
    Gradient, PickedValue, floating_color_picker_with_change, ColorPicker, DropperBuffer,
    FloatingColorPicker,
};
use neverliie_iced_widgets::overlay::{Anchor, Position};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .theme(App::theme)
        .run()
}

struct App {
    log: Vec<String>,
    // === Inline mode: the widget is planted directly into the layout ===
    inline_color: Color,
    inline_live: PickedValue,
    // === Floating modes: a button spawns a draggable window-like dialog ===
    builder_color: Color,
    builder_live: PickedValue,
    helper_color: Color,
    helper_live: PickedValue,
    position_color: Color,
    position_live: PickedValue,
    show_builder_picker: bool,
    show_helper_picker: bool,
    show_position_picker: bool,
    position_choice: PositionChoice,
    // === Eye dropper: shared buffer the app deposits window screenshots in ===
    dropper_buffer: DropperBuffer,
}

fn demo_gradient() -> Gradient {
    Gradient::two(
        Color::from_rgb(0.9, 0.3, 0.6),
        Color::from_rgb(0.3, 0.6, 0.9),
    )
}

/// Seed solid color prop from the live picked value.
fn live_color(picked: &PickedValue) -> Color {
    match picked {
        PickedValue::Solid(color) => *color,
        PickedValue::Gradient(gradient) => gradient.stop(0).map_or(Color::BLACK, |s| s.color),
    }
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                log: vec!["Pick a color or gradient to change each panel's preview swatch.".into()],
                inline_color: Color::from_rgb(0.9, 0.3, 0.6),
                inline_live: PickedValue::Gradient(demo_gradient()),
                builder_color: Color::from_rgb(1.0, 0.55, 0.0),
                builder_live: PickedValue::Solid(Color::from_rgb(1.0, 0.55, 0.0)),
                helper_color: Color::from_rgb(0.3, 0.6, 0.9),
                helper_live: PickedValue::Solid(Color::from_rgb(0.3, 0.6, 0.9)),
                position_color: Color::from_rgb(0.25, 0.8, 0.35),
                position_live: PickedValue::Solid(Color::from_rgb(0.25, 0.8, 0.35)),
                show_builder_picker: false,
                show_helper_picker: false,
                show_position_picker: false,
                position_choice: PositionChoice::BottomRight,
                dropper_buffer: DropperBuffer::new(),
            },
            Task::none(),
        )
    }

    fn theme(&self) -> Theme {
        Theme::Dracula
    }
}

/// The initial position strategies demonstrated by the "Position API" panel.
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
    // Inline mode
    InlineCancel,
    InlineSubmit(Color),
    InlineColorChanged(Color),
    InlineGradientChanged(Gradient),
    InlineGradientSubmit(Gradient),
    InlinePick(PickedValue),
    InlinePickSubmit(PickedValue),
    // Floating modes
    OpenBuilder,
    OpenHelper,
    BuilderCancel,
    BuilderSubmit(Color),
    HelperCancel,
    HelperSubmit(Color),
    BuilderColorChanged(Color),
    HelperColorChanged(Color),
    BuilderGradientChanged(Gradient),
    HelperGradientChanged(Gradient),
    BuilderPick(PickedValue),
    HelperPick(PickedValue),
    BuilderPickSubmit(PickedValue),
    HelperPickSubmit(PickedValue),
    SelectPosition(PositionChoice),
    OpenPosition,
    PositionCancel,
    PositionSubmit(Color),
    PositionColorChanged(Color),
    PositionGradientChanged(Gradient),
    PositionPick(PickedValue),
    PositionPickSubmit(PickedValue),
    // Eye dropper capture round-trip
    DropperCapture,
    DropperShot(Screenshot),
}

impl App {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::InlineCancel => {
                self.log_entry("Inline picker cancelled (kept live value)");
            }
            Message::InlineSubmit(color) => {
                self.inline_color = color;
                self.inline_live = PickedValue::Solid(color);
                self.log_entry(format!("Inline picker submitted: {}", hex_picked(&self.inline_live)));
            }
            Message::InlineColorChanged(color) => {
                self.inline_live = PickedValue::Solid(color);
            }
            Message::InlineGradientChanged(gradient) => {
                self.inline_live = PickedValue::Gradient(gradient);
            }
            Message::InlineGradientSubmit(gradient) => {
                self.inline_live = PickedValue::Gradient(gradient);
                self.log_entry(format!("Inline gradient submitted: {}", hex_picked(&self.inline_live)));
            }
            Message::InlinePick(picked) => {
                self.inline_live = picked;
            }
            Message::InlinePickSubmit(picked) => {
                self.inline_live = picked.clone();
                if let Some(color) = picked.as_solid() {
                    self.inline_color = color;
                }
                self.log_entry(format!("Inline pick submitted: {}", hex_picked(&self.inline_live)));
            }
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
                self.builder_live = PickedValue::Solid(color);
                self.log_entry(format!("Builder picker submitted: {}", hex_picked(&self.builder_live)));
            }
            Message::HelperCancel => {
                self.show_helper_picker = false;
                self.log_entry("Helper picker cancelled");
            }
            Message::HelperSubmit(color) => {
                self.show_helper_picker = false;
                self.helper_color = color;
                self.helper_live = PickedValue::Solid(color);
                self.log_entry(format!("Helper picker submitted: {}", hex_picked(&self.helper_live)));
            }
            Message::BuilderColorChanged(color) => {
                self.builder_live = PickedValue::Solid(color);
            }
            Message::HelperColorChanged(color) => {
                self.helper_live = PickedValue::Solid(color);
            }
            Message::BuilderGradientChanged(gradient) => {
                self.builder_live = PickedValue::Gradient(gradient);
            }
            Message::HelperGradientChanged(gradient) => {
                self.helper_live = PickedValue::Gradient(gradient);
            }
            Message::BuilderPick(picked) => {
                self.builder_live = picked;
            }
            Message::HelperPick(picked) => {
                self.helper_live = picked;
            }
            Message::BuilderPickSubmit(picked) => {
                self.show_builder_picker = false;
                if let Some(color) = picked.as_solid() {
                    self.builder_color = color;
                }
                self.builder_live = picked.clone();
                self.log_entry(format!("Builder pick submitted: {}", hex_picked(&self.builder_live)));
            }
            Message::HelperPickSubmit(picked) => {
                self.show_helper_picker = false;
                if let Some(color) = picked.as_solid() {
                    self.helper_color = color;
                }
                self.helper_live = picked.clone();
                self.log_entry(format!("Helper pick submitted: {}", hex_picked(&self.helper_live)));
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
                self.position_live = PickedValue::Solid(color);
                self.log_entry(format!("Position picker submitted: {}", hex_picked(&self.position_live)));
            }
            Message::PositionColorChanged(color) => {
                self.position_live = PickedValue::Solid(color);
            }
            Message::PositionGradientChanged(gradient) => {
                self.position_live = PickedValue::Gradient(gradient);
            }
            Message::PositionPick(picked) => {
                self.position_live = picked;
            }
            Message::PositionPickSubmit(picked) => {
                self.show_position_picker = false;
                if let Some(color) = picked.as_solid() {
                    self.position_color = color;
                }
                self.position_live = picked.clone();
                self.log_entry(format!("Position pick submitted: {}", hex_picked(&self.position_live)));
            }
            // The picker requested a fresh window snapshot for the eye
            // dropper: capture the window and hand it to the shared buffer.
            Message::DropperCapture => {
                self.log_entry("Eye dropper capturing window...");
                return window::latest().and_then(window::screenshot).map(Message::DropperShot);
            }
            Message::DropperShot(screenshot) => {
                self.dropper_buffer.store(&screenshot);
            }
        }

        Task::none()
    }

    fn log_entry(&mut self, msg: impl Into<String>) {
        self.log.push(msg.into());
        if self.log.len() > 30 {
            self.log.remove(0);
        }
    }

    fn view(&self) -> Element<'_, Message> {
        // === Inline panel: generic widget planted like any other ===
        // Seed from the live picked value so Original matches the app's
        // current set value (solid or gradient).
        let inline_picker = ColorPicker::new(
            live_color(&self.inline_live),
            Message::InlineCancel,
            Message::InlineSubmit,
        )
        .gradient(self.inline_live.as_gradient())
        .on_color_change(Message::InlineColorChanged)
        .on_gradient_change(Message::InlineGradientChanged)
        .on_gradient_submit(Message::InlineGradientSubmit)
        .on_pick(Message::InlinePick)
        .on_pick_submit(Message::InlinePickSubmit)
        .dropper_buffer(self.dropper_buffer.clone())
        .on_dropper_capture(|| Message::DropperCapture);

        let inline_panel = container(
            column![
                text("Inline Widget").size(18),
                rule::horizontal(1),
                text(
                    "ColorPicker::new(...) planted in the layout - \
                     no button or spawn flag needed."
                )
                .size(12),
                space::vertical().height(8),
                self.swatch(self.inline_live.clone()),
                text(hex_picked(&self.inline_live)).size(13),
                space::vertical().height(8),
                scrollable(inline_picker)
                .width(Length::Fill)
                .height(Length::Fill),
            ]
            .spacing(8)
            .padding(20),
        )
        .width(600)
        .height(Length::Fill);

        // === Position API panel (floating) ===
        let position_picker = FloatingColorPicker::new(
            self.show_position_picker,
            live_color(&self.position_live),
            self.pick_button(
                "Position API",
                "Open picker",
                Message::OpenPosition,
            ),
            Message::PositionCancel,
            Message::PositionSubmit,
        )
        .gradient(self.position_live.as_gradient())
        .on_color_change(Message::PositionColorChanged)
        .on_gradient_change(Message::PositionGradientChanged)
        .on_pick(Message::PositionPick)
        .on_pick_submit(Message::PositionPickSubmit)
        .position(self.position_choice.position())
        .dropper_buffer(self.dropper_buffer.clone())
        .on_dropper_capture(|| Message::DropperCapture);

        let choice_buttons = column![
            text("Initial position:").size(12),
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
                text("FloatingColorPicker::new(...) and .position(Position::...); drag by the header afterwards.").size(12),
                space::vertical().height(8),
                self.swatch(self.position_live.clone()),
                text(hex_picked(&self.position_live)).size(13),
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

        // === Builder panel: builder API (`FloatingColorPicker::new`) ===
        let builder_picker = FloatingColorPicker::new(
            self.show_builder_picker,
            live_color(&self.builder_live),
            self.pick_button("Builder API", "Open builder picker", Message::OpenBuilder),
            Message::BuilderCancel,
            Message::BuilderSubmit,
        )
        .gradient(self.builder_live.as_gradient())
        .on_color_change(Message::BuilderColorChanged)
        .on_gradient_change(Message::BuilderGradientChanged)
        .on_pick(Message::BuilderPick)
        .on_pick_submit(Message::BuilderPickSubmit)
        .dropper_buffer(self.dropper_buffer.clone())
        .on_dropper_capture(|| Message::DropperCapture);

        let builder_panel = container(
            column![
                text("Builder API").size(18),
                rule::horizontal(1),
                text("FloatingColorPicker::new(...) spawns a draggable window-style dialog.").size(12),
                space::vertical().height(8),
                self.swatch(self.builder_live.clone()),
                text(hex_picked(&self.builder_live)).size(13),
                space::vertical().height(8),
                builder_picker,
            ]
            .spacing(8)
            .padding(20),
        )
        .width(220)
        .height(Length::Fill);

        // === Center panel: shortcut helper API ===
        let helper_picker = floating_color_picker_with_change(
            self.show_helper_picker,
            live_color(&self.helper_live),
            self.pick_button("Helper API", "Open helper picker", Message::OpenHelper),
            Message::HelperCancel,
            Message::HelperSubmit,
            Message::HelperColorChanged,
        )
        .gradient(self.helper_live.as_gradient())
        .on_gradient_change(Message::HelperGradientChanged)
        .on_pick(Message::HelperPick)
        .on_pick_submit(Message::HelperPickSubmit)
        .dropper_buffer(self.dropper_buffer.clone())
        .on_dropper_capture(|| Message::DropperCapture);

        let helper_panel = container(
            column![
                text("Helper API").size(18),
                rule::horizontal(1),
                text("floating_color_picker_with_change(...) shortcut, same live preview.").size(12),
                space::vertical().height(8),
                self.swatch(self.helper_live.clone()),
                text(hex_picked(&self.helper_live)).size(13),
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

        iced::widget::row![inline_panel, position_panel, builder_panel, helper_panel, log_panel]
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

    fn swatch(&self, picked: PickedValue) -> Element<'_, Message> {
        container(text(""))
            .width(Length::Fill)
            .height(40)
            .style(move |_theme: &Theme| container::Style {
                background: Some(picked.to_background()),
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

fn hex_picked(picked: &PickedValue) -> String {
    match picked {
        PickedValue::Solid(color) => {
            let [r, g, b, _] = color.into_rgba8();
            format!("#{r:02X}{g:02X}{b:02X}")
        }
        PickedValue::Gradient(gradient) => {
            let parts = gradient
                .stops
                .iter()
                .map(|stop| {
                    let [r, g, b, _] = stop.color.into_rgba8();
                    format!("#{r:02X}{g:02X}{b:02X}@{:.2}", stop.offset)
                })
                .collect::<Vec<_>>()
                .join(" -> ");
            format!("{parts}")
        }
    }
}
