//! A custom, theme-aware window frame for Iced.
//!
//! # Architecture
//!
//! Every portable operation goes through Iced's winit-backed
//! [`iced::window`] API: [`window::drag`], [`window::drag_resize`],
//! [`window::minimize`], [`window::toggle_maximize`], [`window::close`],
//! [`window::show_system_menu`] and [`window::is_maximized`].
//!
//! The only native code in the library is a `WM_NCHITTEST` subclass on Windows
//! (see [`platform::windows`]), because the Windows 11 Snap Layout flyout,
//! the caption tooltips and the native resize cursors are all driven by the
//! value an application returns for that one message, and winit exposes no
//! hook for it. See [`platform`] for the full rationale.
//!
//! # Usage
//!
//! ```ignore
//! let frame = NativeFrame::new(
//!     NativeFrameConfig::platform_default()
//!         .decoration_mode(DecorationMode::Custom),
//! );
//!
//! let settings = frame.window_settings(window::Settings {
//!     resizable: true,
//!     ..window::Settings::default()
//! });
//! ```

pub mod action;
pub mod config;

mod icons;
mod platform;
mod resize_handles;
mod style;

pub use action::{CaptionControl, FrameAction};
pub use config::{DecorationMode, NativeFrameConfig};

use iced::widget::{column, container, mouse_area, row, space, text};
use iced::{Center, Element, Fill, Padding, Subscription, Task, Theme, window};

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

/// The frame state for a single window.
///
/// One of these exists per [`window::Id`], so a single [`NativeFrame`] can
/// serve any number of windows without their hover, maximized or activation
/// states colliding.
///
/// On Windows the values are written by the subclass, which runs on the winit
/// thread and cannot hold a `&mut` into the application — hence the atomics.
#[derive(Debug)]
pub(crate) struct WindowState {
    pub(crate) config: NativeFrameConfig,
    pub(crate) hot: AtomicU8,
    pub(crate) pressed: AtomicU8,
    pub(crate) maximized: AtomicBool,
    pub(crate) active: AtomicBool,
}

/// The `hot` / `pressed` encoding: zero means "no control".
pub(crate) fn raw_control(control: Option<CaptionControl>) -> u8 {
    control.map_or(0, |control| control as u8)
}

impl WindowState {
    fn new(config: NativeFrameConfig) -> Self {
        Self {
            config,
            hot: AtomicU8::new(0),
            pressed: AtomicU8::new(0),
            maximized: AtomicBool::new(false),
            active: AtomicBool::new(true),
        }
    }

    pub(crate) fn hot(&self) -> Option<CaptionControl> {
        CaptionControl::from_raw(self.hot.load(Ordering::Acquire))
    }

    pub(crate) fn pressed(&self) -> Option<CaptionControl> {
        CaptionControl::from_raw(self.pressed.load(Ordering::Acquire))
    }

    fn set_hot(&self, value: Option<CaptionControl>) {
        self.hot.store(raw_control(value), Ordering::Release);
    }

    fn set_pressed(&self, value: Option<CaptionControl>) {
        self.pressed.store(raw_control(value), Ordering::Release);
    }

    fn is_maximized(&self) -> bool {
        self.maximized.load(Ordering::Acquire)
    }

    fn set_maximized(&self, value: bool) {
        self.maximized.store(value, Ordering::Release);
    }

    fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    fn set_active(&self, value: bool) {
        self.active.store(value, Ordering::Release);
    }

    fn caption_state(&self, control: CaptionControl) -> style::CaptionState {
        style::CaptionState {
            hot: self.hot() == Some(control),
            pressed: self.pressed() == Some(control),
            active: self.is_active(),
        }
    }
}

type Registry = Mutex<HashMap<window::Id, Arc<WindowState>>>;

