//! Use a color picker as an input element for picking colors.
//!
//! Ported from `iced_aw`'s `widget::color_picker` module, with the dialog
//! reworked to a single-column three-tab layout:
//!
//! * top-level `[Color | Gradient | Library]` tabs (in the draggable header
//!   for the floating window, on top for the inline widget);
//! * Color tab: saturation/value square + hue slider below it, HSV/RGB(A)
//!   tabbed gradient sliders, value fields and a hex input;
//! * Gradient tab: two-stop linear bar (no type selector) + Rect/HSV/RGBA
//!   editor for the selected stop, value fields and a hex input;
//! * Library tab: tabbed swatch sets (with an add-set name prompt and
//!   per-set close marks), an add-current-color button and a recent colors
//!   grid;
//! * shared footer in all tabs: Original/New preview panels and the
//!   Reset/Eyedropper/OK buttons below the hex input.
//!
//! The dialog content is available in two shapes:
//!
//! * [`ColorPicker`] — a generic, always visible widget that can be planted
//!   into any builder like a regular widget. There is no button to spawn
//!   it: the application controls visibility by planting or not planting
//!   the element.
//! * [`FloatingColorPicker`] — wraps an underlay (typically a button) and,
//!   while shown, spawns the same dialog inside a free-floating window-like
//!   shell: a draggable header with `[Color | Gradient | Library]` tabs, a
//!   drag area and a close ("x") button. It is still a regular overlay/widget,
//!   not a separate OS window; the dragged position survives close/reopen.
//!
//! Swatches and recent colors are kept in memory only (no persistence),
//! and all styling is derived from the active iced `Theme` palette.
//!
//! # Example (inline)
//! ```no_run
//! # use neverliie_iced_widgets::color_picker::{color_picker, ColorPicker};
//! # use iced::{Color, Element};
//! #
//! #[derive(Clone, Debug)]
//! enum Message {
//!     Cancel,
//!     Submit(Color),
//! }
//!
//! let picker = color_picker(Color::default(), Message::Cancel, Message::Submit);
//! # let _: Element<Message> = picker.into();
//! ```
//!
//! # Example (floating)
//! ```no_run
//! # use neverliie_iced_widgets::color_picker::floating_color_picker;
//! # use neverliie_iced_widgets::overlay::Position;
//! # use iced::{Color, Element, widget::button};
//! #
//! #[derive(Clone, Debug)]
//! enum Message {
//!     Open,
//!     Cancel,
//!     Submit(Color),
//! }
//!
//! let floating = floating_color_picker(
//!     true,
//!     Color::default(),
//!     button("Pick color").on_press(Message::Open),
//!     Message::Cancel,
//!     Message::Submit,
//! )
//! .position(Position::BottomRight);
//! # let _: Element<Message> = floating.into();
//! ```

mod color;
mod dropper;
pub mod gradient;
pub(crate) mod overlay;
pub mod style;
pub mod style_state;

pub use dropper::DropperBuffer;
pub use gradient::{Gradient, GradientStop, PickedValue};
pub use self::overlay::{PickerTab, SwatchSet, MAX_RECENT, MAX_SWATCHES_PER_SET};

use self::dropper::DropperMode;
use self::overlay::{
    ColorBarDragged, ColorPickerOverlay, ColorPickerOverlayButtons, ColorPickerWindow, DropperLens,
};
use self::style::{Status, Style, StyleFn};

use crate::overlay::Position;

use std::cell::UnsafeCell;

use iced::{
    advanced::{
        layout::{Limits, Node},
        mouse::{self, Cursor},
        renderer,
        widget::{
            Operation,
            tree::{self, Tag, Tree},
        },
        Clipboard, Layout, Shell, Widget,
    },
    widget::Renderer,
    Color, Element, Event, Length, Point, Rectangle, Size, Vector,
};

