//! Platform integration.
//!
//! Everything portable lives outside this module and goes through Iced's
//! winit-backed [`iced::window`] API. What is left here is:
//!
//! * [`windows`] — a `WM_NCHITTEST` subclass, because winit exposes no hook for
//!   the semantic caption regions Windows 11 needs for Snap Layouts.
//! * [`linux`] and [`macos`] — window-settings helpers and capability queries
//!   only. No raw X11, Wayland, Cocoa, Objective-C or AppKit call is made; the
//!   Linux backend probe reads only the discriminant of the window handle winit
//!   already hands out.

use iced::{Color, Task, window};

use std::sync::Arc;

use super::WindowState;
use super::config::NativeFrameConfig;

#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

/// The color of the thin border drawn around the window surface.
///
/// On Windows an active, non-maximized window takes the user's accent color —
/// the same color DWM paints around native windows — while every other case
/// (inactive, maximized, other platforms) keeps the neutral theme border.
pub(crate) fn border_color(fallback: Color, active: bool, maximized: bool) -> Color {
    #[cfg(windows)]
    {
        if active && !maximized {
            windows::accent_color().unwrap_or(fallback)
        } else {
            fallback
        }
    }

    #[cfg(not(windows))]
    {
        let _ = (active, maximized);

        fallback
    }
}

/// Whether the frame must overlay its own invisible resize handles.
///
/// Windows resolves resizing natively inside `WM_NCHITTEST`, and macOS keeps
/// its native decorations in custom mode, so only X11 and Wayland need them.
pub(crate) const USES_OVERLAY_RESIZE_HANDLES: bool = cfg!(not(any(windows, target_os = "macos")));

/// Whether the platform hit-tests the caption controls itself.
///
/// When `true`, the frame draws the controls but attaches no input handling:
/// the press, the hover and the resulting command are all native, which is
/// what keeps the Windows 11 Snap Layout flyout working.
pub(crate) const NATIVE_CAPTION_HIT_TEST: bool = cfg!(windows);

/// Whether [`iced::window::show_system_menu`] does anything here.
///
/// * Windows — yes, the native `WM_SYSMENU` popup.
/// * Wayland — yes, `xdg_toplevel::show_window_menu`; the compositor decides
///   whether it draws one.
/// * X11 — no. winit's X11 `show_window_menu` is an empty function in the
///   pinned revision, so the call is silently dropped. Handle
///   [`FrameAction::WindowIconPressed`] yourself to show an application menu.
/// * macOS — no.
///
/// On Linux the answer comes from the backend observed when the first window
/// was installed, falling back to winit's own environment check before that.
///
/// [`FrameAction::WindowIconPressed`]: super::action::FrameAction::WindowIconPressed
pub(crate) fn supports_system_menu() -> bool {
    #[cfg(windows)]
    {
        true
    }

    #[cfg(target_os = "linux")]
    {
        linux::supports_system_menu()
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        false
    }
}

/// Applies the platform-specific parts of [`iced::window::Settings`].
pub(crate) fn window_settings(
    settings: window::Settings,
    config: NativeFrameConfig,
) -> window::Settings {
    let settings = window::Settings {
        decorations: config.decoration_mode.window_decorations(),
        resizable: config.resizable,
        ..settings
    };

    #[cfg(windows)]
    {
        windows::window_settings(settings, config)
    }

    #[cfg(target_os = "linux")]
    {
        linux::window_settings(settings, config)
    }

    #[cfg(target_os = "macos")]
    {
        macos::window_settings(settings, config)
    }

    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        let _ = config;

        settings
    }
}

/// Installs whatever native integration the platform needs for a window.
pub(crate) fn install(window_id: window::Id, state: Arc<WindowState>) -> Task<Result<(), String>> {
    #[cfg(windows)]
    {
        // The subclass only makes sense when we own the non-client area.
        if state.config.decoration_mode.uses_custom_frame() {
            windows::install(window_id, state)
        } else {
            Task::done(Ok(()))
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Runs in both modes: `supports_system_menu` needs the backend either
        // way.
        let _ = state;

        linux::install(window_id)
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = (window_id, state);

        Task::done(Ok(()))
    }
}

/// Removes the native integration installed by [`install`].
pub(crate) fn uninstall(window_id: window::Id) -> Task<()> {
    #[cfg(windows)]
    {
        windows::uninstall(window_id)
    }

    #[cfg(not(windows))]
    {
        let _ = window_id;

        Task::done(())
    }
}
