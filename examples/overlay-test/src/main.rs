use iced::widget::{button, column, container, rule, scrollable, space, text};
use iced::{Element, Length};

use neverliie_iced_widgets::overlay::{Anchor, Floating, OverlayManager, Position};

fn main() -> iced::Result {
    iced::run(App::update, App::view)
}

#[derive(Default)]
struct App {
    active: Option<PositionId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PositionId {
    Absolute,
    ParentTopLeft,
    ParentTop,
    ParentTopRight,
    ParentLeft,
    ParentCenter,
    ParentRight,
    ParentBottomLeft,
    ParentBottom,
    ParentBottomRight,
    ViewportTopLeft,
    ViewportTop,
    ViewportTopRight,
    ViewportLeft,
    ViewportCenter,
    ViewportRight,
    ViewportBottomLeft,
    ViewportBottom,
    ViewportBottomRight,
    Cursor,
}

#[derive(Debug, Clone)]
enum Message {
    Toggle(PositionId),
    Dismiss,
}

fn popup_style(_: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(iced::Color::from_rgb(0.12, 0.12, 0.18).into()),
        border: iced::Border {
            color: iced::Color::from_rgb(0.35, 0.35, 0.5),
            width: 1.0,
            ..Default::default()
        }
        .rounded(8),
        text_color: Some(iced::Color::WHITE),
        ..Default::default()
    }
}

fn popup<'a, Message: 'a>(
    label: &'a str,
) -> container::Container<'a, Message> {
    container(text(label).size(13))
        .padding([8, 14])
        .style(popup_style)
}

fn parent_box_style(_: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(iced::Color::from_rgb(0.05, 0.08, 0.12).into()),
        border: iced::Border {
            color: iced::Color::from_rgb(0.2, 0.35, 0.5),
            width: 2.0,
            ..Default::default()
        }
        .rounded(4),
        text_color: Some(iced::Color::from_rgb(0.4, 0.6, 0.8)),
        ..Default::default()
    }
}

impl App {
    fn update(&mut self, message: Message) {
        match message {
            Message::Toggle(id) => {
                if self.active == Some(id) {
                    self.active = None;
                } else {
                    self.active = Some(id);
                }
            }
            Message::Dismiss => {
                self.active = None;
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let anchor_label = |a: Anchor| match a {
            Anchor::TopLeft => "TopLeft",
            Anchor::Top => "Top",
            Anchor::TopRight => "TopRight",
            Anchor::Left => "Left",
            Anchor::Center => "Center",
            Anchor::Right => "Right",
            Anchor::BottomLeft => "BottomLeft",
            Anchor::Bottom => "Bottom",
            Anchor::BottomRight => "BottomRight",
        };

        let mut parent_buttons = column![].spacing(3);
        for (id, anchor) in [
            (PositionId::ParentTopLeft, Anchor::TopLeft),
            (PositionId::ParentTop, Anchor::Top),
            (PositionId::ParentTopRight, Anchor::TopRight),
            (PositionId::ParentLeft, Anchor::Left),
            (PositionId::ParentCenter, Anchor::Center),
            (PositionId::ParentRight, Anchor::Right),
            (PositionId::ParentBottomLeft, Anchor::BottomLeft),
            (PositionId::ParentBottom, Anchor::Bottom),
            (PositionId::ParentBottomRight, Anchor::BottomRight),
        ] {
            parent_buttons = parent_buttons.push(
                button(text(format!("Parent::{}", anchor_label(anchor))))
                    .on_press(Message::Toggle(id))
                    .width(Length::Fill),
            );
        }

        let mut viewport_buttons = column![].spacing(3);
        for (id, anchor) in [
            (PositionId::ViewportTopLeft, Anchor::TopLeft),
            (PositionId::ViewportTop, Anchor::Top),
            (PositionId::ViewportTopRight, Anchor::TopRight),
            (PositionId::ViewportLeft, Anchor::Left),
            (PositionId::ViewportCenter, Anchor::Center),
            (PositionId::ViewportRight, Anchor::Right),
            (PositionId::ViewportBottomLeft, Anchor::BottomLeft),
            (PositionId::ViewportBottom, Anchor::Bottom),
            (PositionId::ViewportBottomRight, Anchor::BottomRight),
        ] {
            viewport_buttons = viewport_buttons.push(
                button(text(format!("Viewport::{}", anchor_label(anchor))))
                    .on_press(Message::Toggle(id))
                    .width(Length::Fill),
            );
        }

        let sidebar = scrollable(
            column![
                text("Overlay Position Test").size(20),
                rule::horizontal(1),
                text("Parent Anchors").size(14),
                parent_buttons,
                space::vertical().height(8),
                text("Viewport Anchors").size(14),
                viewport_buttons,
                space::vertical().height(8),
                text("Other Modes").size(14),
                button(text("Absolute (300, 200)"))
                    .on_press(Message::Toggle(PositionId::Absolute))
                    .width(Length::Fill),
                button(text("Cursor Follow"))
                    .on_press(Message::Toggle(PositionId::Cursor))
                    .width(Length::Fill),
            ]
            .spacing(5)
            .padding(16),
        )
        .height(Length::Fill);

        let parent_content = container(
            column![
                space::vertical().height(10),
                text("  PARENT WIDGET").size(11),
                text("  (anchors resolve here)").size(10),
            ]
            .spacing(2),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(10)
        .style(parent_box_style);

        let mut manager = OverlayManager::new(parent_content);

        if let Some(id) = self.active {
            let (pos, label) = match id {
                PositionId::Absolute => (
                    Position::absolute(300.0, 200.0),
                    "Absolute (300, 200)",
                ),
                PositionId::ParentTopLeft => (Position::TopLeft, "Parent::TopLeft"),
                PositionId::ParentTop => (Position::Top, "Parent::Top"),
                PositionId::ParentTopRight => (Position::TopRight, "Parent::TopRight"),
                PositionId::ParentLeft => (Position::Left, "Parent::Left"),
                PositionId::ParentCenter => (Position::Center, "Parent::Center"),
                PositionId::ParentRight => (Position::Right, "Parent::Right"),
                PositionId::ParentBottomLeft => (Position::BottomLeft, "Parent::BottomLeft"),
                PositionId::ParentBottom => (Position::Bottom, "Parent::Bottom"),
                PositionId::ParentBottomRight => (Position::BottomRight, "Parent::BottomRight"),
                PositionId::ViewportTopLeft => (Position::ViewportTopLeft, "Viewport::TopLeft"),
                PositionId::ViewportTop => (Position::ViewportTop, "Viewport::Top"),
                PositionId::ViewportTopRight => (Position::ViewportTopRight, "Viewport::TopRight"),
                PositionId::ViewportLeft => (Position::ViewportLeft, "Viewport::Left"),
                PositionId::ViewportCenter => (Position::ViewportCenter, "Viewport::Center"),
                PositionId::ViewportRight => (Position::ViewportRight, "Viewport::Right"),
                PositionId::ViewportBottomLeft => (Position::ViewportBottomLeft, "Viewport::BottomLeft"),
                PositionId::ViewportBottom => (Position::ViewportBottom, "Viewport::Bottom"),
                PositionId::ViewportBottomRight => (Position::ViewportBottomRight, "Viewport::BottomRight"),
                PositionId::Cursor => (Position::FollowCursor, "Cursor Follow"),
            };

            manager = manager.overlay(Floating::new(popup(label)).position(pos));
        }

        iced::widget::row![sidebar, manager]
            .spacing(0)
            .height(Length::Fill)
            .into()
    }
}