//TODO: Remove ignore when Null is updated. Temp fix for Test runs
/// A color picker widget that can be planted anywhere in a builder.
///
/// Unlike [`FloatingColorPicker`], this is a plain always visible widget:
/// there is no underlay and no spawn flag. Render it whenever the dialog
/// should be visible and stop rendering it when it should be closed.
///
/// While mounted it "owns" the picked value internally; the `color`
/// argument only re-seeds the selection when the application passes a new
/// value that differs from both the previous argument and the current
/// internal selection.
///
/// # Example
/// ```ignore
/// # use neverliie_iced_widgets::color_picker::ColorPicker;
/// # use iced::{Color};
/// #
/// #[derive(Clone, Debug)]
/// enum Message {
///     Cancel,
///     Submit(Color),
/// }
///
/// let picker = ColorPicker::new(
///     Color::default(),
///     Message::Cancel,
///     Message::Submit,
/// );
/// ```
#[allow(missing_debug_implementations)]
pub struct ColorPicker<'a, Message, Theme = iced::Theme>
where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog,
{
    /// The color to show.
    color: Color,
    /// A placeholder message with no user-facing effect: the inline dialog
    /// has no cancel button, and its internal controls only publish it as a
    /// dummy value that the widget intercepts.
    on_cancel: Message,
    /// The function that produces a message when the submit button of the dialog is pressed.
    on_submit: Box<dyn Fn(Color) -> Message>,
    /// Optional function that produces a message when the color changes during selection (real-time updates).
    on_color_change: Option<Box<dyn Fn(Color) -> Message>>,
    /// Initial gradient for the Gradient tab (`None` = solid `color`).
    gradient: Option<Gradient>,
    /// Optional function producing a message with the gradient when submit
    /// is pressed while the Gradient tab is active.
    on_gradient_submit: Option<Box<dyn Fn(Gradient) -> Message>>,
    /// Optional function producing a message when the gradient changes
    /// during selection (real-time updates).
    on_gradient_change: Option<Box<dyn Fn(Gradient) -> Message>>,
    /// Optional unified change callback with the picked value (solid color
    /// or gradient) for the active tab.
    on_pick: Option<Box<dyn Fn(PickedValue) -> Message>>,
    /// Optional unified submit callback with the picked value (solid color
    /// or gradient) for the active tab.
    on_pick_submit: Option<Box<dyn Fn(PickedValue) -> Message>>,
    /// Optional function producing a message when the top-level tab
    /// (`Color | Gradient | Library`) changes.
    on_tab_change: Option<Box<dyn Fn(PickerTab) -> Message>>,
    /// Optional function producing a message when the Library mutates
    /// (swatch sets, recents or active set). The app persists the payload.
    on_library_change: Option<Box<dyn Fn(Vec<SwatchSet>, Vec<PickedValue>, usize) -> Message>>,
    /// Shared buffer receiving window screenshots for the eye dropper; the
    /// eyedropper button stays disabled while this is `None`.
    dropper_buffer: Option<DropperBuffer>,
    /// Optional function producing the message published when the user
    /// activates the eye dropper and a fresh capture is needed.
    on_dropper_capture: Option<Box<dyn Fn() -> Message>>,
    /// Persisted swatch sets to seed/restore the Library tab (`None` =
    /// widget owns them after first creation).
    swatches: Option<Vec<SwatchSet>>,
    /// Persisted recent colors to seed/restore the Library tab (`None` =
    /// widget owns them after first creation).
    recent_colors: Option<Vec<PickedValue>>,
    /// Persisted active swatch tab index (`None` = keep internal).
    active_swatch_tab: Option<usize>,
    /// The style of the dialog.
    class: <Theme as style::Catalog>::Class<'a>,
    /// Tree state holder for the dialog's buttons.
    content_state: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Theme> ColorPicker<'a, Message, Theme>
where
    Message: 'a + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
    for<'c> <Theme as iced::widget::text_input::Catalog>::Class<'c>:
        From<iced::widget::text_input::StyleFn<'c, Theme>>,
{
    /// Builds the transient dialog content view from shared references.
    ///
    /// Builds the transient dialog content view from shared references.
    ///
    /// Used only by the read-only passes (`draw`, `mouse_interaction`):
    /// neither writes to the picker state nor mutates the tree. Returns
    /// `None` before the first layout has captured the child tree handle.
    fn view_immutable<'v>(
        &'v self,
        picker_state: &'v State,
        viewport: Rectangle,
    ) -> Option<ColorPickerOverlay<'v, 'a, Message, Theme>>
    where
        Message: 'static,
    {
        let content_tree = picker_state.content_tree;
        if content_tree.is_null() {
            return None;
        }

        // SAFETY: the handle was refreshed from a unique borrow during the
        // layout/update pass of this frame; iced runs those before draw and
        // mouse interaction and never overlaps the phases.
        let tree = unsafe { &mut *content_tree };
        // SAFETY: no aliasing write occurs during read-only passes (see
        // `State::overlay_state`).
        let overlay_state = unsafe { &mut *picker_state.overlay_state.get() };

        Some(ColorPickerOverlay::new(
            overlay_state,
            self.on_cancel.clone(),
            &self.on_submit,
            self.on_color_change.as_deref(),
            self.on_gradient_submit.as_deref(),
            self.on_gradient_change.as_deref(),
            self.on_pick.as_deref(),
            self.on_pick_submit.as_deref(),
            self.on_tab_change.as_deref(),
            self.on_library_change.as_deref(),
            self.dropper_buffer.as_ref(),
            self.on_dropper_capture.as_deref(),
            false,
            &self.class,
            tree,
            viewport,
        ))
    }

    /// Creates a new inline [`ColorPicker`].
    ///
    /// It expects:
    ///     * the initial color to show.
    ///     * a placeholder message: the dialog has no cancel button, so it is
    ///         only used internally as a dummy publication (any cheap message
    ///         works).
    ///     * a function that will be called when the submit button of the [`ColorPicker`]
    ///         is pressed, which takes the picked [`Color`] value.
    pub fn new<F>(color: Color, on_cancel: Message, on_submit: F) -> Self
    where
        F: 'static + Fn(Color) -> Message,
    {
        Self {
            color,
            on_cancel,
            on_submit: Box::new(on_submit),
            on_color_change: None,
            gradient: None,
            on_gradient_submit: None,
            on_gradient_change: None,
            on_pick: None,
            on_pick_submit: None,
            on_tab_change: None,
            on_library_change: None,
            dropper_buffer: None,
            on_dropper_capture: None,
            swatches: None,
            recent_colors: None,
            active_swatch_tab: None,
            class: <Theme as style::Catalog>::default(),
            content_state: ColorPickerOverlayButtons::default().into(),
        }
    }

    /// Sets a callback that will be called whenever the color changes during selection (real-time updates).
    #[must_use]
    pub fn on_color_change<F>(mut self, on_color_change: F) -> Self
    where
        F: 'static + Fn(Color) -> Message,
    {
        self.on_color_change = Some(Box::new(on_color_change));
        self
    }

    /// Sets the initial gradient edited in the Gradient tab.
    #[must_use]
    pub fn gradient(mut self, gradient: Gradient) -> Self {
        self.gradient = Some(gradient);
        self
    }

    /// Sets the callback producing a message with the gradient when submit
    /// is pressed while the Gradient tab is active.
    #[must_use]
    pub fn on_gradient_submit<F>(mut self, on_gradient_submit: F) -> Self
    where
        F: 'static + Fn(Gradient) -> Message,
    {
        self.on_gradient_submit = Some(Box::new(on_gradient_submit));
        self
    }

    /// Sets the callback producing a message when the gradient changes
    /// during selection (real-time updates).
    #[must_use]
    pub fn on_gradient_change<F>(mut self, on_gradient_change: F) -> Self
    where
        F: 'static + Fn(Gradient) -> Message,
    {
        self.on_gradient_change = Some(Box::new(on_gradient_change));
        self
    }

    /// Sets a unified callback with the picked value (solid or gradient)
    /// for the active tab on every selection change.
    #[must_use]
    pub fn on_pick<F>(mut self, on_pick: F) -> Self
    where
        F: 'static + Fn(PickedValue) -> Message,
    {
        self.on_pick = Some(Box::new(on_pick));
        self
    }

    /// Sets a unified callback with the picked value (solid or gradient)
    /// for the active tab when submit is pressed.
    #[must_use]
    pub fn on_pick_submit<F>(mut self, on_pick_submit: F) -> Self
    where
        F: 'static + Fn(PickedValue) -> Message,
    {
        self.on_pick_submit = Some(Box::new(on_pick_submit));
        self
    }

    /// Sets a callback producing a message when the top-level tab
    /// (`Color | Gradient | Library`) changes.
    #[must_use]
    pub fn on_tab_change<F>(mut self, on_tab_change: F) -> Self
    where
        F: 'static + Fn(PickerTab) -> Message,
    {
        self.on_tab_change = Some(Box::new(on_tab_change));
        self
    }

    /// Sets a callback producing a message when the Library mutates
    /// (swatch sets, recent colors or active set). The payload is the full
    /// snapshot for app-level persistence.
    #[must_use]
    pub fn on_library_change<F>(mut self, on_library_change: F) -> Self
    where
        F: 'static + Fn(Vec<SwatchSet>, Vec<PickedValue>, usize) -> Message,
    {
        self.on_library_change = Some(Box::new(on_library_change));
        self
    }

    /// Enables the eye dropper with the given shared [`DropperBuffer`].
    ///
    /// The buffer is where the application deposits window screenshots in
    /// response to [`Self::on_dropper_capture`] requests; see
    /// [`DropperBuffer`] for the full round-trip. Without a buffer the
    /// eyedropper button is disabled.
    #[must_use]
    pub fn dropper_buffer(mut self, buffer: DropperBuffer) -> Self {
        self.dropper_buffer = Some(buffer);
        self
    }

    /// Sets the message published when the user activates the eye dropper
    /// and a fresh window capture is needed.
    ///
    /// The application should react by running `window::latest()` followed
    /// by `window::screenshot`, then storing the resulting
    /// `Screenshot` into the [`DropperBuffer`] passed to
    /// [`Self::dropper_buffer`].
    #[must_use]
    pub fn on_dropper_capture<F>(mut self, on_dropper_capture: F) -> Self
    where
        F: 'static + Fn() -> Message,
    {
        self.on_dropper_capture = Some(Box::new(on_dropper_capture));
        self
    }

    /// Seeds/restores the Library swatch sets (e.g. loaded from disk).
    ///
    /// The value is applied at state creation and whenever it changes
    /// between frames (echoed values are ignored, mirroring `color`).
    /// While `None` (default) the widget owns the sets internally; read them
    /// back via [`State::swatches`] for persistence.
    #[must_use]
    pub fn swatches(mut self, swatches: Vec<SwatchSet>) -> Self {
        self.swatches = Some(swatches);
        self
    }

    /// Seeds/restores the Library recent colors (e.g. loaded from disk).
    ///
    /// Truncated to [`MAX_RECENT`]. See [`Self::swatches`] for ownership.
    /// Read back via [`State::recent_colors`] for persistence.
    #[must_use]
    pub fn recent_colors(mut self, recent_colors: Vec<PickedValue>) -> Self {
        self.recent_colors = Some(recent_colors);
        self
    }

    /// Restores the active swatch tab index (`None` = keep internal).
    #[must_use]
    pub fn active_swatch_tab(mut self, index: usize) -> Self {
        self.active_swatch_tab = Some(index);
        self
    }

    /// Sets the style of the [`ColorPicker`].
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        <Theme as style::Catalog>::Class<'a>: From<StyleFn<'a, Theme, Style>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme, Style>).into();
        self
    }

    /// Sets the class of the input of the [`ColorPicker`].
    #[must_use]
    pub fn class(
        mut self,
        class: impl Into<<Theme as style::Catalog>::Class<'a>>,
    ) -> Self {
        self.class = class.into();
        self
    }
}

