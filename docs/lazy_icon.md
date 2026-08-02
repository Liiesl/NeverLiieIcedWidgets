# Lazy Icon Widget

A widget that displays an image or SVG icon once its decoder has finished, showing a colored placeholder rectangle until then. Also includes a standalone `placeholder` widget for skeleton loading states.

## Overview

| Type | Purpose |
|------|---------|
| `LazyIcon` | Renders an icon when decoded, a placeholder until then |
| `IconHandle` | Enum wrapping an image or SVG handle |
| `placeholder` | Standalone colored rounded rectangle (skeleton loader) |

## Basic Usage

```rust
use iced::widget::text;
use iced::{Color, Element};
use neverliie_iced_widgets::lazy_icon::{IconHandle, LazyIcon};

enum Message {}

fn view() -> Element<'_, Message> {
    LazyIcon::new(IconHandle::Image(
        iced::widget::image::Handle::from_path("icon.png"),
    ))
    .size(48.0)
    .placeholder_color(Color::from_rgb(0.2, 0.2, 0.2))
    .placeholder_radius(8.0)
    .into()
}
```

## Custom Icons (extracted from files)

For icons extracted from `.exe`, `.dll`, `.ico`, or other sources, use
`iced::widget::image::Handle::from_rgba` to pass decoded pixels:

```rust
use iced::{Color, Element};
use neverliie_iced_widgets::lazy_icon::{IconHandle, LazyIcon};

enum Message {}

fn custom_icon(rgba: Vec<u8>, width: u32, height: u32) -> Element<'_, Message> {
    LazyIcon::new(IconHandle::Image(
        iced::widget::image::Handle::from_rgba(width, height, rgba),
    ))
    .size(48.0)
    .into()
}
```

## Placeholder Behavior

| Handle | Check | Result |
|--------|-------|--------|
| `IconHandle::Image` | [`Renderer::measure_image`](https://docs.rs/iced/0.14/iced/advanced/image/trait.Renderer.html) finished? | Placeholder drawn until the decoder reports dimensions |
| `IconHandle::Svg` | `measure_svg` reports valid dimensions (>1×1)? | SVGs parse synchronously, so the placeholder is rarely visible |

When the placeholder color has full transparency (`alpha == 0.0`), nothing is
drawn until the icon is ready — the widget just reserves its space.

## Skeleton Placeholders

`placeholder` is a standalone colored rounded rectangle, useful as a skeleton
loading indicator independent of `LazyIcon`:

```rust
use iced::{Color, Element};
use neverliie_iced_widgets::lazy_icon::placeholder;

enum Message {}

fn view() -> Element<'_, Message> {
    placeholder(Color::from_rgb(0.2, 0.2, 0.2), 8.0, 48.0).into()
}
```

## Builder Methods

```rust
LazyIcon::new(handle)                 // Create with image or SVG handle
    .size(48.0)                       // Square size in pixels (default 16.0)
    .placeholder_color(color)         // Placeholder fill color (default transparent)
    .placeholder_radius(radius)       // Placeholder corner radius (default 0.0)
```

## How It Works

1. `LazyIcon` stores an [`IconHandle`] (image or SVG)
2. During the draw phase it asks the renderer to measure the handle
3. If the decoder has finished (dimensions > 1×1), the icon is drawn into the
   widget bounds
4. Otherwise a `fill_quad` renders the placeholder rectangle with the
   configured color and radius
5. The widget always reserves its full square size, so layout never jumps
   when the icon finishes loading

## API Reference

### `IconHandle`

```rust
IconHandle::Image(handle)   // Raster image (iced::widget::image::Handle)
IconHandle::Svg(handle)     // Vector SVG (iced::widget::svg::Handle)
```

### `LazyIcon`

```rust
LazyIcon::new(handle)               // Create with an IconHandle

// Configuration
    .size(size)                     // Square icon size in pixels
    .placeholder_color(color)       // Placeholder fill color
    .placeholder_radius(radius)     // Placeholder corner radius

// Conversion
    .into()                         // Convert to Element
```

### `placeholder`

```rust
placeholder(color, radius, size)    // Colored rounded rectangle Element
```