/// A custom window frame.
///
/// Cheap to clone: every clone shares the same per-window state, so a frame
/// stored in your application and a frame captured in a closure stay in sync.
///
/// A single frame can drive several windows; call [`NativeFrame::install`] and
/// [`NativeFrame::view`] once per [`window::Id`].
///
/// A single-window application can use [`NativeFrame::install_latest`] and
/// [`NativeFrame::decorate`] instead and never name a [`window::Id`] at all.
#[derive(Debug, Clone)]
pub struct NativeFrame {
    config: NativeFrameConfig,
    windows: Arc<Registry>,
    primary: Arc<Mutex<Option<window::Id>>>,
}

impl NativeFrame {
    /// Creates a frame from a [`NativeFrameConfig`].
    pub fn new(config: NativeFrameConfig) -> Self {
        Self {
            config,
            windows: Arc::new(Mutex::new(HashMap::new())),
            primary: Arc::new(Mutex::new(None)),
        }
    }

    /// The configuration this frame was created with.
    pub fn config(&self) -> NativeFrameConfig {
        self.config
    }

    /// The [`DecorationMode`] this frame was created with.
    pub fn decoration_mode(&self) -> DecorationMode {
        self.config.decoration_mode
    }

    /// The state for `window_id`, creating it if this is the first sighting.
    fn state(&self, window_id: window::Id) -> Arc<WindowState> {
        Arc::clone(
            self.registry()
                .entry(window_id)
                .or_insert_with(|| Arc::new(WindowState::new(self.config))),
        )
    }

    /// The state for `window_id`, without creating it.
    fn peek(&self, window_id: window::Id) -> Option<Arc<WindowState>> {
        self.registry().get(&window_id).map(Arc::clone)
    }

    fn registry(&self) -> std::sync::MutexGuard<'_, HashMap<window::Id, Arc<WindowState>>> {
        self.windows
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn primary(&self) -> std::sync::MutexGuard<'_, Option<window::Id>> {
        self.primary
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The first window this frame was installed on.
    ///
    /// This is what [`NativeFrame::decorate`] decorates. It is `None` until
    /// [`NativeFrame::install`] has run, and becomes `None` again when that
    /// window closes.
    pub fn primary_window(&self) -> Option<window::Id> {
        *self.primary()
    }

    /// Drops every trace of `window_id` from the shared state.
    fn forget(&self, window_id: window::Id) {
        let _ = self.registry().remove(&window_id);

        // Taken after the registry guard is released; the two locks are never
        // held at the same time.
        let mut primary = self.primary();

        if *primary == Some(window_id) {
            *primary = None;
        }
    }

    /// Whether `window_id` is currently maximized, as last observed.
    pub fn is_maximized(&self, window_id: window::Id) -> bool {
        self.peek(window_id)
            .is_some_and(|state| state.is_maximized())
    }

    /// Whether `window_id` is currently focused, as last observed.
    ///
    /// Defaults to `true` for a window the frame has not seen yet, so an
    /// unknown window renders as active rather than dimmed.
    pub fn is_active(&self, window_id: window::Id) -> bool {
        self.peek(window_id).is_none_or(|state| state.is_active())
    }

    /// How many windows this frame currently tracks.
    pub fn tracked_windows(&self) -> usize {
        self.registry().len()
    }

    /// Whether [`window::show_system_menu`] does anything on this platform.
    ///
    /// `false` on X11 and macOS. Use it to decide whether to fall back to your
    /// own popup when handling [`FrameAction::WindowIconPressed`].
    ///
    /// On Linux this reports the backend winit actually selected, once
    /// [`NativeFrame::install`] has run; see [`platform::supports_system_menu`].
    pub fn supports_system_menu(&self) -> bool {
        platform::supports_system_menu()
    }

    /// Applies the frame's platform-specific requirements to
    /// [`window::Settings`].
    ///
    /// This is the one helper an application needs; it replaces every
    /// `#[cfg(...)]` branch a consumer would otherwise have to write. It sets
    /// `decorations` and `resizable`, and on top of that:
    ///
    /// * **Windows** — `corner_preference` and `undecorated_shadow`, so DWM
    ///   rounds the window.
    /// * **Linux** — `transparent`, but only when the client-side shadow is
    ///   actually enabled.
    /// * **macOS** — `title_hidden`, `titlebar_transparent` and
    ///   `fullsize_content_view`, so the native traffic lights survive while
    ///   the application owns the whole title-bar area.
    pub fn window_settings(&self, settings: window::Settings) -> window::Settings {
        platform::window_settings(settings, self.config)
    }