/// The state of the inline [`ColorPicker`].
pub struct State {
    /// The state of the dialog content.
    ///
    /// Wrapped in an [`UnsafeCell`] so the read-only widget passes (`draw`,
    /// `mouse_interaction`, which only receive `&Tree`) can still hand the
    /// dialog view the `&mut overlay::State` its plumbing expects. Those
    /// passes never write through it; iced guarantees the update, layout,
    /// draw and interaction phases of a frame do not overlap.
    pub(crate) overlay_state: UnsafeCell<overlay::State>,
    /// Handle to the dialog's child tree, refreshed on every mutating pass
    /// (`layout`, `update`, `operate`) so the read-only passes can build
    /// the content view afterwards within the same frame — mirroring how
    /// overlays capture their tree during `Widget::overlay`. The child
    /// count of this widget is fixed, so the address stays valid.
    content_tree: *mut Tree,
    /// The `color` seen during the previous render; used to detect external
    /// changes without clobbering live edits echoed back by the application.
    pub(crate) old_color: Color,
    /// The `gradient` seen during the previous render (`None` = solid).
    pub(crate) old_gradient: Option<Gradient>,
    /// The `swatches` builder value seen during the previous render.
    pub(crate) old_swatches: Option<Vec<SwatchSet>>,
    /// The `recent_colors` builder value seen during the previous render.
    pub(crate) old_recents: Option<Vec<PickedValue>>,
    /// The `active_swatch_tab` builder value seen during the previous render.
    pub(crate) old_active_tab: Option<usize>,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("overlay_state", &self.overlay_state)
            .field("old_color", &self.old_color)
            .finish_non_exhaustive()
    }
}

impl State {
    /// Creates a new [`State`].
    #[must_use]
    pub fn new(color: Color) -> Self {
        Self {
            overlay_state: UnsafeCell::new(overlay::State::new(color)),
            content_tree: std::ptr::null_mut(),
            old_color: color,
            old_gradient: None,
            old_swatches: None,
            old_recents: None,
            old_active_tab: None,
        }
    }

    /// Creates a new [`State`] with an initial gradient.
    #[must_use]
    pub fn with_gradient(color: Color, gradient: Gradient) -> Self {
        Self {
            overlay_state: UnsafeCell::new(overlay::State::with_gradient(
                color,
                gradient.clone(),
            )),
            content_tree: std::ptr::null_mut(),
            old_color: color,
            old_gradient: Some(gradient),
            old_swatches: None,
            old_recents: None,
            old_active_tab: None,
        }
    }

    /// Creates a new [`State`] with persisted swatch sets (e.g. loaded from disk).
    #[must_use]
    pub fn with_swatches(mut self, swatches: Vec<SwatchSet>) -> Self {
        self.overlay_state.get_mut().set_swatches(swatches.clone());
        self.old_swatches = Some(swatches);
        self
    }

    /// Creates a new [`State`] with persisted recent colors.
    #[must_use]
    pub fn with_recent_colors(mut self, recents: Vec<PickedValue>) -> Self {
        self.overlay_state.get_mut().set_recent_colors(recents.clone());
        self.old_recents = Some(recents);
        self
    }

    /// Returns the swatch sets for persistence (clone and save to disk).
    #[must_use]
    pub fn swatches(&self) -> &[SwatchSet] {
        // SAFETY: read-only access; iced never overlaps widget phases.
        unsafe { &*self.overlay_state.get() }.swatches()
    }

    /// Replaces the swatch sets with persisted values.
    pub fn set_swatches(&mut self, swatches: Vec<SwatchSet>) {
        self.overlay_state.get_mut().set_swatches(swatches.clone());
        self.old_swatches = Some(swatches);
    }

    /// Returns the recent colors for persistence.
    #[must_use]
    pub fn recent_colors(&self) -> &[PickedValue] {
        // SAFETY: read-only access; see `swatches`.
        unsafe { &*self.overlay_state.get() }.recent_colors()
    }

    /// Replaces the recent colors with persisted values.
    pub fn set_recent_colors(&mut self, recents: Vec<PickedValue>) {
        self.overlay_state.get_mut().set_recent_colors(recents.clone());
        self.old_recents = Some(recents);
    }

    /// Returns the active swatch tab index.
    #[must_use]
    pub fn active_swatch_tab(&self) -> usize {
        // SAFETY: read-only access; see `swatches`.
        unsafe { &*self.overlay_state.get() }.active_swatch_tab()
    }

    /// Selects the active swatch set.
    pub fn set_active_swatch_tab(&mut self, index: usize) {
        self.overlay_state.get_mut().set_active_swatch_tab(index);
        self.old_active_tab = Some(self.active_swatch_tab());
    }

    /// Resets the color and gradient of the state.
    pub fn reset(&mut self) {
        let state = self.overlay_state.get_mut();
        let default = Color::from_rgb(0.5, 0.25, 0.25);
        state.color = default;
        state.initial_color = default;
        state.gradient = Gradient::two(default, default);
        state.initial_gradient = Gradient::two(default, default);
        state.initial_is_gradient = false;
        state.color_bar_dragged = ColorBarDragged::None;
        state.gradient_bar_dragged = None;
        state.sync_display();
    }

    /// Re-seed the dialog when the application passes a genuinely new
    /// color or gradient: values differing from both the previous argument
    /// (so echoed live updates are ignored) and the current internal
    /// selection re-snapshot Original.
    fn synchronize(
        &mut self,
        color: Color,
        gradient: Option<&Gradient>,
        swatches: Option<&Vec<SwatchSet>>,
        recents: Option<&Vec<PickedValue>>,
        active_tab: Option<usize>,
    ) {
        let overlay_state = self.overlay_state.get_mut();
        if color != self.old_color && color != overlay_state.color {
            overlay_state.force_synchronize(color);
        }
        self.old_color = color;
        match gradient {
            Some(gradient) => {
                let changed_arg = self.old_gradient.as_ref() != Some(gradient);
                let differs_live = overlay_state.gradient != *gradient;
                if changed_arg && differs_live {
                    overlay_state.force_synchronize_gradient(gradient.clone());
                }
                self.old_gradient = Some(gradient.clone());
            }
            None => {
                self.old_gradient = None;
            }
        }
        match swatches {
            Some(swatches) => {
                if self.old_swatches.as_ref() != Some(swatches)
                    && overlay_state.swatches() != swatches.as_slice()
                {
                    overlay_state.set_swatches(swatches.clone());
                }
                self.old_swatches = Some(swatches.clone());
            }
            None => {
                self.old_swatches = None;
            }
        }
        match recents {
            Some(recents) => {
                if self.old_recents.as_ref() != Some(recents)
                    && overlay_state.recent_colors() != recents.as_slice()
                {
                    overlay_state.set_recent_colors(recents.clone());
                }
                self.old_recents = Some(recents.clone());
            }
            None => {
                self.old_recents = None;
            }
        }
        match active_tab {
            Some(index) => {
                if self.old_active_tab != Some(index)
                    && overlay_state.active_swatch_tab() != index
                {
                    overlay_state.set_active_swatch_tab(index);
                }
                self.old_active_tab = Some(index);
            }
            None => {
                self.old_active_tab = None;
            }
        }
    }
}

