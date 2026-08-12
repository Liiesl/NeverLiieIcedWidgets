//! The overlay of the [`ColorPicker`](crate::color_picker::ColorPicker).
//!
//! Ported from `iced_aw`'s `widget::overlay::color_picker` module.

use super::{
    color::{
        clamp_hue, clamp_u8, color_to_hex_argb, hue_from_angle, is_valid_hex, parse_hex_digits,
        Hsv,
    },
    style::{self, Status, Style},
    style_state::StyleState,
    State as WidgetState,
};

use crate::overlay::{clamp_to_viewport, Position as OverlayPosition};

use iced::border::Radius;
use iced::{
    advanced::{
        graphics::geometry::Renderer as _,
        layout::{Layout, Limits, Node},
        mouse::{self, Cursor},
        overlay, renderer,
        text::{self, Renderer as _, Text},
        widget::{self, tree::Tree},
        Clipboard, Overlay, Renderer as _, Shell, Widget,
    },
    alignment::{Horizontal, Vertical},
    event, keyboard,
    widget::{
        Button, Renderer, Row, TextInput, button,
        canvas::{self, LineCap, Path, Stroke},
        text_input,
    },
    touch,
    Background, Border, Color, Element, Event, Font, Length, Pixels,
    Point, Rectangle, Size, Vector,
};
use std::collections::HashMap;

/// The maximal size of the dialog overlay.
const DIALOG_MAX_SIZE: Size = Size::new(640.0, 470.0);
/// The margin around the dialog content (Qt: contentsMargins 15).
const OUTER_MARGIN: f32 = 15.0;
/// The spacing between the left and right pane (Qt: main_h_layout spacing 15).
const PANE_SPACING: f32 = 15.0;
/// The outer dimension of the picker container / hue ring.
const RING_DIM: f32 = 300.0;
/// The width of the hue ring band.
const RING_WIDTH: f32 = 30.0;
/// The padding between the ring band and the ring border.
const RING_PADDING: f32 = 5.0;
/// The size of the saturation/value square: `int(230 * 0.65)`.
const SQUARE_DIM: f32 = 149.0;
/// The inner diameter of the hue ring: `300 - 2 * (30 + 5)`.
const INNER_DIAMETER: f32 = 230.0;
/// The height of the tab bar.
const TAB_BAR_HEIGHT: f32 = 30.0;
/// The spacing between the slider rows (Qt: controls_v_layout spacing 8).
const ROW_SPACING: f32 = 8.0;
/// The spacing between picker -> tab bar -> controls -> hex container.
const CONTROLS_SPACING: f32 = 10.0;
/// The width of the channel labels.
const LABEL_WIDTH: f32 = 24.0;
/// The width of the value fields.
const VALUE_WIDTH: f32 = 48.0;
/// The height of a slider row.
const SLIDER_HEIGHT: f32 = 16.0;
/// The spacing of the swatch/recent grids.
const GRID_SPACING: f32 = 5.0;
/// The size of a swatch cell.
const SWATCH_SIZE: f32 = 30.0;
/// The size of the "add swatch" button.
const ADD_BUTTON_SIZE: f32 = 28.0;
/// The height of the Original/New preview panels.
const PREVIEW_HEIGHT: f32 = 44.0;
/// The fixed width of the right pane.
const RIGHT_PANE_WIDTH: f32 = 230.0;
/// The maximum number of recent colors.
const MAX_RECENT: usize = 12;
/// The maximum number of swatches per set.
const MAX_SWATCHES_PER_SET: usize = 24;
/// The number of columns of the swatch/recent grids.
const GRID_COLS: usize = 5;

/// The active controls tab of the left pane (Qt `QTabWidget::currentTab`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    /// The RGB(A) channel tab.
    Rgb,
    /// The HSV channel tab.
    Hsv,
}

/// A named swatch set of the swatch tab bar.
#[derive(Debug, Clone)]
pub struct SwatchSet {
    /// The display name of the set.
    pub name: String,
    /// The colors in the set.
    pub colors: Vec<Color>,
}

/// Hit-test results of the swatch section, recomputed per frame.
#[derive(Debug, Clone, Default)]
pub struct SwatchHover {
    /// The cursor is over the swatch tab bar.
    pub tab: bool,
    /// The hovered swatch set index.
    pub set_idx: usize,
    /// The hovered close ("x") button of a swatch tab.
    pub close_idx: Option<usize>,
    /// The cursor is over the add-swatch button.
    pub add_btn: bool,
}

/// The step value of the keyboard change of the sat/value color values.
const SAT_VALUE_STEP: f32 = 0.005;
/// The step value of the keyboard change of the hue color value.
const HUE_STEP: i32 = 1;
/// The step value of the keyboard change of the RGBA color values.
const RGBA_STEP: i16 = 1;

/// Index of the hex input tree child.
const HEX_INPUT_INDEX: usize = 2;
/// Index of the first value input tree child; the seven inputs
/// ([R, G, B, A, H, S, V]) occupy `VALUE_INPUTS_INDEX..=VALUE_INPUTS_INDEX + 6`.
const VALUE_INPUTS_INDEX: usize = 3;
/// Index of the "new swatch set" name input tree child.
const NEW_SET_NAME_INDEX: usize = 10;

/// The label and font for the cancel button of the overlay.
///
/// NOTE: the original `iced_aw` implementation uses glyphs from its embedded
/// icon font (`font.ttf` via `iced_fonts`). We use plain text here so the
/// widget needs no custom font - this is a customization point.
fn cancel_icon() -> (&'static str, Font) {
    ("Cancel", Font::default())
}

/// The label and font for the submit button of the overlay.
///
/// NOTE: the original `iced_aw` implementation uses glyphs from its embedded
/// icon font (`font.ttf` via `iced_fonts`). We use plain text here so the
/// widget needs no custom font - this is a customization point.
fn ok_icon() -> (&'static str, Font) {
    ("OK", Font::default())
}

/// Helper trait containing functions for positioning of nodes.
///
/// Ported from `iced_aw`'s `core::overlay::Position` trait.
trait Position {
    /// Centers this node around the given position. If the node is over the
    /// specified bounds it's bouncing back to be fully visible on screen.
    fn center_and_bounce(&mut self, position: Point, bounds: Size);
}

impl Position for Node {
    fn center_and_bounce(&mut self, position: Point, bounds: Size) {
        let size = self.size();

        self.move_to_mut(Point::new(
            (position.x - (size.width / 2.0)).max(0.0),
            (position.y - (size.height / 2.0)).max(0.0),
        ));

        let new_self_bounds = self.bounds();

        self.move_to_mut(Point::new(
            if new_self_bounds.x + new_self_bounds.width > bounds.width {
                (new_self_bounds.x - (new_self_bounds.width - (bounds.width - new_self_bounds.x)))
                    .max(0.0)
            } else {
                new_self_bounds.x
            },
            if new_self_bounds.y + new_self_bounds.height > bounds.height {
                (new_self_bounds.y - (new_self_bounds.height - (bounds.height - new_self_bounds.y)))
                    .max(0.0)
            } else {
                new_self_bounds.y
            },
        ));
    }
}

/// Returns true if a point (relative to the picker bounds origin) lies inside
/// the hue ring band (between the ring's inner and outer radius).
fn is_in_ring_band(position: Point, size: Size) -> bool {
    let dx = position.x - size.width / 2.0;
    let dy = position.y - size.height / 2.0;
    let dist = dx * dx + dy * dy;
    let inner = INNER_DIAMETER / 2.0;
    let outer = size.width.min(size.height) / 2.0;
    dist >= inner * inner && dist <= outer * outer
}

/// The number of grid rows that fit `count` cells in `cols` columns.
fn grid_rows(count: usize, cols: usize) -> usize {
    if count == 0 { 0 } else { count.div_ceil(cols) }
}

/// True if two colors have identical RGBA bytes.
pub(crate) fn same_rgba(a: Color, b: Color) -> bool {
    (a.r * 255.0) as u8 == (b.r * 255.0) as u8
        && (a.g * 255.0) as u8 == (b.g * 255.0) as u8
        && (a.b * 255.0) as u8 == (b.b * 255.0) as u8
        && (a.a * 255.0) as u8 == (b.a * 255.0) as u8
}

/// Inserts `color` at the front of a swatch set: removes an existing
/// byte-exact duplicate, then truncates to [`MAX_SWATCHES_PER_SET`].
fn insert_swatch(colors: &mut Vec<Color>, color: Color) {
    colors.retain(|c| !same_rgba(*c, color));
    colors.insert(0, color);
    colors.truncate(MAX_SWATCHES_PER_SET);
}

/// Inserts a color into the recent colors list: dedupe, insert front,
/// truncate to [`MAX_RECENT`].
fn push_recent(colors: &mut Vec<Color>, color: Color) {
    colors.retain(|c| !same_rgba(*c, color));
    colors.insert(0, color);
    colors.truncate(MAX_RECENT);
}

/// Computes the new active set index after removing the tab at `index`.
/// Refuses (`None`) when `len <= 1` so the last real tab cannot be closed.
fn swatch_remove_index(len: usize, index: usize) -> Option<usize> {
    (len > 1).then(|| index.min(len - 2))
}

/// The estimated width of a swatch tab: 30px padding, 7px per name character,
/// plus room for the close mark when closable.
fn swatch_tab_width(name: &str, closable: bool) -> f32 {
    name.chars().count() as f32 * 7.0 + 30.0 + if closable { 14.0 } else { 0.0 }
}

/// The bounds of every real swatch tab plus the trailing "+" tab.
fn swatch_tab_bounds(bar: Rectangle, sets: &[SwatchSet]) -> (Vec<Rectangle>, Rectangle) {
    let closable = sets.len() > 1;
    let mut x = bar.x;
    let tabs = sets
        .iter()
        .map(|set| {
            let width = swatch_tab_width(&set.name, closable);
            let rect = Rectangle {
                x,
                y: bar.y,
                width,
                height: bar.height,
            };
            x += width;
            rect
        })
        .collect();
    let plus = Rectangle {
        x,
        y: bar.y,
        width: 30.0,
        height: bar.height,
    };
    (tabs, plus)
}

/// The close ("x") mark rect inside a swatch tab.
fn swatch_close_bounds(tab: &Rectangle) -> Rectangle {
    Rectangle {
        x: tab.x + tab.width - 16.0,
        y: tab.y + (tab.height - 12.0) / 2.0,
        width: 12.0,
        height: 12.0,
    }
}

/// The sub-rects of the "new swatch set" prompt page:
/// `(name input, Add button, Cancel button)`.
fn name_prompt_rects(page: Rectangle) -> (Rectangle, Rectangle, Rectangle) {
    let button_width = 48.0;
    let gap = 8.0;
    let cancel = Rectangle {
        x: page.x + page.width - button_width,
        y: page.y,
        width: button_width,
        height: page.height,
    };
    let add = Rectangle {
        x: cancel.x - gap - button_width,
        y: page.y,
        width: button_width,
        height: page.height,
    };
    let input = Rectangle {
        x: page.x,
        y: page.y,
        width: add.x - gap - page.x,
        height: page.height,
    };
    (input, add, cancel)
}

