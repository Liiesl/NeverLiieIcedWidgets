//! The minimum Win32 layer needed for a Windows 11 custom title bar.
//!
//! Everything portable — drag, resize drag, minimize, maximize, close, system
//! menu — goes through Iced's winit-backed [`iced::window`] API. What winit
//! cannot express is the *semantics* of the caption region: Windows decides
//! whether to open the Snap Layout flyout, whether to show a caption tooltip
//! and which resize cursor to display purely from the value an application
//! returns for `WM_NCHITTEST`. There is no winit hook for that, so this module
//! subclasses the window and answers the message itself.
//!
//! It is deliberately limited to:
//!
//! * install / remove the subclass
//! * `WM_NCHITTEST` — the caption and resize regions
//! * `HTMINBUTTON` / `HTMAXBUTTON` / `HTCLOSE` — `HTMAXBUTTON` is what triggers
//!   the Windows 11 Snap Layout flyout
//! * the minimal non-client mouse handling those regions need, so the frame can
//!   paint its own hover and pressed states instead of letting `DefWindowProc`
//!   run its modal caption-button loop
//! * mirroring maximized / activation state back into the frame
//!
//! Ordinary title-bar pixels stay `HTCLIENT`, so Iced keeps receiving input for
//! the application's own title-bar widgets. Rounded corners and the undecorated
//! drop shadow are requested through
//! [`iced::window::settings::PlatformSpecific`], which winit applies at window
//! creation — this module makes no DWM calls of its own.

use iced::Color;
use iced::Task;
use iced::window;
use iced::window::raw_window_handle::RawWindowHandle;
use iced::window::settings::platform::CornerPreference;

use std::collections::HashMap;
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{InvalidateRect, ScreenToClient};
use windows_sys::Win32::System::Registry::{
    HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW,
};
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClientRect, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCLIENT, HTCLOSE, HTLEFT, HTMAXBUTTON,
    HTMINBUTTON, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, IsZoomed, PostMessageW, SC_CLOSE,
    SC_MAXIMIZE, SC_MINIMIZE, SC_RESTORE, SIZE_MAXIMIZED, WM_CAPTURECHANGED, WM_NCACTIVATE,
    WM_NCDESTROY, WM_NCHITTEST, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE,
    WM_SIZE, WM_SYSCOMMAND,
};

use crate::title_bar::action::CaptionControl;
use crate::title_bar::config::NativeFrameConfig;
use crate::title_bar::{WindowState, raw_control};

const SUBCLASS_ID: usize = 0x4943_4544;

type WindowKey = usize;
type Registry = HashMap<WindowKey, Arc<WindowState>>;

static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();

fn window_key(hwnd: HWND) -> WindowKey {
    hwnd as usize
}

fn registry() -> &'static Mutex<Registry> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn registry_guard() -> std::sync::MutexGuard<'static, Registry> {
    registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

static ACCENT_COLOR: OnceLock<Option<Color>> = OnceLock::new();

/// The user's accent color, as configured in Windows personalization.
///
/// Read once per process from the same DWM registry value the system itself
/// uses for window borders (`HKCU\Software\Microsoft\Windows\DWM\AccentColor`,
/// a `COLORREF`). Following live accent changes would require listening for
/// `WM_DWMCOLORIZATIONCOLORCHANGED`, which the frame does not do — restart the
/// application to pick up a new accent.
pub(crate) fn accent_color() -> Option<Color> {
    *ACCENT_COLOR.get_or_init(read_accent_color)
}

fn read_accent_color() -> Option<Color> {
    let mut colorref: u32 = 0;
    let mut size = mem::size_of::<u32>() as u32;

    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            windows_sys::w!("Software\\Microsoft\\Windows\\DWM"),
            windows_sys::w!("AccentColor"),
            RRF_RT_REG_DWORD,
            ptr::null_mut(),
            (&mut colorref as *mut u32).cast(),
            &mut size,
        )
    };

    if status != 0 {
        return None;
    }

    // COLORREF packs as `0x00BBGGRR`.
    Some(Color::from_rgb8(
        (colorref & 0xFF) as u8,
        ((colorref >> 8) & 0xFF) as u8,
        ((colorref >> 16) & 0xFF) as u8,
    ))
}

