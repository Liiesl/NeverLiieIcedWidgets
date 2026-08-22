//! Configuration for [`NativeFrame`].
//!
//! [`NativeFrame`]: super::NativeFrame

/// How a window should be decorated.
///
/// The mode deliberately avoids the words "client side" and "server side".
/// Wayland compositors are free to negotiate client-side decorations even when
/// an application asks for the normal, platform-managed decoration path, so
/// those words would describe an outcome we cannot promise.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DecorationMode {
    /// The application draws its own title bar.
    ///
    /// * The window is created without platform decorations, except on macOS
    ///   (see [`DecorationMode::window_decorations`]).
    /// * [`NativeFrame::view`] renders the custom frame.
    /// * On Windows, a small `WM_NCHITTEST` subclass is installed so the
    ///   caption controls keep their native semantics (Snap Layouts, drag to
    ///   snap, native resize cursors).
    ///
    /// [`NativeFrame::view`]: super::NativeFrame::view
    #[default]
    Custom,

    /// Use the normal platform-managed decoration path.
    ///
    /// The window is created with `decorations = true`, no custom frame is
    /// rendered and no Win32 subclass is installed. What the user actually
    /// sees is decided by the operating system, the window manager or the
    /// Wayland compositor — which may still hand the decorations back to the
    /// client.
    System,
}

impl DecorationMode {
    /// Returns `true` when [`NativeFrame::view`] should wrap the application
    /// content in the custom frame.
    ///
    /// [`NativeFrame::view`]: super::NativeFrame::view
    pub fn uses_custom_frame(self) -> bool {
        matches!(self, Self::Custom)
    }

    /// Returns the value to use for [`iced::window::Settings::decorations`].
    ///
    /// macOS is the documented exception: [`DecorationMode::Custom`] keeps the
    /// decorations enabled so the native traffic lights, the native window
    /// shadow and native edge resizing all keep working. The title bar is made
    /// transparent and the content view is extended underneath it instead, so
    /// the application still draws the whole title bar area.
    pub fn window_decorations(self) -> bool {
        match self {
            Self::System => true,
            Self::Custom => cfg!(target_os = "macos"),
        }
    }
}

/// The look and feel of the custom frame.
///
/// Start from [`NativeFrameConfig::platform_default`] and override what you
/// need with the builder methods.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NativeFrameConfig {
    /// How the window should be decorated.
    pub decoration_mode: DecorationMode,

    /// The height of the title bar, in logical pixels.
    pub title_bar_height: f32,

    /// The width of a single caption control, in logical pixels.
    pub caption_button_width: f32,

    /// Whether the frame draws minimize / maximize / close controls.
    ///
    /// Disabled by default on macOS, where the native traffic lights are used
    /// instead.
    pub caption_buttons: bool,

    /// The thickness of the resize border, in logical pixels.
    ///
    /// On Windows this is the `WM_NCHITTEST` border. Everywhere else it is the
    /// thickness of the invisible resize handles drawn around the frame.
    pub resize_border: f32,

    /// Whether the window can be resized.
    pub resizable: bool,

    /// The radius of the client-side rounded corners, in logical pixels.
    pub corner_radius: f32,

    /// Whether the frame draws a border around the surface.
    ///
    /// On Windows the border takes the user's accent color while the window
    /// is active, mirroring what DWM paints around native windows; it falls
    /// back to a neutral theme color when inactive, maximized, or when the
    /// accent cannot be read.
    pub frame_border: bool,

    /// Whether the frame draws a client-side shadow.
    ///
    /// Requires [`NativeFrameConfig::outer_padding`] to be greater than zero
    /// and the window to be transparent.
    pub client_shadow: bool,

    /// Transparent padding around the surface, in logical pixels.
    ///
    /// This is where the client-side shadow is drawn. Keep it at `0.0` unless
    /// [`NativeFrameConfig::client_shadow`] is enabled: Wayland compositors
    /// include this padding in tiled and maximized geometry, which makes the
    /// window look inset.
    pub outer_padding: f32,

    /// Ask the platform to round the window corners.
    ///
    /// Only Windows 11 honors this today, through
    /// [`iced::window::settings::PlatformSpecific::corner_preference`].
    pub native_rounding: bool,

    /// Ask the platform for the extra undecorated-window drop shadow.
    ///
    /// Only Windows honors this, through winit's `undecorated_shadow`, which
    /// extends the client area by one pixel inside `WM_NCCALCSIZE`.
    ///
    /// Off by default, and measurably so: winit keeps `WS_CAPTION | WS_BORDER`
    /// on undecorated windows, so DWM already draws the drop shadow without
    /// it. Sampling the desktop pixels beside the window on Windows 11
    /// (build 26200) gives the same 16-level luminance ramp either way, while
    /// enabling the flag adds the 1px dark line along the top edge that winit
    /// documents. Turn it on only if your window manager disagrees.
    pub native_shadow: bool,

    /// The size of the optional application icon, in logical pixels.
    pub window_icon_size: f32,

    /// The spacing between the icon and the title, in logical pixels.
    pub title_spacing: f32,

    /// Horizontal padding around the leading icon/title region.
    pub title_padding: f32,

    /// Extra padding inserted before the leading region.
    ///
    /// On macOS this reserves room for the native traffic lights.
    pub leading_inset: f32,

    /// Whether the window title is rendered in the title bar.
    pub show_title: bool,
}