impl<'a, Message, Theme> Widget<Message, Theme, Renderer> for ColorPicker<'a, Message, Theme>
where
    Message: 'static + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
    for<'c> <Theme as iced::widget::text_input::Catalog>::Class<'c>:
        From<iced::widget::text_input::StyleFn<'c, Theme>>,
{
    fn tag(&self) -> Tag {
        Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        let mut state = match &self.gradient {
            Some(gradient) => State::with_gradient(self.color, gradient.clone()),
            None => State::new(self.color),
        };
        if let Some(swatches) = &self.swatches {
            state.set_swatches(swatches.clone());
        }
        if let Some(recents) = &self.recent_colors {
            state.set_recent_colors(recents.clone());
        }
        if let Some(index) = self.active_swatch_tab {
            state.set_active_swatch_tab(index);
        }
        tree::State::new(state)
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content_state)]
    }

    fn diff(&self, tree: &mut Tree) {
        let picker_state = tree.state.downcast_mut::<State>();

        picker_state.synchronize(
            self.color,
            self.gradient.as_ref(),
            self.swatches.as_ref(),
            self.recent_colors.as_ref(),
            self.active_swatch_tab,
        );

        tree.diff_children(std::slice::from_ref(&self.content_state));
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        let picker_state = tree.state.downcast_mut::<State>();
        picker_state.content_tree = &mut tree.children[0];
        let bounds = limits.max();

        // SAFETY: unique access; see `State::overlay_state`.
        let overlay_state = unsafe { &mut *picker_state.overlay_state.get() };

        ColorPickerOverlay::<Message, Theme>::new(
            overlay_state,
            self.on_cancel.clone(),
            &self.on_submit,
            self.on_color_change.as_deref(),
            self.on_gradient_submit.as_deref(),
            self.on_gradient_change.as_deref(),
            self.on_pick.as_deref(),
            self.on_pick_submit.as_deref(),
            self.on_tab_change.as_deref(),
            self.on_library_change.as_deref(),
            self.dropper_buffer.as_ref(),
            self.on_dropper_capture.as_deref(),
            false,
            &self.class,
            &mut tree.children[0],
            Rectangle::with_size(bounds),
        )
        .layout_content(renderer, bounds)
    }

    fn update(
        &mut self,
        state: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let picker_state = state.state.downcast_mut::<State>();
        picker_state.content_tree = &mut state.children[0];

        // SAFETY: unique access; see `State::overlay_state`.
        let overlay_state = unsafe { &mut *picker_state.overlay_state.get() };

        ColorPickerOverlay::<Message, Theme>::new(
            overlay_state,
            self.on_cancel.clone(),
            &self.on_submit,
            self.on_color_change.as_deref(),
            self.on_gradient_submit.as_deref(),
            self.on_gradient_change.as_deref(),
            self.on_pick.as_deref(),
            self.on_pick_submit.as_deref(),
            self.on_tab_change.as_deref(),
            self.on_library_change.as_deref(),
            self.dropper_buffer.as_ref(),
            self.on_dropper_capture.as_deref(),
            false,
            &self.class,
            &mut state.children[0],
            *viewport,
        )
        .update_content(event, layout, cursor, renderer, clipboard, shell);
    }

    fn mouse_interaction(
        &self,
        state: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let picker_state = state.state.downcast_ref::<State>();

        match self.view_immutable(picker_state, *viewport) {
            Some(view) => view.mouse_interaction_content(layout, cursor, renderer),
            None => mouse::Interaction::default(),
        }
    }

    fn draw(
        &self,
        state: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let picker_state = state.state.downcast_ref::<State>();

        if let Some(view) = self.view_immutable(picker_state, *viewport) {
            view.draw_content(renderer, theme, style, layout, cursor);
        }
    }

    fn operate<'b>(
        &'b mut self,
        state: &'b mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation<()>,
    ) {
        let picker_state = state.state.downcast_mut::<State>();
        picker_state.content_tree = &mut state.children[0];

        // SAFETY: unique access; see `State::overlay_state`.
        let overlay_state = unsafe { &mut *picker_state.overlay_state.get() };

        ColorPickerOverlay::<Message, Theme>::new(
            overlay_state,
            self.on_cancel.clone(),
            &self.on_submit,
            self.on_color_change.as_deref(),
            self.on_gradient_submit.as_deref(),
            self.on_gradient_change.as_deref(),
            self.on_pick.as_deref(),
            self.on_pick_submit.as_deref(),
            self.on_tab_change.as_deref(),
            self.on_library_change.as_deref(),
            self.dropper_buffer.as_ref(),
            self.on_dropper_capture.as_deref(),
            false,
            &self.class,
            &mut state.children[0],
            Rectangle::new(layout.bounds().position(), layout.bounds().size()),
        )
        .operate_content(layout, renderer, operation);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        _layout: Layout<'b>,
        _renderer: &Renderer,
        _viewport: &Rectangle,
        _translation: Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, Renderer>> {
        let picker_state = tree.state.downcast_mut::<State>();
        // SAFETY: unique access; see `State::overlay_state`.
        let overlay_state = unsafe { &mut *picker_state.overlay_state.get() };

        if overlay_state.dropper_mode != DropperMode::Picking {
            return None;
        }
        let Some(frame) = overlay_state.dropper_frame.as_ref() else {
            return None;
        };

        // Host the magnifier lens in a full-window overlay so it escapes
        // ancestor clipping and can follow the cursor across the entire
        // application window.
        Some(DropperLens::new(frame, overlay_state.dropper_cursor, &self.class).overlay())
    }
}

impl<'a, Message, Theme> From<ColorPicker<'a, Message, Theme>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'static + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
    for<'c> <Theme as iced::widget::text_input::Catalog>::Class<'c>:
        From<iced::widget::text_input::StyleFn<'c, Theme>>,
{
    fn from(color_picker: ColorPicker<'a, Message, Theme>) -> Self {
        Element::new(color_picker)
    }
}

/// Shortcut helper to create an inline [`ColorPicker`] widget.
///
/// [`ColorPicker`]: crate::color_picker::ColorPicker
pub fn color_picker<'a, Message, Theme, F>(
    color: Color,
    on_cancel: Message,
    on_submit: F,
) -> ColorPicker<'a, Message, Theme>
where
    Message: 'a + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
    for<'c> <Theme as iced::widget::text_input::Catalog>::Class<'c>:
        From<iced::widget::text_input::StyleFn<'c, Theme>>,
    F: 'static + Fn(Color) -> Message,
{
    ColorPicker::new(color, on_cancel, on_submit)
}

/// Shortcut helper to create an inline [`ColorPicker`] widget with real-time
/// color change callback.
///
/// [`ColorPicker`]: crate::color_picker::ColorPicker
pub fn color_picker_with_change<'a, Message, Theme, F, G>(
    color: Color,
    on_cancel: Message,
    on_submit: F,
    on_color_change: G,
) -> ColorPicker<'a, Message, Theme>
where
    Message: 'a + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
    for<'c> <Theme as iced::widget::text_input::Catalog>::Class<'c>:
        From<iced::widget::text_input::StyleFn<'c, Theme>>,
    F: 'static + Fn(Color) -> Message,
    G: 'static + Fn(Color) -> Message,
{
    ColorPicker::new(color, on_cancel, on_submit).on_color_change(on_color_change)
}

