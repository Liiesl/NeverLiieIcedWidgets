# Overlay Widget

The overlay system provides floating content that can be positioned relative to a parent widget, the viewport, or the mouse cursor. It uses iced's built-in overlay rendering, so floating content draws on top of all other widgets.

## Overview

The overlay system has three main types:

| Type | Purpose |
|------|---------|
| `OverlayManager` | Wrapper widget that renders floating children as overlays |
| `Floating` | A floating child element with positioning metadata |
| `Position` | Strategy for where to place the floating content |

## Basic Usage

```rust
use iced::widget::{button, container, text};
use iced::{Element, Vector};
use neverliie_iced_widgets::overlay::{Floating, OverlayManager, Position};

enum Message {
    ShowMenu,
    CloseMenu,
}

fn view(show_menu: bool) -> Element<'_, Message> {
    let content = button("Open Menu").on_press(Message::ShowMenu);

    if show_menu {
        OverlayManager::new(content)
            .overlay(
                Floating::new(
                    container(text("Menu item 1"))
                        .padding(8)
                        .style(container::rounded_box),
                )
                .position(Position::Bottom),
            )
            .on_dismiss(Message::CloseMenu)
            .into()
    } else {
        content.into()
    }
}
```

## Positioning

### Convenience Variants

The simplest way to position floating content. These place the floating element at the given anchor on the **parent widget** with zero offset:

```rust
Position::TopLeft      Position::Top      Position::TopRight
Position::Left         Position::Center    Position::Right
Position::BottomLeft   Position::Bottom    Position::BottomRight
```

Example:

```rust
Floating::new(content).position(Position::Bottom)
```

### Viewport Positioning

Relative to the window edges/corners. Use the `Viewport` prefix:

```rust
Position::ViewportTopLeft      Position::ViewportTop      Position::ViewportTopRight
Position::ViewportLeft         Position::ViewportCenter    Position::ViewportRight
Position::ViewportBottomLeft   Position::ViewportBottom    Position::ViewportBottomRight
```

Example:

```rust
Floating::new(content).position(Position::ViewportBottomRight)
```

### Parent Positioning with Offset

For custom offsets from the parent anchor:

```rust
use iced::Vector;

Position::Parent {
    anchor: Anchor::BottomLeft,
    offset: Vector::new(0.0, 4.0), // 4px below the anchor
}
```

Or use the convenience constructors:

```rust
Position::bottom_left(Vector::new(0.0, 4.0))
Position::top_right(Vector::new(-8.0, 0.0))
```

### Viewport Positioning with Offset

```rust
Position::Viewport {
    anchor: Anchor::BottomRight,
    offset: Vector::new(-10.0, -10.0), // 10px inset from bottom-right
}
```

### Absolute Positioning

Fixed coordinates from the viewport top-left:

```rust
Position::absolute(300.0, 200.0)
// or
Position::Absolute(Point::new(300.0, 200.0))
```

### Cursor Following

The floating content follows the mouse cursor:

```rust
Position::FollowCursor
// or with an offset:
Position::cursor(Vector::new(12.0, 12.0))
```

### Floating-Relative Positioning

Position relative to *another* floating element by index. The `index` refers
to the position of the floating element in the order it was added to the
[`OverlayManager`](overlay.md). Index `0` is the first floating element added.

```rust
Position::Floating {
    index: 0,                               // relative to the first overlay
    anchor: Anchor::TopRight,
    offset: Vector::new(4.0, 0.0),          // 4px to the right of its edge
}
// or use the convenience constructor:
Position::floating(0, Anchor::TopRight, Vector::new(4.0, 0.0))
```

This is useful for submenus that need to appear next to their parent menu item.

## Anchor

The `Anchor` enum represents 9 compass positions on a rectangle:

```
TopLeft    Top    TopRight
Left       Center Right
BottomLeft Bottom BottomRight
```

Anchors are used by `Position::Parent` and `Position::Viewport` to determine which point on the parent/viewport the floating content aligns to. The floating content's own anchor point aligns with the target anchor.

For example, `Anchor::Top` means the floating content's top-center aligns with the target point.

## Multiple Overlays

You can add multiple floating children. They all render as overlays on top of the base content:

```rust
OverlayManager::new(content)
    .overlay(
        Floating::new(tooltip_text)
            .position(Position::Top),
    )
    .overlay(
        Floating::new(badge)
            .position(Position::TopRight),
    )
    .into()
```

## Z-Index

Floating elements are stacked in the order they are added, but you can control
the stacking order explicitly with `.index()`. Higher indices render on top of
lower indices. Defaults to `1.0`.

```rust
OverlayManager::new(content)
    .overlay(
        Floating::new(menu)
            .position(Position::FollowCursor)
            .index(1.0),
    )
    .overlay(
        Floating::new(tooltip)
            .position(Position::Top)
            .index(2.0),  // tooltip renders on top of the menu
    )
    .into()
```

## Dismissal

`OverlayManager` has no built-in dismiss message: it is a pure layout wrapper.
Control dismissal from your application state — for example, by toggling a
`bool` in your `Message::update` and only adding the overlay while it is
`true` (see the [overlay-test example](../examples/overlay-test)).

## How It Works

1. `OverlayManager` wraps your base content widget
2. Floating children are stored but do not affect the base layout
3. During the overlay phase, each floating child is positioned using its `Position` strategy
4. The floating content is rendered on top of everything else via iced's overlay system
5. All floating content is clamped to the viewport bounds to prevent off-screen rendering

## API Reference

### `OverlayManager`

```rust
OverlayManager::new(content)          // Create with base content
    .overlay(floating)                // Add a floating child
    .into()                           // Convert to Element
```

### `Floating`

```rust
Floating::new(content)                // Create floating element
    .position(position)               // Set position strategy
    .index(z)                         // Set z-index (default 1.0)
```

### `Position`

```rust
// Convenience (parent-relative, zero offset)
Position::TopLeft, Position::Top, Position::TopRight
Position::Left, Position::Center, Position::Right
Position::BottomLeft, Position::Bottom, Position::BottomRight

// Viewport convenience (zero offset)
Position::ViewportTopLeft, Position::ViewportTop, Position::ViewportTopRight
Position::ViewportLeft, Position::ViewportCenter, Position::ViewportRight
Position::ViewportBottomLeft, Position::ViewportBottom, Position::ViewportBottomRight

// Cursor
Position::FollowCursor

// Floating-relative
Position::Floating { index, anchor, offset }

// Advanced
Position::Parent { anchor, offset }
Position::Viewport { anchor, offset }
Position::Cursor { offset }
Position::Absolute(point)

// Constructors
Position::absolute(x, y)
Position::cursor(offset)
Position::floating(index, anchor, offset)
Position::top_left(offset)
Position::top_right(offset)
Position::bottom_left(offset)
Position::bottom_right(offset)
```
