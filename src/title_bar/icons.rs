//! Embedded [Lucide](https://lucide.dev) glyphs for the caption controls.
//!
//! They are compiled into the binary, so the frame has no asset dependency.

use iced::widget::svg;

use super::action::CaptionControl;

/// Lucide "minus".
const MINUS_SVG: &[u8] = br#"
<svg
    xmlns="http://www.w3.org/2000/svg"
    width="24"
    height="24"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
>
    <path d="M5 12h14"/>
</svg>
"#;

/// Lucide "square".
const SQUARE_SVG: &[u8] = br#"
<svg
    xmlns="http://www.w3.org/2000/svg"
    width="24"
    height="24"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2.5"
    stroke-linecap="round"
    stroke-linejoin="round"
>
    <rect
        width="18"
        height="18"
        x="3"
        y="3"
        rx="2"
    />
</svg>
"#;

/// Lucide "copy", used as the restore glyph.
const COPY_SVG: &[u8] = br#"
<svg
    xmlns="http://www.w3.org/2000/svg"
    width="24"
    height="24"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2.5"
    stroke-linecap="round"
    stroke-linejoin="round"
>
    <rect
        width="14"
        height="14"
        x="8"
        y="8"
        rx="2"
    />
    <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>
</svg>
"#;

/// Lucide "x".
const X_SVG: &[u8] = br#"
<svg
    xmlns="http://www.w3.org/2000/svg"
    width="24"
    height="24"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
>
    <path d="M18 6 6 18"/>
    <path d="m6 6 12 12"/>
</svg>
"#;

/// Returns the embedded glyph for a caption control.
pub(crate) fn handle(control: CaptionControl, maximized: bool) -> svg::Handle {
    let bytes: &'static [u8] = match control {
        CaptionControl::Minimize => MINUS_SVG,
        CaptionControl::Maximize if maximized => COPY_SVG,
        CaptionControl::Maximize => SQUARE_SVG,
        CaptionControl::Close => X_SVG,
    };

    svg::Handle::from_memory(bytes)
}