//TODO: Remove ignore when Null is updated. Temp fix for Test runs
/// An input element for picking colors that floats above the interface.
///
/// Wraps an underlay element (typically a button) and, while `show_picker`
/// is true, spawns the dialog inside a free-floating window-like shell
/// ([`ColorPickerWindow`]): a draggable header strip with an empty drag
/// area and a close ("x") button publishing `on_cancel`. The window can be
/// dragged anywhere inside the viewport; the dragged position survives
/// close/reopen. This is still a regular overlay widget, not a separate OS
/// window.
///
/// # Example
/// ```ignore
/// # use neverliie_iced_widgets::color_picker::FloatingColorPicker;
/// # use iced::{Color, widget::{button, Button, Text}};
/// #
/// #[derive(Clone, Debug)]
/// enum Message {
///     Open,
///     Cancel,
///     Submit(Color),
/// }
///
/// let color_picker = FloatingColorPicker::new(
///     true,
///     Color::default(),
///     Button::new(Text::new("Pick color"))
///         .on_press(Message::Open),
///     Message::Cancel,
///     Message::Submit,
/// );
/// ```
#[allow(missing_debug_implementations)]
pub struct FloatingColorPicker<'a, Message, Theme = iced::Theme>
where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog,
{
    /// Show the picker.
    show_picker: bool,
    /// The color to show.
    color: Color,
    /// The underlying element.
    underlay: Element<'a, Message, Theme, Renderer>,
    /// The message that is sent when the header close ("x") button of the
    /// floating window is pressed.
    on_cancel: Message,
    /// The function that produces a message when the submit button of the dialog is pressed.
    on_submit: Box<dyn Fn(Color) -> Message>,
    /// Optional function that produces a message when the color changes during selection (real-time updates).
    on_color_change: Option<Box<dyn Fn(Color) -> Message>>,
    /// Initial gradient for the Gradient tab (`None` = solid `color`).
    gradient: Option<Gradient>,
    /// Optional function producing a message with the gradient when submit
    /// is pressed while the Gradient tab is active.
    on_gradient_submit: Option<Box<dyn Fn(Gradient) -> Message>>,
    /// Optional function producing a message when the gradient changes
    /// during selection (real-time updates).
    on_gradient_change: Option<Box<dyn Fn(Gradient) -> Message>>,
    /// Optional unified change callback with the picked value (solid color
    /// or gradient) for the active tab.
    on_pick: Option<Box<dyn Fn(PickedValue) -> Message>>,
    /// Optional unified submit callback with the picked value (solid color
    /// or gradient) for the active tab.
    on_pick_submit: Option<Box<dyn Fn(PickedValue) -> Message>>,
    /// Optional function producing a message when the top-level tab
    /// (`Color | Gradient | Library`) changes.
    on_tab_change: Option<Box<dyn Fn(PickerTab) -> Message>>,
    /// Optional function producing a message when the Library mutates
    /// (swatch sets, recents or active set). The app persists the payload.
    on_library_change: Option<Box<dyn Fn(Vec<SwatchSet>, Vec<PickedValue>, usize) -> Message>>,
    /// Shared buffer receiving window screenshots for the eye dropper; the
    /// eyedropper button stays disabled while this is `None`.
    dropper_buffer: Option<DropperBuffer>,
    /// Optional function producing the message published when the user
    /// activates the eye dropper and a fresh capture is needed.
    on_dropper_capture: Option<Box<dyn Fn() -> Message>>,
    /// Persisted swatch sets to seed/restore the Library tab (`None` =
    /// widget owns them after first creation).
    swatches: Option<Vec<SwatchSet>>,
    /// Persisted recent colors to seed/restore the Library tab.
    recent_colors: Option<Vec<PickedValue>>,
    /// Persisted active swatch tab index.
    active_swatch_tab: Option<usize>,
    /// The style of the dialog.
    class: <Theme as style::Catalog>::Class<'a>,
    /// The initial position of the dialog window; dragging overrides it.
    position: Option<Position>,
    /// The buttons of the overlay.
    overlay_state: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Theme> FloatingColorPicker<'a, Message, Theme>
where
    Message: 'a + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
    for<'c> <Theme as iced::widget::text_input::Catalog>::Class<'c>:
        From<iced::widget::text_input::StyleFn<'c, Theme>>,
{
    /// Creates a new [`FloatingColorPicker`] wrapping around the given underlay.
    ///
    /// It expects:
    ///     * if the overlay of the color picker is visible.
    ///     * the initial color to show.
    ///     * the underlay [`Element`] on which this [`FloatingColorPicker`]
    ///         will be wrapped around.
    ///     * a message that will be send when the header close ("x") button
    ///         of the [`FloatingColorPicker`] window is pressed.
    ///     * a function that will be called when the submit button of the
    ///         [`FloatingColorPicker`] is pressed, which takes the picked
    ///         [`Color`] value.
    pub fn new<U, F>(
        show_picker: bool,
        color: Color,
        underlay: U,
        on_cancel: Message,
        on_submit: F,
    ) -> Self
    where
        U: Into<Element<'a, Message, Theme, Renderer>>,
        F: 'static + Fn(Color) -> Message,
    {
        Self {
            show_picker,
            color,
            underlay: underlay.into(),
            on_cancel,
            on_submit: Box::new(on_submit),
            on_color_change: None,
            gradient: None,
            on_gradient_submit: None,
            on_gradient_change: None,
            on_pick: None,
            on_pick_submit: None,
            on_tab_change: None,
            on_library_change: None,
            dropper_buffer: None,
            on_dropper_capture: None,
            swatches: None,
            recent_colors: None,
            active_swatch_tab: None,
            class: <Theme as style::Catalog>::default(),
            position: None,
            overlay_state: ColorPickerOverlayButtons::default().into(),
        }
    }

    /// Sets a callback that will be called whenever the color changes during selection (real-time updates).
    #[must_use]
    pub fn on_color_change<F>(mut self, on_color_change: F) -> Self
    where
        F: 'static + Fn(Color) -> Message,
    {
        self.on_color_change = Some(Box::new(on_color_change));
        self
    }

    /// Sets the initial gradient edited in the Gradient tab.
    #[must_use]
    pub fn gradient(mut self, gradient: Gradient) -> Self {
        self.gradient = Some(gradient);
        self
    }

    /// Sets the callback producing a message with the gradient when submit
    /// is pressed while the Gradient tab is active.
    #[must_use]
    pub fn on_gradient_submit<F>(mut self, on_gradient_submit: F) -> Self
    where
        F: 'static + Fn(Gradient) -> Message,
    {
        self.on_gradient_submit = Some(Box::new(on_gradient_submit));
        self
    }

    /// Sets the callback producing a message when the gradient changes
    /// during selection (real-time updates).
    #[must_use]
    pub fn on_gradient_change<F>(mut self, on_gradient_change: F) -> Self
    where
        F: 'static + Fn(Gradient) -> Message,
    {
        self.on_gradient_change = Some(Box::new(on_gradient_change));
        self
    }

    /// Sets a unified callback with the picked value (solid or gradient)
    /// for the active tab on every selection change.
    #[must_use]
    pub fn on_pick<F>(mut self, on_pick: F) -> Self
    where
        F: 'static + Fn(PickedValue) -> Message,
    {
        self.on_pick = Some(Box::new(on_pick));
        self
    }

    /// Sets a unified callback with the picked value (solid or gradient)
    /// for the active tab when submit is pressed.
    #[must_use]
    pub fn on_pick_submit<F>(mut self, on_pick_submit: F) -> Self
    where
        F: 'static + Fn(PickedValue) -> Message,
    {
        self.on_pick_submit = Some(Box::new(on_pick_submit));
        self
    }

    /// Sets a callback producing a message when the top-level tab
    /// (`Color | Gradient | Library`) changes.
    #[must_use]
    pub fn on_tab_change<F>(mut self, on_tab_change: F) -> Self
    where
        F: 'static + Fn(PickerTab) -> Message,
    {
        self.on_tab_change = Some(Box::new(on_tab_change));
        self
    }

    /// Sets a callback producing a message when the Library mutates
    /// (swatch sets, recent colors or active set). The payload is the full
    /// snapshot for app-level persistence.
    #[must_use]
    pub fn on_library_change<F>(mut self, on_library_change: F) -> Self
    where
        F: 'static + Fn(Vec<SwatchSet>, Vec<PickedValue>, usize) -> Message,
    {
        self.on_library_change = Some(Box::new(on_library_change));
        self
    }

    /// Enables the eye dropper with the given shared [`DropperBuffer`].
    ///
    /// The buffer is where the application deposits window screenshots in
    /// response to [`Self::on_dropper_capture`] requests; see
    /// [`DropperBuffer`] for the full round-trip. Without a buffer the
    /// eyedropper button is disabled.
    #[must_use]
    pub fn dropper_buffer(mut self, buffer: DropperBuffer) -> Self {
        self.dropper_buffer = Some(buffer);
        self
    }

    /// Sets the message published when the user activates the eye dropper
    /// and a fresh window capture is needed.
    ///
    /// The application should react by running `window::latest()` followed
    /// by `window::screenshot`, then storing the resulting
    /// `Screenshot` into the [`DropperBuffer`] passed to
    /// [`Self::dropper_buffer`].
    #[must_use]
    pub fn on_dropper_capture<F>(mut self, on_dropper_capture: F) -> Self
    where
        F: 'static + Fn() -> Message,
    {
        self.on_dropper_capture = Some(Box::new(on_dropper_capture));
        self
    }

    /// Seeds/restores the Library swatch sets (e.g. loaded from disk).
    ///
    /// Applied at state creation and on reopen; read back via
    /// [`FloatingState::swatches`] for persistence.
    #[must_use]
    pub fn swatches(mut self, swatches: Vec<SwatchSet>) -> Self {
        self.swatches = Some(swatches);
        self
    }

    /// Seeds/restores the Library recent colors (truncated to [`MAX_RECENT`]).
    #[must_use]
    pub fn recent_colors(mut self, recent_colors: Vec<PickedValue>) -> Self {
        self.recent_colors = Some(recent_colors);
        self
    }

    /// Restores the active swatch tab index.
    #[must_use]
    pub fn active_swatch_tab(mut self, index: usize) -> Self {
        self.active_swatch_tab = Some(index);
        self
    }

    /// Sets the initial position of the dialog window.
    ///
    /// Uses the same [`Position`] strategies as the overlay widget. The user
    /// can drag the window anywhere afterwards; the dragged position then
    /// wins and survives close/reopen.
    ///
    /// ```ignore
    /// floating_color_picker(true, color, underlay, Message::Cancel, Message::Submit)
    ///     .position(Position::BottomRight)
    /// ```
    #[must_use]
    pub fn position(mut self, position: Position) -> Self {
        self.position = Some(position);
        self
    }

    /// Sets the style of the [`FloatingColorPicker`].
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        <Theme as style::Catalog>::Class<'a>: From<StyleFn<'a, Theme, Style>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme, Style>).into();
        self
    }

    /// Sets the class of the input of the [`FloatingColorPicker`].
    #[must_use]
    pub fn class(
        mut self,
        class: impl Into<<Theme as style::Catalog>::Class<'a>>,
    ) -> Self {
        self.class = class.into();
        self
    }
}