/// Requests rounded corners through winit instead of calling DWM directly.
///
/// winit applies `corner_preference` at window creation as
/// `DWMWA_WINDOW_CORNER_PREFERENCE`, which is exactly the call this module
/// used to make itself; reading the attribute back on a live window confirms
/// it lands (`DWMWCP_ROUND` when `native_rounding` is set, `DWMWCP_DONOTROUND`
/// when it is not).
///
/// `undecorated_shadow` is left to the configuration and defaults to off: DWM
/// already draws the drop shadow, because winit keeps `WS_CAPTION | WS_BORDER`
/// on undecorated windows. See [`NativeFrameConfig::native_shadow`].
pub(crate) fn window_settings(
    settings: window::Settings,
    config: NativeFrameConfig,
) -> window::Settings {
    if !config.decoration_mode.uses_custom_frame() {
        return settings;
    }

    let corner_preference = if config.native_rounding && config.corner_radius > 0.0 {
        CornerPreference::Round
    } else {
        CornerPreference::DoNotRound
    };

    window::Settings {
        // The DWM shadow and DWM rounding both need an opaque surface.
        transparent: false,
        platform_specific: window::settings::PlatformSpecific {
            undecorated_shadow: config.native_shadow,
            corner_preference,
            ..settings.platform_specific
        },
        ..settings
    }
}

/// Installs the subclass on the window backing `window_id`.
pub(crate) fn install(window_id: window::Id, state: Arc<WindowState>) -> Task<Result<(), String>> {
    window::run(window_id, move |window| {
        let handle = window
            .window_handle()
            .map_err(|error| format!("Could not obtain native window handle: {error}"))?;

        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return Err("Iced window did not provide a Win32 window handle".to_owned());
        };

        let hwnd = handle.hwnd.get() as HWND;

        state
            .maximized
            .store(unsafe { IsZoomed(hwnd) != 0 }, Ordering::Release);

        registry_guard().insert(window_key(hwnd), Arc::clone(&state));

        let installed = unsafe { SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, 0) };

        if installed == 0 {
            registry_guard().remove(&window_key(hwnd));

            return Err("SetWindowSubclass failed for the Iced window".to_owned());
        }

        redraw(hwnd);

        Ok(())
    })
}