    /// Prepares `window_id` for use with this frame.
    ///
    /// Call it once per window. On Windows in [`DecorationMode::Custom`] it
    /// installs the `WM_NCHITTEST` subclass; on Linux it records which winit
    /// backend is in use. Everywhere else it just registers the window.
    pub fn install(&self, window_id: window::Id) -> Task<Result<(), String>> {
        let state = self.state(window_id);

        // The first window installed becomes the one `decorate` decorates.
        self.primary().get_or_insert(window_id);

        platform::install(window_id, state)
    }

    /// Installs on the window Iced most recently opened.
    ///
    /// The single-window counterpart to [`NativeFrame::install`]: it folds the
    /// [`window::latest`] query into the same [`Task`], so a single-window
    /// application never has to hold a [`window::Id`] of its own. Pair it with
    /// [`NativeFrame::decorate`].
    ///
    /// ```ignore
    /// iced::application(
    ///     move || (App::default(), frame.install_latest().discard()),
    ///     App::update,
    ///     App::view,
    /// )
    /// ```
    ///
    /// Use [`NativeFrame::install`] when you open windows yourself and already
    /// know which one you mean.
    pub fn install_latest(&self) -> Task<Result<(), String>> {
        let frame = self.clone();

        window::latest().then(move |window_id| match window_id {
            Some(window_id) => frame.install(window_id),

            None => Task::done(Err(String::from(
                "no window is open; `install_latest` runs after the first window",
            ))),
        })
    }

    /// Releases everything [`NativeFrame::install`] set up for `window_id`.
    ///
    /// [`NativeFrame::subscription`] already does this when the window closes;
    /// call it directly only to detach early.
    pub fn uninstall(&self, window_id: window::Id) -> Task<()> {
        self.forget(window_id);

        platform::uninstall(window_id)
    }

    /// Window events the frame needs in order to mirror the platform state.
    ///
    /// On Windows in [`DecorationMode::Custom`] this narrows to close events
    /// alone: the subclass already reports maximization and activation
    /// authoritatively, including the changes the frame never sees, such as
    /// Aero Snap and <kbd>Win</kbd>+<kbd>Up</kbd>.
    ///
    /// Forward it to [`NativeFrame::update`] or the frame will keep per-window
    /// state for windows that no longer exist.
    pub fn subscription(&self) -> Subscription<FrameAction> {
        // `Subscription::map` and `filter_map` require a zero-sized closure, so
        // the mode is branched on out here rather than captured.
        if platform::NATIVE_CAPTION_HIT_TEST && self.config.decoration_mode.uses_custom_frame() {
            return window::close_events().map(FrameAction::WindowClosed);
        }

        window::events().filter_map(|(id, event)| match event {
            window::Event::Closed => Some(FrameAction::WindowClosed(id)),
            window::Event::Focused => Some(FrameAction::FocusChanged(id, true)),
            window::Event::Unfocused => Some(FrameAction::FocusChanged(id, false)),
            window::Event::Resized(_) | window::Event::Opened { .. } => {
                Some(FrameAction::SyncState(id))
            }
            _ => None,
        })
    }

