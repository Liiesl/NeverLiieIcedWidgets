//! Floating content positioned relative to parent, viewport, or cursor.
//!
//! This module provides [`OverlayManager`], a wrapper widget that renders
//! floating children as overlays using iced's overlay system.
//!
//! # Overview
//!
//! The overlay system has three main types:
//!
//! - [`OverlayManager`] — wraps base content and renders floating children on top
//! - [`Floating`] — a floating child element with positioning metadata
//! - [`Position`] — strategy for where to place the floating content
//!
//! # Example
//!
//! ```no_run
//! use iced::widget::{button, container, text};
//! use iced::{Element, Vector};
//! use never_lie_iced_widgets::overlay::{Floating, OverlayManager, Position};
//!
//! enum Message {
//!     ShowPopup,
//! }
//!
//! fn view(show_popup: bool) -> Element<'_, Message> {
//!     let content = button("Show Popup").on_press(Message::ShowPopup);
//!
//!     if show_popup {
//!         OverlayManager::new(content)
//!             .overlay(
//!                 Floating::new(text("Hello from overlay!"))
//!                     .position(Position::BottomLeft),
//!             )
//!             .into()
//!     } else {
//!         content.into()
//!     }
//! }
//! ```
//!
//! # Positioning
//!
//! [`Position`] provides convenience variants for common placements:
//!
//! - **Parent-relative**: `Position::TopLeft`, `Position::Bottom`, etc.
//! - **Viewport-relative**: `Position::ViewportTopLeft`, `Position::ViewportBottom`, etc.
//! - **Cursor**: `Position::FollowCursor`
//! - **Absolute**: `Position::absolute(x, y)`
//! - **Floating-relative**: `Position::Floating { index, anchor, offset }`
//!
//! For custom offsets, use the struct variants:
//!
//! ```ignore
//! Position::Parent { anchor: Anchor::BottomLeft, offset: Vector::new(0.0, 4.0) }
//! Position::Viewport { anchor: Anchor::BottomRight, offset: Vector::new(-10.0, -10.0) }
//! Position::Floating { index: 0, anchor: Anchor::TopRight, offset: Vector::new(4.0, 0.0) }
//! ```
//!
//! # Z-Index
//!
//! Floating elements support z-index ordering via [`Floating::index`]:
//!
//! ```ignore
//! Floating::new(tooltip).position(Position::Top).index(2.0)
//! Floating::new(menu).position(Position::FollowCursor).index(1.0)
//! // tooltip renders on top of menu
//! ```
//!
//! [`OverlayManager`]: struct.OverlayManager
//! [`Floating`]: struct.Floating
//! [`Position`]: enum.Position

mod manager;
mod util;

pub use manager::OverlayManager;
pub use util::clamp_to_viewport;

use iced::{Element, Point, Rectangle, Vector};

/// Anchor point for positioning floating content.
///
/// Represents 9 compass positions on a rectangle:
///
/// ```text
/// TopLeft    Top    TopRight
/// Left       Center Right
/// BottomLeft Bottom BottomRight
/// ```
///
/// Used by [`Position::Parent`] and [`Position::Viewport`] to determine
/// which point on the parent/viewport the floating content aligns to.
///
/// The floating content's own anchor point aligns with the target anchor.
/// For example, [`Anchor::Top`] means the floating content's top-center
/// aligns with the target point.
///
/// [`Anchor::Top`]: Anchor::Top
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    #[default]
    BottomLeft,
    Bottom,
    BottomRight,
}

impl Anchor {
    /// Resolves this anchor to a [`Point`] on the given rectangle.
    pub fn resolve(self, rect: Rectangle) -> Point {
        match self {
            Anchor::TopLeft => Point::new(rect.x, rect.y),
            Anchor::Top => Point::new(rect.x + rect.width / 2.0, rect.y),
            Anchor::TopRight => Point::new(rect.x + rect.width, rect.y),
            Anchor::Left => Point::new(rect.x, rect.y + rect.height / 2.0),
            Anchor::Center => Point::new(
                rect.x + rect.width / 2.0,
                rect.y + rect.height / 2.0,
            ),
            Anchor::Right => Point::new(
                rect.x + rect.width,
                rect.y + rect.height / 2.0,
            ),
            Anchor::BottomLeft => Point::new(rect.x, rect.y + rect.height),
            Anchor::Bottom => {
                Point::new(rect.x + rect.width / 2.0, rect.y + rect.height)
            }
            Anchor::BottomRight => {
                Point::new(rect.x + rect.width, rect.y + rect.height)
            }
        }
    }