/// Removes the subclass.
///
/// Not strictly required — `WM_NCDESTROY` cleans up too — but it lets an
/// application switch decoration modes at runtime.
pub(crate) fn uninstall(window_id: window::Id) -> Task<()> {
    window::run(window_id, move |window| {
        let Ok(handle) = window.window_handle() else {
            return;
        };

        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return;
        };

        let hwnd = handle.hwnd.get() as HWND;

        if registry_guard().remove(&window_key(hwnd)).is_some() {
            unsafe {
                RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
            }

            redraw(hwnd);
        }
    })
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    _reference_data: usize,
) -> LRESULT {
    if message == WM_NCDESTROY {
        registry_guard().remove(&window_key(hwnd));

        unsafe {
            RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);

            return DefSubclassProc(hwnd, message, wparam, lparam);
        }
    }

    let state = registry_guard().get(&window_key(hwnd)).cloned();

    let Some(state) = state else {
        return unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    };

    match message {
        // The whole point of this module. Windows reads the caption semantics
        // straight out of the return value.
        WM_NCHITTEST => {
            let hit = hit_test(hwnd, lparam, &state);
            let control = CaptionControl::from_hit_test(hit);

            set_control(hwnd, &state.hot, control);

            if control.is_some() {
                track_non_client_leave(hwnd);
            }

            return hit;
        }

        // Non-client mouse tracking, so the frame can paint its own hover and
        // pressed states for the caption controls.
        WM_NCMOUSEMOVE => {
            let control = CaptionControl::from_hit_test(wparam as LRESULT);

            set_control(hwnd, &state.hot, control);
            track_non_client_leave(hwnd);
        }

        WM_NCMOUSELEAVE => {
            set_control(hwnd, &state.hot, None);
            set_control(hwnd, &state.pressed, None);
        }

        // Swallowing the press keeps `DefWindowProc` out of its modal
        // caption-button loop, which would otherwise freeze the frame's own
        // rendering for the duration of the click. `HTMAXBUTTON` is
        // deliberately *not* swallowed before the hit test has run, so the
        // Snap Layout flyout still opens on hover.
        WM_NCLBUTTONDOWN => {
            let control = CaptionControl::from_hit_test(wparam as LRESULT);

            if control.is_some() {
                set_control(hwnd, &state.pressed, control);
                set_control(hwnd, &state.hot, control);
                track_non_client_leave(hwnd);

                return 0;
            }
        }

        WM_NCLBUTTONUP => {
            let released = CaptionControl::from_hit_test(wparam as LRESULT);
            let pressed = state.pressed();

            set_control(hwnd, &state.pressed, None);

            // Only fire when the release lands on the control that was
            // pressed, exactly like a native caption button.
            if let Some(pressed) = pressed
                && released == Some(pressed)
            {
                execute_caption_command(hwnd, pressed);
            }

            if released.is_some() {
                return 0;
            }
        }

        WM_CAPTURECHANGED => {
            set_control(hwnd, &state.pressed, None);
        }

        // Authoritative maximized state: it also covers Snap Layouts, Aero
        // Snap and the Win+Up shortcut, none of which go through the frame.
        WM_SIZE => {
            set_bool(hwnd, &state.maximized, wparam as u32 == SIZE_MAXIMIZED);
        }

        WM_NCACTIVATE => {
            set_bool(hwnd, &state.active, wparam != 0);
        }

        _ => {}
    }

    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

/// Maps a screen point to a semantic caption or resize region.
///
/// Anything that is not a caption control or a resize border stays
/// `HTCLIENT`, so Iced keeps receiving the input for the application's own
/// title-bar widgets.
fn hit_test(hwnd: HWND, lparam: LPARAM, state: &WindowState) -> LRESULT {
    let mut point = POINT {
        x: signed_low_word(lparam),
        y: signed_high_word(lparam),
    };

    if unsafe { ScreenToClient(hwnd, &mut point) } == 0 {
        return HTCLIENT as LRESULT;
    }

    let mut client_rect: RECT = unsafe { mem::zeroed() };

    if unsafe { GetClientRect(hwnd, &mut client_rect) } == 0 {
        return HTCLIENT as LRESULT;
    }

    // The frame is laid out in logical pixels; `WM_NCHITTEST` is physical.
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let scale = dpi as f32 / 96.0;
    let to_physical = |logical: f32| (logical * scale).round() as i32;

    let config = state.config;

    let width = client_rect.right - client_rect.left;
    let height = client_rect.bottom - client_rect.top;

    let resize_border = to_physical(config.resize_border).max(1);
    let title_bar_height = to_physical(config.title_bar_height).max(1);
    let caption_button_width = to_physical(config.caption_button_width).max(1);

    let maximized = unsafe { IsZoomed(hwnd) != 0 };

    // Native resize regions: eight directions, native cursors, native
    // double-click-to-maximize on the top and bottom edges.
    if config.resizable && !maximized {
        let left = point.x >= -resize_border && point.x < resize_border;
        let right = point.x >= width - resize_border && point.x < width + resize_border;
        let top = point.y >= -resize_border && point.y < resize_border;
        let bottom = point.y >= height - resize_border && point.y < height + resize_border;

        match (left, right, top, bottom) {
            (true, _, true, _) => return HTTOPLEFT as LRESULT,
            (_, true, true, _) => return HTTOPRIGHT as LRESULT,
            (true, _, _, true) => return HTBOTTOMLEFT as LRESULT,
            (_, true, _, true) => return HTBOTTOMRIGHT as LRESULT,
            (true, ..) => return HTLEFT as LRESULT,
            (_, true, ..) => return HTRIGHT as LRESULT,
            (_, _, true, _) => return HTTOP as LRESULT,
            (.., true) => return HTBOTTOM as LRESULT,
            _ => {}
        }
    }

    // The frame may be inset by transparent padding; the caption regions move
    // with the surface, not with the window.
    let padding = to_physical(config.surface_padding()).max(0);

    if !config.caption_buttons
        || point.y < padding
        || point.y >= padding + title_bar_height
        || point.x < padding
        || point.x >= width - padding
    {
        return HTCLIENT as LRESULT;
    }

    let width = width - padding;
    let close_left = width - caption_button_width;
    let maximize_left = close_left - caption_button_width;
    let minimize_left = maximize_left - caption_button_width;

    if point.x >= close_left && point.x < width {
        return HTCLOSE as LRESULT;
    }

    // `HTMAXBUTTON` is what makes Windows 11 offer the Snap Layout flyout.
    if point.x >= maximize_left && point.x < close_left {
        return HTMAXBUTTON as LRESULT;
    }

    if point.x >= minimize_left && point.x < maximize_left {
        return HTMINBUTTON as LRESULT;
    }

    // Everything else in the title bar, including the application icon and the
    // application's own title-bar widgets. Dragging and double-click-to-
    // maximize are handled on the Iced side through `window::drag` and
    // `window::toggle_maximize`.
    HTCLIENT as LRESULT
}