/// The state of the [`FloatingColorPicker`].
#[derive(Debug, Default)]
pub struct FloatingState {
    /// The state of the dialog.
    pub(crate) overlay_state: overlay::State,
    /// Was the window shown during the previous render?
    pub(crate) old_show_picker: bool,
    /// The last known cursor position, for cursor-following [`Position`]s.
    pub(crate) last_cursor_position: Point,
}

impl FloatingState {    /// Creates a new [`FloatingState`].
    #[must_use]
    pub fn new(color: Color) -> Self {
        Self {
            overlay_state: overlay::State::new(color),
            old_show_picker: false,
            last_cursor_position: Point::ORIGIN,
        }
    }

    /// Creates a new [`FloatingState`] with an initial gradient.
    #[must_use]
    pub fn with_gradient(color: Color, gradient: Gradient) -> Self {
        Self {
            overlay_state: overlay::State::with_gradient(color, gradient),
            old_show_picker: false,
            last_cursor_position: Point::ORIGIN,
        }
    }

    /// Creates a new [`FloatingState`] with persisted swatch sets.
    #[must_use]
    pub fn with_swatches(mut self, swatches: Vec<SwatchSet>) -> Self {
        self.overlay_state.set_swatches(swatches);
        self
    }

    /// Creates a new [`FloatingState`] with persisted recent colors.
    #[must_use]
    pub fn with_recent_colors(mut self, recents: Vec<PickedValue>) -> Self {
        self.overlay_state.set_recent_colors(recents);
        self
    }

    /// Returns the swatch sets for persistence (clone and save to disk).
    #[must_use]
    pub fn swatches(&self) -> &[SwatchSet] {
        self.overlay_state.swatches()
    }

    /// Replaces the swatch sets with persisted values.
    pub fn set_swatches(&mut self, swatches: Vec<SwatchSet>) {
        self.overlay_state.set_swatches(swatches);
    }

    /// Returns the recent colors for persistence.
    #[must_use]
    pub fn recent_colors(&self) -> &[PickedValue] {
        self.overlay_state.recent_colors()
    }

    /// Replaces the recent colors with persisted values.
    pub fn set_recent_colors(&mut self, recents: Vec<PickedValue>) {
        self.overlay_state.set_recent_colors(recents);
    }

    /// Returns the active swatch tab index.
    #[must_use]
    pub fn active_swatch_tab(&self) -> usize {
        self.overlay_state.active_swatch_tab()
    }

    /// Selects the active swatch set.
    pub fn set_active_swatch_tab(&mut self, index: usize) {
        self.overlay_state.set_active_swatch_tab(index);
    }

    /// Resets the color and gradient of the state.
    pub fn reset(&mut self) {
        let default = Color::from_rgb(0.5, 0.25, 0.25);
        self.overlay_state.color = default;
        self.overlay_state.initial_color = default;
        self.overlay_state.gradient = Gradient::two(default, default);
        self.overlay_state.initial_gradient = Gradient::two(default, default);
        self.overlay_state.initial_is_gradient = false;
        self.overlay_state.color_bar_dragged = ColorBarDragged::None;
        self.overlay_state.gradient_bar_dragged = None;
        self.overlay_state.sync_display();
    }