/// The overlay of the [`ColorPicker`](crate::color_picker::ColorPicker).
#[allow(missing_debug_implementations)]
pub struct ColorPickerOverlay<'a, 'b, Message, Theme>
where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog + iced::widget::text_input::Catalog,
    'b: 'a,
{
    /// The state of the [`ColorPickerOverlay`].
    state: &'a mut State,
    /// The cancel button of the [`ColorPickerOverlay`].
    cancel_button: Button<'a, Message, Theme, Renderer>,
    /// The submit button of the [`ColorPickerOverlay`].
    submit_button: Button<'a, Message, Theme, Renderer>,
    /// The hex text input of the [`ColorPickerOverlay`].
    hex_input: TextInput<'a, Message, Theme, Renderer>,
    /// The seven channel value inputs (`[R, G, B, A, H, S, V]`).
    value_inputs: [TextInput<'a, Message, Theme, Renderer>; 7],
    /// The "new swatch set" name input of the swatch section.
    new_set_name_input: TextInput<'a, Message, Theme, Renderer>,
    /// The function that produces a message when the submit button of the [`ColorPickerOverlay`].
    on_submit: &'a dyn Fn(Color) -> Message,
    /// Optional function that produces a message when the color changes during selection (real-time updates).
    on_color_change: Option<&'a dyn Fn(Color) -> Message>,
    /// The position strategy of the [`ColorPickerOverlay`]; `None` centers
    /// the dialog over the underlay.
    position: Option<OverlayPosition>,
    /// The bounds of the underlay widget, for parent-relative positions.
    parent_bounds: Rectangle,
    /// The underlay center, used as the anchor point when `position` is
    /// [`None`] (the default behavior).
    fallback_center: Point,
    /// The last known cursor position, for cursor-following positions.
    cursor_position: Point,
    /// The style of the [`ColorPickerOverlay`].
    class: &'a <Theme as style::Catalog>::Class<'b>,
    /// The reference to the tree holding the state of this overlay.
    tree: &'a mut Tree,
    viewport: Rectangle,
}

impl<'a, 'b, Message, Theme> ColorPickerOverlay<'a, 'b, Message, Theme>
where
    Message: 'static + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
    'b: 'a,
{
    /// Creates a new [`ColorPickerOverlay`] at the given position strategy.
    ///
    /// A [`None`] position centers the dialog over `fallback_center` and
    /// bounces it back into the viewport; a [`Some`] position resolves like
    /// the [`OverlayManager`](crate::overlay::OverlayManager) and is clamped
    /// to the viewport.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        state: &'a mut WidgetState,
        on_cancel: Message,
        on_submit: &'a dyn Fn(Color) -> Message,
        on_color_change: Option<&'a dyn Fn(Color) -> Message>,
        position: Option<OverlayPosition>,
        parent_bounds: Rectangle,
        fallback_center: Point,
        cursor_position: Point,
        class: &'a <Theme as style::Catalog>::Class<'b>,
        tree: &'a mut Tree,
        viewport: Rectangle,
    ) -> Self {
        let WidgetState { overlay_state, .. } = state;

        let (cancel_content, cancel_font) = cancel_icon();
        let (submit_content, submit_font) = ok_icon();

        let state_ptr: *mut State = overlay_state;
        let hex_fake = on_cancel.clone();
        let hex_input = TextInput::new("", unsafe { &(*state_ptr).hex_input })
            .padding([4, 8])
            .size(13)
            .on_input(move |text: String| {
                unsafe { (*state_ptr).hex_input = text; }
                hex_fake.clone()
            });
        let mut value_inputs = std::array::from_fn(|_| {
            TextInput::new("", "").padding([3, 4]).size(13)
        });
        for (i, text_input) in value_inputs.iter_mut().enumerate() {
            let slot: *mut String = unsafe { &mut (*state_ptr).value_inputs[i] };
            let fake = on_cancel.clone();
            *text_input = TextInput::new("", unsafe { &*slot })
                .padding([3, 4])
                .size(13)
                .width(Length::Fixed(VALUE_WIDTH))
                .on_input(move |text: String| {
                    unsafe { *slot = text; }
                    fake.clone()
                });
        }

        // Name input of the "new swatch set" prompt. `on_input` only writes
        // the text into the state (the display value is mirrored from
        // `pending_swatch_name`); Enter pushes the fake message that `update`
        // interprets as "add the set".
        let name_fake = on_cancel.clone();
        let name_slot: *mut String = unsafe { &mut (*state_ptr).pending_swatch_name };
        let name_input = TextInput::new("Name", unsafe { &*name_slot })
            .padding([4, 8])
            .size(13)
            .width(Length::Fill)
            .on_input({
                let on_input_fake = on_cancel.clone();
                move |text: String| {
                    unsafe { *name_slot = text; }
                    on_input_fake.clone()
                }
            })
            .on_submit(name_fake);

        ColorPickerOverlay {
            state: overlay_state,
            cancel_button: Button::new(
                widget::Text::new(cancel_content)
                    .align_x(Horizontal::Center)
                    .width(Length::Fill)
                    .font(cancel_font),
            )
            .width(Length::Fill)
            .on_press(on_cancel.clone()),
            submit_button: Button::new(
                widget::Text::new(submit_content)
                    .align_x(Horizontal::Center)
                    .width(Length::Fill)
                    .font(submit_font),
            )
            .width(Length::Fill)
            .on_press(on_cancel), // Sending a fake message
            hex_input,
            value_inputs,
            new_set_name_input: name_input,
            on_submit,
            on_color_change,
            position,
            parent_bounds,
            fallback_center,
            cursor_position,
            class,
            tree,
            viewport,
        }
    }

    /// Turn this [`ColorPickerOverlay`] into an overlay [`Element`](overlay::Element).
    #[must_use]
    pub fn overlay(self) -> overlay::Element<'a, Message, Theme, Renderer> {
        overlay::Element::new(Box::new(self))
    }

    /// Force redraw all components if the internal state was changed
    fn clear_cache(&self) {
        self.state.clear_cache();
    }

    /// The event handling for the HSV color area (hue ring + sat/value square).
    fn on_event_hsv_color(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        shell: &mut Shell<Message>,
    ) -> event::Status {
        let mut hsv_color_children = layout.children();

        let hsv_color: Hsv = self.state.hsv();
        let mut color_changed = false;

        let sat_value_bounds = hsv_color_children
            .next()
            .expect("widget: Layout should have a sat/value layout")
            .bounds();
        let hue_bounds = hsv_color_children
            .next()
            .expect("widget: Layout should have a hue layout")
            .bounds();

        let is_in_ring = |position: Point| is_in_ring_band(position, hue_bounds.size());

        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => match delta {
                mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => {
                    let move_value =
                        |value: u16, y: f32| ((i32::from(value) + y as i32).rem_euclid(360)) as u16;

                    if cursor.is_over(hue_bounds) {
                        let hue = move_value(hsv_color.hue, *y);
                        self.state.apply_color(Color {
                            a: self.state.color.a,
                            ..Hsv {
                                hue,
                                ..hsv_color
                            }
                            .into()
                        });
                        self.state.hue = hue;
                        color_changed = true;
                    }
                }
            },
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if cursor.is_over(sat_value_bounds) {
                    self.state.color_bar_dragged = ColorBarDragged::SatValue;
                    self.state.focus = Focus::Square;
                }
                if cursor.is_over(hue_bounds)
                    && cursor.position_in(hue_bounds).is_some_and(is_in_ring)
                {
                    self.state.color_bar_dragged = ColorBarDragged::Hue;
                    self.state.focus = Focus::Ring;
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. }) => {
                self.state.color_bar_dragged = ColorBarDragged::None;
            }
            _ => {}
        }

        // The percentages are computed from the absolute cursor position so
        // the drag keeps tracking when the cursor leaves the bounds, like
        // iced's scrollbar. The result is clamped to the widget ends; when the
        // cursor is unavailable entirely, the previous value is kept.
        let calc_percentage_sat = |cursor_position: Point| {
            ((cursor_position.x - sat_value_bounds.x) / sat_value_bounds.width).clamp(0.0, 1.0)
        };

        let calc_percentage_value = |cursor_position: Point| {
            ((cursor_position.y - sat_value_bounds.y) / sat_value_bounds.height).clamp(0.0, 1.0)
        };

        let calc_hue = |cursor_position: Point| {
            let dx = cursor_position.x - hue_bounds.x - hue_bounds.width / 2.0;
            let dy = cursor_position.y - hue_bounds.y - hue_bounds.height / 2.0;
            hue_from_angle(dy.atan2(dx).to_degrees())
        };

        match self.state.color_bar_dragged {
            ColorBarDragged::SatValue => {
                self.state.apply_color(Color {
                    a: self.state.color.a,
                    ..Hsv {
                        saturation: cursor
                            .land()
                            .position()
                            .map(calc_percentage_sat)
                            .unwrap_or(hsv_color.saturation),
                        value: cursor
                            .land()
                            .position()
                            .map(calc_percentage_value)
                            .unwrap_or(hsv_color.value),
                        ..hsv_color
                    }
                    .into()
                });
                color_changed = true;
            }
            ColorBarDragged::Hue => {
                let hue = cursor
                    .land()
                    .position()
                    .map(calc_hue)
                    .unwrap_or(hsv_color.hue);
                self.state.apply_color(Color {
                    a: self.state.color.a,
                    ..Hsv {
                        hue,
                        ..hsv_color
                    }
                    .into()
                });
                self.state.hue = hue;
                color_changed = true;
            }
            _ => {}
        }

        if color_changed {
            // Call on_color_change callback for real-time updates
            if let Some(on_color_change) = self.on_color_change {
                shell.publish(on_color_change(self.state.color));
            }
            event::Status::Captured
        } else {
            event::Status::Ignored
        }
    }

    /// The event handling for the slider rows of the active tab
    /// (RGB(A) or HSV channels).
    #[allow(clippy::too_many_lines)]
    fn on_event_sliders(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        shell: &mut Shell<Message>,
    ) -> event::Status {
        let mut slider_children = layout.children();
        let mut color_changed = false;
        let mut captured = false;

        let mut row_bounds = Vec::new();
        for _ in 0..4 {
            let mut row_children = slider_children
                .next()
                .expect("widget: Layout should have a slider row layout")
                .children();
            let _ = row_children.next();
            let bar_bounds = row_children
                .next()
                .expect("widget: Layout should have a bar layout")
                .bounds();
            row_bounds.push(bar_bounds);
        }

        let channels = self.active_tab_channels();

        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => match delta {
                mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => {
                    let move_rgba = |value: f32, y: f32| value.mul_add(255.0, y).clamp(0.0, 255.0) / 255.0;
                    let move_hue = |hue: u16, y: f32| ((i32::from(hue) + y as i32).rem_euclid(360)) as u16;
                    let move_fraction = |value: f32, y: f32| (value + y * SAT_VALUE_STEP).clamp(0.0, 1.0);

                    for (row, bounds) in row_bounds.iter().enumerate() {
                        let Some(&channel) = channels.get(row) else { continue; };
                        if !cursor.is_over(*bounds) {
                            continue;
                        }
                        captured = true;
                        match channel {
                            0..=3 => {
                                let value = match channel {
                                    0 => self.state.color.r,
                                    1 => self.state.color.g,
                                    2 => self.state.color.b,
                                    _ => self.state.color.a,
                                };
                                let new_value = move_rgba(value, *y);
                                self.state.apply_color(Color {
                                    r: if channel == 0 { new_value } else { self.state.color.r },
                                    g: if channel == 1 { new_value } else { self.state.color.g },
                                    b: if channel == 2 { new_value } else { self.state.color.b },
                                    a: if channel == 3 { new_value } else { self.state.color.a },
                                });
                                color_changed = true;
                            }
                            4 => {
                                let hsv: Hsv = self.state.hsv();
                                let hue = move_hue(hsv.hue, *y);
                                self.state.apply_color(Color {
                                    a: self.state.color.a,
                                    ..Hsv {
                                        hue,
                                        ..hsv
                                    }
                                    .into()
                                });
                                self.state.hue = hue;
                                color_changed = true;
                            }
                            5 => {
                                let hsv: Hsv = self.state.hsv();
                                self.state.apply_color(Color {
                                    a: self.state.color.a,
                                    ..Hsv {
                                        saturation: move_fraction(hsv.saturation, *y),
                                        ..hsv
                                    }
                                    .into()
                                });
                                color_changed = true;
                            }
                            _ => {
                                let hsv: Hsv = self.state.hsv();
                                self.state.apply_color(Color {
                                    a: self.state.color.a,
                                    ..Hsv {
                                        value: move_fraction(hsv.value, *y),
                                        ..hsv
                                    }
                                    .into()
                                });
                                color_changed = true;
                            }
                        }
                    }
                }
            },
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                for (row, bounds) in row_bounds.iter().enumerate() {
                    if !cursor.is_over(*bounds) {
                        continue;
                    }
                    let Some(&channel) = channels.get(row) else { continue; };
                    captured = true;
                    let (dragged, focus) = (
                        match channel {
                            0 => ColorBarDragged::Red,
                            1 => ColorBarDragged::Green,
                            2 => ColorBarDragged::Blue,
                            3 => ColorBarDragged::Alpha,
                            4 => ColorBarDragged::HsvHue,
                            5 => ColorBarDragged::HsvSat,
                            _ => ColorBarDragged::HsvVal,
                        },
                        channel_focus(channel),
                    );
                    self.state.color_bar_dragged = dragged;
                    self.state.focus = focus;
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. }) => {
                self.state.color_bar_dragged = ColorBarDragged::None;
            }
            _ => {}
        }

        let calc_percentage = |bounds: Rectangle, cursor_position: Point| {
            ((cursor_position.x - bounds.x) / bounds.width).clamp(0.0, 1.0)
        };

        match self.state.color_bar_dragged {
            ColorBarDragged::Red => {
                self.state.apply_color(Color {
                    r: cursor
                        .land()
                        .position()
                        .map(|position| calc_percentage(row_bounds[0], position))
                        .unwrap_or(self.state.color.r),
                    ..self.state.color
                });
                color_changed = true;
            }
            ColorBarDragged::Green => {
                self.state.apply_color(Color {
                    g: cursor
                        .land()
                        .position()
                        .map(|position| calc_percentage(row_bounds[1], position))
                        .unwrap_or(self.state.color.g),
                    ..self.state.color
                });
                color_changed = true;
            }
            ColorBarDragged::Blue => {
                self.state.apply_color(Color {
                    b: cursor
                        .land()
                        .position()
                        .map(|position| calc_percentage(row_bounds[2], position))
                        .unwrap_or(self.state.color.b),
                    ..self.state.color
                });
                color_changed = true;
            }
            ColorBarDragged::Alpha => {
                self.state.apply_color(Color {
                    a: cursor
                        .land()
                        .position()
                        .map(|position| calc_percentage(row_bounds[3], position))
                        .unwrap_or(self.state.color.a),
                    ..self.state.color
                });
                color_changed = true;
            }
            ColorBarDragged::HsvHue => {
                let hsv: Hsv = self.state.hsv();
                let hue = cursor
                    .land()
                    .position()
                    .map(|position| {
                        (calc_percentage(row_bounds[0], position) * 360.0) as u16 % 360
                    })
                    .unwrap_or(hsv.hue);
                self.state.apply_color(Color {
                    a: self.state.color.a,
                    ..Hsv {
                        hue,
                        ..hsv
                    }
                    .into()
                });
                self.state.hue = hue;
                color_changed = true;
            }
            ColorBarDragged::HsvSat => {
                let hsv: Hsv = self.state.hsv();
                self.state.apply_color(Color {
                    a: self.state.color.a,
                    ..Hsv {
                        saturation: cursor
                            .land()
                            .position()
                            .map(|position| calc_percentage(row_bounds[1], position))
                            .unwrap_or(hsv.saturation),
                        ..hsv
                    }
                    .into()
                });
                color_changed = true;
            }
            ColorBarDragged::HsvVal => {
                let hsv: Hsv = self.state.hsv();
                self.state.apply_color(Color {
                    a: self.state.color.a,
                    ..Hsv {
                        value: cursor
                            .land()
                            .position()
                            .map(|position| calc_percentage(row_bounds[2], position))
                            .unwrap_or(hsv.value),
                        ..hsv
                    }
                    .into()
                });
                color_changed = true;
            }
            _ => {}
        }

        if color_changed {
            // Call on_color_change callback for real-time updates
            if let Some(on_color_change) = self.on_color_change {
                shell.publish(on_color_change(self.state.color));
            }
            event::Status::Captured
        } else if captured {
            event::Status::Captured
        } else {
            event::Status::Ignored
        }
    }

    /// The even handling for the keyboard input.
    fn on_event_keyboard(&mut self, event: &Event, shell: &mut Shell<Message>) -> event::Status {
        if self.state.focus == Focus::None
            || self.state.hex_focused
            || self.state.value_focus.is_some()
        {
            return event::Status::Ignored;
        }

        if let Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event {
            let mut status = event::Status::Ignored;

            if matches!(key, keyboard::Key::Named(keyboard::key::Named::Tab)) {
                if self.state.keyboard_modifiers.shift() {
                    self.state.focus = previous_focus(
                        self.state.focus,
                        self.state.active_tab,
                        self.state.naming_new_set,
                    );
                } else {
                    self.state.focus = next_focus(
                        self.state.focus,
                        self.state.active_tab,
                        self.state.naming_new_set,
                    );
                }
                // The name input only accepts typing while internally focused.
                if self.state.focus == Focus::NewSetName {
                    self.focus_name_input();
                }
                // TODO: maybe place this better
                self.clear_cache();
            } else {
                let sat_value_handle = |key_code: &keyboard::Key, color: &mut Color, mut hsv_color: Hsv| {
                    let mut status = event::Status::Ignored;

                    match key_code {
                        keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                            hsv_color.saturation -= SAT_VALUE_STEP;
                            status = event::Status::Captured;
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                            hsv_color.saturation += SAT_VALUE_STEP;
                            status = event::Status::Captured;
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                            hsv_color.value -= SAT_VALUE_STEP;
                            status = event::Status::Captured;
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                            hsv_color.value += SAT_VALUE_STEP;
                            status = event::Status::Captured;
                        }
                        _ => {}
                    }

                    hsv_color.saturation = hsv_color.saturation.clamp(0.0, 1.0);
                    hsv_color.value = hsv_color.value.clamp(0.0, 1.0);

                    *color = Color {
                        a: color.a,
                        ..hsv_color.into()
                    };
                    status
                };

                let hsv_sat_handle = |key_code: &keyboard::Key, color: &mut Color, mut hsv_color: Hsv| {
                    let mut status = event::Status::Ignored;

                    match key_code {
                        keyboard::Key::Named(
                            keyboard::key::Named::ArrowLeft | keyboard::key::Named::ArrowDown,
                        ) => {
                            hsv_color.saturation -= SAT_VALUE_STEP;
                            status = event::Status::Captured;
                        }
                        keyboard::Key::Named(
                            keyboard::key::Named::ArrowRight | keyboard::key::Named::ArrowUp,
                        ) => {
                            hsv_color.saturation += SAT_VALUE_STEP;
                            status = event::Status::Captured;
                        }
                        _ => {}
                    }

                    hsv_color.saturation = hsv_color.saturation.clamp(0.0, 1.0);
                    *color = Color {
                        a: color.a,
                        ..hsv_color.into()
                    };
                    status
                };

                let hsv_val_handle = |key_code: &keyboard::Key, color: &mut Color, mut hsv_color: Hsv| {
                    let mut status = event::Status::Ignored;

                    match key_code {
                        keyboard::Key::Named(
                            keyboard::key::Named::ArrowLeft | keyboard::key::Named::ArrowDown,
                        ) => {
                            hsv_color.value -= SAT_VALUE_STEP;
                            status = event::Status::Captured;
                        }
                        keyboard::Key::Named(
                            keyboard::key::Named::ArrowRight | keyboard::key::Named::ArrowUp,
                        ) => {
                            hsv_color.value += SAT_VALUE_STEP;
                            status = event::Status::Captured;
                        }
                        _ => {}
                    }

                    hsv_color.value = hsv_color.value.clamp(0.0, 1.0);
                    *color = Color {
                        a: color.a,
                        ..hsv_color.into()
                    };
                    status
                };

                let hue_handle = |key_code: &keyboard::Key, color: &mut Color, mut hsv_color: Hsv, hue: &mut u16| {
                    let mut status = event::Status::Ignored;

                    let mut value = i32::from(hsv_color.hue);

                    match key_code {
                        keyboard::Key::Named(
                            keyboard::key::Named::ArrowLeft | keyboard::key::Named::ArrowDown,
                        ) => {
                            value -= HUE_STEP;
                            status = event::Status::Captured;
                        }
                        keyboard::Key::Named(
                            keyboard::key::Named::ArrowRight | keyboard::key::Named::ArrowUp,
                        ) => {
                            value += HUE_STEP;
                            status = event::Status::Captured;
                        }
                        _ => {}
                    }

                    hsv_color.hue = value.rem_euclid(360) as u16;
                    *hue = hsv_color.hue;

                    *color = Color {
                        a: color.a,
                        ..hsv_color.into()
                    };

                    status
                };

                let rgba_bar_handle = |key_code: &keyboard::Key, value: &mut f32| {
                    let mut byte_value = (*value * 255.0) as i16;
                    let mut status = event::Status::Captured;

                    match key_code {
                        keyboard::Key::Named(
                            keyboard::key::Named::ArrowLeft | keyboard::key::Named::ArrowDown,
                        ) => {
                            byte_value -= RGBA_STEP;
                            status = event::Status::Captured;
                        }
                        keyboard::Key::Named(
                            keyboard::key::Named::ArrowRight | keyboard::key::Named::ArrowUp,
                        ) => {
                            byte_value += RGBA_STEP;
                            status = event::Status::Captured;
                        }
                        _ => {}
                    }
                    *value = f32::from(byte_value.clamp(0, 255)) / 255.0;

                    status
                };

                match self.state.focus {
                    Focus::Square => {
                        let hsv = self.state.hsv();
                        status = sat_value_handle(key, &mut self.state.color, hsv);
                    }
                    Focus::Ring => {
                        let hsv = self.state.hsv();
                        status = hue_handle(key, &mut self.state.color, hsv, &mut self.state.hue);
                    }
                    Focus::Red => status = rgba_bar_handle(key, &mut self.state.color.r),
                    Focus::Green => status = rgba_bar_handle(key, &mut self.state.color.g),
                    Focus::Blue => status = rgba_bar_handle(key, &mut self.state.color.b),
                    Focus::Alpha => status = rgba_bar_handle(key, &mut self.state.color.a),
                    Focus::HsvHue => {
                        let hsv = self.state.hsv();
                        status = hue_handle(key, &mut self.state.color, hsv, &mut self.state.hue);
                    }
                    Focus::HsvSat => {
                        let hsv = self.state.hsv();
                        status = hsv_sat_handle(key, &mut self.state.color, hsv);
                    }
                    Focus::HsvVal => {
                        let hsv = self.state.hsv();
                        status = hsv_val_handle(key, &mut self.state.color, hsv);
                    }
                    Focus::TabRgb => {
                        status = match key {
                            keyboard::Key::Named(
                                keyboard::key::Named::Enter | keyboard::key::Named::Space,
                            ) => {
                                self.set_active_tab(ActiveTab::Rgb, shell);
                                event::Status::Captured
                            }
                            keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                                self.state.focus = Focus::TabHsv;
                                event::Status::Captured
                            }
                            _ => event::Status::Ignored,
                        };
                    }
                    Focus::TabHsv => {
                        status = match key {
                            keyboard::Key::Named(
                                keyboard::key::Named::Enter | keyboard::key::Named::Space,
                            ) => {
                                self.set_active_tab(ActiveTab::Hsv, shell);
                                event::Status::Captured
                            }
                            keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                                self.state.focus = Focus::TabRgb;
                                event::Status::Captured
                            }
                            _ => event::Status::Ignored,
                        };
                    }
                    Focus::Reset => {
                        status = match key {
                            keyboard::Key::Named(
                                keyboard::key::Named::Enter | keyboard::key::Named::Space,
                            ) => {
                                self.state.apply_color(self.state.initial_color);
                                self.state.sync_display();
                                self.state.clear_cache();
                                event::Status::Captured
                            }
                            _ => event::Status::Ignored,
                        };
                    }
                    Focus::Swatches => {
                        let set_len = self
                            .state
                            .swatch_sets
                            .get(self.state.active_swatch_tab)
                            .map_or(0, |set| set.colors.len());
                        let mut idx = self
                            .state
                            .focused_swatch
                            .filter(|(set, _)| *set == self.state.active_swatch_tab)
                            .map_or(0, |(_, idx)| idx);

                        status = match key {
                            keyboard::Key::Named(
                                keyboard::key::Named::Enter | keyboard::key::Named::Space,
                            ) => {
                                if let Some(color) = self
                                    .state
                                    .swatch_sets
                                    .get(self.state.active_swatch_tab)
                                    .and_then(|set| set.colors.get(idx))
                                {
                                    self.select_color_from_swatch(*color, shell);
                                    event::Status::Captured
                                } else {
                                    event::Status::Ignored
                                }
                            }
                            keyboard::Key::Named(keyboard::key::Named::ArrowLeft
                            | keyboard::key::Named::ArrowRight
                            | keyboard::key::Named::ArrowUp
                            | keyboard::key::Named::ArrowDown) => {
                                if set_len > 0 {
                                    let delta = match key {
                                        keyboard::Key::Named(
                                            keyboard::key::Named::ArrowLeft,
                                        ) => -1,
                                        keyboard::Key::Named(
                                            keyboard::key::Named::ArrowRight,
                                        ) => 1,
                                        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                                            -(GRID_COLS as i32)
                                        }
                                        _ => GRID_COLS as i32,
                                    };
                                    idx = (idx as i32 + delta).clamp(0, set_len as i32 - 1) as usize;
                                    self.state.focused_swatch =
                                        Some((self.state.active_swatch_tab, idx));
                                    event::Status::Captured
                                } else {
                                    event::Status::Ignored
                                }
                            }
                            _ => event::Status::Ignored,
                        };
                    }
                    _ => {}
                }

                // If color changed via keyboard, call on_color_change callback
                if status == event::Status::Captured
                    && let Some(on_color_change) = self.on_color_change
                {
                    shell.publish(on_color_change(self.state.color));
                }
            }

            status
        } else if let Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) = event {
            self.state.keyboard_modifiers = *modifiers;
            event::Status::Ignored
        } else {
            event::Status::Ignored
        }
    }
}