    /// Handles a [`FrameAction`].
    ///
    /// `map_action` is the same constructor passed to [`NativeFrame::view`];
    /// it lets the frame chain follow-up state queries back into your update
    /// loop.
    pub fn update<Message>(
        &self,
        action: FrameAction,
        map_action: impl Fn(FrameAction) -> Message + Clone + Send + 'static,
    ) -> Task<Message>
    where
        Message: Send + 'static,
    {
        match action {
            FrameAction::Drag(id) => window::drag(id),
            FrameAction::Resize(id, direction) => window::drag_resize(id, direction),
            FrameAction::Minimize(id) => window::minimize(id, true),
            FrameAction::Close(id) => window::close(id),

            FrameAction::ToggleMaximize(id) => {
                let state = self.state(id);

                // Optimistic, so the glyph flips on the same frame; the
                // chained query corrects it if the platform disagrees.
                state.set_maximized(!state.is_maximized());

                window::toggle_maximize(id).chain(sync_state(id, map_action))
            }

            // The documented default: show the system menu where the platform
            // has one. Intercept `WindowIconPressed` in your own `update` to
            // do something else instead.
            FrameAction::WindowIconPressed(id) | FrameAction::ShowSystemMenu(id) => {
                if platform::supports_system_menu() {
                    window::show_system_menu(id)
                } else {
                    Task::none()
                }
            }

            FrameAction::Hover(id, control) => {
                self.state(id).set_hot(Some(control));

                Task::none()
            }

            FrameAction::Leave(id, control) => {
                let state = self.state(id);

                if state.hot() == Some(control) {
                    state.set_hot(None);
                }

                state.set_pressed(None);

                Task::none()
            }

            FrameAction::MaximizedChanged(id, maximized) => {
                self.state(id).set_maximized(maximized);

                Task::none()
            }

            FrameAction::FocusChanged(id, active) => {
                self.state(id).set_active(active);

                Task::none()
            }

            FrameAction::SyncState(id) => sync_state(id, map_action),

            FrameAction::WindowClosed(id) => {
                self.forget(id);

                Task::none()
            }
        }
    }

    /// Wraps `content` in the frame.
    ///
    /// In [`DecorationMode::System`] this returns `content` untouched.
    ///
    /// The title bar is laid out as:
    ///
    /// ```text
    /// [leading inset][icon][title][application-owned fill row][caption controls]
    /// ```
    ///
    /// `title_content` receives all the width left between the title and the
    /// caption controls, and is free to contain buttons, menus or anything
    /// else interactive. Children that capture a click keep it; anything that
    /// does not — including [`iced::widget::space::horizontal`] — falls
    /// through to the draggable region behind it.
    pub fn view<'a, Message>(
        &self,
        window_id: window::Id,
        title: &'a str,
        window_icon: Option<Element<'a, Message>>,
        title_content: Option<Element<'a, Message>>,
        content: impl Into<Element<'a, Message>>,
        map_action: impl Fn(FrameAction) -> Message + Clone + 'a,
    ) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        let config = self.config;

        if !config.decoration_mode.uses_custom_frame() {
            return content.into();
        }

        let state = self.state(window_id);

        // The 1px surface border follows the platform accent while the window
        // is active, and falls back to the neutral palette color everywhere
        // else — see [`platform::border_color`].
        let border = {
            let state = Arc::clone(&state);

            move |theme: &Theme| {
                platform::border_color(
                    theme.extended_palette().background.strong.color,
                    state.is_active(),
                    state.is_maximized(),
                )
            }
        };

        let surface = container(
            column![
                title_bar(
                    &state,
                    window_id,
                    title,
                    window_icon,
                    title_content,
                    map_action.clone(),
                ),
                content.into(),
            ]
            .width(Fill)
            .height(Fill),
        )
        .width(Fill)
        .height(Fill)
        .clip(true)
        .style(move |theme| style::surface(theme, config, border(theme)));

        let padding = config.surface_padding();

        let framed: Element<'a, Message> = if padding > 0.0 {
            container(surface)
                .padding(padding)
                .width(Fill)
                .height(Fill)
                .into()
        } else {
            surface.into()
        };

        if config.resizable && platform::USES_OVERLAY_RESIZE_HANDLES {
            resize_handles::overlay(window_id, config.resize_border, framed, map_action)
        } else {
            framed
        }
    }

    /// Wraps `content` in the frame, on the window [`NativeFrame::install`]
    /// first ran on.
    ///
    /// The single-window counterpart to [`NativeFrame::view`]. Because
    /// [`iced::application`] does not hand its view function a
    /// [`window::Id`] — only [`iced::daemon`] does — this reads the id back
    /// from the frame instead, which is what lets a single-window application
    /// drop the `window::Id` it would otherwise have to keep in its state.
    ///
    /// Until [`NativeFrame::install_latest`] resolves, `content` is returned
    /// undecorated. In practice that is the first frame only.
    ///
    /// The title bar it draws has no icon and no application-owned row. Reach
    /// for [`NativeFrame::view`] with [`NativeFrame::primary_window`] when you
    /// want either:
    ///
    /// ```ignore
    /// let Some(window_id) = self.frame.primary_window() else {
    ///     return content.into();
    /// };
    ///
    /// self.frame.view(window_id, "Title", Some(icon), Some(menus), content, Message::Frame)
    /// ```
    pub fn decorate<'a, Message>(
        &self,
        title: &'a str,
        content: impl Into<Element<'a, Message>>,
        map_action: impl Fn(FrameAction) -> Message + Clone + 'a,
    ) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        match self.primary_window() {
            Some(window_id) => self.view(window_id, title, None, None, content, map_action),
            None => content.into(),
        }
    }
}