    /// Synchronize with the provided color when the picker is (re)opened.
    ///
    /// Keep the overlay state in sync. While the window is open it "owns"
    /// the value: the initial color must stay frozen at the open-time color
    /// so the Original preview panel does not track live `on_color_change`
    /// updates. When it is reopened, reset the color to the provided one.
    /// Persisted library values (`swatches`/`recents`/`active_tab`) are
    /// applied on reopen and whenever the builder value changes.
    fn synchronize(
        &mut self,
        show_picker: bool,
        color: Color,
        gradient: Option<&Gradient>,
        swatches: Option<&Vec<SwatchSet>>,
        recents: Option<&Vec<PickedValue>>,
        active_tab: Option<usize>,
    ) {
        if show_picker && !self.old_show_picker {
            self.overlay_state.force_synchronize(color);
            if let Some(gradient) = gradient {
                self.overlay_state.force_synchronize_gradient(gradient.clone());
            }
            if let Some(swatches) = swatches {
                self.overlay_state.set_swatches(swatches.clone());
            }
            if let Some(recents) = recents {
                self.overlay_state.set_recent_colors(recents.clone());
            }
            if let Some(index) = active_tab {
                self.overlay_state.set_active_swatch_tab(index);
            }
        } else if show_picker {
            if let Some(swatches) = swatches
                && self.overlay_state.swatches() != swatches.as_slice()
            {
                self.overlay_state.set_swatches(swatches.clone());
            }
            if let Some(recents) = recents
                && self.overlay_state.recent_colors() != recents.as_slice()
            {
                self.overlay_state.set_recent_colors(recents.clone());
            }
            if let Some(index) = active_tab
                && self.overlay_state.active_swatch_tab() != index
            {
                self.overlay_state.set_active_swatch_tab(index);
            }
        }
        self.old_show_picker = show_picker;
    }
}