impl<'a, 'b, Message, Theme> ColorPickerOverlay<'a, 'b, Message, Theme>
where
    Message: 'static + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
    'b: 'a,
{
    /// The channel indices of the rows of the active tab: `[R,G,B,A]` or
    /// `[H,S,V]`.
    fn active_tab_channels(&self) -> Vec<usize> {
        match self.state.active_tab {
            ActiveTab::Rgb => vec![0, 1, 2, 3],
            ActiveTab::Hsv => vec![4, 5, 6],
        }
    }

    /// Whether the tree child at `index` is a focused [`TextInput`].
    fn text_input_internal_focus(&self, index: usize) -> bool {
        let Some(child) = self.tree.children.get(index) else {
            return false;
        };
        child
            .state
            .downcast_ref::<text_input::State<<Renderer as text::Renderer>::Paragraph>>()
            .is_focused()
    }

    /// Blurs every TextInput of the overlay.
    fn unfocus_all_text_inputs(&mut self) {
        for i in [
            HEX_INPUT_INDEX,
            VALUE_INPUTS_INDEX,
            VALUE_INPUTS_INDEX + 1,
            VALUE_INPUTS_INDEX + 2,
            VALUE_INPUTS_INDEX + 3,
            VALUE_INPUTS_INDEX + 4,
            VALUE_INPUTS_INDEX + 5,
            VALUE_INPUTS_INDEX + 6,
        ] {
            if let Some(child) = self.tree.children.get_mut(i) {
                child
                    .state
                    .downcast_mut::<text_input::State<<Renderer as text::Renderer>::Paragraph>>()
                    .unfocus();
            }
        }
    }

    /// Port of `MainDialog.hex_changed`. Returns whether the color changed.
    fn on_hex_input(&mut self) -> bool {
        let text = self.state.hex_input.trim().to_uppercase();
        let mut input = text.clone();

        if !input.starts_with('#')
            && matches!(input.len(), 3 | 4 | 6 | 8)
            && input.chars().all(|c| c.is_ascii_hexdigit())
        {
            input.insert(0, '#');
            self.state.hex_input = input.clone();
        }

        if is_valid_hex(&input) {
            if let Some((r, g, b, alpha)) = parse_hex_digits(input.trim_start_matches('#')) {
                let digits = input.trim_start_matches('#');
                let alpha = if matches!(digits.len(), 3 | 6) {
                    (self.state.color.a * 255.0) as u8
                } else {
                    alpha
                };
                self.state.apply_color(Color::from_rgba8(r, g, b, f32::from(alpha) / 255.0));
                self.state.hex_input = color_to_hex_argb(self.state.color);
                return true;
            }
            false
        } else {
            // Keep only valid characters and write the cleaned string back.
            let clean: String = input
                .chars()
                .filter(|c| c.is_ascii_hexdigit() || *c == '#')
                .take(9)
                .collect();
            self.state.hex_input = clean;
            false
        }
    }

    /// Applies a changed channel value text to the color.
    /// Returns whether the color changed.
    fn on_value_input(&mut self, i: usize) -> bool {
        let text = self.state.value_inputs[i].trim().to_owned();
        let parsed = match i {
            0..=3 => text.parse::<i32>().ok().map(|v| u16::from(clamp_u8(v))),
            4 => text.parse::<i32>().ok().map(clamp_hue),
            5 | 6 => text.parse::<i32>().ok().map(|v| u16::from(clamp_u8(v))),
            _ => None,
        };
        if let Some(value) = parsed {
            match i {
                0..=3 => {
                    self.state.apply_color(Color {
                        r: if i == 0 { f32::from(value) / 255.0 } else { self.state.color.r },
                        g: if i == 1 { f32::from(value) / 255.0 } else { self.state.color.g },
                        b: if i == 2 { f32::from(value) / 255.0 } else { self.state.color.b },
                        a: if i == 3 { f32::from(value) / 255.0 } else { self.state.color.a },
                    });
                    self.state.value_inputs[i] = value.to_string();
                    self.state.hex_input = color_to_hex_argb(self.state.color);
                }
                4 => {
                    let mut hsv: Hsv = self.state.hsv();
                    hsv.hue = value;
                    self.state.apply_color(Color {
                        a: self.state.color.a,
                        ..hsv.into()
                    });
                    self.state.hue = value;
                    self.state.value_inputs[4] = value.to_string();
                    self.state.hex_input = color_to_hex_argb(self.state.color);
                }
                5 => {
                    let mut hsv: Hsv = self.state.hsv();
                    hsv.saturation = f32::from(value) / 255.0;
                    self.state.apply_color(Color {
                        a: self.state.color.a,
                        ..hsv.into()
                    });
                    self.state.value_inputs[5] = value.to_string();
                    self.state.hex_input = color_to_hex_argb(self.state.color);
                }
                _ => {
                    let mut hsv: Hsv = self.state.hsv();
                    hsv.value = f32::from(value) / 255.0;
                    self.state.apply_color(Color {
                        a: self.state.color.a,
                        ..hsv.into()
                    });
                    self.state.value_inputs[6] = value.to_string();
                    self.state.hex_input = color_to_hex_argb(self.state.color);
                }
            }
            true
        } else {
            self.state.sync_display();
            false
        }
    }

    /// Switches the active controls tab and refreshes the field values.
    fn set_active_tab(&mut self, tab: ActiveTab, shell: &mut Shell<Message>) {
        if self.state.active_tab != tab {
            self.state.active_tab = tab;
            self.state.sync_display();
            self.state.clear_cache();
            shell.invalidate_layout();
        }
    }

    /// Applies a color picked from a swatch: updates the color, the hex and
    /// value fields and publishes `on_color_change`.
    fn select_color_from_swatch(&mut self, color: Color, shell: &mut Shell<Message>) {
        self.state.apply_color(color);
        self.state.sync_display();
        self.state.clear_cache();
        if let Some(on_color_change) = self.on_color_change {
            shell.publish(on_color_change(color));
        }
    }

    /// Pushes a new swatch set with the given name, selects it and closes
    /// the name prompt. Empty names are ignored.
    fn add_swatch_set(&mut self, name: String, shell: &mut Shell<Message>) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        self.state.swatch_sets.push(SwatchSet {
            name: name.to_owned(),
            colors: Vec::new(),
        });
        self.state.active_swatch_tab = self.state.swatch_sets.len() - 1;
        self.state.naming_new_set = false;
        self.state.pending_swatch_name.clear();
        self.state.clear_cache();
        shell.invalidate_layout();
    }

    /// Aborts the "new swatch set" prompt.
    fn abort_new_set(&mut self, shell: &mut Shell<Message>) {
        self.state.naming_new_set = false;
        self.state.pending_swatch_name.clear();
        self.state.focus = Focus::Swatches;
        self.state.clear_cache();
        shell.invalidate_layout();
    }

    /// Focuses the "new swatch set" name input of the widget tree.
    fn focus_name_input(&mut self) {
        if let Some(child) = self.tree.children.get_mut(NEW_SET_NAME_INDEX) {
            child
                .state
                .downcast_mut::<text_input::State<<Renderer as text::Renderer>::Paragraph>>()
                .focus();
        }
    }

    /// The event handling of the swatch section: the tab bar (switch, close,
    /// add set), the grid of the active set, the add-current-color button
    /// and the "new swatch set" name prompt. Returns whether the event was
    /// captured.
    #[allow(clippy::too_many_arguments)]
    fn on_event_swatches(
        &mut self,
        event: &Event,
        cursor: Cursor,
        shell: &mut Shell<Message>,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        tab_bar_layout: Layout<'_>,
        page_layout: Layout<'_>,
        add_btn_layout: Layout<'_>,
    ) -> bool {
        // Escape aborts the name prompt.
        if self.state.naming_new_set
            && matches!(
                event,
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                })
            )
        {
            self.abort_new_set(shell);
            return true;
        }

        let bar = tab_bar_layout.bounds();
        let (tabs, plus) = swatch_tab_bounds(bar, &self.state.swatch_sets);

        // Refresh the hover bookkeeping.
        if matches!(
            event,
            Event::Mouse(
                mouse::Event::CursorMoved { .. }
                    | mouse::Event::ButtonPressed(_)
                    | mouse::Event::ButtonReleased(_),
            ) | Event::Touch(touch::Event::FingerMoved { .. })
        ) {
            self.state.swatch_hover.tab = cursor.is_over(bar);
            self.state.swatch_hover.set_idx = tabs
                .iter()
                .position(|tab| cursor.is_over(*tab))
                .unwrap_or_default();
            self.state.swatch_hover.close_idx = tabs
                .iter()
                .enumerate()
                .find_map(|(i, tab)| {
                    cursor.is_over(swatch_close_bounds(tab)).then_some(i)
                });
            self.state.swatch_hover.add_btn = cursor.is_over(add_btn_layout.bounds());
            self.state.plus_tab_hovered = cursor.is_over(plus);
        }

        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) = event
        {
            // Name prompt buttons.
            if self.state.naming_new_set {
                let (_, add_rect, cancel_rect) = name_prompt_rects(page_layout.bounds());
                if cursor.is_over(add_rect) {
                    self.add_swatch_set(self.state.pending_swatch_name.clone(), shell);
                    return true;
                }
                if cursor.is_over(cancel_rect) {
                    self.abort_new_set(shell);
                    return true;
                }
            }

            // Close ("x") marks have priority over tab switching.
            if let Some(i) = self.state.swatch_hover.close_idx
                && let Some(new_active) =
                    swatch_remove_index(self.state.swatch_sets.len(), i)
            {
                self.state.swatch_sets.remove(i);
                self.state.active_swatch_tab = new_active;
                self.state.clear_cache();
                shell.invalidate_layout();
                return true;
            }

            // Real tabs.
            let hovered_tab = self.state.swatch_hover.set_idx;
            if hovered_tab < tabs.len() && self.state.active_swatch_tab != hovered_tab {
                self.state.active_swatch_tab = hovered_tab;
                self.state.clear_cache();
                shell.invalidate_layout();
                return true;
            }

            // The "+" tab opens the name prompt.
            if cursor.is_over(plus) {
                self.state.naming_new_set = true;
                self.state.focus = Focus::NewSetName;
                self.focus_name_input();
                shell.invalidate_layout();
                return true;
            }

            // Grid cells of the active set.
            for (i, cell) in page_layout.children().enumerate() {
                if cursor.is_over(cell.bounds())
                    && let Some(color) = self
                        .state
                        .swatch_sets
                        .get(self.state.active_swatch_tab)
                        .and_then(|set| set.colors.get(i))
                {
                    self.select_color_from_swatch(*color, shell);
                    return true;
                }
            }

            // The add-current-color button.
            if cursor.is_over(add_btn_layout.bounds()) {
                let color = self.state.color;
                let set_idx = self.state.active_swatch_tab;
                if let Some(set) = self.state.swatch_sets.get_mut(set_idx) {
                    insert_swatch(&mut set.colors, color);
                    self.state.clear_cache();
                    shell.invalidate_layout();
                }
                return true;
            }
        }

        // Forward events to the name input of the open prompt.
        if self.state.naming_new_set {
            let name_before = self.state.pending_swatch_name.clone();
            if let Some(tree_child) = self.tree.children.get_mut(NEW_SET_NAME_INDEX)
                && let Some(input_layout) = page_layout.children().next()
            {
                let mut local_messages = Vec::new();
                {
                    let mut local_shell = Shell::new(&mut local_messages);
                    self.new_set_name_input.update(
                        tree_child,
                        event,
                        input_layout,
                        cursor,
                        renderer,
                        clipboard,
                        &mut local_shell,
                        &input_layout.bounds(),
                    );
                    if local_shell.is_event_captured() {
                        shell.capture_event();
                    }
                    shell.request_redraw_at(local_shell.redraw_request());
                    shell.request_input_method(local_shell.input_method());
                }
                // The name input only pushes a message on Enter (its
                // `on_input` writes into the state directly); a message with
                // an unchanged name means Enter was pressed.
                if !local_messages.is_empty() && self.state.pending_swatch_name == name_before {
                    self.add_swatch_set(self.state.pending_swatch_name.clone(), shell);
                    return true;
                }
            }
        }

        false
    }
}

impl<'a, Message, Theme> Overlay<Message, Theme, Renderer>
    for ColorPickerOverlay<'a, '_, Message, Theme>