impl Default for NativeFrame {
    fn default() -> Self {
        Self::new(NativeFrameConfig::default())
    }
}

fn title_bar<'a, Message>(
    state: &Arc<WindowState>,
    window_id: window::Id,
    title: &'a str,
    window_icon: Option<Element<'a, Message>>,
    title_content: Option<Element<'a, Message>>,
    map_action: impl Fn(FrameAction) -> Message + Clone + 'a,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let config = state.config;

    let mut bar = row![]
        .width(Fill)
        .height(config.title_bar_height)
        .align_y(Center);

    // Reserved, non-interactive room for platform-drawn controls — the macOS
    // traffic lights. Left outside the draggable region so it never competes
    // with them.
    if config.leading_inset > 0.0 {
        bar = bar.push(space().width(config.leading_inset));
    }

    // [ clickable icon ][ draggable title ]
    //
    // The icon's own `mouse_area` captures its press first, so clicking it
    // publishes `WindowIconPressed` instead of starting a window drag.
    let mut leading = row![].spacing(config.title_spacing).align_y(Center);

    if let Some(icon) = window_icon {
        leading = leading.push(
            mouse_area(
                container(icon)
                    .width(config.window_icon_size)
                    .height(config.window_icon_size)
                    .center_x(config.window_icon_size)
                    .center_y(config.window_icon_size)
                    .clip(true),
            )
            // No `.interaction(..)`: a native title-bar icon keeps the normal
            // arrow cursor.
            .on_press(map_action.clone()(FrameAction::WindowIconPressed(
                window_id,
            ))),
        );
    }

    if config.show_title {
        leading = leading.push(text(title).size(13));
    }

    let leading = container(leading)
        .padding(Padding {
            top: 0.0,
            right: config.title_padding,
            bottom: 0.0,
            left: config.title_padding,
        })
        .height(config.title_bar_height)
        .center_y(config.title_bar_height);

    bar = bar.push(draggable(window_id, leading, map_action.clone()));

    // The application-owned row consumes every remaining pixel between the
    // title and the caption controls.
    let filler: Element<'a, Message> = match title_content {
        Some(content) => container(content)
            .width(Fill)
            .height(config.title_bar_height)
            .center_y(config.title_bar_height)
            .into(),

        None => space().width(Fill).height(config.title_bar_height).into(),
    };

    bar = bar.push(draggable(window_id, filler, map_action.clone()));

    if config.caption_buttons {
        let controls = CaptionControl::ALL
            .map(|control| caption_button(state, window_id, control, map_action.clone()));

        bar = bar.push(
            row(controls)
                .height(config.title_bar_height)
                .align_y(Center),
        );
    }

    container(bar)
        .width(Fill)
        .height(config.title_bar_height)
        .style(style::title_bar)
        .into()
}