impl<'a, Message, Theme> Widget<Message, Theme, Renderer> for FloatingColorPicker<'a, Message, Theme>
where
    Message: 'static + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
    for<'c> <Theme as iced::widget::text_input::Catalog>::Class<'c>:
        From<iced::widget::text_input::StyleFn<'c, Theme>>,
{
    fn tag(&self) -> Tag {
        Tag::of::<FloatingState>()
    }

    fn state(&self) -> tree::State {
        let mut state = match &self.gradient {
            Some(gradient) => FloatingState::with_gradient(self.color, gradient.clone()),
            None => FloatingState::new(self.color),
        };
        if let Some(swatches) = &self.swatches {
            state.set_swatches(swatches.clone());
        }
        if let Some(recents) = &self.recent_colors {
            state.set_recent_colors(recents.clone());
        }
        if let Some(index) = self.active_swatch_tab {
            state.set_active_swatch_tab(index);
        }
        tree::State::new(state)
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.underlay), Tree::new(&self.overlay_state)]
    }

    fn diff(&self, tree: &mut Tree) {
        let picker_state = tree.state.downcast_mut::<FloatingState>();

        picker_state.synchronize(
            self.show_picker,
            self.color,
            self.gradient.as_ref(),
            self.swatches.as_ref(),
            self.recent_colors.as_ref(),
            self.active_swatch_tab,
        );

        tree.diff_children(&[&self.underlay, &self.overlay_state]);
    }

    fn size(&self) -> Size<Length> {
        self.underlay.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        self.underlay
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn update(
        &mut self,
        state: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        // Track the cursor position for cursor-following positions, and
        // request redraws so the window keeps following the mouse.
        if let Event::Mouse(mouse::Event::CursorMoved { position }) = event {
            state.state.downcast_mut::<FloatingState>().last_cursor_position = *position;
            if self
                .position
                .is_some_and(|p| matches!(p, Position::Cursor { .. } | Position::FollowCursor))
            {
                shell.request_redraw();
            }
        }

        self.underlay.as_widget_mut().update(
            &mut state.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        state: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.underlay.as_widget().mouse_interaction(
            &state.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        state: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        self.underlay.as_widget().draw(
            &state.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn operate<'b>(
        &'b mut self,
        state: &'b mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation<()>,
    ) {
        self.underlay
            .as_widget_mut()
            .operate(&mut state.children[0], layout, renderer, operation);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, Renderer>> {
        let picker_state: &mut FloatingState = tree.state.downcast_mut();

        if !self.show_picker {
            return self.underlay.as_widget_mut().overlay(
                &mut tree.children[0],
                layout,
                renderer,
                viewport,
                translation,
            );
        }

        let bounds = layout.bounds();
        let fallback_center = Point::new(bounds.center_x(), bounds.center_y());
        let parent_bounds = bounds + translation;
        let cursor_position = picker_state.last_cursor_position;

        Some(
            ColorPickerWindow::new(
                &mut picker_state.overlay_state,
                self.on_cancel.clone(),
                &self.on_submit,
                self.on_color_change.as_deref(),
                self.on_gradient_submit.as_deref(),
                self.on_gradient_change.as_deref(),
                self.on_pick.as_deref(),
                self.on_pick_submit.as_deref(),
                self.on_tab_change.as_deref(),
                self.on_library_change.as_deref(),
                self.dropper_buffer.as_ref(),
                self.on_dropper_capture.as_deref(),
                self.position,
                parent_bounds,
                fallback_center,
                cursor_position,
                &self.class,
                &mut tree.children[1],
                *viewport,
            )
            .overlay(),
        )
    }
}

impl<'a, Message, Theme> From<FloatingColorPicker<'a, Message, Theme>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'static + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
    for<'c> <Theme as iced::widget::text_input::Catalog>::Class<'c>:
        From<iced::widget::text_input::StyleFn<'c, Theme>>,
{
    fn from(color_picker: FloatingColorPicker<'a, Message, Theme>) -> Self {
        Element::new(color_picker)
    }
}

/// Shortcut helper to create a [`FloatingColorPicker`] widget.
///
/// [`FloatingColorPicker`]: crate::color_picker::FloatingColorPicker
pub fn floating_color_picker<'a, Message, Theme, U, F>(
    show_picker: bool,
    color: Color,
    underlay: U,
    on_cancel: Message,
    on_submit: F,
) -> FloatingColorPicker<'a, Message, Theme>
where
    U: Into<Element<'a, Message, Theme, Renderer>>,
    Message: 'a + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
    for<'c> <Theme as iced::widget::text_input::Catalog>::Class<'c>:
        From<iced::widget::text_input::StyleFn<'c, Theme>>,
    F: 'static + Fn(Color) -> Message,
{
    FloatingColorPicker::new(show_picker, color, underlay, on_cancel, on_submit)
}

/// Shortcut helper to create a [`FloatingColorPicker`] widget with real-time
/// color change callback.
///
/// [`FloatingColorPicker`]: crate::color_picker::FloatingColorPicker
pub fn floating_color_picker_with_change<'a, Message, Theme, U, F, G>(
    show_picker: bool,
    color: Color,
    underlay: U,
    on_cancel: Message,
    on_submit: F,
    on_color_change: G,
) -> FloatingColorPicker<'a, Message, Theme>
where
    U: Into<Element<'a, Message, Theme, Renderer>>,
    Message: 'a + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
    for<'c> <Theme as iced::widget::text_input::Catalog>::Class<'c>:
        From<iced::widget::text_input::StyleFn<'c, Theme>>,
    F: 'static + Fn(Color) -> Message,
    G: 'static + Fn(Color) -> Message,
{
    FloatingColorPicker::new(show_picker, color, underlay, on_cancel, on_submit)
        .on_color_change(on_color_change)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    enum TestMessage {
        Cancel,
        Submit(Color),
    }

    type TestColorPicker<'a> = ColorPicker<'a, TestMessage, iced::Theme>;
    type TestFloatingPicker<'a> = FloatingColorPicker<'a, TestMessage, iced::Theme>;

    fn create_test_button() -> iced::widget::Button<'static, TestMessage, iced::Theme> {
        iced::widget::button(iced::widget::Text::new("Pick"))
    }

    #[test]
    fn color_picker_new() {
        let color = Color::from_rgb(0.5, 0.5, 0.5);

        let picker = TestColorPicker::new(color, TestMessage::Cancel, TestMessage::Submit);

        assert_eq!(picker.color, color);
    }

    #[test]
    fn color_picker_default_has_no_color_change() {
        let picker = TestColorPicker::new(
            Color::from_rgb(0.3, 0.6, 0.9),
            TestMessage::Cancel,
            TestMessage::Submit,
        );

        assert!(picker.on_color_change.is_none());
    }

    #[test]
    fn color_picker_with_change_builder_stores_callback() {
        let picker = TestColorPicker::new(
            Color::from_rgb(0.3, 0.6, 0.9),
            TestMessage::Cancel,
            TestMessage::Submit,
        )
        .on_color_change(TestMessage::Submit);

        assert!(picker.on_color_change.is_some());
    }

    #[test]
    fn floating_picker_new_with_picker_hidden() {
        let color = Color::from_rgb(0.5, 0.5, 0.5);
        let button = create_test_button();

        let picker = TestFloatingPicker::new(
            false,
            color,
            button,
            TestMessage::Cancel,
            TestMessage::Submit,
        );

        assert!(!picker.show_picker);
        assert_eq!(picker.color, color);
    }

    #[test]
    fn floating_picker_new_with_picker_shown() {
        let color = Color::from_rgb(0.3, 0.6, 0.9);
        let button = create_test_button();

        let picker = TestFloatingPicker::new(
            true,
            color,
            button,
            TestMessage::Cancel,
            TestMessage::Submit,
        );

        assert!(picker.show_picker);
        assert_eq!(picker.color, color);
    }

    #[test]
    fn floating_picker_default_position_is_none() {
        let button = create_test_button();
        let picker = TestFloatingPicker::new(
            false,
            Color::from_rgb(0.5, 0.5, 0.5),
            button,
            TestMessage::Cancel,
            TestMessage::Submit,
        );

        assert!(picker.position.is_none());
    }

    #[test]
    fn floating_picker_position_builder_stores_position() {
        let button = create_test_button();
        let picker = TestFloatingPicker::new(
            false,
            Color::from_rgb(0.5, 0.5, 0.5),
            button,
            TestMessage::Cancel,
            TestMessage::Submit,
        )
        .position(Position::BottomRight);

        assert_eq!(picker.position, Some(Position::BottomRight));
    }

    #[test]
    fn inline_state_new_seeds_old_color() {
        let color = Color::from_rgb(0.5, 0.5, 0.5);
        let mut state = State::new(color);

        assert_eq!(state.old_color, color);
        assert_eq!(state.overlay_state.get_mut().color, color);
    }

    #[test]
    fn inline_state_reset() {
        let color = Color::from_rgb(0.5, 0.5, 0.5);
        let mut state = State::new(color);

        state.reset();

        assert_eq!(
            state.overlay_state.get_mut().color,
            Color::from_rgb(0.5, 0.25, 0.25)
        );
    }

    #[test]
    fn inline_synchronize_ignores_echoed_live_updates() {
        let open_color = Color::from_rgb(0.3, 0.6, 0.9);
        let live_color = Color::from_rgb(1.0, 0.0, 0.0);
        let mut state = State::new(open_color);

        // The user edits internally; the application echoes the same value
        // back through the `color` argument. That echo must not clobber the
        // internal selection.
        state.overlay_state.get_mut().apply_color(live_color);
        state.synchronize(live_color, None, None, None, None);
        assert_eq!(state.overlay_state.get_mut().color, live_color);

        // A genuinely different external color re-seeds the dialog.
        let reset_color = Color::from_rgb(0.0, 1.0, 0.0);
        state.synchronize(reset_color, None, None, None, None);
        assert_eq!(state.overlay_state.get_mut().color, reset_color);
    }

    #[test]
    fn inline_synchronize_ignores_stale_argument() {
        let open_color = Color::from_rgb(0.3, 0.6, 0.9);
        let mut state = State::new(open_color);

        // The application does not echo live updates: the stale argument
        // keeps arriving every frame and must not freeze the dialog.
        let edited = Color::from_rgb(0.2, 0.2, 0.8);
        state.overlay_state.get_mut().apply_color(edited);
        state.synchronize(open_color, None, None, None, None);
        assert_eq!(state.overlay_state.get_mut().color, edited);
    }

    #[test]
    fn floating_synchronize_freeze_initial_color_while_open() {
        let open_color = Color::from_rgb(0.3, 0.6, 0.9);
        let live_color = Color::from_rgb(1.0, 0.0, 0.0);
        let mut state = FloatingState::new(open_color);

        state.synchronize(true, open_color, None, None, None, None);
        assert_eq!(state.overlay_state.color, open_color);
        assert_eq!(state.overlay_state.initial_color, open_color);

        // Live `on_color_change` re-renders must not clobber the initial
        // color while the picker is open.
        state.synchronize(true, live_color, None, None, None, None);
        assert_eq!(state.overlay_state.color, open_color);
        assert_eq!(state.overlay_state.initial_color, open_color);

        // Reopening with a different color resets both.
        state.synchronize(false, open_color, None, None, None, None);
        state.synchronize(true, live_color, None, None, None, None);
        assert_eq!(state.overlay_state.color, live_color);
        assert_eq!(state.overlay_state.initial_color, live_color);
    }

    #[test]
    fn inline_state_library_persistence_round_trip() {
        let color = Color::from_rgb(0.5, 0.5, 0.5);
        let mut state = State::new(color)
            .with_swatches(vec![SwatchSet::with_colors(
                "Mine",
                vec![PickedValue::Solid(Color::BLACK)],
            )])
            .with_recent_colors(vec![PickedValue::Solid(Color::WHITE)]);

        assert_eq!(state.swatches().len(), 1);
        assert_eq!(state.swatches()[0].name(), "Mine");
        assert_eq!(state.recent_colors().len(), 1);

        state.set_active_swatch_tab(0);
        assert_eq!(state.active_swatch_tab(), 0);

        // Empty restore falls back to the default set.
        state.set_swatches(Vec::new());
        assert_eq!(state.swatches().len(), 1);
    }

    #[test]
    fn inline_synchronize_applies_library_builder_values() {
        let color = Color::from_rgb(0.5, 0.5, 0.5);
        let mut state = State::new(color);
        let swatches = vec![SwatchSet::new("Persisted")];
        let recents = vec![PickedValue::Solid(Color::BLACK)];

        state.synchronize(color, None, Some(&swatches), Some(&recents), Some(0));
        assert_eq!(state.swatches()[0].name(), "Persisted");
        assert_eq!(state.recent_colors().len(), 1);
    }

    #[test]
    fn floating_state_library_persistence_round_trip() {
        let color = Color::from_rgb(0.5, 0.5, 0.5);
        let mut state = FloatingState::new(color)
            .with_swatches(vec![SwatchSet::new("Mine")])
            .with_recent_colors(vec![PickedValue::Solid(Color::WHITE)]);

        assert_eq!(state.swatches()[0].name(), "Mine");
        assert_eq!(state.recent_colors().len(), 1);

        state.set_swatches(vec![SwatchSet::new("Other")]);
        assert_eq!(state.swatches()[0].name(), "Other");
    }
}