where
    Message: 'static + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> Node {
        let limits = Limits::new(Size::ZERO, bounds)
            .shrink(Size::new(OUTER_MARGIN, OUTER_MARGIN))
            .width(Length::Fill)
            .height(Length::Fill)
            .max_width(DIALOG_MAX_SIZE.width)
            .max_height(DIALOG_MAX_SIZE.height);

        // Fixed two-pane row: left pane (ring + controls) and right pane
        // (previews, swatches, recent, buttons).
        let divider = Row::<(), Theme, Renderer>::new()
            .spacing(PANE_SPACING)
            .push(Row::new().width(Length::Fixed(RING_DIM)).height(Length::Fill))
            .push(
                Row::new()
                    .width(Length::Fixed(RIGHT_PANE_WIDTH))
                    .height(Length::Fill),
            )
            .layout(self.tree, renderer, &limits);

        let mut divider_children = divider.children().iter();

        let block1_bounds = divider_children
            .next()
            .expect("Divider should have a first child")
            .bounds();
        let block2_bounds = divider_children
            .next()
            .expect("Divider should have a second child")
            .bounds();

        // ----------- Block 1 (left pane) ----------------------
        let block1_node = left_pane_layout(self, renderer, block1_bounds);

        // ----------- Block 2 (right pane) ----------------------
        let block2_node = right_pane_layout(self, renderer, block2_bounds);

        let (width, height) = (
            block1_node.size().width + block2_node.size().width + PANE_SPACING,
            block2_node.size().height.max(block1_node.size().height),
        );

        let mut node =
            Node::with_children(Size::new(width, height), vec![block1_node, block2_node]);

        if let Some(position) = self.position {
            let viewport = Rectangle::with_size(bounds);
            let content = Rectangle::new(Point::ORIGIN, node.size());
            let point = position.resolve(
                self.parent_bounds,
                self.cursor_position,
                viewport,
                content,
                &[],
            );
            node.move_to_mut(clamp_to_viewport(point, node.size(), viewport));
        } else {
            node.center_and_bounce(self.fallback_center, bounds);
        }
        node
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<Message>,
    ) {
        // Refresh the TextInput focus bookkeeping from the widget tree.
        self.state.hex_focused = self.text_input_internal_focus(HEX_INPUT_INDEX);
        self.state.value_focus = None;
        for i in 0..7 {
            if self.text_input_internal_focus(VALUE_INPUTS_INDEX + i) {
                self.state.value_focus = Some(i);
                break;
            }
        }

        let mut children = layout.children();
        // ----------- Block 1 (left pane) ----------------------
        let block1_layout = children
            .next()
            .expect("widget: Layout should have a 1. block layout");
        let mut block1_children = block1_layout.children();

        let picker_layout = block1_children
            .next()
            .expect("widget: Layout should have a picker layout");
        let mut picker_children = picker_layout.children();
        let _sat_value_layout = picker_children
            .next()
            .expect("widget: Layout should have a sat/value layout");
        let _ring_bounds = picker_children
            .next()
            .expect("widget: Layout should have a hue layout")
            .bounds();

        let tab_bar_layout = block1_children
            .next()
            .expect("widget: Layout should have a tab bar layout");

        let controls_layout = block1_children
            .next()
            .expect("widget: Layout should have a controls layout");

        let hex_layout = block1_children
            .next()
            .expect("widget: Layout should have a hex container layout");
        // ----------- Block 1 end ------------------

        // ----------- Block 2 (right pane) ----------------------
        let block2_layout = children
            .next()
            .expect("widget: Layout should have a 2. block layout");
        let mut block2_children = block2_layout.children();

        let mut fake_messages: Vec<Message> = Vec::new();

        let _preview_layout = block2_children.next();
        let _swatch_label_layout = block2_children.next();
        let swatch_tab_bar_layout = block2_children
            .next()
            .expect("widget: Layout should have a swatch tab bar layout");
        let swatch_page_layout = block2_children
            .next()
            .expect("widget: Layout should have a swatch tab page layout");
        let add_btn_layout = block2_children
            .next()
            .expect("widget: Layout should have an add-swatch button layout");
        let _divider_layout = block2_children.next();
        let _recent_label_layout = block2_children.next();
        let recent_grid_layout = block2_children
            .next()
            .expect("widget: Layout should have a recent grid layout");
        let mut buttons_layout = block2_children
            .next()
            .expect("widget: Layout should have a buttons layout")
            .children();
        let _reset_button_layout = buttons_layout
            .next()
            .expect("widget: Layout should have a reset button layout");
        let cancel_button_layout = buttons_layout
            .next()
            .expect("widget: Layout should have a cancel button layout for a ColorPicker");
        let submit_button_layout = buttons_layout
            .next()
            .expect("widget: Layout should have a submit button layout for a ColorPicker");
        // ----------- Block 2 end ------------------

        if event::Status::Captured == self.on_event_keyboard(event, shell) {
            self.clear_cache();
            shell.capture_event();
            shell.request_redraw();
            return;
        }

        // Forward events to the hex input and the channel inputs of the
        // active tab. Every TextInput mutates its slot in `state` from its
        // `on_input` closure and pushes a fake message; a non-empty message
        // list means that this input's value changed (submit_button pattern).
        let mut hex_changed = false;
        if let Some(tree_child) = self.tree.children.get_mut(HEX_INPUT_INDEX)
            && let Some(input_layout) = hex_input_layout(hex_layout)
        {
            let mut local_messages = Vec::new();
            {
                let mut local_shell = Shell::new(&mut local_messages);
                self.hex_input.update(
                    tree_child,
                    event,
                    input_layout,
                    cursor,
                    renderer,
                    clipboard,
                    &mut local_shell,
                    &input_layout.bounds(),
                );
                if local_shell.is_event_captured() {
                    shell.capture_event();
                }
                shell.request_redraw_at(local_shell.redraw_request());
                shell.request_input_method(local_shell.input_method());
            }
            hex_changed = !local_messages.is_empty();
        }

        let mut value_changed_indices = Vec::new();
        for i in self.active_tab_channels() {
            if self.tree.children.len() <= VALUE_INPUTS_INDEX + i {
                continue;
            }
            let Some(input_layout) = value_cell_layout(controls_layout, self, i) else {
                continue;
            };
            let mut local_messages = Vec::new();
            {
                let mut local_shell = Shell::new(&mut local_messages);
                self.value_inputs[i].update(
                    &mut self.tree.children[VALUE_INPUTS_INDEX + i],
                    event,
                    input_layout,
                    cursor,
                    renderer,
                    clipboard,
                    &mut local_shell,
                    &input_layout.bounds(),
                );
                if local_shell.is_event_captured() {
                    shell.capture_event();
                }
                shell.request_redraw_at(local_shell.redraw_request());
                shell.request_input_method(local_shell.input_method());
            }
            if !local_messages.is_empty() {
                value_changed_indices.push(i);
            }
        }

        let mut captured = hex_changed || !value_changed_indices.is_empty();
        let mut color_changed = false;
        if hex_changed {
            color_changed |= self.on_hex_input();
        }
        for i in value_changed_indices {
            color_changed |= self.on_value_input(i);
        }

        if color_changed
            && let Some(on_color_change) = self.on_color_change
        {
            shell.publish(on_color_change(self.state.color));
        }

        if hex_changed || self.state.hex_focused || self.state.value_focus.is_some() {
            match event {
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                | Event::Touch(touch::Event::FingerPressed { .. }) => {
                    // Clicking outside all TextInputs blurs them.
                    let over_hex = hex_input_layout(hex_layout)
                        .is_some_and(|l| cursor.position_in(l.bounds()).is_some());
                    let over_value = self
                        .active_tab_channels()
                        .iter()
                        .any(|i| {
                            value_cell_layout(controls_layout, self, *i)
                                .is_some_and(|l| cursor.position_in(l.bounds()).is_some())
                        });
                    if !over_hex && !over_value {
                        self.unfocus_all_text_inputs();
                        self.state.hex_focused = false;
                        self.state.value_focus = None;
                    } else if over_hex {
                        self.state.focus = Focus::Hex;
                    } else if let Some(i) = self
                        .active_tab_channels()
                        .iter()
                        .find(|i| {
                            value_cell_layout(controls_layout, self, **i)
                                .is_some_and(|l| cursor.position_in(l.bounds()).is_some())
                        })
                    {
                        self.state.value_focus = Some(*i);
                        self.state.focus = channel_focus(*i);
                    }
                }
                _ => {}
            }
        }

        if captured {
            self.clear_cache();
            shell.capture_event();
            shell.request_redraw();
            return;
        }

        // Clicking a tab bar shows that tab.
        match event {
            Event::Mouse(
                mouse::Event::CursorMoved { .. }
                | mouse::Event::ButtonPressed(_)
                | mouse::Event::ButtonReleased(_),
            )
            | Event::Touch(touch::Event::FingerMoved { .. }) => {
                let bounds = tab_bar_layout.bounds();
                let gap = 2.0;
                let half = (bounds.width - gap) / 2.0;
                let rgb_tab_bounds = Rectangle {
                    x: bounds.x,
                    y: bounds.y,
                    width: half,
                    height: bounds.height,
                };
                let hsv_tab_bounds = Rectangle {
                    x: bounds.x + half + gap,
                    y: bounds.y,
                    width: half,
                    height: bounds.height,
                };
                self.state.tab_rgb_hovered = cursor.is_over(rgb_tab_bounds);
                self.state.tab_hsv_hovered = cursor.is_over(hsv_tab_bounds);
            }
            _ => {}
        }
        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) = event
        {
            let bounds = tab_bar_layout.bounds();
            let gap = 2.0;
            let half = (bounds.width - gap) / 2.0;
            let rgb_tab_bounds = Rectangle {
                x: bounds.x,
                y: bounds.y,
                width: half,
                height: bounds.height,
            };
            let hsv_tab_bounds = Rectangle {
                x: bounds.x + half + gap,
                y: bounds.y,
                width: half,
                height: bounds.height,
            };
            if cursor.is_over(rgb_tab_bounds) && self.state.active_tab != ActiveTab::Rgb {
                self.set_active_tab(ActiveTab::Rgb, shell);
                captured = true;
            } else if cursor.is_over(hsv_tab_bounds) && self.state.active_tab != ActiveTab::Hsv {
                self.set_active_tab(ActiveTab::Hsv, shell);
                captured = true;
            }
        }

        if event::Status::Captured == self.on_event_hsv_color(event, picker_layout, cursor, shell)
        {
            captured = true;
        }

        if event::Status::Captured == self.on_event_sliders(event, controls_layout, cursor, shell) {
            captured = true;
        }

        if self.on_event_swatches(
            event,
            cursor,
            shell,
            renderer,
            clipboard,
            swatch_tab_bar_layout,
            swatch_page_layout,
            add_btn_layout,
        ) {
            captured = true;
        }

        // Clicking a recent color selects it.
        if matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                | Event::Touch(touch::Event::FingerPressed { .. })
        ) {
            for (i, cell) in recent_grid_layout.children().enumerate() {
                if cursor.is_over(cell.bounds())
                    && let Some(color) = self.state.recent_colors.get(i)
                {
                    self.select_color_from_swatch(*color, shell);
                    captured = true;
                    break;
                }
            }
        }

        // Track the pressed state of the buttons. The draw pass cannot rely on
        // the `Button` widgets' internal status, since overlays are rebuilt
        // fresh for every draw; the state survives in the overlay `State`
        // instead.
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if cursor.is_over(cancel_button_layout.bounds()) {
                    self.state.cancel_pressed = true;
                }
                if cursor.is_over(submit_button_layout.bounds()) {
                    self.state.submit_pressed = true;
                }
                if cursor.is_over(_reset_button_layout.bounds()) {
                    self.state.reset_pressed = true;
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. }) => {
                self.state.cancel_pressed = false;
                self.state.submit_pressed = false;
                // Releasing inside the Reset button resets the color to the
                // initial one (Qt behavior: the dialog stays open).
                if self.state.reset_pressed && cursor.is_over(_reset_button_layout.bounds()) {
                    self.state.reset_pressed = false;
                    self.state.apply_color(self.state.initial_color);
                    self.state.sync_display();
                    self.state.clear_cache();
                    if let Some(on_color_change) = self.on_color_change {
                        shell.publish(on_color_change(self.state.color));
                    }
                }
                self.state.reset_pressed = false;
            }
            _ => {}
        }

        self.cancel_button.update(
            &mut self.tree.children[0],
            event,
            cancel_button_layout,
            cursor,
            renderer,
            clipboard,
            shell,
            &layout.bounds(),
        );

        self.submit_button.update(
            &mut self.tree.children[1],
            event,
            submit_button_layout,
            cursor,
            renderer,
            clipboard,
            &mut Shell::new(&mut fake_messages),
            &layout.bounds(),
        );

        if !fake_messages.is_empty() {
            push_recent(&mut self.state.recent_colors, self.state.color);
            self.state.clear_cache();
            shell.publish((self.on_submit)(self.state.color));
            shell.capture_event();
            shell.request_redraw();
        }

        if captured {
            self.clear_cache();
            shell.capture_event();
            shell.request_redraw();
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let mut children = layout.children();

        let mouse_interaction = mouse::Interaction::default();

        // Block 1 (left pane)
        let block1_layout = children
            .next()
            .expect("Graphics: Layout should have a 1. block layout");
        let mut block1_mouse_interaction = mouse::Interaction::default();
        let mut block1_children = block1_layout.children();

        // Picker: ring + square
        let picker_layout = block1_children
            .next()
            .expect("Graphics: Layout should have a picker layout");
        let mut picker_children = picker_layout.children();
        let square_layout = picker_children
            .next()
            .expect("Graphics: Layout should have a sat/value layout");
        if cursor.is_over(square_layout.bounds()) {
            block1_mouse_interaction = block1_mouse_interaction.max(mouse::Interaction::Pointer);
        }
        let ring_layout = picker_children
            .next()
            .expect("Graphics: Layout should have a hue layout");
        if cursor
            .position_in(ring_layout.bounds())
            .is_some_and(|position| is_in_ring_band(position, ring_layout.bounds().size()))
        {
            block1_mouse_interaction = block1_mouse_interaction.max(mouse::Interaction::Pointer);
        }

        let _tab_bar_layout = block1_children
            .next()
            .expect("Graphics: Layout should have a tab bar layout");

        // Slider rows
        let controls_layout = block1_children
            .next()
            .expect("Graphics: Layout should have a controls layout");
        let mut controls_children = controls_layout.children();

        let f = |layout: Layout<'_>, cursor: Cursor| {
            let mut children = layout.children();

            let _label_layout = children.next();
            let bar_layout = children
                .next()
                .expect("Graphics: Layout should have a bar layout");

            if cursor.is_over(bar_layout.bounds()) {
                mouse::Interaction::ResizingHorizontally
            } else {
                mouse::Interaction::default()
            }
        };
        for _ in 0..4 {
            if let Some(row_layout) = controls_children.next() {
                block1_mouse_interaction =
                    block1_mouse_interaction.max(f(row_layout, cursor));
            }
        }

        // Text inputs: hex at the bottom, channel fields of the active tab.
        let hex_layout = block1_children
            .next()
            .expect("Graphics: Layout should have a hex container layout");
        if let Some(tree_child) = self.tree.children.get(HEX_INPUT_INDEX)
            && let Some(input_layout) = hex_input_layout(hex_layout)
        {
            let hex_interaction = self.hex_input.mouse_interaction(
                tree_child,
                input_layout,
                cursor,
                &input_layout.bounds(),
                renderer,
            );
            block1_mouse_interaction = block1_mouse_interaction.max(hex_interaction);
        }
        for i in self.active_tab_channels() {
            if self.tree.children.len() <= VALUE_INPUTS_INDEX + i {
                continue;
            }
            let Some(input_layout) = value_cell_layout(controls_layout, self, i) else {
                continue;
            };
            let input_interaction = self.value_inputs[i].mouse_interaction(
                &self.tree.children[VALUE_INPUTS_INDEX + i],
                input_layout,
                cursor,
                &input_layout.bounds(),
                renderer,
            );
            block1_mouse_interaction = block1_mouse_interaction.max(input_interaction);
        }

        // Block 2 (right pane)
        let block2_layout = children
            .next()
            .expect("Graphics: Layout should have a 2. block layout");
        let mut block2_mouse_interaction = mouse::Interaction::default();
        let mut block2_children = block2_layout.children();

        // Swatch section: tab bar, grid cells, add button and the name
        // prompt of the open "new swatch set" flow.
        let _preview_layout = block2_children.next();
        let _swatch_label_layout = block2_children.next();
        let swatch_tab_bar_layout = block2_children
            .next()
            .expect("Graphics: Layout should have a swatch tab bar layout");
        let swatch_page_layout = block2_children
            .next()
            .expect("Graphics: Layout should have a swatch tab page layout");
        let add_btn_layout = block2_children
            .next()
            .expect("Graphics: Layout should have an add-swatch button layout");

        if cursor.is_over(swatch_tab_bar_layout.bounds())
            || cursor.is_over(add_btn_layout.bounds())
        {
            block2_mouse_interaction =
                block2_mouse_interaction.max(mouse::Interaction::Pointer);
        }
        for cell in swatch_page_layout.children() {
            if cursor.is_over(cell.bounds()) {
                block2_mouse_interaction =
                    block2_mouse_interaction.max(mouse::Interaction::Pointer);
            }
        }
        if self.state.naming_new_set {
            let (_, add_rect, cancel_rect) = name_prompt_rects(swatch_page_layout.bounds());
            if cursor.is_over(add_rect) || cursor.is_over(cancel_rect) {
                block2_mouse_interaction =
                    block2_mouse_interaction.max(mouse::Interaction::Pointer);
            }
        }

        let _divider_layout = block2_children.next();
        let _recent_label_layout = block2_children.next();
        let recent_grid_layout = block2_children
            .next()
            .expect("Graphics: Layout should have a recent grid layout");
        for cell in recent_grid_layout.children() {
            if cursor.is_over(cell.bounds()) {
                block2_mouse_interaction =
                    block2_mouse_interaction.max(mouse::Interaction::Pointer);
            }
        }

        let mut buttons_layout = block2_children
            .next()
            .expect("Graphics: Layout should have a buttons layout")
            .children();
        let _reset_button_layout = buttons_layout
            .next()
            .expect("Graphics: Layout should have a reset button layout");
        let cancel_button_layout = buttons_layout
            .next()
            .expect("Graphics: Layout should have a cancel button layout for a ColorPicker");
        let cancel_mouse_interaction = self.cancel_button.mouse_interaction(
            &self.tree.children[1],
            cancel_button_layout,
            cursor,
            &self.viewport,
            renderer,
        );

        let submit_button_layout = buttons_layout
            .next()
            .expect("Graphics: Layout should have a submit button layout for a ColorPicker");
        let submit_mouse_interaction = self.submit_button.mouse_interaction(
            &self.tree.children[1],
            submit_button_layout,
            cursor,
            &self.viewport,
            renderer,
        );

        mouse_interaction
            .max(block1_mouse_interaction)
            .max(block2_mouse_interaction)
            .max(cancel_mouse_interaction)
            .max(submit_mouse_interaction)
    }

    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        let mut children = layout.children();

        // Skip block 1 (left pane)
        let _block1_layout = children.next();

        // Block 2 contains the buttons
        if let Some(block2_layout) = children.next() {
            let mut block2_children = block2_layout.children();

            // Skip previews, swatches and recent grids / labels / divider
            let _preview_layout = block2_children.next();
            let _swatch_label_layout = block2_children.next();
            let _tab_bar_layout = block2_children.next();
            let _tab_page_layout = block2_children.next();
            let _add_btn_layout = block2_children.next();
            let _divider_layout = block2_children.next();
            let _recent_label_layout = block2_children.next();
            let _recent_grid_layout = block2_children.next();

            // Operate on the buttons row
            if let Some(buttons_layout) = block2_children.next() {
                let mut button_children = buttons_layout.children();
                let _reset_layout = button_children.next();

                if let Some(cancel_layout) = button_children.next() {
                    Widget::operate(
                        &mut self.cancel_button,
                        &mut self.tree.children[0],
                        cancel_layout,
                        renderer,
                        operation,
                    );
                }

                if let Some(submit_layout) = button_children.next() {
                    Widget::operate(
                        &mut self.submit_button,
                        &mut self.tree.children[1],
                        submit_layout,
                        renderer,
                        operation,
                    );
                }
            }
        }
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
    ) {
        let bounds = layout.bounds();
        let mut children = layout.children();

        let mut style_sheet: HashMap<StyleState, Style> = HashMap::new();
        let _ = style_sheet.insert(
            StyleState::Active,
            style::Catalog::style(theme, self.class, Status::Active),
        );
        let _ = style_sheet.insert(
            StyleState::Selected,
            style::Catalog::style(theme, self.class, Status::Selected),
        );
        let _ = style_sheet.insert(
            StyleState::Hovered,
            style::Catalog::style(theme, self.class, Status::Hovered),
        );
        let _ = style_sheet.insert(
            StyleState::Focused,
            style::Catalog::style(theme, self.class, Status::Focused),
        );

        let mut style_state = StyleState::Active;
        if self.state.focus == Focus::Overlay {
            style_state = style_state.max(StyleState::Focused);
        }
        if cursor.is_over(bounds) {
            style_state = style_state.max(StyleState::Hovered);
        }

        if (bounds.width > 0.) && (bounds.height > 0.) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: Border {
                        radius: style_sheet[&style_state].border_radius.into(),
                        width: style_sheet[&style_state].border_width,
                        color: style_sheet[&style_state].border_color,
                    },
                    ..renderer::Quad::default()
                },
                style_sheet[&style_state].background,
            );
        }

        // ----------- Block 1 ----------------------
        let block1_layout = children
            .next()
            .expect("Graphics: Layout should have a 1. block layout");
        block1(
            renderer,
            self,
            block1_layout,
            cursor,
            theme,
            style,
            &style_sheet,
        );

        // ----------- Block 2 ----------------------
        let block2_layout = children
            .next()
            .expect("Graphics: Layout should have a 2. block layout");
        block2(
            renderer,
            self,
            block2_layout,
            cursor,
            theme,
            style,
            &style_sheet,
        );
    }
}

/// Defines the layout of the left pane: picker (ring + sat/value square),
/// tab bar, slider controls column and the hex container.
fn left_pane_layout<'a, Message, Theme>(
    color_picker: &mut ColorPickerOverlay<'_, '_, Message, Theme>,
    renderer: &Renderer,
    bounds: Rectangle,
) -> Node
where
    Message: 'static + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    // ---- Picker container: 300x300, containing the 149x149 sat/value
    // square (centered, child 0) and the full-size hue ring (child 1).
    let picker_limits = Limits::new(Size::ZERO, Size::new(RING_DIM, RING_DIM))
        .width(Length::Fixed(RING_DIM))
        .height(Length::Fixed(RING_DIM));

    let square_limits = Limits::new(Size::ZERO, Size::new(SQUARE_DIM, SQUARE_DIM));
    let mut square_node = Row::<(), Theme, Renderer>::new()
        .width(Length::Fixed(SQUARE_DIM))
        .height(Length::Fixed(SQUARE_DIM))
        .layout(color_picker.tree, renderer, &square_limits);
    square_node = square_node.move_to(Point::new(
        (RING_DIM - SQUARE_DIM) / 2.0,
        (RING_DIM - SQUARE_DIM) / 2.0,
    ));

    let ring_node = Row::<(), Theme, Renderer>::new()
        .width(Length::Fixed(RING_DIM))
        .height(Length::Fixed(RING_DIM))
        .layout(color_picker.tree, renderer, &picker_limits);

    let picker_node = Node::with_children(
        Size::new(RING_DIM, RING_DIM),
        vec![square_node, ring_node],
    );

    // ---- Tab bar placeholder.
    let tab_bar_limits = Limits::new(Size::ZERO, Size::new(RING_DIM, TAB_BAR_HEIGHT))
        .width(Length::Fill)
        .height(Length::Fixed(TAB_BAR_HEIGHT));
    let tab_bar_node = Row::<(), Theme, Renderer>::new()
        .width(Length::Fill)
        .height(Length::Fixed(TAB_BAR_HEIGHT))
        .layout(color_picker.tree, renderer, &tab_bar_limits);

    // ---- Slider controls column: 4 rows x (label, groove, value).
    let controls_height = 4.0 * SLIDER_HEIGHT + 3.0 * ROW_SPACING;
    let groove_width = RING_DIM - LABEL_WIDTH - VALUE_WIDTH;

    let mut controls_children = Vec::new();
    for row in 0..4 {
        let y = row as f32 * (SLIDER_HEIGHT + ROW_SPACING);

        let label_node = Node::with_children(
            Size::new(LABEL_WIDTH, SLIDER_HEIGHT),
            Vec::new(),
        )
        .move_to(Point::new(0.0, y));
        let groove_node = Node::with_children(
            Size::new(groove_width, SLIDER_HEIGHT),
            Vec::new(),
        )
        .move_to(Point::new(LABEL_WIDTH, y));

        // The value cells host the channel [`TextInput`]s of the active tab;
        // the readonly alpha cell of the HSV tab is a plain text cell.
        let value_input_index = match (color_picker.state.active_tab, row) {
            (ActiveTab::Rgb, i) => Some(i),
            (ActiveTab::Hsv, 3) => None,
            (ActiveTab::Hsv, i) => Some(4 + i),
        };
        let value_child = if let Some(value_input_index) = value_input_index {
            let input_tree = if let Some(child_tree) = color_picker.tree.children.get_mut(VALUE_INPUTS_INDEX + value_input_index) {
                child_tree.diff(&mut color_picker.value_inputs[value_input_index]
                    as &mut dyn Widget<Message, Theme, Renderer>);
                child_tree
            } else {
                let child_tree = Tree::new(&color_picker.value_inputs[value_input_index]
                    as &dyn Widget<Message, Theme, Renderer>);
                color_picker.tree.children.push(child_tree);
                color_picker.tree.children.last_mut().unwrap()
            };
            color_picker.value_inputs[value_input_index]
                .layout(
                    input_tree,
                    renderer,
                    &Limits::new(
                        Size::ZERO,
                        Size::new(VALUE_WIDTH, SLIDER_HEIGHT),
                    ),
                    Some(&text_input::Value::new(&color_picker.state.value_inputs[value_input_index]))
                )
                .move_to(Point::new(LABEL_WIDTH + groove_width, y))
        } else {
            Node::with_children(
                Size::new(VALUE_WIDTH, SLIDER_HEIGHT),
                Vec::new(),
            )
            .move_to(Point::new(LABEL_WIDTH + groove_width, y))
        };

        controls_children.push(Node::with_children(
            Size::new(RING_DIM, SLIDER_HEIGHT),
            vec![label_node, groove_node, value_child],
        ));
    }
    let controls_node = Node::with_children(
        Size::new(RING_DIM, controls_height),
        controls_children,
    );

    // ---- Hex container: "Hex:" label + hex TextInput.
    let hex_input_tree = if let Some(child_tree) = color_picker.tree.children.get_mut(HEX_INPUT_INDEX) {
        child_tree.diff(&mut color_picker.hex_input as &mut dyn Widget<Message, Theme, Renderer>);
        child_tree
    } else {
        let child_tree = Tree::new(&color_picker.hex_input as &dyn Widget<Message, Theme, Renderer>);
        color_picker.tree.children.push(child_tree);
        color_picker.tree.children.last_mut().unwrap()
    };
    let mut hex_input_node = color_picker
        .hex_input
        .layout(
            hex_input_tree,
            renderer,
            &Limits::new(
                Size::ZERO,
                Size::new(RING_DIM - 2.0 * PANE_SPACING, HEX_CONTAINER_HEIGHT - 2.0 * ROW_SPACING),
            ),
            Some(&text_input::Value::new(&color_picker.state.hex_input)),
        );

    let hex_label_node = Node::with_children(
        Size::new(32.0, HEX_CONTAINER_HEIGHT),
        Vec::new(),
    )
    .move_to(Point::new(0.0, 0.0));
    hex_input_node = hex_input_node.move_to(Point::new(32.0, 0.0));
    let hex_node = Node::with_children(
        Size::new(RING_DIM, HEX_CONTAINER_HEIGHT),
        vec![hex_label_node, hex_input_node],
    );

    // ---- Stack the left pane children vertically.
    let spacing = CONTROLS_SPACING;
    let mut offset_y = 0.0;

    let picker_node = picker_node.move_to(Point::new(0.0, offset_y));
    offset_y += picker_node.size().height + spacing;

    let tab_bar_node = tab_bar_node.move_to(Point::new(0.0, offset_y));
    offset_y += tab_bar_node.size().height + spacing;

    let controls_node = controls_node.move_to(Point::new(0.0, offset_y));
    offset_y += controls_node.size().height + spacing;

    let hex_node = hex_node.move_to(Point::new(0.0, offset_y));
    offset_y += hex_node.size().height;

    let left_pane = Node::with_children(
        Size::new(RING_DIM, offset_y),
        vec![picker_node, tab_bar_node, controls_node, hex_node],
    );

    left_pane.move_to(Point::new(bounds.x, bounds.y))
}

