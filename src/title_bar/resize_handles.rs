//! Invisible resize handles for platforms without a native hit test.
//!
//! Windows resolves resizing inside `WM_NCHITTEST`, which gives it the native
//! cursors, the native double-click-to-maximize edge behavior and Aero Snap for
//! free. X11 and Wayland have no equivalent hook available through winit, so
//! the frame overlays eight thin, otherwise invisible regions that call
//! [`iced::window::drag_resize`] straight from the mouse press.
//!
//! The overlay only reacts within [`NativeFrameConfig::resize_border`] of each
//! edge. Everything in between is an inert [`iced::widget::space`], which
//! reports [`mouse::Interaction::None`] and therefore lets the layer below keep
//! both the cursor and the events.
//!
//! [`NativeFrameConfig::resize_border`]: super::config::NativeFrameConfig::resize_border

use iced::widget::{column, row, space};
use iced::{Element, Fill, Length, mouse, window};

use super::action::FrameAction;

/// Wraps `content` in a [`iced::widget::stack`] with the eight resize handles
/// layered on top.
pub(crate) fn overlay<'a, Message>(
    window_id: window::Id,
    thickness: f32,
    content: Element<'a, Message>,
    map_action: impl Fn(FrameAction) -> Message + Clone + 'a,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let handle = |direction: window::Direction| {
        let edge = Length::from(thickness);

        let (width, height): (Length, Length) = match direction {
            window::Direction::North | window::Direction::South => (Fill, edge),
            window::Direction::East | window::Direction::West => (edge, Fill),
            _ => (edge, edge),
        };

        let element: Element<'a, Message> =
            iced::widget::mouse_area(space().width(width).height(height))
                .on_press(map_action.clone()(FrameAction::Resize(
                    window_id, direction,
                )))
                .interaction(interaction(direction))
                .into();

        element
    };

    let handles = column![
        row![
            handle(window::Direction::NorthWest),
            handle(window::Direction::North),
            handle(window::Direction::NorthEast),
        ]
        .height(thickness),
        row![
            handle(window::Direction::West),
            // Inert: the application content underneath keeps the cursor and
            // every event in this region.
            space().width(Fill).height(Fill),
            handle(window::Direction::East),
        ]
        .height(Fill),
        row![
            handle(window::Direction::SouthWest),
            handle(window::Direction::South),
            handle(window::Direction::SouthEast),
        ]
        .height(thickness),
    ]
    .width(Fill)
    .height(Fill);

    iced::widget::stack![content, handles]
        .width(Fill)
        .height(Fill)
        .into()
}

fn interaction(direction: window::Direction) -> mouse::Interaction {
    match direction {
        window::Direction::North | window::Direction::South => {
            mouse::Interaction::ResizingVertically
        }
        window::Direction::East | window::Direction::West => {
            mouse::Interaction::ResizingHorizontally
        }
        window::Direction::NorthWest | window::Direction::SouthEast => {
            mouse::Interaction::ResizingDiagonallyDown
        }
        window::Direction::NorthEast | window::Direction::SouthWest => {
            mouse::Interaction::ResizingDiagonallyUp
        }
    }
}
