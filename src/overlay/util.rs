use iced::{Point, Rectangle};

use super::Position;

impl Position {
    /// Resolves this position to a [`Point`] given the current context.
    ///
    /// - `parent_bounds`: bounds of the parent widget
    /// - `cursor_position`: current cursor position
    /// - `viewport`: the viewport/window bounds
    /// - `content_bounds`: bounds of the floating content being positioned
    /// - `floating_bounds`: pre-computed bounds of other floating elements
    ///   (for `Position::Floating` inter-element positioning)
    pub fn resolve(
        &self,
        parent_bounds: Rectangle,
        cursor_position: Point,
        viewport: Rectangle,
        content_bounds: Rectangle,
        floating_bounds: &[Rectangle],
    ) -> Point {
        match *self {
            Position::Absolute(point) => point,
            Position::Cursor { offset } => cursor_position + offset,
            Position::FollowCursor => cursor_position,
            pos => {
                if let Some((anchor, offset)) = pos.parent_anchor_offset() {
                    anchor.resolve(parent_bounds)
                        + anchor.content_offset(content_bounds)
                        + offset
                } else if let Some((anchor, offset)) =
                    pos.viewport_anchor_offset()
                {
                    anchor.resolve(viewport)
                        + anchor.content_offset(content_bounds)
                        + offset
                } else if let Some((fi, anchor, offset)) =
                    pos.floating_index_anchor_offset()
                {
                    if let Some(target_bounds) = floating_bounds.get(fi) {
                        anchor.resolve(*target_bounds)
                            + anchor.content_offset(content_bounds)
                            + offset
                    } else {
                        Point::ORIGIN
                    }
                } else {
                    Point::ORIGIN
                }
            }
        }
    }
}

/// Clamps a position so the content at that position stays within the viewport.
pub fn clamp_to_viewport(
    position: Point,
    content_size: iced::Size,
    viewport: Rectangle,
) -> Point {
    let mut clamped = position;
    if clamped.x + content_size.width > viewport.x + viewport.width {
        clamped.x = viewport.x + viewport.width - content_size.width;
    }
    if clamped.y + content_size.height > viewport.y + viewport.height {
        clamped.y = viewport.y + viewport.height - content_size.height;
    }
    if clamped.x < viewport.x {
        clamped.x = viewport.x;
    }
    if clamped.y < viewport.y {
        clamped.y = viewport.y;
    }
    clamped
}
