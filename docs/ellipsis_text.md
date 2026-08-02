# Ellipsis Text Widget

A line-clamped text widget that truncates with a trailing `…` when the content does not fit within a maximum number of lines.

## Overview

| Type | Purpose |
|------|---------|
| `EllipsisText` | Text widget clamped to `max_lines`, truncating with `…` |
| `ellipsis_text` | Function shorthand for `EllipsisText::new` |

## Basic Usage

```rust
use iced::Element;
use neverliie_iced_widgets::ellipsis_text::{ellipsis_text, EllipsisText};

enum Message {}

fn view() -> Element<'_, Message> {
    EllipsisText::new("A very long file name that must be clamped")
        .size(12)
        .max_lines(2)
        .into()
}

fn view_helper() -> Element<'_, Message> {
    ellipsis_text("Short label").max_lines(2).into()
}
```

## Features

- **Real measurement-based truncation**: Uses the renderer's actual paragraph
  measurement to decide where to clip, so it works correctly with any font and
  glyph widths (CJK, emoji, etc.)
- **Grapheme-aware**: Truncation happens at grapheme boundaries, so combining
  characters and emoji are never split mid-sequence
- **Binary search clipping**: Finds the longest fitting prefix efficiently
  instead of trying every character
- **Fully compliant with `text` API**: All standard text options are supported

## Builder Methods

### Basic Configuration

```rust
EllipsisText::new("content")
    .size(14)                        // Text size in pixels
    .color(iced::Color::WHITE)       // Text color
    .max_lines(3)                    // Max lines before truncating with …
    .align_x(alignment::Horizontal::Center)  // Text alignment
    .shaping(text::Shaping::Advanced)        // Glyph shaping strategy
```

### `max_lines`

The only behavior that differs from `iced::widget::text`. Sets the maximum
number of lines before the content is truncated with `…`:

```rust
EllipsisText::new("A very long file name that must be clamped")
    .max_lines(2)   // Values below 1 are clamped to 1
```

Text that fits within the limit is rendered exactly like `text`. Content that
exceeds the limit is truncated to the longest fitting prefix plus `…`.

## How It Works

1. `EllipsisText` delegates to iced's `advanced::text` paragraph machinery
2. During layout, the full content is measured against the available width
3. If the resulting height exceeds `max_lines` line heights, a binary search
   over grapheme offsets finds the longest prefix (plus `…`) that fits
4. The paragraph is re-laid out with the truncated content and drawn like
   standard iced text

## API Reference

### `EllipsisText`

```rust
EllipsisText::new(content)      // Create with text content

// Configuration
    .size(size)                 // Set text size
    .color(color)               // Set text color
    .max_lines(max)             // Set max lines before truncation
    .align_x(alignment)         // Set horizontal alignment
    .shaping(shaping)           // Set shaping strategy

// Conversion
    .into()                     // Convert to Element
```

### `ellipsis_text`

```rust
ellipsis_text(content)          // Function shorthand for EllipsisText::new
```
