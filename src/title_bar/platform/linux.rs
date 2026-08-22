//! Linux integration — X11 and Wayland.
//!
//! There is no native code here. Everything the frame needs is available
//! through Iced's winit-backed [`iced::window`] API:
//!
//! * [`iced::window::drag`] — `_NET_WM_MOVERESIZE` on X11,
//!   `xdg_toplevel::move` on Wayland.
//! * [`iced::window::drag_resize`] — the same, for all eight directions.
//! * [`iced::window::minimize`], [`iced::window::toggle_maximize`],
//!   [`iced::window::close`], [`iced::window::is_maximized`].
//!
//! ## Wayland
//!
//! The compositor owns move and resize. The frame only *requests* them and
//! never repositions the window itself. `DecorationMode::System` asks for
//! normal decorations, but the compositor decides whether the result is
//! server-side or client-side — which is why the public mode is not called
//! `ServerSide`.
//!
//! Transparent outer padding is included in tiled and maximized geometry by
//! most compositors, so the client-side shadow is off by default. Enable it
//! with [`NativeFrameConfig::client_shadow`] if your compositor behaves.
//!
//! ## X11
//!
//! [`iced::window::show_system_menu`] resolves to winit's X11
//! `show_window_menu`, which is an empty function in the pinned winit
//! revision (`05b8ff17`). The call is therefore a silent no-op. Rather than
//! adding raw Xlib/XCB code just for a window menu, the frame publishes
//! [`FrameAction::WindowIconPressed`] and lets the application show its own
//! popup.
//!
//! [`NativeFrameConfig::client_shadow`]: super::NativeFrameConfig::client_shadow
//! [`FrameAction::WindowIconPressed`]: crate::title_bar::FrameAction::WindowIconPressed

use iced::window;
use iced::window::raw_window_handle::RawWindowHandle;

use std::sync::atomic::{AtomicU8, Ordering};

use super::NativeFrameConfig;

const UNKNOWN: u8 = 0;
const WAYLAND: u8 = 1;
const X11: u8 = 2;

/// Which winit backend this process ended up on.
///
/// winit selects one backend for the whole process, so this is global rather
/// than per-window.
static BACKEND: AtomicU8 = AtomicU8::new(UNKNOWN);

/// Records the backend by looking at which [`RawWindowHandle`] variant winit
/// produced.
///
/// This only reads the handle's discriminant — no Xlib, XCB or Wayland
/// protocol call is made, and the handle itself is never dereferenced. It is
/// authoritative in a way the environment is not: `WAYLAND_DISPLAY` can be set
/// in a session where Iced was built without the `wayland` feature, in which
/// case winit silently falls back to X11.
pub(crate) fn install(window_id: window::Id) -> iced::Task<Result<(), String>> {
    window::run(window_id, |window| {
        let handle = window
            .window_handle()
            .map_err(|error| format!("Could not obtain native window handle: {error}"))?;

        let backend = match handle.as_raw() {
            RawWindowHandle::Wayland(_) => WAYLAND,
            RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_) => X11,
            other => {
                return Err(format!("Unexpected window handle on Linux: {other:?}"));
            }
        };

        BACKEND.store(backend, Ordering::Release);

        Ok(())
    })
}

/// Whether [`iced::window::show_system_menu`] does anything.
///
/// Prefers the backend observed at [`install`] time. Before the first window
/// is installed it falls back to the same environment check winit itself uses:
/// Wayland when `WAYLAND_DISPLAY` or `WAYLAND_SOCKET` is set and non-empty,
/// otherwise X11.
pub(crate) fn supports_system_menu() -> bool {
    match BACKEND.load(Ordering::Acquire) {
        WAYLAND => true,
        X11 => false,
        _ => wayland_in_environment(),
    }
}

fn wayland_in_environment() -> bool {
    ["WAYLAND_DISPLAY", "WAYLAND_SOCKET"]
        .iter()
        .any(|name| std::env::var(name).is_ok_and(|value| !value.is_empty()))
}

pub(crate) fn window_settings(
    settings: window::Settings,
    config: NativeFrameConfig,
) -> window::Settings {
    let wants_transparency = config.decoration_mode.uses_custom_frame()
        && config.client_shadow
        && config.outer_padding > 0.0;

    window::Settings {
        // A transparent surface is what makes the client-side shadow and the
        // rounded corners visible. Requesting it unconditionally would cost a
        // compositing round trip for nothing.
        transparent: settings.transparent || wants_transparency,
        ..settings
    }
}
