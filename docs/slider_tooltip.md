# Slider Tooltip Widget

A slider wrapper that displays a floating tooltip showing the current value during hover and drag.

## Overview

The slider tooltip system has two main types:

| Type | Purpose |
|------|---------|
| `SliderTooltip` | Wraps a slider and adds a value tooltip |
| `TooltipPosition` | Controls whether the tooltip appears above or below |

## Basic Usage

```rust
use iced::Element;
use neverliie_iced_widgets::slider_tooltip::{SliderTooltip, TooltipPosition};

enum Message {
    ValueChanged(f64),
}

fn view() -> Element<'_, Message> {
    SliderTooltip::new(0.0..=100.0, 50.0, Message::ValueChanged)
        .tooltip_position(TooltipPosition::Top)
        .tooltip_gap(12.0)
        .into()
}
```

## Tooltip Position

Control whether the tooltip appears above or below the slider handle:

```rust
SliderTooltip::new(0.0..=100.0, 50.0, Message::ValueChanged)
    .tooltip_position(TooltipPosition::Bottom)
```

| Position | Description |
|----------|-------------|
| `Top` | Tooltip appears above the handle (default) |
| `Bottom` | Tooltip appears below the handle |

## Tooltip Gap

Set the distance between the tooltip and the slider handle:

```rust
SliderTooltip::new(0.0..=100.0, 50.0, Message::ValueChanged)
    .tooltip_gap(12.0)  // 12px gap
```

## Tooltip Delay

Add a delay before the tooltip appears on hover:

```rust
use std::time::Duration;

SliderTooltip::new(0.0..=100.0, 50.0, Message::ValueChanged)
    .tooltip_delay(Duration::from_millis(300))
```

The tooltip appears immediately when dragging, regardless of the delay.

## Custom Formatting

Override the default tooltip text with a custom formatter:

```rust
SliderTooltip::new(0.0..=100.0, 50.0, Message::ValueChanged)
    .tooltip_format(|v| format!("{}%", v))
```

The default formatter adapts decimal places based on the range size:
- Range < 1.0: two decimal places (`0.75`)
- Range < 10.0: one decimal place (`5.3`)
- Range >= 10.0: no decimal places (`50`)

## Custom Tooltip Styling

Apply a custom style to the tooltip container:

```rust
use iced::widget::container;

SliderTooltip::new(0.0..=100.0, 50.0, Message::ValueChanged)
    .tooltip_style(|_theme| container::Style {
        background: Some(iced::Background::Color(
            iced::Color::from_rgba(0.2, 0.2, 0.2, 0.95),
        )),
        text_color: Some(iced::Color::WHITE),
        ..container::Style::default()
    })
```

## Handle Width

Set the width of the slider handle in pixels. This affects tooltip positioning:

```rust
SliderTooltip::new(0.0..=100.0, 50.0, Message::ValueChanged)
    .handle_width(20.0)
```

## Slider Configuration

`SliderTooltip` forwards all of iced's `Slider` builder methods, giving you full control over the underlying slider:

### Width and Height

```rust
SliderTooltip::new(0.0..=100.0, 50.0, Message::ValueChanged)
    .width(300)
    .height(20)
```

### Step Size

```rust
SliderTooltip::new(0.0..=100.0, 50.0, Message::ValueChanged)
    .step(5)
```

### Shift Step

```rust
SliderTooltip::new(0.0..=100.0, 50.0, Message::ValueChanged)
    .shift_step(0.1)
```

### Default Value

Sets a value the slider resets to when ctrl-clicked or command-clicked:

```rust
SliderTooltip::new(0.0..=100.0, 50.0, Message::ValueChanged)
    .default_value(50.0)
```

### Release Message

Emits a message when the user releases the slider (useful for expensive operations):

```rust
SliderTooltip::new(0.0..=100.0, 50.0, Message::ValueChanged)
    .on_release(Message::Save)
```

### Slider Styling

```rust
use iced::widget::slider;

SliderTooltip::new(0.0..=100.0, 50.0, Message::ValueChanged)
    .style(|theme, status| {
        let palette = theme.extended_palette();
        slider::Style {
            rail: slider::Rail {
                backgrounds: (palette.primary.strong.color.into(), palette.background.strong.color.into()),
                width: 4.0,
                border: iced::Border::default(),
            },
            handle: slider::Handle {
                shape: slider::HandleShape::Circle { radius: 8.0 },
                background: palette.primary.strong.color.into(),
                border_color: palette.primary.strong.color,
                border_width: 2.0,
            },
        }
    })
```

## How It Works

1. `SliderTooltip` wraps an iced `Slider` and tracks hover/drag state
2. On hover, a timer starts; after the configured delay, the tooltip becomes visible
3. While dragging, the tooltip is always visible regardless of the delay
4. During the overlay phase, a `TooltipOverlay` is created above (or below) the slider handle
5. The tooltip is horizontally centered on the handle position
6. The tooltip bounds are clamped to the viewport to prevent off-screen rendering
7. The overlay renders a rounded container with the formatted value text

## API Reference

### `SliderTooltip`

```rust
SliderTooltip::new(range, value, on_change)  // Create with range, value, and change handler

// Tooltip configuration
    .tooltip_position(position)               // Set tooltip position (Top or Bottom)
    .tooltip_gap(gap)                         // Set gap between tooltip and handle
    .tooltip_delay(delay)                     // Set hover delay before tooltip appears
    .tooltip_format(formatter)                // Custom text formatter
    .tooltip_style(style_fn)                  // Custom container style
    .handle_width(width)                      // Set handle width in pixels

// Slider configuration (forwarded from iced's Slider)
    .width(width)                             // Set slider width
    .height(height)                           // Set slider height
    .step(step)                               // Set step size
    .shift_step(shift_step)                   // Set shift-key step
    .default_value(default)                   // Set ctrl-click reset value
    .on_release(message)                      // Set mouse release message
    .style(style_fn)                          // Custom slider style
```

### `TooltipPosition`

```rust
TooltipPosition::Top     // Above the slider handle (default)
TooltipPosition::Bottom  // Below the slider handle
```
