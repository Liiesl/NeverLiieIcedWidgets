# NeverLiie Iced Widgets

A clean, reusable widget library for [iced](https://iced.rs) — the Elm-inspired GUI library for Rust.

## Overview

This library provides additional widgets for iced that are not available in the core distribution. Each widget is designed to follow iced's conventions: builder pattern, generic `Message`/`Theme`/`Renderer` parameters, and Elm-architecture compatibility.

## Widgets

| Widget | Description | Status |
|--------|-------------|--------|
| [Overlay](docs/overlay.md) | Floating content positioned relative to parent, viewport, cursor, or other overlays | Stable |
| [Context Menu](docs/context_menu.md) | Right-click context menu with submenu, shortcuts, and keyboard navigation | Stable |
| [Confirmation Dialog](docs/confirmation_dialog.md) | Modal confirmation dialog with configurable buttons and blocking mode | Stable |
| [Ghost Text Input](docs/ghost_text_input.md) | Text input with animated ghost trail cursor effect | Stable |
| [Slider Tooltip](docs/slider_tooltip.md) | Slider with floating value tooltip during hover and drag | Stable |
| [Lazy Icon](docs/lazy_icon.md) | Lazy-loading icon with placeholder, plus skeleton `placeholder` widget | Stable |
| [Ellipsis Text](docs/ellipsis_text.md) | Line-clamped text with ellipsis (`…`) truncation | Stable |
| [Color Picker](docs/color_picker.md) | Dialog-style color picker with hue ring, RGB/HSV sliders, swatches and recent colors | Stable |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
neverliie-iced-widgets = { path = "path/to/NeverLiieIcedWidgets" }
iced = { version = "0.14", features = ["default", "advanced", "image", "svg"] }
```

## Quick Start

```rust
use iced::widget::{button, text};
use iced::Element;
use neverliie_iced_widgets::overlay::{Floating, OverlayManager, Position};

#[derive(Clone)]
enum Message {
    ShowPopup,
    DismissPopup,
}

fn view(show_popup: bool) -> Element<'_, Message> {
    let content = button("Show Popup").on_press(Message::ShowPopup);

    if show_popup {
        OverlayManager::new(content)
            .overlay(
                Floating::new(text("Hello from overlay!"))
                    .position(Position::BottomLeft),
            )
            .into()
    } else {
        content.into()
    }
}
```

## Examples

All widgets ship with runnable examples. Each example is a workspace member, so run any of them from the repository root:

```sh
# Overlay positioning demo (all anchors, viewport modes, cursor follow)
cargo run -p overlay-test

# Right-click context menu with submenus, shortcuts, and theming
cargo run -p context-menu-test

# Modal confirmation dialogs, including blocking and custom buttons
cargo run -p confirmation-dialog-test

# Ghost trail text input (basic, secure, and custom styled)
cargo run -p ghost-text-input-test

# Slider tooltips (position, delay, custom format and style)
cargo run -p slider-tooltip

# Lazy icon loading with placeholders and SVG sources
cargo run -p lazy-icon-test

# Ellipsis text truncation across lines and fonts
cargo run -p ellipsis-text-test

# Color picker dialog (hue ring, RGB/HSV tabs, swatches, recent colors)
cargo run -p color-picker-test

# Launcher menu for the demos above
cargo run -p launcher
```

## Documentation

- Per-widget guides: [`docs/`](docs/)
- API reference: `cargo doc --no-deps` or run doctests with `cargo test --doc`

## License

MIT