/// Height of the hex container.
const HEX_CONTAINER_HEIGHT: f32 = 44.0;
/// Height of the preview area (panels + labels) in the right pane.
const PREVIEW_AREA_HEIGHT: f32 = PREVIEW_HEIGHT + 18.0 + 2.0;
/// Height of the "new swatch set" name prompt row.
const NAME_PROMPT_HEIGHT: f32 = 32.0;
/// Margin of the swatch grids inside the tab page.
const SWATCH_PAGE_MARGIN: f32 = 5.0;
/// Height of the section headings ("Swatches", "Recent").
const LABEL_HEIGHT: f32 = 18.0;
/// Height of the divider.
const DIVIDER_HEIGHT: f32 = 2.0;
/// Height of the buttons row in the right pane.
const BUTTONS_HEIGHT: f32 = 32.0;
/// Width of the Reset button.
const RESET_WIDTH: f32 = 64.0;
/// Spacing between the right pane children.
const RIGHT_PANE_SPACING: f32 = 10.0;

/// Defines the layout of the right pane: previews, swatches, recent colors
/// and the Reset/OK/Cancel buttons.
fn right_pane_layout<'a, Message, Theme>(
    color_picker: &mut ColorPickerOverlay<'_, '_, Message, Theme>,
    renderer: &Renderer,
    bounds: Rectangle,
) -> Node
where
    Message: 'static + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    let spacing = RIGHT_PANE_SPACING;
    let width = RIGHT_PANE_WIDTH;

    let mut offset_y = 0.0;
    let mut children: Vec<Node> = Vec::new();

    // [0] Preview area (panels + labels), drawn manually by `block2`.
    let preview_node = Node::with_children(
        Size::new(width, PREVIEW_AREA_HEIGHT),
        Vec::new(),
    )
    .move_to(Point::new(0.0, offset_y));
    children.push(preview_node);
    offset_y += PREVIEW_AREA_HEIGHT + spacing;

    // [1] "Swatches" heading.
    let label_node = Row::<(), Theme, Renderer>::new()
        .width(Length::Fill)
        .height(Length::Fixed(LABEL_HEIGHT))
        .layout(
            color_picker.tree,
            renderer,
            &Limits::new(Size::ZERO, Size::new(width, LABEL_HEIGHT)),
        )
        .move_to(Point::new(0.0, offset_y));
    children.push(label_node);
    offset_y += LABEL_HEIGHT + spacing;

    // [2] Swatch tab bar.
    let tab_bar_node = Row::<(), Theme, Renderer>::new()
        .width(Length::Fill)
        .height(Length::Fixed(TAB_BAR_HEIGHT))
        .layout(
            color_picker.tree,
            renderer,
            &Limits::new(Size::ZERO, Size::new(width, TAB_BAR_HEIGHT)),
        )
        .move_to(Point::new(0.0, offset_y));
    children.push(tab_bar_node);
    offset_y += TAB_BAR_HEIGHT + spacing;

    // [3] Swatch tab page: either the active set's grid or the "new swatch
    // set" name prompt.
    let page_height = if color_picker.state.naming_new_set {
        NAME_PROMPT_HEIGHT
    } else {
        let count = color_picker
            .state
            .swatch_sets
            .get(color_picker.state.active_swatch_tab)
            .map_or(0, |set| set.colors.len());
        let rows = grid_rows(count, GRID_COLS);
        if rows == 0 {
            0.0
        } else {
            rows as f32 * SWATCH_SIZE + (rows - 1) as f32 * GRID_SPACING
                + 2.0 * SWATCH_PAGE_MARGIN
        }
    };

    let mut page_children: Vec<Node> = Vec::new();
    if color_picker.state.naming_new_set {
        let (input_rect, _, _) = name_prompt_rects(Rectangle {
            x: 0.0,
            y: 0.0,
            width,
            height: page_height,
        });
        let name_tree =
            if let Some(child_tree) = color_picker.tree.children.get_mut(NEW_SET_NAME_INDEX) {
                child_tree.diff(
                    &mut color_picker.new_set_name_input
                        as &mut dyn Widget<Message, Theme, Renderer>,
                );
                child_tree
            } else {
                let child_tree = Tree::new(
                    &color_picker.new_set_name_input as &dyn Widget<Message, Theme, Renderer>,
                );
                color_picker.tree.children.push(child_tree);
                color_picker.tree.children.last_mut().unwrap()
            };
        let input_node = color_picker
            .new_set_name_input
            .layout(
                name_tree,
                renderer,
                &Limits::new(Size::ZERO, input_rect.size()),
                Some(&text_input::Value::new(&color_picker.state.pending_swatch_name)),
            )
            .move_to(Point::new(input_rect.x, input_rect.y));
        page_children.push(input_node);
    } else if let Some(set) = color_picker.state.swatch_sets.get(color_picker.state.active_swatch_tab) {
        for (i, _) in set.colors.iter().enumerate() {
            let row = i / GRID_COLS;
            let col = i % GRID_COLS;
            let cell = Node::with_children(Size::new(SWATCH_SIZE, SWATCH_SIZE), Vec::new())
                .move_to(Point::new(
                    SWATCH_PAGE_MARGIN + col as f32 * (SWATCH_SIZE + GRID_SPACING),
                    SWATCH_PAGE_MARGIN + row as f32 * (SWATCH_SIZE + GRID_SPACING),
                ));
            page_children.push(cell);
        }
    }

    let tab_page_node = Node::with_children(Size::new(width, page_height), page_children)
        .move_to(Point::new(0.0, offset_y));
    children.push(tab_page_node);
    offset_y += page_height + spacing;

    // [4] Add-swatch button.
    let add_btn_node = Row::<(), Theme, Renderer>::new()
        .width(Length::Fixed(ADD_BUTTON_SIZE))
        .height(Length::Fixed(ADD_BUTTON_SIZE))
        .layout(
            color_picker.tree,
            renderer,
            &Limits::new(
                Size::ZERO,
                Size::new(ADD_BUTTON_SIZE, ADD_BUTTON_SIZE),
            ),
        )
        .move_to(Point::new((width - ADD_BUTTON_SIZE) / 2.0, offset_y));
    children.push(add_btn_node);
    offset_y += ADD_BUTTON_SIZE + spacing;

    // [5] Divider.
    let divider_node = Row::<(), Theme, Renderer>::new()
        .width(Length::Fill)
        .height(Length::Fixed(DIVIDER_HEIGHT))
        .layout(
            color_picker.tree,
            renderer,
            &Limits::new(Size::ZERO, Size::new(width, DIVIDER_HEIGHT)),
        )
        .move_to(Point::new(0.0, offset_y));
    children.push(divider_node);
    offset_y += DIVIDER_HEIGHT + spacing;

    // [6] "Recent" heading.
    let recent_label_node = Row::<(), Theme, Renderer>::new()
        .width(Length::Fill)
        .height(Length::Fixed(LABEL_HEIGHT))
        .layout(
            color_picker.tree,
            renderer,
            &Limits::new(Size::ZERO, Size::new(width, LABEL_HEIGHT)),
        )
        .move_to(Point::new(0.0, offset_y));
    children.push(recent_label_node);
    offset_y += LABEL_HEIGHT + spacing;

    // [7] Recent grid: `GRID_COLS` columns, dynamic height from the count.
    let recent_count = color_picker.state.recent_colors.len();
    let recent_rows = grid_rows(recent_count, GRID_COLS);
    let recent_grid_height = if recent_rows == 0 {
        5.0
    } else {
        recent_rows as f32 * SWATCH_SIZE + (recent_rows - 1) as f32 * GRID_SPACING
            + 5.0
    };
    let mut recent_children: Vec<Node> = Vec::new();
    for (i, _) in color_picker.state.recent_colors.iter().enumerate() {
        let row = i / GRID_COLS;
        let col = i % GRID_COLS;
        let cell = Node::with_children(Size::new(SWATCH_SIZE, SWATCH_SIZE), Vec::new())
            .move_to(Point::new(
                SWATCH_PAGE_MARGIN + col as f32 * (SWATCH_SIZE + GRID_SPACING),
                SWATCH_PAGE_MARGIN + row as f32 * (SWATCH_SIZE + GRID_SPACING),
            ));
        recent_children.push(cell);
    }
    let recent_grid_node = Node::with_children(
        Size::new(width, recent_grid_height),
        recent_children,
    )
    .move_to(Point::new(0.0, offset_y));
    children.push(recent_grid_node);
    offset_y += recent_grid_height + spacing;

    // [8] Buttons row: Reset (left) + stretch + Cancel + OK.
    let reset_node = Row::<(), Theme, Renderer>::new()
        .width(Length::Fixed(RESET_WIDTH))
        .height(Length::Fixed(BUTTONS_HEIGHT))
        .layout(
            color_picker.tree,
            renderer,
            &Limits::new(Size::ZERO, Size::new(RESET_WIDTH, BUTTONS_HEIGHT)),
        )
        .move_to(Point::new(0.0, offset_y));

    let available = width - RESET_WIDTH - 2.0 * 5.0;
    let button_width = available / 2.0;

    let cancel_button = color_picker
        .cancel_button
        .layout(
            &mut color_picker.tree.children[0],
            renderer,
            &Limits::new(Size::ZERO, Size::new(button_width, BUTTONS_HEIGHT)),
        )
        .move_to(Point::new(RESET_WIDTH + 5.0, offset_y));

    let submit_button = color_picker
        .submit_button
        .layout(
            &mut color_picker.tree.children[1],
            renderer,
            &Limits::new(Size::ZERO, Size::new(button_width, BUTTONS_HEIGHT)),
        )
        .move_to(Point::new(RESET_WIDTH + 5.0 + button_width + 5.0, offset_y));

    let buttons_row = Node::with_children(Size::new(width, BUTTONS_HEIGHT), vec![
            reset_node,
            cancel_button,
            submit_button,
        ]);
    children.push(buttons_row);
    offset_y += BUTTONS_HEIGHT;

    let right_pane = Node::with_children(
        Size::new(width, offset_y),
        children,
    );

    right_pane.move_to(Point::new(bounds.x, bounds.y))
}

/// Draws the left pane: picker (ring + sat/value square), tab bar
/// placeholder, slider controls and the hex container.
#[allow(clippy::too_many_arguments)]
fn block1<Message, Theme>(
    renderer: &mut Renderer,
    color_picker: &ColorPickerOverlay<'_, '_, Message, Theme>,
    layout: Layout<'_>,
    cursor: Cursor,
    theme: &Theme,
    style: &renderer::Style,
    style_sheet: &HashMap<StyleState, Style>,
) where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    // ----------- Block 1 ----------------------
    let mut block1_children = layout.children();

    // [0] Picker: hue ring + sat/value square.
    let picker_layout = block1_children
        .next()
        .expect("Graphics: Layout should have a picker layout");
    hsv_color(
        renderer,
        color_picker,
        picker_layout,
        cursor,
        style_sheet,
    );

    // [1] Tab bar.
    let tab_bar_layout = block1_children
        .next()
        .expect("Graphics: Layout should have a tab bar layout");
    tab_bar_placeholder(
        renderer,
        color_picker,
        tab_bar_layout,
        cursor,
        style_sheet,
    );

    // [2] Controls: gradient slider rows of the active tab.
    let controls_layout = block1_children
        .next()
        .expect("Graphics: Layout should have a controls layout");
    slider_rows(
        renderer,
        color_picker,
        controls_layout,
        cursor,
        theme,
        style,
        style_sheet,
        color_picker.state.focus,
    );

    // [3] Hex container.
    let hex_layout = block1_children
        .next()
        .expect("Graphics: Layout should have a hex container layout");
    hex_input(
        renderer,
        theme,
        color_picker,
        hex_layout,
        cursor,
        style_sheet,
    );

    // ----------- Block 1 end ------------------
}

/// Draws a placeholder for the RGB(A)/HSV tab bar.
fn tab_bar_placeholder<Message, Theme>(
    renderer: &mut Renderer,
    color_picker: &ColorPickerOverlay<'_, '_, Message, Theme>,
    layout: Layout<'_>,
    cursor: Cursor,
    style_sheet: &HashMap<StyleState, Style>,
) where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    let active_style = style_sheet[&StyleState::Active];
    let bounds = layout.bounds();
    let gap = 2.0;
    let half = (bounds.width - gap) / 2.0;

    let tabs = [("RGB(A)", ActiveTab::Rgb, 0.0), ("HSV", ActiveTab::Hsv, half + gap)];

    for (label, tab, x) in tabs {
        let tab_bounds = Rectangle {
            x: bounds.x + x,
            y: bounds.y,
            width: half,
            height: bounds.height,
        };

        let background = if color_picker.state.active_tab == tab {
            style_sheet[&StyleState::Selected].tab_selected_background
        } else if cursor.is_over(tab_bounds)
            || matches!(
                (tab, color_picker.state.tab_rgb_hovered, color_picker.state.tab_hsv_hovered),
                (ActiveTab::Rgb, true, _) | (ActiveTab::Hsv, _, true)
            )
        {
            active_style.tab_hover_background
        } else {
            active_style.tab_background
        };
        let text_color = if color_picker.state.active_tab == tab {
            active_style.text_primary
        } else {
            active_style.text_secondary
        };

        renderer.fill_quad(
            renderer::Quad {
                bounds: tab_bounds,
                border: Border {
                    radius: Radius::default().top(5.0),
                    width: 1.0,
                    color: active_style.tab_border_color,
                },
                ..renderer::Quad::default()
            },
            background,
        );

        renderer.fill_text(
            Text {
                content: label.to_owned(),
                bounds: Size::new(tab_bounds.width, tab_bounds.height),
                size: renderer.default_size(),
                font: renderer.default_font(),
                align_x: text::Alignment::Center,
                align_y: Vertical::Center,
                line_height: text::LineHeight::Relative(1.3),
                shaping: text::Shaping::Basic,
                wrapping: text::Wrapping::None,
            },
            Point::new(tab_bounds.center_x(), tab_bounds.center_y()),
            text_color,
            tab_bounds,
        );
    }
}

/// Draws the right pane: previews, swatch tab bar + page + add button,
/// recent heading (grid drawn in a later feature) and the buttons.
#[allow(clippy::too_many_arguments)]
fn block2<Message, Theme>(
    renderer: &mut Renderer,
    color_picker: &ColorPickerOverlay<'_, '_, Message, Theme>,
    layout: Layout<'_>,
    cursor: Cursor,
    theme: &Theme,
    _style: &renderer::Style,
    style_sheet: &HashMap<StyleState, Style>,
) where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    // ----------- Block 2 ----------------------
    let mut block2_children = layout.children();

    // [0] Preview area (placeholder rects, no checkerboard yet).
    let preview_layout = block2_children
        .next()
        .expect("Graphics: Layout should have a preview layout");
    preview_placeholder(
        renderer,
        color_picker,
        preview_layout,
        cursor,
        style_sheet,
    );

    let active_style = style_sheet[&StyleState::Active];

    // [1] "Swatches" heading.
    let swatch_label_layout = block2_children
        .next()
        .expect("Graphics: Layout should have a swatch label layout");
    draw_section_heading(
        renderer,
        swatch_label_layout,
        "Swatches",
        active_style.text_secondary,
    );

    // [2] Swatch tab bar.
    let swatch_tab_bar_layout = block2_children
        .next()
        .expect("Graphics: Layout should have a swatch tab bar layout");
    swatch_tab_bar(
        renderer,
        color_picker,
        swatch_tab_bar_layout,
        cursor,
        style_sheet,
    );

    // [3] Tab page: the active set's grid or the name prompt.
    let tab_page_layout = block2_children
        .next()
        .expect("Graphics: Layout should have a swatch tab page layout");
    swatch_page(
        renderer,
        theme,
        color_picker,
        tab_page_layout,
        cursor,
        style_sheet,
    );

    // [4] Add-current-color button.
    let add_btn_layout = block2_children
        .next()
        .expect("Graphics: Layout should have an add-swatch button layout");
    draw_add_button(renderer, add_btn_layout, cursor, style_sheet);

    // [5] Divider.
    let divider_layout = block2_children
        .next()
        .expect("Graphics: Layout should have a divider layout");
    {
        let bounds = divider_layout.bounds();
        if (bounds.width > 0.) && (bounds.height > 0.) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    ..renderer::Quad::default()
                },
                active_style.panel_border_color,
            );
        }
    }

    // [6] "Recent" heading.
    let recent_label_layout = block2_children
        .next()
        .expect("Graphics: Layout should have a recent label layout");
    draw_section_heading(
        renderer,
        recent_label_layout,
        "Recent",
        active_style.text_secondary,
    );

    let recent_grid_layout = block2_children
        .next()
        .expect("Graphics: Layout should have a recent grid layout");
    draw_recent_grid(
        renderer,
        color_picker,
        recent_grid_layout,
        cursor,
        style_sheet,
    );

    // [8] Buttons: Reset + Cancel + OK.
    let mut buttons_layout = block2_children
        .next()
        .expect("Graphics: Layout should have a buttons layout")
        .children();

    let reset_button_layout = buttons_layout
        .next()
        .expect("Graphics: Layout should have a reset button layout");
    draw_reset_button(
        renderer,
        reset_button_layout.bounds(),
        color_picker.state.reset_pressed,
        cursor,
        style_sheet,
    );

    let cancel_button_layout = buttons_layout
        .next()
        .expect("Graphics: Layout should have a cancel button layout for a ColorPicker");

    draw_overlay_button(
        renderer,
        theme,
        cancel_icon().0,
        cancel_button_layout.bounds(),
        color_picker.state.cancel_pressed,
        cursor,
    );

    let submit_button_layout = buttons_layout
        .next()
        .expect("Graphics: Layout should have a submit button layout for a ColorPicker");

    draw_overlay_button(
        renderer,
        theme,
        ok_icon().0,
        submit_button_layout.bounds(),
        color_picker.state.submit_pressed,
        cursor,
    );

    // Focus borders for the buttons.
    draw_focus_border(
        renderer,
        color_picker,
        reset_button_layout.bounds(),
        Focus::Reset,
        style_sheet,
    );
    draw_focus_border(
        renderer,
        color_picker,
        cancel_button_layout.bounds(),
        Focus::Cancel,
        style_sheet,
    );
    draw_focus_border(
        renderer,
        color_picker,
        submit_button_layout.bounds(),
        Focus::Submit,
        style_sheet,
    );

    // ----------- Block 2 end ------------------
}

