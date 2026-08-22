//! The events published by the custom frame.

use iced::window;

/// One of the three semantic caption controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CaptionControl {
    /// The minimize control.
    Minimize = 1,
    /// The maximize / restore control.
    Maximize = 2,
    /// The close control.
    Close = 3,
}

impl CaptionControl {
    /// Every control, in title-bar order.
    pub const ALL: [Self; 3] = [Self::Minimize, Self::Maximize, Self::Close];

    pub(crate) fn from_raw(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Minimize),
            2 => Some(Self::Maximize),
            3 => Some(Self::Close),
            _ => None,
        }
    }
}

/// Something the frame wants the application to forward back to
/// [`NativeFrame::update`].
///
/// Every variant names the window it applies to, so a single [`NativeFrame`]
/// can serve any number of windows without their states colliding.
///
/// [`NativeFrame::update`]: super::NativeFrame::update
/// [`NativeFrame`]: super::NativeFrame
//
// No `PartialEq`: `iced::window::Direction` does not implement it.
#[derive(Debug, Clone, Copy)]
pub enum FrameAction {
    /// Start a platform-managed window drag.
    Drag(window::Id),

    /// Start a platform-managed resize drag in the given direction.
    Resize(window::Id, Direction),

    /// Minimize the window.
    Minimize(window::Id),

    /// Maximize the window, or restore it if it is already maximized.
    ToggleMaximize(window::Id),

    /// Close the window.
    Close(window::Id),

    /// The application icon was clicked.
    ///
    /// This is intentionally *not* [`FrameAction::ShowSystemMenu`]: the
    /// application decides what a click on its own icon does. Forwarding it to
    /// [`NativeFrame::update`] runs the default behavior, which is to show the
    /// system menu on the platforms that support it and do nothing elsewhere.
    ///
    /// Intercept it in your own `update` to show an application menu instead.
    ///
    /// [`NativeFrame::update`]: super::NativeFrame::update
    WindowIconPressed(window::Id),

    /// Ask the platform to show the window's system menu.
    ///
    /// Unsupported on X11 and macOS; see
    /// [`NativeFrame::supports_system_menu`].
    ///
    /// [`NativeFrame::supports_system_menu`]: super::NativeFrame::supports_system_menu
    ShowSystemMenu(window::Id),

    /// The pointer entered a caption control.
    Hover(window::Id, CaptionControl),

    /// The pointer left a caption control.
    Leave(window::Id, CaptionControl),

    /// The maximized state of a window changed.
    MaximizedChanged(window::Id, bool),

    /// The focused state of a window changed.
    FocusChanged(window::Id, bool),

    /// Re-read the window state that the frame mirrors.
    SyncState(window::Id),

    /// A window was closed; the frame can drop the state it kept for it.
    WindowClosed(window::Id),
}

impl FrameAction {
    /// The window this action applies to.
    pub fn window_id(self) -> window::Id {
        match self {
            Self::Drag(id)
            | Self::Resize(id, _)
            | Self::Minimize(id)
            | Self::ToggleMaximize(id)
            | Self::Close(id)
            | Self::WindowIconPressed(id)
            | Self::ShowSystemMenu(id)
            | Self::Hover(id, _)
            | Self::Leave(id, _)
            | Self::MaximizedChanged(id, _)
            | Self::FocusChanged(id, _)
            | Self::SyncState(id)
            | Self::WindowClosed(id) => id,
        }
    }
}

/// Re-exported so consumers do not have to name [`iced::window::Direction`].
pub use iced::window::Direction;