/// Wraps `content` so that a press that no child captured drags the window.
///
/// [`iced::widget::MouseArea`] checks `shell.is_event_captured()` before
/// acting, which is what lets buttons and menus inside the title bar stay
/// clickable while the gaps between them remain draggable.
fn draggable<'a, Message>(
    window_id: window::Id,
    content: impl Into<Element<'a, Message>>,
    map_action: impl Fn(FrameAction) -> Message + Clone + 'a,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    mouse_area(content)
        .on_press(map_action.clone()(FrameAction::Drag(window_id)))
        .on_double_click(map_action.clone()(FrameAction::ToggleMaximize(window_id)))
        .on_right_press(map_action(FrameAction::ShowSystemMenu(window_id)))
        .into()
}

fn caption_button<'a, Message>(
    state: &Arc<WindowState>,
    window_id: window::Id,
    control: CaptionControl,
    map_action: impl Fn(FrameAction) -> Message + Clone + 'a,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let config = state.config;

    let icon_size = match control {
        CaptionControl::Minimize | CaptionControl::Close => 16.0,
        CaptionControl::Maximize => 12.0,
    };

    let icon = iced::widget::svg(icons::handle(control, state.is_maximized()))
        .width(icon_size)
        .height(icon_size)
        .style({
            let state = Arc::clone(state);

            move |theme: &Theme, _status| iced::widget::svg::Style {
                color: Some(style::caption_icon(
                    theme,
                    control,
                    state.caption_state(control),
                )),
            }
        });

    let visual = container(icon)
        .width(config.caption_button_width)
        .height(config.title_bar_height)
        .center_x(config.caption_button_width)
        .center_y(config.title_bar_height)
        .style({
            let state = Arc::clone(state);

            move |theme| style::caption_button(theme, control, state.caption_state(control))
        });

    // On Windows the press, the hover and the resulting command are all
    // native: the button region is reported as `HTMINBUTTON` / `HTMAXBUTTON` /
    // `HTCLOSE`, which is what makes the Windows 11 Snap Layout flyout appear
    // over the maximize button. Attaching a `mouse_area` here would be dead
    // code — Iced never sees those pixels.
    if platform::NATIVE_CAPTION_HIT_TEST {
        let _ = (window_id, map_action);

        return visual.into();
    }

    let action = match control {
        CaptionControl::Minimize => FrameAction::Minimize(window_id),
        CaptionControl::Maximize => FrameAction::ToggleMaximize(window_id),
        CaptionControl::Close => FrameAction::Close(window_id),
    };

    mouse_area(visual)
        .on_enter(map_action.clone()(FrameAction::Hover(window_id, control)))
        .on_exit(map_action.clone()(FrameAction::Leave(window_id, control)))
        .on_press(map_action(action))
        .interaction(iced::mouse::Interaction::Pointer)
        .into()
}