    /// Returns the offset from the floating content's top-left to its own
    /// anchor point, so that the content's anchor aligns with the target
    /// anchor.
    ///
    /// For example, `Anchor::Top` returns `(width/2, 0)` so the content's
    /// top-center aligns to the target point.
    pub fn content_offset(self, content: Rectangle) -> Vector {
        match self {
            Anchor::TopLeft => Vector::ZERO,
            Anchor::Top => Vector::new(-content.width / 2.0, 0.0),
            Anchor::TopRight => Vector::new(-content.width, 0.0),
            Anchor::Left => Vector::new(0.0, -content.height / 2.0),
            Anchor::Center => {
                Vector::new(-content.width / 2.0, -content.height / 2.0)
            }
            Anchor::Right => {
                Vector::new(-content.width, -content.height / 2.0)
            }
            Anchor::BottomLeft => Vector::new(0.0, -content.height),
            Anchor::Bottom => {
                Vector::new(-content.width / 2.0, -content.height)
            }
            Anchor::BottomRight => {
                Vector::new(-content.width, -content.height)
            }
        }
    }
}

/// Positioning strategy for floating content.
///
/// # Variants
///
/// - **Parent-relative**: Positions relative to the parent widget's bounds.
///   Use convenience variants like [`Position::Bottom`] or the struct
///   variant [`Position::Parent`] with custom offset.
///
/// - **Viewport-relative**: Positions relative to the window edges/corners.
///   Use convenience variants like [`Position::ViewportBottomRight`] or
///   the struct variant [`Position::Viewport`] with custom offset.
///
/// - **Cursor**: Follows the mouse cursor. Use [`Position::FollowCursor`]
///   or [`Position::Cursor`] with a custom offset.
///
/// - **Absolute**: Fixed coordinates from the viewport top-left.
///   Use [`Position::absolute`] or [`Position::Absolute`].
///
/// # Examples
///
/// ```ignore
/// // Simple parent-relative
/// Position::Bottom
///
/// // Parent-relative with offset
/// Position::Parent { anchor: Anchor::BottomLeft, offset: Vector::new(0.0, 4.0) }
///
/// // Viewport corner
/// Position::ViewportTopRight
///
/// // Follow cursor
/// Position::FollowCursor
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Position {
    /// Absolute position from viewport top-left.
    Absolute(Point),

    /// Relative to the parent widget's layout bounds.
    Parent {
        anchor: Anchor,
        offset: Vector,
    },

    /// Follow the mouse cursor with an offset.
    Cursor { offset: Vector },

    /// Relative to the viewport (window) edges/corners.
    Viewport {
        anchor: Anchor,
        offset: Vector,
    },

    /// Relative to another floating element by index.
    ///
    /// The `index` refers to the position of the floating element in the
    /// order they were added to the [`OverlayManager`]. For example, index
    /// `0` refers to the first floating element added.
    ///
    /// This is useful for submenus that need to appear next to their parent
    /// menu item.
    Floating {
        index: usize,
        anchor: Anchor,
        offset: Vector,
    },

    // Parent-relative convenience (zero offset)
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,

    // Viewport-relative convenience (zero offset)
    ViewportTopLeft,
    ViewportTop,
    ViewportTopRight,
    ViewportLeft,
    ViewportCenter,
    ViewportRight,
    ViewportBottomLeft,
    ViewportBottom,
    ViewportBottomRight,

    /// Shorthand for `Cursor { offset: Vector::ZERO }`.
    FollowCursor,
}

impl Position {
    /// Creates an absolute position at the given coordinates.
    pub fn absolute(x: f32, y: f32) -> Self {
        Position::Absolute(Point::new(x, y))
    }

    /// Creates a cursor-following position with the given offset.
    pub fn cursor(offset: Vector) -> Self {
        Position::Cursor { offset }
    }

    /// Creates a parent-relative position anchored to the bottom-left.
    pub fn bottom_left(offset: Vector) -> Self {
        Position::Parent { anchor: Anchor::BottomLeft, offset }
    }

    /// Creates a parent-relative position anchored to the bottom-right.
    pub fn bottom_right(offset: Vector) -> Self {
        Position::Parent { anchor: Anchor::BottomRight, offset }
    }

