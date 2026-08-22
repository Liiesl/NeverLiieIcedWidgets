//! macOS integration.
//!
//! There is no Objective-C, Cocoa or AppKit code here, and no raw `NSWindow`
//! handle is touched. The hybrid title bar is built entirely out of the macOS
//! settings the pinned Iced revision exposes:
//!
//! ```ignore
//! window::settings::PlatformSpecific {
//!     title_hidden: true,
//!     titlebar_transparent: true,
//!     fullsize_content_view: true,
//! }
//! ```
//!
//! `DecorationMode::Custom` therefore keeps `decorations = true`. That is the
//! documented macOS exception, and it is what preserves:
//!
//! * the native traffic lights, with their real hover and window-group
//!   behavior
//! * the native window shadow and corner rounding
//! * native edge and corner resizing, in all eight directions
//! * native full-screen and window tiling
//!
//! The frame reserves [`NativeFrameConfig::leading_inset`] logical pixels
//! before its own content so nothing collides with the traffic lights, and
//! draws no minimize / maximize / close buttons of its own.
//!
//! ## Limitations
//!
//! * Custom edge resizing is not offered. The native decorations already
//!   handle it, and overlaying resize handles would fight them.
//! * [`iced::window::show_system_menu`] is a no-op: macOS has no per-window
//!   system menu. A click on the application icon publishes
//!   [`FrameAction::WindowIconPressed`] so the application can show its own.
//! * The traffic lights cannot be repositioned without `NSWindow` access, so
//!   very short title bars will clip them. Keep `title_bar_height` at 28 or
//!   more.
//!
//! [`NativeFrameConfig::leading_inset`]: super::NativeFrameConfig::leading_inset
//! [`FrameAction::WindowIconPressed`]: crate::title_bar::FrameAction::WindowIconPressed

use iced::window;

use super::NativeFrameConfig;

pub(crate) fn window_settings(
    settings: window::Settings,
    config: NativeFrameConfig,
) -> window::Settings {
    if !config.decoration_mode.uses_custom_frame() {
        return settings;
    }

    window::Settings {
        platform_specific: window::settings::PlatformSpecific {
            title_hidden: true,
            titlebar_transparent: true,
            fullsize_content_view: true,
        },
        ..settings
    }
}
