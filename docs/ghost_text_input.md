# Ghost Trail Text Input Widget

A custom text input widget with an animated cursor that leaves a fading gradient trail when moving.

## Overview

| Type | Purpose |
|------|---------|
| `GhostTrailTextInput` | Text input with animated ghost trail cursor |
| `Status` | Widget state: Active, Hovered, Focused, Disabled |
| `Style` | Visual styling: background, border, colors |
| `Cursor` | Cursor position and selection tracking |
| `Value` | Unicode-aware text storage with grapheme support |

## Basic Usage

```rust
use iced::Element;
use neverliie_iced_widgets::ghost_text_input::GhostTrailTextInput;

enum Message {
    InputChanged(String),
}

fn view() -> Element<'_, Message> {
    GhostTrailTextInput::new("Type something...", "")
        .on_input(Message::InputChanged)
        .into()
}
```

## Features

- **Animated cursor trail**: When the cursor moves, it smoothly slides from the old position to the new one, leaving a fading gradient trail
- **Blinking cursor**: Standard 500ms blink interval when focused and idle
- **Secure mode**: Optionally masks input with dot characters for passwords
- **Icon support**: Display an optional icon on the left or right side
- **Focus callbacks**: Optional message when the input loses focus
- **Full keyboard shortcuts**: Ctrl/Cmd+C/X/V/A, Home/End, arrow keys with Shift and Ctrl/Alt
- **IME support**: Handles preedit and commit events for international keyboard input
- **Unicode support**: Full grapheme-aware text editing via `unicode-segmentation`

## Builder Methods

### Basic Configuration

```rust
GhostTrailTextInput::new("placeholder", "value")
    .on_input(Message::Changed)        // Text change callback
    .on_submit(Message::Submitted)     // Enter key callback
    .on_paste(Message::Pasted)         // Paste callback
    .on_lose_focus(Message::Blurred)   // Focus loss callback
    .secure(true)                      // Password mode
    .width(300)                        // Widget width
    .padding(10)                       // Inner padding
    .size(16)                          // Text size in pixels
    .font(my_font)                     // Custom font
    .align_x(alignment::Horizontal::Center)  // Text alignment
```

### Color Overrides

Override individual colors while keeping the theme's other styles:

```rust
GhostTrailTextInput::new("placeholder", "")
    .cursor_color(iced::Color::from_rgb(0.2, 0.8, 0.4))
    .text_color(iced::Color::from_rgb(0.9, 0.9, 0.9))
    .placeholder_color(iced::Color::from_rgb(0.5, 0.5, 0.5))
    .cursor_width(3.0)  // Thicker cursor
```

### Custom Styling

Apply a full custom style based on theme and status:

```rust
GhostTrailTextInput::new("placeholder", "")
    .style(|theme, status| {
        let palette = theme.extended_palette();
        ghost_text_input::Style {
            background: iced::Background::Color(palette.background.base.color),
            border: iced::Border {
                radius: 8.0.into(),
                width: 2.0,
                color: palette.primary.strong.color,
            },
            icon: palette.background.weak.text,
            placeholder: palette.secondary.base.color,
            value: palette.background.base.text,
            selection: palette.primary.weak.color,
        }
    })
```

## Focus Loss Callback

The `on_lose_focus` message is published when the input loses focus through user interaction:

- Clicking or tapping **outside** the input while it is focused
- Pressing **Escape** while it is focused

It does **not** fire when the window itself loses focus (the input keeps its focus state and resumes blinking when the window regains focus), nor for programmatic focus changes via widget operations.

```rust
GhostTrailTextInput::new("placeholder", "")
    .on_input(Message::Changed)
    .on_lose_focus(Message::FocusLost)  // e.g. save/validate on blur
```

## Style Properties

```rust
pub struct Style {
    pub background: Background,   // Input background
    pub border: Border,           // Border radius, width, color
    pub icon: Color,              // Icon color
    pub placeholder: Color,       // Placeholder text color
    pub value: Color,             // Input text color
    pub selection: Color,         // Text selection highlight
}
```

## Status Variants

| Status | Description |
|--------|-------------|
| `Active` | Input is not focused |
| `Hovered` | Mouse is over the input |
| `Focused { is_hovered }` | Input has focus, optionally hovered |
| `Disabled` | Input is disabled |

## How It Works

1. `GhostTrailTextInput` is a fully custom Iced widget (not a wrapper around `text_input`)
2. Text is stored as a `Value` of Unicode graphemes for correct handling of emoji and combining characters
3. The `Cursor` tracks position and selection range with word-boundary awareness
4. The `Editor` performs atomic text mutations (insert, paste, backspace, delete)
5. During the draw phase:
   - If the cursor is moving: a linear gradient quad is drawn from current to target position
   - If the cursor is static: a solid rectangle is drawn at the target position
   - The cursor blinks on a 500ms interval when idle
6. The cursor animation uses cubic-bezier easing for smooth motion

## API Reference

### `GhostTrailTextInput`

```rust
GhostTrailTextInput::new(placeholder, value)  // Create new input

// Callbacks
    .on_input(callback)         // Text change handler
    .on_input_maybe(callback)   // Optional text change handler
    .on_submit(message)         // Enter key handler
    .on_submit_maybe(message)   // Optional enter key handler
    .on_paste(callback)         // Paste handler
    .on_paste_maybe(callback)   // Optional paste handler
    .on_lose_focus(message)     // Focus loss handler
    .on_lose_focus_maybe(message) // Optional focus loss handler

// Configuration
    .id(id)                     // Widget ID for operations
    .secure(bool)               // Password mode
    .font(font)                 // Custom font
    .icon(icon)                 // Add icon
    .width(width)               // Set width
    .padding(padding)           // Set padding
    .size(pixels)               // Set text size
    .line_height(height)        // Set line height
    .align_x(alignment)         // Set horizontal alignment

// Styling
    .style(style_fn)            // Custom style function
    .class(class)               // Theme class
    .cursor_color(color)        // Override cursor color
    .cursor_width(width)        // Override cursor width
    .text_color(color)          // Override text color
    .placeholder_color(color)   // Override placeholder color
```

### `Icon`

```rust
Icon {
    font: Font,           // Icon font
    code_point: char,     // Icon character
    size: Option<Pixels>, // Icon size
    spacing: f32,         // Space between icon and text
    side: Side,           // Left or Right
}
```

### `Value`

```rust
Value::new(string)           // Create from string
value.len()                  // Grapheme count
value.is_empty()             // Whether empty
value.to_string()            // Convert back to String
```

### `Cursor`

```rust
cursor.selection(&value)     // Get selection range if any
cursor.move_to(pos)          // Jump to position
cursor.move_left(&value)     // Move left one grapheme
cursor.move_right(&value)    // Move right one grapheme
cursor.select_all(&value)    // Select all text
```