/// Draws a right-pane section heading ("Swatches", "Recent").
fn draw_section_heading(
    renderer: &mut Renderer,
    layout: Layout<'_>,
    label: &str,
    color: Color,
) {
    let bounds = layout.bounds();
    renderer.fill_text(
        Text {
            content: label.to_owned(),
            bounds: Size::new(bounds.width, bounds.height),
            size: Pixels(13.0),
            font: renderer.default_font(),
            align_x: text::Alignment::Left,
            align_y: Vertical::Center,
            line_height: text::LineHeight::Relative(1.3),
            shaping: text::Shaping::Basic,
            wrapping: text::Wrapping::None,
        },
        Point::new(bounds.x, bounds.center_y()),
        color,
        bounds,
    );
}

/// Draws the swatch tab bar: one tab per set (active: selected background,
/// hover: hover background; close "x" mark when another set remains) plus
/// the trailing "+" tab that opens the name prompt.
fn swatch_tab_bar<Message, Theme>(
    renderer: &mut Renderer,
    color_picker: &ColorPickerOverlay<'_, '_, Message, Theme>,
    layout: Layout<'_>,
    cursor: Cursor,
    style_sheet: &HashMap<StyleState, Style>,
) where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    let active_style = style_sheet[&StyleState::Active];
    let bar = layout.bounds();
    let (tabs, plus) = swatch_tab_bounds(bar, &color_picker.state.swatch_sets);
    let closable = color_picker.state.swatch_sets.len() > 1;

    for (i, tab) in tabs.iter().enumerate() {
        let selected = color_picker.state.active_swatch_tab == i;
        let background = if selected {
            style_sheet[&StyleState::Selected].tab_selected_background
        } else if cursor.is_over(*tab) {
            active_style.tab_hover_background
        } else {
            active_style.tab_background
        };
        let text_color = if selected {
            active_style.text_primary
        } else {
            active_style.text_secondary
        };

        renderer.fill_quad(
            renderer::Quad {
                bounds: *tab,
                border: Border {
                    radius: Radius::default().top(5.0),
                    width: 1.0,
                    color: active_style.tab_border_color,
                },
                ..renderer::Quad::default()
            },
            background,
        );

        // The name, left-aligned in the area before the close mark.
        let mut name_bounds = *tab;
        if closable {
            name_bounds.width -= 12.0;
        }
        renderer.fill_text(
            Text {
                content: color_picker.state.swatch_sets[i].name.clone(),
                bounds: Size::new(name_bounds.width, name_bounds.height),
                size: renderer.default_size(),
                font: renderer.default_font(),
                align_x: text::Alignment::Center,
                align_y: Vertical::Center,
                line_height: text::LineHeight::Relative(1.3),
                shaping: text::Shaping::Basic,
                wrapping: text::Wrapping::None,
            },
            Point::new(name_bounds.center_x(), name_bounds.center_y()),
            text_color,
            name_bounds,
        );

        // The close ("x") mark.
        if closable {
            let close_bounds = swatch_close_bounds(tab);
            renderer.fill_text(
                Text {
                    content: "x".to_owned(),
                    bounds: Size::new(close_bounds.width, close_bounds.height),
                    size: Pixels(11.0),
                    font: renderer.default_font(),
                    align_x: text::Alignment::Center,
                    align_y: Vertical::Center,
                    line_height: text::LineHeight::Relative(1.3),
                    shaping: text::Shaping::Basic,
                    wrapping: text::Wrapping::None,
                },
                Point::new(close_bounds.center_x(), close_bounds.center_y()),
                if cursor.is_over(close_bounds) {
                    active_style.text_primary
                } else {
                    active_style.text_secondary
                },
                close_bounds,
            );
        }
    }

    // The "+" tab.
    renderer.fill_quad(
        renderer::Quad {
            bounds: plus,
            border: Border {
                radius: Radius::default().top(5.0),
                width: 1.0,
                color: active_style.tab_border_color,
            },
            ..renderer::Quad::default()
        },
        if color_picker.state.plus_tab_hovered {
            active_style.tab_hover_background
        } else {
            active_style.tab_background
        },
    );
    renderer.fill_text(
        Text {
            content: "+".to_owned(),
            bounds: Size::new(plus.width, plus.height),
            size: renderer.default_size(),
            font: renderer.default_font(),
            align_x: text::Alignment::Center,
            align_y: Vertical::Center,
            line_height: text::LineHeight::Relative(1.3),
            shaping: text::Shaping::Basic,
            wrapping: text::Wrapping::None,
        },
        Point::new(plus.center_x(), plus.center_y()),
        active_style.text_secondary,
        plus,
    );
}

/// Draws the swatch tab page: the active set's grid (cached checkerboard +
/// color fill + hover/focus border per cell), or the "new swatch set" name
/// prompt when it is open.
fn swatch_page<Message, Theme>(
    renderer: &mut Renderer,
    theme: &Theme,
    color_picker: &ColorPickerOverlay<'_, '_, Message, Theme>,
    layout: Layout<'_>,
    cursor: Cursor,
    style_sheet: &HashMap<StyleState, Style>,
) where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    if color_picker.state.naming_new_set {
        draw_name_prompt(renderer, theme, color_picker, layout, cursor, style_sheet);
        return;
    }

    let active_style = style_sheet[&StyleState::Active];
    let Some(set) = color_picker
        .state
        .swatch_sets
        .get(color_picker.state.active_swatch_tab)
    else {
        return;
    };

    let checker_1 = active_style.checker_color_1;
    let checker_2 = active_style.checker_color_2;
    let tile = 10.0;

    for (i, cell_layout) in layout.children().enumerate() {
        let cell = cell_layout.bounds();
        let Some(color) = set.colors.get(i) else {
            continue;
        };

        // Checkerboard behind the color; tiles are drawn as solid quads so
        // they stack below the color fill (the renderer batches quads and
        // meshes separately, and a mesh would always draw on top of a quad).
        draw_checkerboard(renderer, cell, tile, checker_1, checker_2);

        // Color fill on top (alpha-composited over the checkerboard).
        if (cell.width > 0.) && (cell.height > 0.) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: cell,
                    ..renderer::Quad::default()
                },
                *color,
            );
        }

        // Border: hover / keyboard focus highlight.
        let focused = color_picker.state.focused_swatch
            == Some((color_picker.state.active_swatch_tab, i));
        let border_color = if cursor.is_over(cell) || focused {
            active_style.swatch_hover_border_color
        } else {
            active_style.swatch_border_color
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds: cell,
                border: Border {
                    radius: 2.0.into(),
                    width: 1.0,
                    color: border_color,
                },
                ..renderer::Quad::default()
            },
            Color::TRANSPARENT,
        );
    }
}

/// Draws the "new swatch set" prompt: the name [`TextInput`] (tree child
/// [`NEW_SET_NAME_INDEX`]) plus the Add / Cancel manual buttons.
fn draw_name_prompt<Message, Theme>(
    renderer: &mut Renderer,
    theme: &Theme,
    color_picker: &ColorPickerOverlay<'_, '_, Message, Theme>,
    layout: Layout<'_>,
    cursor: Cursor,
    style_sheet: &HashMap<StyleState, Style>,
) where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    let page = layout.bounds();
    let (_, add_rect, cancel_rect) = name_prompt_rects(page);

    if let Some(input_layout) = layout.children().next()
        && let Some(tree_child) = color_picker.tree.children.get(NEW_SET_NAME_INDEX)
        && input_layout.children().next().is_some()
    {
        color_picker.new_set_name_input.draw(
            tree_child,
            renderer,
            theme,
            input_layout,
            cursor,
            Some(&text_input::Value::new(&color_picker.state.pending_swatch_name)),
            &input_layout.bounds(),
        );
    }

    draw_small_button(renderer, "Add", add_rect, cursor, style_sheet, true);
    draw_small_button(renderer, "Cancel", cancel_rect, cursor, style_sheet, false);
}

/// Draws a small button of the name prompt. `primary` buttons use the
/// selected-tab background, secondary ones the plain tab background.
fn draw_small_button(
    renderer: &mut Renderer,
    label: &str,
    bounds: Rectangle,
    cursor: Cursor,
    style_sheet: &HashMap<StyleState, Style>,
    primary: bool,
) {
    let active_style = style_sheet[&StyleState::Active];
    let hovered = cursor.is_over(bounds);
    let background = if primary {
        if hovered {
            Background::Color(active_style.slider_handle_background)
        } else {
            active_style.tab_selected_background
        }
    } else if hovered {
        active_style.tab_hover_background
    } else {
        active_style.tab_background
    };

    renderer.fill_quad(
        renderer::Quad {
            bounds,
            border: Border {
                radius: 5.0.into(),
                width: 1.0,
                color: active_style.panel_border_color,
            },
            ..renderer::Quad::default()
        },
        background,
    );

    renderer.fill_text(
        Text {
            content: label.to_owned(),
            bounds: Size::new(bounds.width, bounds.height),
            size: renderer.default_size(),
            font: renderer.default_font(),
            align_x: text::Alignment::Center,
            align_y: Vertical::Center,
            line_height: text::LineHeight::Relative(1.3),
            shaping: text::Shaping::Basic,
            wrapping: text::Wrapping::None,
        },
        Point::new(bounds.center_x(), bounds.center_y()),
        if primary {
            active_style.slider_handle_border_color
        } else {
            active_style.text_primary
        },
        bounds,
    );
}

/// Draws the add-current-color button ("+") below the swatch grid.
fn draw_add_button(
    renderer: &mut Renderer,
    layout: Layout<'_>,
    cursor: Cursor,
    style_sheet: &HashMap<StyleState, Style>,
) {
    let active_style = style_sheet[&StyleState::Active];
    let bounds = layout.bounds();
    let hovered = cursor.is_over(bounds);

    renderer.fill_quad(
        renderer::Quad {
            bounds,
            border: Border {
                radius: 5.0.into(),
                width: 1.0,
                color: active_style.panel_border_color,
            },
            ..renderer::Quad::default()
        },
        if hovered {
            active_style.tab_hover_background
        } else {
            active_style.tab_background
        },
    );

    renderer.fill_text(
        Text {
            content: "+".to_owned(),
            bounds: Size::new(bounds.width, bounds.height),
            size: renderer.default_size(),
            font: renderer.default_font(),
            align_x: text::Alignment::Center,
            align_y: Vertical::Center,
            line_height: text::LineHeight::Relative(1.3),
            shaping: text::Shaping::Basic,
            wrapping: text::Wrapping::None,
        },
        Point::new(bounds.center_x(), bounds.center_y()),
        active_style.text_primary,
        bounds,
    );
}

/// Draws the recent colors grid: cached checkerboard + color fill + hover
/// border per cell (same drawing as the swatch grid).
fn draw_recent_grid<Message, Theme>(
    renderer: &mut Renderer,
    color_picker: &ColorPickerOverlay<'_, '_, Message, Theme>,
    layout: Layout<'_>,
    cursor: Cursor,
    style_sheet: &HashMap<StyleState, Style>,
) where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    let active_style = style_sheet[&StyleState::Active];
    let checker_1 = active_style.checker_color_1;
    let checker_2 = active_style.checker_color_2;
    let tile = 10.0;

    for (i, cell_layout) in layout.children().enumerate() {
        let cell = cell_layout.bounds();
        let Some(color) = color_picker.state.recent_colors.get(i) else {
            continue;
        };

        draw_checkerboard(renderer, cell, tile, checker_1, checker_2);

        if (cell.width > 0.) && (cell.height > 0.) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: cell,
                    ..renderer::Quad::default()
                },
                *color,
            );
        }

        renderer.fill_quad(
            renderer::Quad {
                bounds: cell,
                border: Border {
                    radius: 2.0.into(),
                    width: 1.0,
                    color: if cursor.is_over(cell) {
                        active_style.swatch_hover_border_color
                    } else {
                        active_style.swatch_border_color
                    },
                },
                ..renderer::Quad::default()
            },
            Color::TRANSPARENT,
        );
    }
}

/// Draws a checkerboard of solid quads behind a color, clamping the edge
/// tiles to `bounds`.
///
/// Quads are used on purpose: the renderer batches quads and meshes
/// separately (all quads are drawn before any mesh), so a canvas mesh would
/// always end up rendering *on top of* a later `fill_quad` color.
#[allow(clippy::too_many_arguments)]
fn draw_checkerboard(
    renderer: &mut Renderer,
    bounds: Rectangle,
    tile: f32,
    checker_1: Color,
    checker_2: Color,
) {
    let columns = (bounds.width / tile).ceil() as u16;
    let rows = (bounds.height / tile).ceil() as u16;
    let right = bounds.x + bounds.width;
    let bottom = bounds.y + bounds.height;

    for column in 0..columns {
        for row in 0..rows {
            let tile_color = if (column + row) % 2 == 0 {
                checker_1
            } else {
                checker_2
            };
            let tile = Rectangle {
                x: bounds.x + column as f32 * tile,
                y: bounds.y + row as f32 * tile,
                width: right.min(bounds.x + (column as f32 + 1.0) * tile)
                    - (bounds.x + column as f32 * tile),
                height: bottom.min(bounds.y + (row as f32 + 1.0) * tile)
                    - (bounds.y + row as f32 * tile),
            };
            renderer.fill_quad(
                renderer::Quad {
                    bounds: tile,
                    ..renderer::Quad::default()
                },
                tile_color,
            );
        }
    }
}

/// Draws the placeholder preview panels (Original / New).
fn preview_placeholder<Message, Theme>(
    renderer: &mut Renderer,
    color_picker: &ColorPickerOverlay<'_, '_, Message, Theme>,
    layout: Layout<'_>,
    _cursor: Cursor,
    style_sheet: &HashMap<StyleState, Style>,
) where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    let active_style = style_sheet[&StyleState::Active];
    let bounds = layout.bounds();

    let panel_width = (bounds.width - 5.0) / 2.0;
    let panels = [
        (
            "Original",
            color_picker.state.initial_color,
            Rectangle {
                x: bounds.x,
                y: bounds.y,
                width: panel_width,
                height: PREVIEW_HEIGHT,
            },
        ),
        (
            "New",
            color_picker.state.color,
            Rectangle {
                x: bounds.x + panel_width + 5.0,
                y: bounds.y,
                width: panel_width,
                height: PREVIEW_HEIGHT,
            },
        ),
    ];

    let checker_1 = active_style.checker_color_1;
    let checker_2 = active_style.checker_color_2;
    let tile = 10.0;

    for (label, color, panel_bounds) in panels {
        // Checkerboard behind the color; tiles are drawn as solid quads so
        // they stack below the color fill (the renderer batches quads and
        // meshes separately, and a mesh would always draw on top of a quad).
        draw_checkerboard(renderer, panel_bounds, tile, checker_1, checker_2);

        // Color fill on top (alpha-composited over the checkerboard).
        if (panel_bounds.width > 0.) && (panel_bounds.height > 0.) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: panel_bounds,
                    ..renderer::Quad::default()
                },
                color,
            );
        }

        // Border.
        renderer.fill_quad(
            renderer::Quad {
                bounds: panel_bounds,
                border: Border {
                    radius: 4.0.into(),
                    width: 1.0,
                    color: active_style.preview_border_color,
                },
                ..renderer::Quad::default()
            },
            Color::TRANSPARENT,
        );

        let mut label_bounds = panel_bounds;
        label_bounds.y += PREVIEW_HEIGHT + 2.0;
        label_bounds.height = bounds.y + bounds.height - label_bounds.y;

        renderer.fill_text(
            Text {
                content: label.to_owned(),
                bounds: Size::new(label_bounds.width, label_bounds.height),
                size: Pixels(13.0),
                font: renderer.default_font(),
                align_x: text::Alignment::Center,
                align_y: Vertical::Center,
                line_height: text::LineHeight::Relative(1.3),
                shaping: text::Shaping::Basic,
                wrapping: text::Wrapping::None,
            },
            Point::new(label_bounds.center_x(), label_bounds.center_y()),
            active_style.text_secondary,
            label_bounds,
        );
    }
}

/// Draws the Reset button with its danger palette colors.
fn draw_reset_button(
    renderer: &mut Renderer,
    bounds: Rectangle,
    pressed: bool,
    cursor: Cursor,
    style_sheet: &HashMap<StyleState, Style>,
) {
    let active_style = style_sheet[&StyleState::Active];

    let background = if pressed && cursor.is_over(bounds) {
        active_style.reset_hover_background
    } else if cursor.is_over(bounds) {
        active_style.reset_hover_background
    } else {
        active_style.reset_background
    };

    renderer.fill_quad(
        renderer::Quad {
            bounds,
            border: Border {
                radius: 5.0.into(),
                width: 1.0,
                color: active_style.panel_border_color,
            },
            ..renderer::Quad::default()
        },
        background,
    );

    renderer.fill_text(
        Text {
            content: "Reset".to_owned(),
            bounds: Size::new(bounds.width, bounds.height),
            size: renderer.default_size(),
            font: renderer.default_font(),
            align_x: text::Alignment::Center,
            align_y: Vertical::Center,
            line_height: text::LineHeight::Relative(1.3),
            shaping: text::Shaping::Basic,
            wrapping: text::Wrapping::None,
        },
        Point::new(bounds.center_x(), bounds.center_y()),
        active_style.text_primary,
        bounds,
    );
}

/// Draws the focus border of the given button if it is focused.
fn draw_focus_border<Message, Theme>(
    renderer: &mut Renderer,
    color_picker: &ColorPickerOverlay<'_, '_, Message, Theme>,
    bounds: Rectangle,
    target: Focus,
    style_sheet: &HashMap<StyleState, Style>,
) where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    if (color_picker.state.focus == target) && (bounds.width > 0.) && (bounds.height > 0.) {
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border {
                    radius: style_sheet[&StyleState::Focused].border_radius.into(),
                    width: style_sheet[&StyleState::Focused].border_width,
                    color: style_sheet[&StyleState::Focused].border_color,
                },
                ..renderer::Quad::default()
            },
            Color::TRANSPARENT,
        );
    }
}