fn sync_state<Message>(
    id: window::Id,
    map_action: impl Fn(FrameAction) -> Message + Send + 'static,
) -> Task<Message>
where
    Message: Send + 'static,
{
    window::is_maximized(id)
        .map(move |maximized| map_action(FrameAction::MaximizedChanged(id, maximized)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(action: FrameAction) -> FrameAction {
        action
    }

    #[test]
    fn state_is_scoped_per_window() {
        let frame = NativeFrame::default();

        let a = window::Id::unique();
        let b = window::Id::unique();

        let _ = frame.update(FrameAction::MaximizedChanged(a, true), identity);
        let _ = frame.update(FrameAction::Hover(a, CaptionControl::Close), identity);
        let _ = frame.update(FrameAction::FocusChanged(a, false), identity);

        assert!(frame.is_maximized(a));
        assert!(!frame.is_active(a));

        // `b` is untouched by anything that happened to `a`.
        assert!(!frame.is_maximized(b));
        assert!(frame.is_active(b));

        let _ = frame.update(FrameAction::MaximizedChanged(b, true), identity);

        assert!(frame.is_maximized(a));
        assert!(frame.is_maximized(b));
    }

    #[test]
    fn clones_share_the_same_per_window_state() {
        let frame = NativeFrame::default();
        let clone = frame.clone();

        let id = window::Id::unique();
        let _ = frame.update(FrameAction::MaximizedChanged(id, true), identity);

        assert!(clone.is_maximized(id));
    }

    #[test]
    fn closing_a_window_releases_its_state() {
        let frame = NativeFrame::default();

        let a = window::Id::unique();
        let b = window::Id::unique();

        let _ = frame.update(FrameAction::MaximizedChanged(a, true), identity);
        let _ = frame.update(FrameAction::MaximizedChanged(b, true), identity);
        assert_eq!(frame.tracked_windows(), 2);

        let _ = frame.update(FrameAction::WindowClosed(a), identity);

        assert_eq!(frame.tracked_windows(), 1);
        assert!(!frame.is_maximized(a));
        assert!(frame.is_maximized(b));
    }

    #[test]
    fn the_first_installed_window_becomes_the_primary_one() {
        let frame = NativeFrame::default();

        let a = window::Id::unique();
        let b = window::Id::unique();

        assert_eq!(frame.primary_window(), None);

        // The tasks are lazy, so nothing native runs here.
        let _ = frame.install(a);
        assert_eq!(frame.primary_window(), Some(a));

        // A second window joins the registry without stealing the primary.
        let _ = frame.install(b);
        assert_eq!(frame.primary_window(), Some(a));

        // Closing the secondary leaves the primary alone.
        let _ = frame.update(FrameAction::WindowClosed(b), identity);
        assert_eq!(frame.primary_window(), Some(a));

        let _ = frame.update(FrameAction::WindowClosed(a), identity);
        assert_eq!(frame.primary_window(), None);
        assert_eq!(frame.tracked_windows(), 0);
    }

    #[test]
    fn clones_share_the_primary_window() {
        let frame = NativeFrame::default();
        let clone = frame.clone();

        let id = window::Id::unique();
        let _ = frame.install(id);

        assert_eq!(clone.primary_window(), Some(id));
    }

    #[test]
    fn reading_an_unknown_window_does_not_register_it() {
        let frame = NativeFrame::default();
        let id = window::Id::unique();

        assert!(!frame.is_maximized(id));
        assert!(frame.is_active(id));
        assert_eq!(frame.tracked_windows(), 0);
    }

    #[test]
    fn leave_only_clears_the_control_it_names() {
        let frame = NativeFrame::default();
        let id = window::Id::unique();

        let _ = frame.update(FrameAction::Hover(id, CaptionControl::Close), identity);

        // A stale `Leave` for a different control must not clear the hover.
        let _ = frame.update(FrameAction::Leave(id, CaptionControl::Minimize), identity);
        assert_eq!(frame.state(id).hot(), Some(CaptionControl::Close));

        let _ = frame.update(FrameAction::Leave(id, CaptionControl::Close), identity);
        assert_eq!(frame.state(id).hot(), None);
    }

    #[test]
    fn every_action_names_its_window() {
        let id = window::Id::unique();

        for action in [
            FrameAction::Drag(id),
            FrameAction::Resize(id, window::Direction::North),
            FrameAction::Minimize(id),
            FrameAction::ToggleMaximize(id),
            FrameAction::Close(id),
            FrameAction::WindowIconPressed(id),
            FrameAction::ShowSystemMenu(id),
            FrameAction::Hover(id, CaptionControl::Close),
            FrameAction::Leave(id, CaptionControl::Close),
            FrameAction::MaximizedChanged(id, true),
            FrameAction::FocusChanged(id, true),
            FrameAction::SyncState(id),
            FrameAction::WindowClosed(id),
        ] {
            assert_eq!(action.window_id(), id, "{action:?} lost its window id");
        }
    }
}