    /// Creates a parent-relative position anchored to the top-left.
    pub fn top_left(offset: Vector) -> Self {
        Position::Parent { anchor: Anchor::TopLeft, offset }
    }

    /// Creates a parent-relative position anchored to the top-right.
    pub fn top_right(offset: Vector) -> Self {
        Position::Parent { anchor: Anchor::TopRight, offset }
    }

    /// Creates a position relative to another floating element.
    pub fn floating(index: usize, anchor: Anchor, offset: Vector) -> Self {
        Position::Floating { index, anchor, offset }
    }

    /// Returns the anchor and offset for this position, if it is
    /// parent-relative.
    fn parent_anchor_offset(self) -> Option<(Anchor, Vector)> {
        match self {
            Position::Parent { anchor, offset } => Some((anchor, offset)),
            Position::TopLeft => Some((Anchor::TopLeft, Vector::ZERO)),
            Position::Top => Some((Anchor::Top, Vector::ZERO)),
            Position::TopRight => Some((Anchor::TopRight, Vector::ZERO)),
            Position::Left => Some((Anchor::Left, Vector::ZERO)),
            Position::Center => Some((Anchor::Center, Vector::ZERO)),
            Position::Right => Some((Anchor::Right, Vector::ZERO)),
            Position::BottomLeft => Some((Anchor::BottomLeft, Vector::ZERO)),
            Position::Bottom => Some((Anchor::Bottom, Vector::ZERO)),
            Position::BottomRight => Some((Anchor::BottomRight, Vector::ZERO)),
            _ => None,
        }
    }

    /// Returns the anchor and offset for this position, if it is
    /// viewport-relative.
    fn viewport_anchor_offset(self) -> Option<(Anchor, Vector)> {
        match self {
            Position::Viewport { anchor, offset } => Some((anchor, offset)),
            Position::ViewportTopLeft => Some((Anchor::TopLeft, Vector::ZERO)),
            Position::ViewportTop => Some((Anchor::Top, Vector::ZERO)),
            Position::ViewportTopRight => Some((Anchor::TopRight, Vector::ZERO)),
            Position::ViewportLeft => Some((Anchor::Left, Vector::ZERO)),
            Position::ViewportCenter => Some((Anchor::Center, Vector::ZERO)),
            Position::ViewportRight => Some((Anchor::Right, Vector::ZERO)),
            Position::ViewportBottomLeft => Some((Anchor::BottomLeft, Vector::ZERO)),
            Position::ViewportBottom => Some((Anchor::Bottom, Vector::ZERO)),
            Position::ViewportBottomRight => Some((Anchor::BottomRight, Vector::ZERO)),
            _ => None,
        }
    }

    /// Returns the index, anchor, and offset for this position, if it is
    /// floating-relative.
    fn floating_index_anchor_offset(self) -> Option<(usize, Anchor, Vector)> {
        match self {
            Position::Floating { index, anchor, offset } => Some((index, anchor, offset)),
            _ => None,
        }
    }
}

impl Default for Position {
    fn default() -> Self {
        Position::Absolute(Point::ORIGIN)
    }
}

/// A floating child element with positioning metadata.
///
/// Create with [`Floating::new`] and set the position with
/// [`Floating::position`]. Then add to an [`OverlayManager`].
///
/// # Example
///
/// ```ignore
/// Floating::new(text("Hello!"))
///     .position(Position::Bottom)
/// ```
pub struct Floating<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Theme: 'a,
    Renderer: 'a,
{
    pub(crate) content: Element<'a, Message, Theme, Renderer>,
    pub(crate) position: Position,
    pub(crate) index: f32,
}

impl<'a, Message, Theme, Renderer> Floating<'a, Message, Theme, Renderer>
where
    Theme: 'a,
    Renderer: 'a,
{
    /// Creates a new floating element.
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            content: content.into(),
            position: Position::default(),
            index: 1.0,
        }
    }

    /// Sets the position of this floating element.
    #[must_use]
    pub fn position(mut self, position: Position) -> Self {
        self.position = position;
        self
    }

    /// Sets the z-index of this floating element.
    ///
    /// Overlays with a higher index are rendered on top of overlays with
    /// a lower index. Defaults to `1.0`.
    #[must_use]
    pub fn index(mut self, index: f32) -> Self {
        self.index = index;
        self
    }
}