impl NativeFrameConfig {
    /// Returns the recommended configuration for the current platform.
    pub fn platform_default() -> Self {
        let base = Self {
            decoration_mode: DecorationMode::Custom,
            title_bar_height: 34.0,
            caption_button_width: 44.0,
            caption_buttons: true,
            resize_border: 6.0,
            resizable: true,

            corner_radius: 8.0,
            frame_border: true,
            client_shadow: false,
            outer_padding: 0.0,
            native_rounding: false,
            native_shadow: false,

            window_icon_size: 16.0,
            title_spacing: 8.0,
            title_padding: 8.0,
            leading_inset: 0.0,
            show_title: true,
        };

        #[cfg(windows)]
        {
            Self {
                title_bar_height: 32.0,
                caption_button_width: 46.0,
                native_rounding: true,
                native_shadow: false,
                ..base
            }
        }

        #[cfg(target_os = "linux")]
        {
            // Client-side shadows require a transparent window, and the
            // transparent padding leaks into tiled/maximized geometry on
            // Wayland. Off by default; opt in with `client_shadow(true)`.
            Self {
                client_shadow: false,
                outer_padding: 0.0,
                ..base
            }
        }

        #[cfg(target_os = "macos")]
        {
            // The native traffic lights stay visible, so we neither draw
            // caption controls nor a client-side border: the window keeps its
            // native rounding and shadow.
            Self {
                title_bar_height: 28.0,
                caption_buttons: false,
                corner_radius: 0.0,
                frame_border: false,
                leading_inset: 78.0,
                ..base
            }
        }

        #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
        {
            base
        }
    }

    /// Sets the [`DecorationMode`].
    #[must_use]
    pub fn decoration_mode(mut self, mode: DecorationMode) -> Self {
        self.decoration_mode = mode;
        self
    }

    /// Sets the height of the title bar, in logical pixels.
    #[must_use]
    pub fn title_bar_height(mut self, height: f32) -> Self {
        self.title_bar_height = height;
        self
    }

    /// Sets the width of a single caption control, in logical pixels.
    #[must_use]
    pub fn caption_button_width(mut self, width: f32) -> Self {
        self.caption_button_width = width;
        self
    }

    /// Sets whether the frame draws its own caption controls.
    #[must_use]
    pub fn caption_buttons(mut self, enabled: bool) -> Self {
        self.caption_buttons = enabled;
        self
    }

    /// Sets the thickness of the resize border, in logical pixels.
    #[must_use]
    pub fn resize_border(mut self, thickness: f32) -> Self {
        self.resize_border = thickness;
        self
    }

    /// Sets whether the window can be resized.
    #[must_use]
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Sets the radius of the client-side rounded corners.
    #[must_use]
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }

    /// Sets whether the frame draws a border around the surface.
    #[must_use]
    pub fn frame_border(mut self, enabled: bool) -> Self {
        self.frame_border = enabled;
        self
    }

    /// Enables the client-side shadow and the transparent padding it needs.
    #[must_use]
    pub fn client_shadow(mut self, enabled: bool) -> Self {
        self.client_shadow = enabled;

        if enabled && self.outer_padding <= 0.0 {
            self.outer_padding = 12.0;
        }

        self
    }

    /// Sets the transparent padding around the surface.
    #[must_use]
    pub fn outer_padding(mut self, padding: f32) -> Self {
        self.outer_padding = padding;
        self
    }

    /// Sets whether the platform is asked to round the window corners.
    #[must_use]
    pub fn native_rounding(mut self, enabled: bool) -> Self {
        self.native_rounding = enabled;
        self
    }

    /// Sets whether the platform is asked to draw an undecorated shadow.
    #[must_use]
    pub fn native_shadow(mut self, enabled: bool) -> Self {
        self.native_shadow = enabled;
        self
    }

    /// Sets the size of the optional application icon.
    #[must_use]
    pub fn window_icon_size(mut self, size: f32) -> Self {
        self.window_icon_size = size;
        self
    }

    /// Sets the spacing between the icon and the title.
    #[must_use]
    pub fn title_spacing(mut self, spacing: f32) -> Self {
        self.title_spacing = spacing;
        self
    }

    /// Sets the horizontal padding around the leading icon/title region.
    #[must_use]
    pub fn title_padding(mut self, padding: f32) -> Self {
        self.title_padding = padding;
        self
    }

    /// Sets the extra padding inserted before the leading region.
    #[must_use]
    pub fn leading_inset(mut self, inset: f32) -> Self {
        self.leading_inset = inset;
        self
    }

    /// Sets whether the window title is rendered in the title bar.
    #[must_use]
    pub fn show_title(mut self, enabled: bool) -> Self {
        self.show_title = enabled;
        self
    }

    /// The padding that separates the surface from the window edges.
    pub(crate) fn surface_padding(&self) -> f32 {
        if self.decoration_mode.uses_custom_frame() {
            self.outer_padding
        } else {
            0.0
        }
    }
}

impl Default for NativeFrameConfig {
    fn default() -> Self {
        Self::platform_default()
    }
}
