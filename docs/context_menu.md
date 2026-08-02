# Context Menu Widget

A right-click context menu with submenu support. Wraps any content widget and shows a floating menu when the user right-clicks on it.

## Overview

The context menu system has three main types:

| Type | Purpose |
|------|---------|
| `ContextMenu` | Wrapper widget that shows a menu on right-click |
| `Menu` | Builder for menu items |
| `MenuItem` | Individual menu entry (action or separator) |
| `Item` | A clickable action item with optional shortcut |

## Basic Usage

```rust
use iced::widget::{container, text};
use iced::Element;
use neverliie_iced_widgets::context_menu::{ContextMenu, Menu};

enum Message {
    Copy,
    Paste,
    DismissMenu,
}

fn view() -> Element<'_, Message> {
    let content = container(text("Right-click me"))
        .center_x(200)
        .center_y(200);

    let menu = Menu::new()
        .item("Copy", Message::Copy)
        .item("Paste", Message::Paste);

    ContextMenu::new(content, menu)
        .on_dismiss(Message::DismissMenu)
        .into()
}
```

## Menu Items

### Enabled Items

```rust
Menu::new()
    .item("Copy", Message::Copy)
    .item("Paste", Message::Paste)
```

### Disabled Items

```rust
Menu::new()
    .item("Copy", Message::Copy)
    .item_disabled("Undo")  // greyed out, no action
```

### Separators

```rust
Menu::new()
    .item("Copy", Message::Copy)
    .separator()
    .item("Paste", Message::Paste)
```

### Shortcuts

```rust
Menu::new()
    .item("Copy", Message::Copy)
    .shortcut("Ctrl+C")
    .item("Paste", Message::Paste)
    .shortcut("Ctrl+V")
```

## Submenus

Items can have nested submenus that appear on hover:

```rust
Menu::new()
    .item("Copy", Message::Copy)
    .separator()
    .submenu(
        "More",
        Menu::new()
            .item("Option A", Message::A)
            .item("Option B", Message::B),
    )
```

Submenus open to the right of the parent item. If there's not enough room on the right, the menu is clamped to the viewport.

## Keyboard Navigation

When the menu is open, the following keys are handled:

| Key | Action |
|-----|--------|
| `Arrow Up` | Move hover to previous item (wraps around) |
| `Arrow Down` | Move hover to next item (wraps around) |
| `Arrow Right` | Open submenu of hovered item |
| `Arrow Left` | Close current submenu |
| `Enter` | Activate hovered item (or open its submenu) |
| `Escape` | Dismiss the menu |

## Dismiss Behavior

Call `.on_dismiss(message)` to emit a message when the user clicks outside the menu:

```rust
ContextMenu::new(content, menu)
    .on_dismiss(Message::DismissMenu)
    .into()
```

Right-clicking while the menu is open also dismisses it and allows a new context menu to open at the new cursor position.

## How It Works

1. `ContextMenu` wraps your base content widget and tracks the cursor position
2. On right-click, it captures the cursor position and sets `is_open = true`
3. During the overlay phase, a `MenuOverlay` is created at the cursor position
4. The overlay renders the menu background, items, separators, shortcuts, and submenu arrows
5. Submenus are rendered as nested overlays positioned to the right of the parent item
6. All menu content is clamped to the viewport bounds to prevent off-screen rendering
7. Clicking outside or pressing Escape triggers the dismiss message

## Theming

Styling is controlled via the `Catalog` trait, implemented for `iced::Theme` by default. Use `.style()` for a custom style function or `.class()` for a theme class.

### Style Properties

```rust
pub struct Style {
    pub background: Background,           // Menu background
    pub border: Border,                   // Menu border
    pub shadow: Shadow,                   // Menu drop shadow
    pub text_color: Color,                // Default item text color
    pub disabled_text_color: Color,       // Disabled item text color
    pub selected_background: Background,  // Hovered item background
    pub selected_text_color: Color,       // Hovered item text color
    pub separator_color: Color,           // Separator line color
    pub shortcut_text_color: Color,       // Shortcut text color
}
```

## API Reference

### `ContextMenu`

```rust
ContextMenu::new(content, menu)     // Create with base content and menu
    .on_dismiss(message)            // Set dismiss message
    .on_right_click(message)        // Emit message on right-click, before menu opens
    .style(style_fn)                // Custom style function
    .class(class)                   // Theme class
    .text_size(size)                // Custom text size for menu items
```

### `Menu`

```rust
Menu::new()                         // Create empty menu
    .item(label, action)            // Add enabled item
    .item_disabled(label)           // Add disabled item
    .separator()                    // Add separator line
    .shortcut(text)                 // Set shortcut on last item
    .submenu(label, child_menu)     // Add submenu
```

### `Item`

```rust
Item::new(label, action)            // Create enabled item
    .shortcut(text)                 // Set shortcut text
    .submenu(child_menu)            // Attach submenu
```