/// Draws one of the overlay buttons (Cancel / OK) with a proper
/// active/hovered/pressed appearance.
///
/// The underlying [`Button`] widgets handle events, but iced re-creates
/// overlays for every draw call, which discards the buttons' internal status.
/// The pressed state therefore lives in the overlay [`State`], and the hover
/// state is derived from the cursor position here.
#[allow(clippy::too_many_arguments)]
fn draw_overlay_button<Theme>(
    renderer: &mut Renderer,
    theme: &Theme,
    label: &str,
    bounds: Rectangle,
    pressed: bool,
    cursor: Cursor,
) where
    Theme: iced::widget::button::Catalog,
{
    let status = if pressed && cursor.is_over(bounds) {
        button::Status::Pressed
    } else if cursor.is_over(bounds) {
        button::Status::Hovered
    } else {
        button::Status::Active
    };

    let style = iced::widget::button::Catalog::style(
        theme,
        &<Theme as iced::widget::button::Catalog>::default(),
        status,
    );

    renderer.fill_quad(
        renderer::Quad {
            bounds,
            border: style.border,
            shadow: style.shadow,
            snap: style.snap,
        },
        style
            .background
            .unwrap_or(Background::Color(Color::TRANSPARENT)),
    );

    renderer.fill_text(
        Text {
            content: label.to_owned(),
            bounds: Size::new(bounds.width, bounds.height),
            size: renderer.default_size(),
            font: renderer.default_font(),
            align_x: text::Alignment::Center,
            align_y: Vertical::Center,
            line_height: text::LineHeight::Relative(1.3),
            shaping: text::Shaping::Basic,
            wrapping: text::Wrapping::None,
        },
        Point::new(bounds.center_x(), bounds.center_y()),
        style.text_color,
        bounds,
    );
}
#[allow(clippy::too_many_lines)]
fn hsv_color<Message, Theme>(
    renderer: &mut Renderer,
    color_picker: &ColorPickerOverlay<'_, '_, Message, Theme>,
    layout: Layout<'_>,
    cursor: Cursor,
    style_sheet: &HashMap<StyleState, Style>,
) where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    let mut hsv_color_children = layout.children();
    let hsv_color: Hsv = color_picker.state.hsv();

    let sat_value_layout = hsv_color_children
        .next()
        .expect("Graphics: Layout should have a sat/value layout");
    let mut sat_value_style_state = StyleState::Active;
    if color_picker.state.focus == Focus::Square {
        sat_value_style_state = sat_value_style_state.max(StyleState::Focused);
    }
    if cursor.is_over(sat_value_layout.bounds()) {
        sat_value_style_state = sat_value_style_state.max(StyleState::Hovered);
    }

    let geometry = color_picker.state.sat_value_canvas_cache.draw(
        renderer,
        sat_value_layout.bounds().size(),
        |frame| {
            let column_count = frame.width() as u16;
            let row_count = frame.height() as u16;

            for column in 0..column_count {
                for row in 0..row_count {
                    let saturation = f32::from(column) / frame.width();
                    let value = f32::from(row) / frame.height();

                    frame.fill_rectangle(
                        Point::new(f32::from(column), f32::from(row)),
                        Size::new(1.0, 1.0),
                        Color::from(Hsv::from_hsv(hsv_color.hue, saturation, value)),
                    );
                }
            }

            let stroke = Stroke {
                style: canvas::Style::Solid(
                    Hsv {
                        hue: 0,
                        saturation: 0.0,
                        value: 1.0 - hsv_color.value,
                    }
                    .into(),
                ),
                width: 3.0,
                line_cap: LineCap::Round,
                ..Stroke::default()
            };

            let saturation = hsv_color.saturation * frame.width();
            let value = hsv_color.value * frame.height();

            let indicator_radius = style_sheet
                .get(&sat_value_style_state)
                .expect("Style Sheet not found.")
                .sv_square_indicator_radius;

            frame.stroke(
                &Path::circle(Point::new(saturation, value), indicator_radius),
                stroke,
            );

            let stroke = Stroke {
                style: canvas::Style::Solid(
                    style_sheet
                        .get(&sat_value_style_state)
                        .expect("Style Sheet not found.")
                        .bar_border_color,
                ),
                width: 2.0,
                line_cap: LineCap::Round,
                ..Stroke::default()
            };

            frame.stroke(
                &Path::rectangle(
                    Point::new(0.0, 0.0),
                    Size::new(frame.size().width - 0.0, frame.size().height - 0.0),
                ),
                stroke,
            );
        },
    );

    let translation = Vector::new(sat_value_layout.bounds().x, sat_value_layout.bounds().y);
    renderer.with_translation(translation, |renderer| {
        renderer.draw_geometry(geometry);
    });

    let hue_layout = hsv_color_children
        .next()
        .expect("Graphics: Layout should have a hue layout");
    let mut hue_style_state = StyleState::Active;
    if color_picker.state.focus == Focus::Ring {
        hue_style_state = hue_style_state.max(StyleState::Focused);
    }
    if is_in_ring_band(
        cursor.position_in(hue_layout.bounds()).unwrap_or(Point::ORIGIN),
        hue_layout.bounds().size(),
    ) {
        hue_style_state = hue_style_state.max(StyleState::Hovered);
    }

    let geometry =
        color_picker
            .state
            .hue_canvas_cache
            .draw(renderer, hue_layout.bounds().size(), |frame| {
                let size = frame.size();
                let center = Point::new(size.width / 2.0, size.height / 2.0);
                let outer = size.width.min(size.height) / 2.0;
                let inner = outer - (RING_WIDTH + RING_PADDING);
                let inner_sq = inner * inner;
                let outer_sq = outer * outer;

                let column_count = frame.width() as u16;
                let row_count = frame.height() as u16;

                for column in 0..column_count {
                    for row in 0..row_count {
                        let dx = f32::from(column) + 0.5 - center.x;
                        let dy = f32::from(row) + 0.5 - center.y;
                        let dist = dx * dx + dy * dy;

                        if dist >= inner_sq && dist <= outer_sq {
                            let hue = hue_from_angle(dy.atan2(dx).to_degrees());
                            let ring_color = Color::from(Hsv::from_hsv(hue, 1.0, 1.0));
                            frame.fill_rectangle(
                                Point::new(f32::from(column), f32::from(row)),
                                Size::new(1.0, 1.0),
                                ring_color,
                            );
                        }
                    }
                }

                // Indicator: white filled circle with a black outline at the
                // center-line of the current hue angle.
                let indicator_radius = (inner + outer) / 2.0;
                let angle = f32::from(hsv_color.hue).to_radians();
                let indicator_center = Point::new(
                    center.x + angle.cos() * indicator_radius,
                    center.y + angle.sin() * indicator_radius,
                );

                frame.fill(&Path::circle(indicator_center, 7.5), Color::WHITE);
                frame.stroke(
                    &Path::circle(indicator_center, 7.5),
                    Stroke {
                        style: canvas::Style::Solid(Color::BLACK),
                        width: 1.5,
                        ..Stroke::default()
                    },
                );

                // Band border (inner + outer circle).
                let stroke = Stroke {
                    style: canvas::Style::Solid(
                        style_sheet
                            .get(&hue_style_state)
                            .expect("Style Sheet not found.")
                            .bar_border_color,
                    ),
                    width: 1.0,
                    ..Stroke::default()
                };

                frame.stroke(&Path::circle(center, inner - 0.5), stroke);
                frame.stroke(&Path::circle(center, outer - 0.5), stroke);
            });

    let translation = Vector::new(hue_layout.bounds().x, hue_layout.bounds().y);
    renderer.with_translation(translation, |renderer| {
        renderer.draw_geometry(geometry);
    });
}

/// The layout of the value cell hosting the channel input with the given
/// channel index, within the controls column of the given layout tree.
fn value_cell_layout<'l, Message, Theme>(
    controls: Layout<'l>,
    color_picker: &ColorPickerOverlay<'_, '_, Message, Theme>,
    channel: usize,
) -> Option<Layout<'l>>
where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    let row = match (color_picker.state.active_tab, channel) {
        (ActiveTab::Rgb, 0..=3) => channel,
        (ActiveTab::Hsv, 4..=6) => channel - 4,
        _ => return None,
    };
    controls
        .children()
        .nth(row)
        .and_then(|row_layout| row_layout.children().nth(2))
}

/// Whether the layout of a channel value cell carries the children of a
/// real `TextInput` node.
fn value_input_children_exist(value_layout: Layout<'_>) -> bool {
    value_layout.children().next().is_some()
}

/// The layout of the hex input cell within the hex container.
fn hex_input_layout<'l>(hex_container: Layout<'l>) -> Option<Layout<'l>> {
    hex_container.children().nth(1)
}

/// Draws the gradient slider rows of the active tab (RGB channels or HSV
/// channels) including the value fields.
#[allow(clippy::too_many_lines)]
fn slider_rows<Message, Theme>(
    renderer: &mut Renderer,
    color_picker: &ColorPickerOverlay<'_, '_, Message, Theme>,
    layout: Layout<'_>,
    cursor: Cursor,
    theme: &Theme,
    style: &renderer::Style,
    style_sheet: &HashMap<StyleState, Style>,
    focus: Focus,
) where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    let active_style = style_sheet[&StyleState::Active];
    let color = color_picker.state.color;
    let hsv: Hsv = color_picker.state.hsv();
    let mut slider_children = layout.children();

    let labels = match color_picker.state.active_tab {
        ActiveTab::Rgb => ["R", "G", "B", "A"],
        ActiveTab::Hsv => ["H", "S", "V", "A"],
    };

    for (row, label) in labels.iter().enumerate() {
        let mut row_children = slider_children
            .next()
            .expect("Graphics: Layout should have a slider row layout")
            .children();

        let label_layout = row_children
            .next()
            .expect("Graphics: Layout should have a label layout");
        let bar_layout = row_children
            .next()
            .expect("Graphics: Layout should have a bar layout");
        let value_layout = row_children
            .next()
            .expect("Graphics: Layout should have a value layout");

        renderer.fill_text(
            Text {
                content: format!("{label}:"),
                bounds: Size::new(label_layout.bounds().width, label_layout.bounds().height),
                size: renderer.default_size(),
                font: renderer.default_font(),
                align_x: text::Alignment::Center,
                align_y: Vertical::Center,
                line_height: text::LineHeight::Relative(1.3),
                shaping: text::Shaping::Basic,
                wrapping: text::Wrapping::None,
            },
            Point::new(
                label_layout.bounds().center_x(),
                label_layout.bounds().center_y(),
            ),
            style.text_color,
            label_layout.bounds(),
        );

        let bar_bounds = bar_layout.bounds();
        let value_bounds = value_layout.bounds();

        let bar_style_state = if cursor.is_over(bar_bounds) {
            StyleState::Hovered
        } else {
            StyleState::Active
        };
        let bar_style = style_sheet
            .get(&bar_style_state)
            .expect("Style Sheet not found.");

        let channel = match (color_picker.state.active_tab, row) {
            (ActiveTab::Rgb, 0) => 0,
            (ActiveTab::Rgb, 1) => 1,
            (ActiveTab::Rgb, 2) => 2,
            (ActiveTab::Rgb, 3) => 3,
            (ActiveTab::Hsv, 0) => 4,
            (ActiveTab::Hsv, 1) => 5,
            (ActiveTab::Hsv, 2) => 6,
            _ => usize::MAX,
        };

        // Fraction of the channel value inside the groove.
        let fraction = match channel {
            0 => color.r,
            1 => color.g,
            2 => color.b,
            3 => color.a,
            4 => f32::from(hsv.hue) / 360.0,
            5 => hsv.saturation,
            _ => hsv.value,
        };

        let groove_color = |t: f32| -> Color {
            match channel {
                0 => Color::from_rgb(t, color.g, color.b),
                1 => Color::from_rgb(color.r, t, color.b),
                2 => Color::from_rgb(color.r, color.g, t),
                3 => Color::from_rgba(color.r, color.g, color.b, t),
                4 => Hsv::from_hsv((t * 360.0) as u16 % 360, 1.0, 1.0).into(),
                5 => Hsv::from_hsv(hsv.hue, t, hsv.value).into(),
                _ => Hsv::from_hsv(hsv.hue, hsv.saturation, t).into(),
            }
        };

        // Groove background: checkered for the alpha channel, flat otherwise.
        if (bar_bounds.width > 0.) && (bar_bounds.height > 0.) {
            let tile = if channel == 3 {
                (active_style.checker_alpha_1, active_style.checker_alpha_2)
            } else {
                (active_style.checker_color_1, active_style.checker_color_2)
            };
            if channel == 3 || channel == usize::MAX {
                for x in 0..bar_bounds.width as i32 {
                    let tile_color = if (x / 6) as usize % 2 == 0 { tile.0 } else { tile.1 };
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: Rectangle::new(
                                Point::new(bar_bounds.x + x as f32, bar_bounds.y),
                                Size::new(1.0, bar_bounds.height),
                            ),
                            ..renderer::Quad::default()
                        },
                        tile_color,
                    );
                }
            }
        }

        // Gradient columns.
        if (bar_bounds.width > 0.) && (bar_bounds.height > 0.) && channel != usize::MAX {
            for x in 0..bar_bounds.width as i32 {
                let t = if bar_bounds.width > 1.0 {
                    x as f32 / (bar_bounds.width - 1.0)
                } else {
                    0.0
                };
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle::new(
                            Point::new(bar_bounds.x + x as f32, bar_bounds.y),
                            Size::new(1.0, bar_bounds.height),
                        ),
                        ..renderer::Quad::default()
                    },
                    groove_color(t),
                );
            }
        }

        // Value handle.
        if channel != usize::MAX {
            let handle_center = Point::new(
                bar_bounds.x + bar_bounds.width * fraction,
                bar_bounds.y + bar_bounds.height / 2.0,
            );
            let handle_bounds = Rectangle {
                x: handle_center.x - 8.0,
                y: handle_center.y - 8.0,
                width: 16.0,
                height: 16.0,
            };
            let handle_background = if cursor.is_over(handle_bounds) {
                active_style.slider_handle_hover_background
            } else {
                active_style.slider_handle_background
            };
            renderer.fill_quad(
                renderer::Quad {
                    bounds: handle_bounds,
                    border: Border {
                        radius: 8.0.into(),
                        width: 1.0,
                        color: active_style.slider_handle_border_color,
                    },
                    ..renderer::Quad::default()
                },
                handle_background,
            );
        }

        // Groove border.
        if (bar_bounds.width > 0.) && (bar_bounds.height > 0.) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: bar_bounds,
                    border: Border {
                        radius: bar_style.bar_border_radius.into(),
                        width: bar_style.bar_border_width,
                        color: active_style.slider_groove_border_color,
                    },
                    ..renderer::Quad::default()
                },
                Color::TRANSPARENT,
            );
        }

        // Value field: channel TextInput (active tab) or readonly text
        // (the alpha cell of the HSV tab).
        let value_input_index = match (color_picker.state.active_tab, row) {
            (ActiveTab::Rgb, i) => i,
            (ActiveTab::Hsv, 0) => 4,
            (ActiveTab::Hsv, 1) => 5,
            (ActiveTab::Hsv, 2) => 6,
            _ => usize::MAX,
        };
        if value_input_index != usize::MAX {
            if let Some(tree_child) = color_picker
                .tree
                .children
                .get(VALUE_INPUTS_INDEX + value_input_index)
                && value_input_children_exist(value_layout)
            {
                color_picker.value_inputs[value_input_index].draw(
                    tree_child,
                    renderer,
                    theme,
                    value_layout,
                    cursor,
                    Some(&text_input::Value::new(&color_picker.state.value_inputs[value_input_index])),
                    &value_layout.bounds(),
                );
            }
        } else {
            renderer.fill_text(
                Text {
                    content: color_picker.state.value_inputs[3].clone(),
                    bounds: Size::new(value_bounds.width, value_bounds.height),
                    size: renderer.default_size(),
                    font: renderer.default_font(),
                    align_x: text::Alignment::Center,
                    align_y: Vertical::Center,
                    line_height: text::LineHeight::Relative(1.3),
                    shaping: text::Shaping::Basic,
                    wrapping: text::Wrapping::None,
                },
                Point::new(value_bounds.center_x(), value_bounds.center_y()),
                active_style.text_secondary,
                value_bounds,
            );
        }

        // Keyboard focus border around the row.
        let row_bounds = value_layout.bounds().union(&bar_bounds);
        let target = channel_focus(channel);
        if channel != usize::MAX
            && (focus == target)
            && (row_bounds.width > 0.)
            && (row_bounds.height > 0.)
        {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: row_bounds,
                    border: Border {
                        radius: style_sheet
                            .get(&StyleState::Focused)
                            .expect("Style Sheet not found.")
                            .border_radius
                            .into(),
                        width: style_sheet
                            .get(&StyleState::Focused)
                            .expect("Style Sheet not found.")
                            .border_width,
                        color: style_sheet
                            .get(&StyleState::Focused)
                            .expect("Style Sheet not found.")
                            .border_color,
                    },
                    ..renderer::Quad::default()
                },
                Color::TRANSPARENT,
            );
        }
    }
}

/// Draws the hex container: "Hex:" label + the hex TextInput.
fn hex_input<Message, Theme>(
    renderer: &mut Renderer,
    theme: &Theme,
    color_picker: &ColorPickerOverlay<'_, '_, Message, Theme>,
    layout: Layout<'_>,
    cursor: Cursor,
    style_sheet: &HashMap<StyleState, Style>,
) where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    let active_style = style_sheet[&StyleState::Active];
    let bounds = layout.bounds();

    if (bounds.width > 0.) && (bounds.height > 0.) {
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border {
                    radius: active_style.panel_border_radius.into(),
                    width: 1.0,
                    color: active_style.panel_border_color,
                },
                ..renderer::Quad::default()
            },
            active_style.panel_background,
        );
    }

    let mut hex_children = layout.children();
    if let Some(label_layout) = hex_children.next() {
        renderer.fill_text(
            Text {
                content: "Hex:".to_owned(),
                bounds: Size::new(label_layout.bounds().width, label_layout.bounds().height),
                size: renderer.default_size(),
                font: renderer.default_font(),
                align_x: text::Alignment::Center,
                align_y: Vertical::Center,
                line_height: text::LineHeight::Relative(1.3),
                shaping: text::Shaping::Basic,
                wrapping: text::Wrapping::None,
            },
            Point::new(
                label_layout.bounds().center_x(),
                label_layout.bounds().center_y(),
            ),
            active_style.text_secondary,
            label_layout.bounds(),
        );
    }
    let input_layout = hex_children.next();

    if let Some(tree_child) = color_picker.tree.children.get(HEX_INPUT_INDEX)
        && let Some(input_layout) = input_layout
        && input_layout.children().next().is_some()
    {
        color_picker.hex_input.draw(
            tree_child,
            renderer,
            theme,
            input_layout,
            cursor,
            Some(&text_input::Value::new(&color_picker.state.hex_input)),
            &input_layout.bounds(),
        );
    }
}

/// The state of the [`ColorPickerOverlay`].
#[derive(Debug)]
pub struct State {
    /// The selected color of the [`ColorPickerOverlay`].
    pub(crate) color: Color,
    /// The color used to initialize [`ColorPickerOverlay`].
    pub(crate) initial_color: Color,
    /// The last known hue of the picked color.
    ///
    /// Achromatic colors (white, black, grays) have no hue; deriving `hue: 0`
    /// from them would snap the ring/square to red. This field keeps the last
    /// meaningful hue for display and for subsequent picks.
    pub(crate) hue: u16,
    /// The cache of the sat/value canvas of the [`ColorPickerOverlay`].
    pub(crate) sat_value_canvas_cache: canvas::Cache,
    /// The cache of the hue ring canvas of the [`ColorPickerOverlay`].
    pub(crate) hue_canvas_cache: canvas::Cache,
    /// The dragged color bar of the [`ColorPickerOverlay`].
    pub(crate) color_bar_dragged: ColorBarDragged,
    /// the focus of the [`ColorPickerOverlay`].
    pub(crate) focus: Focus,
    /// The previously pressed keyboard modifiers.
    pub(crate) keyboard_modifiers: keyboard::Modifiers,
    /// Whether the cancel button is currently pressed.
    pub(crate) cancel_pressed: bool,
    /// Whether the submit button is currently pressed.
    pub(crate) submit_pressed: bool,
    /// Whether the reset button is currently pressed.
    pub(crate) reset_pressed: bool,
    /// The active controls tab of the left pane.
    pub(crate) active_tab: ActiveTab,
    /// The text of the hex input field (e.g. `"#FF800080"`).
    pub(crate) hex_input: String,
    /// Whether the hex input field has the text cursor.
    pub(crate) hex_focused: bool,
    /// The text of the value fields: `[R, G, B, A, H, S, V]`.
    pub(crate) value_inputs: [String; 7],
    /// The value field that currently has the text cursor.
    pub(crate) value_focus: Option<usize>,
    /// The swatch sets shown in the swatch tab bar.
    pub(crate) swatch_sets: Vec<SwatchSet>,
    /// The active swatch set.
    pub(crate) active_swatch_tab: usize,
    /// Whether the "new swatch set" prompt is active.
    pub(crate) naming_new_set: bool,
    /// The name typed into the "new swatch set" prompt.
    pub(crate) pending_swatch_name: String,
    /// The recently submitted colors.
    pub(crate) recent_colors: Vec<Color>,
    /// Hit-test results of the swatch section.
    pub(crate) swatch_hover: SwatchHover,
    /// Whether the RGB(A) tab is hovered.
    pub(crate) tab_rgb_hovered: bool,
    /// Whether the HSV tab is hovered.
    pub(crate) tab_hsv_hovered: bool,
    /// Whether the "+" swatch tab is hovered.
    pub(crate) plus_tab_hovered: bool,
    /// The swatch cell targeted by the keyboard cursor while
    /// [`Focus::Swatches`] is active: `(set index, color index)`.
    pub(crate) focused_swatch: Option<(usize, usize)>,
}