/// Runs the caption command for a completed click.
///
/// This stays native. The portable path — `window::minimize`,
/// `window::toggle_maximize`, `window::close` — would have to travel back
/// through the Iced message loop, and by then the non-client mouse capture that
/// Windows established for the click has been broken, which loses the restore
/// animation and desynchronizes the pressed state. `WM_SYSCOMMAND` is the same
/// message `DefWindowProc` would have posted.
fn execute_caption_command(hwnd: HWND, control: CaptionControl) {
    let command = match control {
        CaptionControl::Minimize => SC_MINIMIZE,

        CaptionControl::Maximize => {
            if unsafe { IsZoomed(hwnd) != 0 } {
                SC_RESTORE
            } else {
                SC_MAXIMIZE
            }
        }

        CaptionControl::Close => SC_CLOSE,
    };

    unsafe {
        PostMessageW(hwnd, WM_SYSCOMMAND, command as WPARAM, 0);
    }
}

fn track_non_client_leave(hwnd: HWND) {
    let mut tracking = TRACKMOUSEEVENT {
        cbSize: mem::size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: TME_LEAVE | TME_NONCLIENT,
        hwndTrack: hwnd,
        dwHoverTime: 0,
    };

    unsafe {
        TrackMouseEvent(&mut tracking);
    }
}

fn set_control(hwnd: HWND, atomic: &AtomicU8, control: Option<CaptionControl>) {
    let raw = raw_control(control);

    if atomic.swap(raw, Ordering::AcqRel) != raw {
        redraw(hwnd);
    }
}

fn set_bool(hwnd: HWND, atomic: &AtomicBool, value: bool) {
    if atomic.swap(value, Ordering::AcqRel) != value {
        redraw(hwnd);
    }
}

fn redraw(hwnd: HWND) {
    unsafe {
        InvalidateRect(hwnd, ptr::null(), 0);
    }
}

fn signed_low_word(value: LPARAM) -> i32 {
    ((value as u32 & 0xFFFF) as u16) as i16 as i32
}

fn signed_high_word(value: LPARAM) -> i32 {
    (((value as u32 >> 16) & 0xFFFF) as u16) as i16 as i32
}

impl CaptionControl {
    /// The semantic caption region a `WM_NCHITTEST` result names, if any.
    fn from_hit_test(hit: LRESULT) -> Option<Self> {
        if hit == HTMINBUTTON as LRESULT {
            Some(Self::Minimize)
        } else if hit == HTMAXBUTTON as LRESULT {
            Some(Self::Maximize)
        } else if hit == HTCLOSE as LRESULT {
            Some(Self::Close)
        } else {
            None
        }
    }
}
