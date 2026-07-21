# NeverLiie Iced Widgets

A clean, reusable widget library for [iced](https://iced.rs) — Elm-inspired GUI library for Rust.

## Overview

This library provides additional widgets for iced that are not available in the core distribution. Each widget is designed to follow iced's conventions: builder pattern, generic `Message`/`Theme`/`Renderer` parameters, and Elm-architecture compatibility.

## Widgets

| Widget | Description | Status |
|--------|-------------|--------|
| [Overlay](docs/overlay.md) | Floating content positioned relative to parent, viewport, or cursor | Stable |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
never-lie-iced-widgets = { path = "path/to/NeverLiieIcedWidgets" }
iced = { version = "0.14", features = ["default", "advanced"] }
```

## Quick Start

```rust
use iced::widget::{button, text};
use iced::{Element, Vector};
use never_lie_iced_widgets::overlay::{Floating, OverlayManager, Position};

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
            .on_dismiss(Message::Dismiss)
            .into()
    } else {
        content.into()
    }
}
```

## Examples

Run the included examples:

```sh
# Overlay positioning demo
cargo run -p overlay-test

# Widget launcher
cargo run -p launcher
```

## License

MIT