impl State {
    /// Creates a new State with the given color.
    #[must_use]
    pub fn new(color: Color) -> Self {
        let hue = Hsv::from(color).hue;
        Self {
            color,
            initial_color: color,
            hue,
            hex_input: color_to_hex_argb(color),
            value_inputs: value_inputs_from_color(color),
            ..Self::default()
        }
    }

    /// Reset cached canvas when internal state is modified.
    ///
    /// If the color has changed, empty all canvas caches
    /// as they (unfortunately) do not depend on the picker state.
    fn clear_cache(&self) {
        self.sat_value_canvas_cache.clear();
        self.hue_canvas_cache.clear();
    }

    /// Refresh the hex input and value field texts to match `self.color`.
    pub(crate) fn sync_display(&mut self) {
        self.hex_input = color_to_hex_argb(self.color);
        self.value_inputs = value_inputs_from_color(self.color);
    }

    /// Sets the current color, remembering its hue when it has one.
    ///
    /// Achromatic colors (saturation 0) do not overwrite the remembered hue;
    /// see [`Self::hue`].
    pub(crate) fn apply_color(&mut self, color: Color) {
        let hsv: Hsv = color.into();
        if hsv.saturation > 0.0 {
            self.hue = hsv.hue;
        }
        self.color = color;
    }

    /// The HSV of the current color for display purposes.
    ///
    /// Achromatic colors (saturation 0) have no hue; the last meaningful hue
    /// is substituted so the ring/square do not snap to red.
    pub(crate) fn hsv(&self) -> Hsv {
        let hsv: Hsv = self.color.into();
        if hsv.saturation > 0.0 {
            hsv
        } else {
            Hsv {
                hue: self.hue,
                ..hsv
            }
        }
    }

    /// Synchronize the color with an externally provided value.
    pub(crate) fn force_synchronize(&mut self, color: Color) {
        self.initial_color = color;
        self.apply_color(color);
        self.sync_display();
        self.clear_cache();
    }
}

impl Default for State {
    fn default() -> Self {
        let default_color = Color::from_rgb(0.5, 0.25, 0.25);
        Self {
            color: default_color,
            initial_color: default_color,
            hue: Hsv::from(default_color).hue,
            sat_value_canvas_cache: canvas::Cache::default(),
            hue_canvas_cache: canvas::Cache::default(),
            color_bar_dragged: ColorBarDragged::None,
            focus: Focus::default(),
            keyboard_modifiers: keyboard::Modifiers::default(),
            cancel_pressed: false,
            submit_pressed: false,
            reset_pressed: false,
            active_tab: ActiveTab::Rgb,
            hex_focused: false,
            value_focus: None,
            swatch_sets: vec![SwatchSet {
                name: "Default".to_owned(),
                colors: Vec::new(),
            }],
            active_swatch_tab: 0,
            naming_new_set: false,
            pending_swatch_name: String::new(),
            recent_colors: Vec::new(),
            swatch_hover: SwatchHover::default(),
            tab_rgb_hovered: false,
            tab_hsv_hovered: false,
            plus_tab_hovered: false,
            focused_swatch: None,
            hex_input: color_to_hex_argb(default_color),
            value_inputs: value_inputs_from_color(default_color),
        }
    }
}

/// Fills the seven value field texts (`[R, G, B, A, H, S, V]`) from a color.
/// Saturation and value are on the 0-255 scale of the Python spinboxes.
fn value_inputs_from_color(color: Color) -> [String; 7] {
    let hsv: Hsv = color.into();
    [
        ((color.r * 255.0) as u8).to_string(),
        ((color.g * 255.0) as u8).to_string(),
        ((color.b * 255.0) as u8).to_string(),
        ((color.a * 255.0) as u8).to_string(),
        hsv.hue.to_string(),
        ((hsv.saturation * 255.0) as u8).to_string(),
        ((hsv.value * 255.0) as u8).to_string(),
    ]
}

/// Just a workaround to pass the button states from the tree to the overlay
#[allow(missing_debug_implementations)]
pub struct ColorPickerOverlayButtons<'a, Message, Theme>
where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog,
{
    /// The cancel button of the [`ColorPickerOverlay`].
    cancel_button: Element<'a, Message, Theme, Renderer>,
    /// The submit button of the [`ColorPickerOverlay`].
    submit_button: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Theme> Default for ColorPickerOverlayButtons<'a, Message, Theme>
where
    Message: 'a + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    fn default() -> Self {
        let (cancel_content, cancel_font) = cancel_icon();
        let (submit_content, submit_font) = ok_icon();

        Self {
            cancel_button: Button::new(widget::Text::new(cancel_content).font(cancel_font)).into(),
            submit_button: Button::new(widget::Text::new(submit_content).font(submit_font)).into(),
        }
    }
}

#[allow(clippy::unimplemented)]
impl<Message, Theme> Widget<Message, Theme, Renderer>
    for ColorPickerOverlayButtons<'_, Message, Theme>
where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    fn children(&self) -> Vec<Tree> {
        vec![
            Tree::new(&self.cancel_button),
            Tree::new(&self.submit_button),
        ]
    }

    // Do nothing so the overlay tree children ([2] hex input, [3..=9] value
    // inputs, [10] name input) are not cleared between frames.
    fn diff(&self, _tree: &mut Tree) {}

    fn size(&self) -> Size<Length> {
        unimplemented!("This should never be reached!")
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) -> Node {
        unimplemented!("This should never be reached!")
    }

    fn draw(
        &self,
        _state: &Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
    ) {
        unimplemented!("This should never be reached!")
    }
}

impl<'a, Message, Theme> From<ColorPickerOverlayButtons<'a, Message, Theme>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    fn from(overlay: ColorPickerOverlayButtons<'a, Message, Theme>) -> Self {
        Self::new(overlay)
    }
}

/// The state of the currently dragged area.
#[derive(Copy, Clone, Debug, Default)]
pub enum ColorBarDragged {
    /// No area is focussed.
    #[default]
    None,

    /// The saturation/value area is focussed.
    SatValue,

    /// The hue area is focussed.
    Hue,

    /// The red area is focussed.
    Red,

    /// The green area is focussed.
    Green,

    /// The blue area is focussed.
    Blue,

    /// The alpha area is focussed.
    Alpha,

    /// The hue area of the HSV tab is focussed.
    HsvHue,

    /// The saturation area of the HSV tab is focussed.
    HsvSat,

    /// The value area of the HSV tab is focussed.
    HsvVal,
}

/// An enumeration of all focusable element of the [`ColorPickerOverlay`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Focus {
    /// Nothing is in focus.
    #[default]
    None,

    /// The overlay itself is in focus.
    Overlay,

    /// The saturation and value square is in focus.
    Ring,

    /// The hue ring is in focus.
    Square,

    /// The red bar is in focus.
    Red,

    /// The green bar is in focus.
    Green,

    /// The blue bar is in focus.
    Blue,

    /// The alpha bar is in focus.
    Alpha,

    /// The hex text input is in focus.
    Hex,

    /// The hue bar of the HSV tab is in focus.
    HsvHue,

    /// The saturation bar of the HSV tab is in focus.
    HsvSat,

    /// The value bar of the HSV tab is in focus.
    HsvVal,

    /// The RGB(A) tab of the left pane is in focus.
    TabRgb,

    /// The HSV tab of the left pane is in focus.
    TabHsv,

    /// The swatch section is in focus.
    Swatches,

    /// The "new swatch set" name input is in focus.
    NewSetName,

    /// The reset button is in focus.
    Reset,

    /// The cancel button is in focus.
    Cancel,

    /// The submit button is in focus.
    Submit,
}

/// The focus of a value input channel (`[R,G,B,A,H,S,V] -> Focus`).
#[must_use]
fn channel_focus(channel: usize) -> Focus {
    match channel {
        0 => Focus::Red,
        1 => Focus::Green,
        2 => Focus::Blue,
        3 => Focus::Alpha,
        4 => Focus::HsvHue,
        5 => Focus::HsvSat,
        _ => Focus::HsvVal,
    }
}

/// The ordered focus cycle of the overlay. The channel foci of the inactive
/// tab are skipped, and the "new swatch set" input is only reachable while
/// the naming prompt is active.
fn focus_cycle(active_tab: ActiveTab, naming_new_set: bool) -> Vec<Focus> {
    let (first, second, third) = match active_tab {
        ActiveTab::Rgb => (Focus::Red, Focus::Green, Focus::Blue),
        ActiveTab::Hsv => (Focus::HsvHue, Focus::HsvSat, Focus::HsvVal),
    };

    let mut cycle = vec![
        Focus::Overlay,
        Focus::Ring,
        Focus::Square,
        first,
        second,
        third,
        Focus::Alpha,
        Focus::Hex,
    ];
    if naming_new_set {
        cycle.push(Focus::NewSetName);
    }
    cycle.extend([
        Focus::TabRgb,
        Focus::TabHsv,
        Focus::Swatches,
        Focus::Reset,
        Focus::Cancel,
        Focus::Submit,
    ]);
    cycle
}

/// Gets the next focusable element.
#[must_use]
fn next_focus(focus: Focus, active_tab: ActiveTab, naming_new_set: bool) -> Focus {
    let cycle = focus_cycle(active_tab, naming_new_set);
    let Some(position) = cycle.iter().position(|f| *f == focus) else {
        // Not part of the cycle (e.g. `None` or a channel focus of the
        // inactive tab): jump to the first element.
        return Focus::Overlay;
    };
    cycle[(position + 1) % cycle.len()]
}

/// Gets the previous focusable element.
#[must_use]
fn previous_focus(focus: Focus, active_tab: ActiveTab, naming_new_set: bool) -> Focus {
    let cycle = focus_cycle(active_tab, naming_new_set);
    let Some(position) = cycle.iter().position(|f| *f == focus) else {
        // Not part of the cycle: stay unfocused.
        return Focus::None;
    };
    cycle[(position + cycle.len() - 1) % cycle.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_cycle_rgb_forwards() {
        let mut focus = Focus::None;
        for expected in [
            Focus::Overlay,
            Focus::Ring,
            Focus::Square,
            Focus::Red,
            Focus::Green,
            Focus::Blue,
            Focus::Alpha,
            Focus::Hex,
            Focus::TabRgb,
            Focus::TabHsv,
            Focus::Swatches,
            Focus::Reset,
            Focus::Cancel,
            Focus::Submit,
        ] {
            focus = next_focus(focus, ActiveTab::Rgb, false);
            assert_eq!(focus, expected);
        }
        assert_eq!(next_focus(focus, ActiveTab::Rgb, false), Focus::Overlay);
    }

    #[test]
    fn focus_cycle_hsv_forwards() {
        let mut focus = Focus::None;
        for expected in [
            Focus::Overlay,
            Focus::Ring,
            Focus::Square,
            Focus::HsvHue,
            Focus::HsvSat,
            Focus::HsvVal,
            Focus::Alpha,
            Focus::Hex,
            Focus::TabRgb,
            Focus::TabHsv,
            Focus::Swatches,
            Focus::Reset,
            Focus::Cancel,
            Focus::Submit,
        ] {
            focus = next_focus(focus, ActiveTab::Hsv, false);
            assert_eq!(focus, expected);
        }
        assert_eq!(next_focus(focus, ActiveTab::Hsv, false), Focus::Overlay);
    }

    #[test]
    fn focus_cycle_rgb_backwards() {
        let mut focus = Focus::Overlay;
        for expected in [
            Focus::Submit,
            Focus::Cancel,
            Focus::Reset,
            Focus::Swatches,
            Focus::TabHsv,
            Focus::TabRgb,
            Focus::Hex,
            Focus::Alpha,
            Focus::Blue,
            Focus::Green,
            Focus::Red,
            Focus::Square,
            Focus::Ring,
        ] {
            focus = previous_focus(focus, ActiveTab::Rgb, false);
            assert_eq!(focus, expected);
        }
        assert_eq!(previous_focus(focus, ActiveTab::Rgb, false), Focus::Overlay);
    }

    #[test]
    fn focus_cycle_hsv_backwards() {
        let mut focus = Focus::Overlay;
        for expected in [
            Focus::Submit,
            Focus::Cancel,
            Focus::Reset,
            Focus::Swatches,
            Focus::TabHsv,
            Focus::TabRgb,
            Focus::Hex,
            Focus::Alpha,
            Focus::HsvVal,
            Focus::HsvSat,
            Focus::HsvHue,
            Focus::Square,
            Focus::Ring,
        ] {
            focus = previous_focus(focus, ActiveTab::Hsv, false);
            assert_eq!(focus, expected);
        }
        assert_eq!(previous_focus(focus, ActiveTab::Hsv, false), Focus::Overlay);
    }

    #[test]
    fn focus_cycle_naming_new_set() {
        assert_eq!(
            next_focus(Focus::Hex, ActiveTab::Rgb, true),
            Focus::NewSetName
        );
        assert_eq!(
            next_focus(Focus::NewSetName, ActiveTab::Rgb, true),
            Focus::TabRgb
        );
        assert_eq!(
            previous_focus(Focus::TabRgb, ActiveTab::Rgb, true),
            Focus::NewSetName
        );
        assert_eq!(
            previous_focus(Focus::NewSetName, ActiveTab::Rgb, true),
            Focus::Hex
        );
        assert_eq!(
            next_focus(Focus::Hex, ActiveTab::Rgb, false),
            Focus::TabRgb
        );
    }

    #[test]
    fn focus_cycle_stray_and_unfocused() {
        // A channel of the inactive tab is normalized: it is not part of
        // the cycle.
        assert_eq!(next_focus(Focus::Red, ActiveTab::Hsv, false), Focus::Overlay);
        assert_eq!(previous_focus(Focus::Red, ActiveTab::Hsv, false), Focus::None);
        // Unfocused elements enter/leave the cycle at its start.
        assert_eq!(next_focus(Focus::None, ActiveTab::Hsv, false), Focus::Overlay);
        assert_eq!(previous_focus(Focus::None, ActiveTab::Rgb, false), Focus::None);
    }

    #[test]
    fn value_inputs_filled_from_color() {
        // Default dialog color: 0.5 / 0.25 / 0.25 -> R=127, G=63, B=63.
        let inputs = value_inputs_from_color(Color::from_rgb(0.5, 0.25, 0.25));
        assert_eq!(inputs[0], "127");
        assert_eq!(inputs[1], "63");
        assert_eq!(inputs[2], "63");
        assert_eq!(inputs[3], "255");
        assert_eq!(inputs[4], "0");
        // Saturation = 0.5, value = 0.5 on the 0-255 scale.
        assert_eq!(inputs[5], "127");
        assert_eq!(inputs[6], "127");
    }

    #[test]
    fn insert_swatch_dedupes_front_and_truncates() {
        let red = Color::from_rgb8(255, 0, 0);
        let green = Color::from_rgb8(0, 255, 0);
        let blue = Color::from_rgb8(0, 0, 255);

        let mut colors = vec![red, green];
        // Byte-exact duplicate is removed and the color moves to the front.
        insert_swatch(&mut colors, red);
        assert_eq!(colors, vec![red, green]);
        // A color whose floats differ but whose RGBA bytes match is a duplicate:
// the old color is removed and the new instance inserted at the front.
        insert_swatch(&mut colors, Color { r: 0.001, ..green });
        assert_eq!(colors, vec![Color { r: 0.001, ..green }, red]);
        insert_swatch(&mut colors, blue);
        assert_eq!(colors, vec![blue, Color { r: 0.001, ..green }, red]);

        // Truncation at MAX_SWATCHES_PER_SET: the oldest entries fall off the
        // end. The list was [0..=26] oldest-first; after inserting the new
        // color at the front the tail [23..=26] is dropped.
        let mut many = (0..MAX_SWATCHES_PER_SET + 3)
            .map(|i| Color::from_rgb8(i as u8, 0, 0))
            .collect::<Vec<_>>();
        let new_color = Color::from_rgb8(200, 200, 200);
        insert_swatch(&mut many, new_color);
        assert_eq!(many.len(), MAX_SWATCHES_PER_SET);
        assert_eq!(many[0], new_color);
        assert_eq!(many[1], Color::from_rgb8(0, 0, 0));
        assert_eq!(many.last(), Some(&Color::from_rgb8(22, 0, 0)));
    }

    #[test]
    fn push_recent_dedupes_front_and_truncates() {
        let red = Color::from_rgb8(255, 0, 0);
        let green = Color::from_rgb8(0, 255, 0);

        let mut recent = Vec::new();
        push_recent(&mut recent, red);
        push_recent(&mut recent, green);
        assert_eq!(recent, vec![green, red]);
        // Re-submitting the same color only moves it to the front.
        push_recent(&mut recent, red);
        assert_eq!(recent, vec![red, green]);

        // Truncation at MAX_RECENT.
        let mut many = (0..MAX_RECENT + 2)
            .map(|i| Color::from_rgb8(i as u8, 0, 0))
            .collect::<Vec<_>>();
        push_recent(&mut many, Color::from_rgb8(9, 9, 9));
        assert_eq!(many.len(), MAX_RECENT);
        assert_eq!(many[0], Color::from_rgb8(9, 9, 9));
    }

    #[test]
    fn swatch_remove_index_refuses_last_real_tab() {
        assert_eq!(swatch_remove_index(1, 0), None);
        assert_eq!(swatch_remove_index(0, 0), None);
        // With two sets, the remaining active index is clamped back.
        assert_eq!(swatch_remove_index(2, 0), Some(0));
        assert_eq!(swatch_remove_index(2, 1), Some(0));
        assert_eq!(swatch_remove_index(3, 1), Some(1));
        // An out-of-range index (the "+" tab) is clamped to the last real tab.
        assert_eq!(swatch_remove_index(3, 5), Some(1));
    }

    #[test]
    fn grid_rows_counts() {
        assert_eq!(grid_rows(0, GRID_COLS), 0);
        assert_eq!(grid_rows(1, GRID_COLS), 1);
        assert_eq!(grid_rows(5, GRID_COLS), 1);
        assert_eq!(grid_rows(6, GRID_COLS), 2);
        assert_eq!(grid_rows(11, GRID_COLS), 3);
        assert_eq!(grid_rows(12, GRID_COLS), 3);
    }

    #[test]
    fn swatch_tab_bounds_stacked_plus_tab() {
        let sets = vec![
            SwatchSet { name: "A".to_owned(), colors: Vec::new() },
            SwatchSet { name: "BB".to_owned(), colors: Vec::new() },
        ];
        let bar = Rectangle {
            x: 10.0,
            y: 20.0,
            width: 200.0,
            height: 30.0,
        };
        let (tabs, plus) = swatch_tab_bounds(bar, &sets);
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].x, 10.0);
        assert_eq!(tabs[1].x, 10.0 + swatch_tab_width("A", true));
        assert_eq!(plus.x, tabs[1].x + swatch_tab_width("BB", true));
        assert_eq!(plus.width, 30.0);
        assert_eq!(tabs[0].y, 20.0);
        assert_eq!(tabs[0].height, 30.0);
    }
}