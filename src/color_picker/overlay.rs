//! The overlay of the [`ColorPicker`](crate::color_picker::ColorPicker).
//!
//! Ported from `iced_aw`'s `widget::overlay::color_picker` module.

use super::{
    color::{
        clamp_hue, clamp_u8, color_to_hex_argb, is_valid_hex, parse_hex_digits,
        Hsv,
    },
    dropper::{DropperBuffer, DropperMode, Frame},
    gradient::{Gradient, PickedValue, same_picked},
    style::{self, Status, Style},
    style_state::StyleState,
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

/// The maximal size of the dialog content (single column).
#[allow(dead_code)]
const DIALOG_MAX_SIZE: Size = Size::new(380.0, 700.0);
/// The fixed width of the single-column content.
const CONTENT_WIDTH: f32 = 300.0;
/// The height of the draggable window header of the
/// [`ColorPickerWindow`]. The header is an empty drag strip with a
/// close button on the right.
const HEADER_HEIGHT: f32 = 28.0;
/// The size of the square close ("x") button inside the window header.
const CLOSE_BUTTON_SIZE: f32 = 20.0;
/// The margin around the dialog content (Qt: contentsMargins 15).
#[allow(dead_code)]
const OUTER_MARGIN: f32 = 15.0;
/// The spacing between the left and right pane (Qt: main_h_layout spacing 15).
#[allow(dead_code)]
const PANE_SPACING: f32 = 15.0;
/// The outer dimension of the picker container (legacy two-pane width,
/// kept for the single-column content width).
#[allow(dead_code)]
const RING_DIM: f32 = 300.0;
/// The width of the hue ring band (legacy; the ring is now a slider).
#[allow(dead_code)]
const RING_WIDTH: f32 = 30.0;
/// The padding between the ring band and the ring border (legacy).
#[allow(dead_code)]
const RING_PADDING: f32 = 5.0;
/// The size of the saturation/value square: `int(230 * 0.65)`.
const SQUARE_DIM: f32 = 149.0;
/// The inner diameter of the hue ring (legacy).
const INNER_DIAMETER: f32 = 230.0;
/// Height of the hue slider placed below the S/V square.
const HUE_SLIDER_HEIGHT: f32 = 16.0;
/// Width of one top-level tab in the header bar.
const TOP_TAB_WIDTH: f32 = 62.0;
/// Height of one top-level tab in the header bar.
const TOP_TAB_HEIGHT: f32 = 20.0;
/// Height of the gradient stop bar (color strip only).
const GRADIENT_BAR_HEIGHT: f32 = 28.0;
/// Width/height of a gradient stop pin body. Pins sit on top of the bar,
/// Figma-style, with a pointer nub stabbed into the exact bar position.
const GRADIENT_PIN_WIDTH: f32 = 22.0;
const GRADIENT_PIN_HEIGHT: f32 = 20.0;
/// Gap between the pin body bottom and the bar top; the pointer bridges it.
const GRADIENT_PIN_GAP: f32 = 2.0;
/// Height of the pin row above the bar.
const GRADIENT_PIN_ROW: f32 = GRADIENT_PIN_HEIGHT + GRADIENT_PIN_GAP;
/// How deep into the bar the pointer apex reaches.
const GRADIENT_PIN_APEX_DEPTH: f32 = 8.0;
/// Half-width of the pointer base (base = half the pin body width).
const GRADIENT_PIN_POINTER_HALF: f32 = 5.5;
/// Total height of the gradient bar section (pins + bar).
const GRADIENT_BAR_TOTAL: f32 = GRADIENT_PIN_ROW + GRADIENT_BAR_HEIGHT;
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
#[allow(dead_code)]
const RIGHT_PANE_WIDTH: f32 = 230.0;
/// The maximum number of recent colors.
const MAX_RECENT: usize = 12;
/// The maximum number of swatches per set.
const MAX_SWATCHES_PER_SET: usize = 24;

/// Half-extent of the eye dropper magnifier source window: the lens samples
/// a `(2 * LENS_SRC_RADIUS + 1)²` pixel neighborhood around the hovered
/// pixel.
const LENS_SRC_RADIUS: i32 = 6;
/// The size of one zoomed source pixel inside the lens, in logical pixels.
const LENS_CELL: f32 = 12.0;
/// The gap between zoomed pixels; forms the pixel grid of the lens.
const LENS_GAP: f32 = 1.0;
/// Padding between the lens backdrop border and the pixel grid / pill.
const LENS_PAD: f32 = 5.0;
/// The height of the hex readout pill below the pixel grid.
const LENS_PILL_HEIGHT: f32 = 20.0;
/// The preferred gap between the cursor and the near corner of the lens.
const LENS_CURSOR_MARGIN: f32 = 18.0;

/// The active controls tab of the left pane (Qt `QTabWidget::currentTab`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    /// The RGB(A) channel tab.
    Rgb,
    /// The HSV channel tab.
    Hsv,
}

/// The top-level tab of the dialog: raw color picking or the swatch library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PickerTab {
    /// Raw color controls (square + hue slider + RGB/HSV + hex).
    #[default]
    Color,
    /// Two-stop gradient editor (stop bar + Rect/HSV/RGBA for the
    /// selected stop + hex).
    Gradient,
    /// Swatch sets + recent colors library.
    Library,
}

/// The editor shown below the gradient stop bar: the S/V square ("Rect")
/// or the channel sliders for the selected stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GradientEditorTab {
    /// Saturation/value square + hue slider for the selected stop.
    #[default]
    Rect,
    /// HSV channel sliders for the selected stop.
    Hsv,
    /// RGBA channel sliders for the selected stop.
    Rgba,
}

/// A named swatch set of the swatch tab bar.
#[derive(Debug, Clone)]
pub struct SwatchSet {
    /// The display name of the set.
    pub name: String,
    /// The picked values (solids or gradients) in the set.
    pub colors: Vec<PickedValue>,
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

/// The label and font for the eyedropper button of the overlay (replaces
/// the former cancel button; the floating window keeps its header "x" as a
/// cancel path).
///
/// NOTE: the original `iced_aw` implementation uses glyphs from its embedded
/// icon font (`font.ttf` via `iced_fonts`). We use plain text here so the
/// widget needs no custom font - this is a customization point.
fn dropper_icon() -> (&'static str, Font) {
    ("Eyedropper", Font::default())
}

/// The label and font for the submit button of the overlay.
///
/// NOTE: the original `iced_aw` implementation uses glyphs from its embedded
/// icon font (`font.ttf` via `iced_fonts`). We use plain text here so the
/// widget needs no custom font - this is a customization point.
fn ok_icon() -> (&'static str, Font) {
    ("OK", Font::default())
}

/// The glyph for the close ("x") button of the
/// [`ColorPickerWindow`] header (legacy text fallback; the header now draws
/// [`CANCEL_SVG`]).
#[allow(dead_code)]
fn close_symbol() -> &'static str {
    "\u{00D7}"
}

/// Lucide `pipette` icon for the eyedropper button.
const EYEDROPPER_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m2 22 1-1h3l9-9"/><path d="M3 21v-3l9-9"/><path d="m15 6 3.4-3.4a2.1 2.1 0 1 1 3 3L18 9l.4.4a2.1 2.1 0 1 1-3 3l-3.8-3.8a2.1 2.1 0 1 1 3-3l.4.4Z"/></svg>"#;

/// Lucide `x` icon for the header close button.
const CANCEL_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>"#;

/// Lucide `rotate-ccw` icon for the reset button.
const RESET_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/></svg>"#;

/// Size of the eyedropper button beside the hue slider.
const DROPPER_SIZE: f32 = 28.0;
/// Gap between the hue slider and the eyedropper button.
const HUE_DROPPER_GAP: f32 = 8.0;
/// Size of a centered SVG glyph inside an icon button.
const ICON_GLYPH_SIZE: f32 = 16.0;

/// Draws an SVG glyph centered inside `bounds`, tinted with `color`.
fn draw_svg_icon(renderer: &mut Renderer, svg: &'static [u8], bounds: Rectangle, clip: Rectangle, color: Color) {
    use iced::advanced::svg::Renderer as _;
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return;
    }
    let handle = iced::widget::svg::Handle::from_memory(svg);
    let size = ICON_GLYPH_SIZE
        .min(bounds.width - 4.0)
        .min(bounds.height - 4.0)
        .max(8.0);
    let icon = Rectangle {
        x: bounds.center_x() - size / 2.0,
        y: bounds.center_y() - size / 2.0,
        width: size,
        height: size,
    };
    renderer.draw_svg(
        iced::advanced::svg::Svg {
            handle,
            color: Some(color),
            rotation: iced::Radians(0.0),
            opacity: 1.0,
        },
        icon,
        clip,
    );
}

/// Centers a dialog of the given `size` over `position` and bounces it back
/// so it stays fully inside `bounds`.
fn centered_bounded_point(position: Point, size: Size, bounds: Size) -> Point {
    let x = (position.x - size.width / 2.0).max(0.0);
    let y = (position.y - size.height / 2.0).max(0.0);

    let x = if x + size.width > bounds.width {
        (bounds.width - size.width).max(0.0)
    } else {
        x
    };
    let y = if y + size.height > bounds.height {
        (bounds.height - size.height).max(0.0)
    } else {
        y
    };

    Point::new(x, y)
}

/// Linearly interpolates between two colors, clamping `t` to `0..=1`.
fn lerp(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

/// The rectangle of the close ("x") button within the window `header`.
fn close_button_rect(header: Rectangle) -> Rectangle {
    Rectangle {
        x: header.x + header.width - CLOSE_BUTTON_SIZE - 6.0,
        y: header.y + (header.height - CLOSE_BUTTON_SIZE) / 2.0,
        width: CLOSE_BUTTON_SIZE,
        height: CLOSE_BUTTON_SIZE,
    }
}

/// The rectangles of the top-level `[Color | Gradient | Library]` tabs
/// inside the window `header`. Tabs are left-aligned; the middle strip
/// stays draggable.
fn top_tab_rects(header: Rectangle) -> (Rectangle, Rectangle, Rectangle) {
    let y = header.y + (header.height - TOP_TAB_HEIGHT) / 2.0;
    let color = Rectangle {
        x: header.x + 6.0,
        y,
        width: TOP_TAB_WIDTH,
        height: TOP_TAB_HEIGHT,
    };
    let gradient = Rectangle {
        x: color.x + color.width + 6.0,
        y,
        width: TOP_TAB_WIDTH,
        height: TOP_TAB_HEIGHT,
    };
    let library = Rectangle {
        x: gradient.x + gradient.width + 6.0,
        y,
        width: TOP_TAB_WIDTH,
        height: TOP_TAB_HEIGHT,
    };
    (color, gradient, library)
}

/// The pin body rect of a stop for the given section rect (the whole
/// pins + bar area). The strip is inset by half a pin width on each side,
/// so the body sits in the pin row above the bar with the pointer apex
/// landing exactly on the strip's edge at offsets 0 and 1.
fn gradient_handle_rect(section: Rectangle, offset: f32) -> Rectangle {
    let strip = gradient_strip_rect(section);
    let cx = strip.x + strip.width * offset.clamp(0.0, 1.0);
    Rectangle {
        x: cx - GRADIENT_PIN_WIDTH / 2.0,
        y: section.y,
        width: GRADIENT_PIN_WIDTH,
        height: GRADIENT_PIN_HEIGHT,
    }
}

/// The pointer nub rect of a stop: from inside the pin body down to the
/// apex stabbed into the bar. Used for hit-testing the pointer.
fn gradient_pointer_rect(section: Rectangle, offset: f32) -> Rectangle {
    let body = gradient_handle_rect(section, offset);
    Rectangle {
        x: body.center_x() - GRADIENT_PIN_POINTER_HALF,
        y: body.y + body.height - 2.0,
        width: GRADIENT_PIN_POINTER_HALF * 2.0,
        height: GRADIENT_PIN_ROW + GRADIENT_PIN_APEX_DEPTH - body.height + 2.0,
    }
}

/// The color strip rect inside the section rect, inset by half a pin
/// width on each side so end stops point at the strip's edge.
fn gradient_strip_rect(section: Rectangle) -> Rectangle {
    Rectangle {
        x: section.x + GRADIENT_PIN_WIDTH / 2.0,
        y: section.y + GRADIENT_PIN_ROW,
        width: (section.width - GRADIENT_PIN_WIDTH).max(1.0),
        height: GRADIENT_BAR_HEIGHT,
    }
}

/// Returns true if a point (relative to the picker bounds origin) lies inside
/// the hue ring band (between the ring's inner and outer radius).
/// Legacy helper kept for the old two-pane layout fns below.
#[allow(dead_code)]
fn is_in_ring_band(position: Point, size: Size) -> bool {
    let dx = position.x - size.width / 2.0;
    let dy = position.y - size.height / 2.0;
    let dist = dx * dx + dy * dy;
    let inner = INNER_DIAMETER / 2.0;
    let outer = size.width.min(size.height) / 2.0;
    dist >= inner * inner && dist <= outer * outer
}

/// The pitch of the strip cells: cell size plus spacing (used for both
/// axes).
const CELL_PITCH: f32 = SWATCH_SIZE + GRID_SPACING;
/// The number of rows of the swatch/recent strips.
const STRIP_ROWS: usize = 3;

/// The number of columns that fit into a strip of the given width.
fn visible_cols(width: f32) -> usize {
    ((width - 2.0 * SWATCH_PAGE_MARGIN + GRID_SPACING) / CELL_PITCH)
        .floor()
        .max(1.0) as usize
}

/// The number of columns occupied by `count` cells flowing down
/// [`STRIP_ROWS`] rows; at least one viewport worth of columns.
fn strip_content_cols(count: usize, viewport_width: f32) -> usize {
    strip_content_cols_rows(count, viewport_width, STRIP_ROWS)
}

/// The number of columns occupied by `count` cells flowing down `rows`
/// rows; at least one viewport worth of columns.
fn strip_content_cols_rows(count: usize, viewport_width: f32, rows: usize) -> usize {
    count
        .max(rows * visible_cols(viewport_width))
        .div_ceil(rows.max(1))
}

/// The total width occupied by the columns of a strip with `count` cells.
fn strip_content_width(count: usize, viewport_width: f32) -> f32 {
    strip_content_width_rows(count, viewport_width, STRIP_ROWS)
}

/// The total width occupied by the columns of a strip with `count` cells
/// flowing down `rows` rows.
fn strip_content_width_rows(count: usize, viewport_width: f32, rows: usize) -> f32 {
    strip_content_cols_rows(count, viewport_width, rows) as f32 * CELL_PITCH - GRID_SPACING
}

/// The maximal scroll offset of a strip with `count` cells inside a
/// viewport of the given width.
fn strip_max_scroll(count: usize, viewport_width: f32) -> f32 {
    strip_max_scroll_rows(count, viewport_width, STRIP_ROWS)
}

/// The maximal scroll offset of a strip with `count` cells flowing down
/// `rows` rows inside a viewport of the given width.
fn strip_max_scroll_rows(count: usize, viewport_width: f32, rows: usize) -> f32 {
    (strip_content_width_rows(count, viewport_width, rows) + 2.0 * SWATCH_PAGE_MARGIN
        - viewport_width)
        .max(0.0)
}

/// Clamps a scroll offset against the content extent of a strip with
/// `count` cells inside a viewport of the given width.
fn clamp_strip_scroll(offset: f32, count: usize, viewport_width: f32) -> f32 {
    clamp_strip_scroll_rows(offset, count, viewport_width, STRIP_ROWS)
}

/// Clamps a scroll offset against the content extent of a strip with
/// `count` cells flowing down `rows` rows inside a viewport of the given
/// width.
fn clamp_strip_scroll_rows(offset: f32, count: usize, viewport_width: f32, rows: usize) -> f32 {
    offset.clamp(0.0, strip_max_scroll_rows(count, viewport_width, rows))
}

/// True if two colors have identical RGBA bytes.
pub(crate) fn same_rgba(a: Color, b: Color) -> bool {
    (a.r * 255.0) as u8 == (b.r * 255.0) as u8
        && (a.g * 255.0) as u8 == (b.g * 255.0) as u8
        && (a.b * 255.0) as u8 == (b.b * 255.0) as u8
        && (a.a * 255.0) as u8 == (b.a * 255.0) as u8
}

/// Inserts `picked` at the front of a swatch set: removes an existing
/// duplicate, then truncates to [`MAX_SWATCHES_PER_SET`].
fn insert_swatch(colors: &mut Vec<PickedValue>, picked: PickedValue) {
    colors.retain(|c| !same_picked(c, &picked));
    colors.insert(0, picked);
    colors.truncate(MAX_SWATCHES_PER_SET);
}

/// Inserts a picked value into the recent list: dedupe, insert front,
/// truncate to [`MAX_RECENT`].
fn push_recent(colors: &mut Vec<PickedValue>, picked: PickedValue) {
    colors.retain(|c| !same_picked(c, &picked));
    colors.insert(0, picked);
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
/// `(name input, Add button, Cancel button)`. The controls form a
/// horizontal band centered inside the page so the rest of the fixed
/// strip height stays empty.
fn name_prompt_rects(page: Rectangle) -> (Rectangle, Rectangle, Rectangle) {
    let button_width = 48.0;
    let gap = 8.0;
    let y = page.y + (page.height - NAME_PROMPT_HEIGHT) / 2.0;
    let cancel = Rectangle {
        x: page.x + page.width - button_width,
        y,
        width: button_width,
        height: NAME_PROMPT_HEIGHT,
    };
    let add = Rectangle {
        x: cancel.x - gap - button_width,
        y,
        width: button_width,
        height: NAME_PROMPT_HEIGHT,
    };
    let input = Rectangle {
        x: page.x,
        y,
        width: add.x - gap - page.x,
        height: NAME_PROMPT_HEIGHT,
    };
    (input, add, cancel)
}

/// The dialog content of the [`ColorPicker`](crate::color_picker::ColorPicker).
///
/// This is the shared core view used by both public entry points: it is
/// planted as a regular widget by the inline [`ColorPicker`] and hosted
/// inside a draggable window shell by
/// [`FloatingColorPicker`](crate::color_picker::FloatingColorPicker).
#[allow(missing_debug_implementations)]
pub struct ColorPickerOverlay<'a, 'b, Message, Theme>
where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog + iced::widget::text_input::Catalog,
    'b: 'a,
{
    /// The state of the [`ColorPickerOverlay`].
    state: &'a mut State,
    /// The eyedropper button of the [`ColorPickerOverlay`]. Replaces the
    /// former cancel button; the floating window keeps its header "x" as a
    /// cancel path.
    dropper_button: Button<'a, Message, Theme, Renderer>,
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
    /// Optional function producing a message with the gradient when the
    /// submit button is pressed while the Gradient tab is active.
    on_gradient_submit: Option<&'a dyn Fn(Gradient) -> Message>,
    /// Optional function producing a message when the gradient changes
    /// during selection (real-time updates).
    on_gradient_change: Option<&'a dyn Fn(Gradient) -> Message>,
    /// Optional unified change callback with the picked value (solid or
    /// gradient) for the active tab.
    on_pick: Option<&'a dyn Fn(PickedValue) -> Message>,
    /// Optional unified submit callback with the picked value (solid or
    /// gradient) for the active tab.
    on_pick_submit: Option<&'a dyn Fn(PickedValue) -> Message>,
    /// The shared buffer where the application deposits window screenshots
    /// for the eye dropper. The eyedropper button is disabled while this is
    /// `None`.
    dropper_buffer: Option<&'a DropperBuffer>,
    /// Optional function producing the message published when the user
    /// activates the eye dropper and a fresh capture is needed.
    on_dropper_capture: Option<&'a dyn Fn() -> Message>,
    /// Whether the magnifier lens is drawn by this content view (`true`
    /// for the floating window shell) or hosted in the inline widget's
    /// full-window [`DropperLens`] overlay, which escapes ancestor clipping.
    lens_in_content_draw: bool,
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
    /// Creates a new [`ColorPickerOverlay`] dialog content view.
    ///
    /// The content is laid out relative to its host; positioning is the
    /// responsibility of whoever plants it (the inline widget or the
    /// [`ColorPickerWindow`] shell).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        state: &'a mut State,
        on_cancel: Message,
        on_submit: &'a dyn Fn(Color) -> Message,
        on_color_change: Option<&'a dyn Fn(Color) -> Message>,
        on_gradient_submit: Option<&'a dyn Fn(Gradient) -> Message>,
        on_gradient_change: Option<&'a dyn Fn(Gradient) -> Message>,
        on_pick: Option<&'a dyn Fn(PickedValue) -> Message>,
        on_pick_submit: Option<&'a dyn Fn(PickedValue) -> Message>,
        dropper_buffer: Option<&'a DropperBuffer>,
        on_dropper_capture: Option<&'a dyn Fn() -> Message>,
        lens_in_content_draw: bool,
        class: &'a <Theme as style::Catalog>::Class<'b>,
        tree: &'a mut Tree,
        viewport: Rectangle,
    ) -> Self
    where
        for<'c> <Theme as iced::widget::text_input::Catalog>::Class<'c>:
            From<iced::widget::text_input::StyleFn<'c, Theme>>,
    {
        let (dropper_content, dropper_font) = dropper_icon();
        let (submit_content, submit_font) = ok_icon();

        let state_ptr: *mut State = state;
        let hex_fake = on_cancel.clone();
        let hex_input = TextInput::new("", unsafe { &(*state_ptr).hex_input })
            .padding([2, 6])
            .size(13)
            .style(style::hex_text_input)
            .on_input(move |text: String| {
                unsafe { (*state_ptr).hex_input = text; }
                hex_fake.clone()
            });
        let mut value_inputs = std::array::from_fn(|_| {
            TextInput::new("", "")
                .padding([3, 4])
                .size(13)
                .style(style::text_input)
        });
        for (i, text_input) in value_inputs.iter_mut().enumerate() {
            let slot: *mut String = unsafe { &mut (*state_ptr).value_inputs[i] };
            let fake = on_cancel.clone();
            *text_input = TextInput::new("", unsafe { &*slot })
                .padding([3, 4])
                .size(13)
                .width(Length::Fixed(VALUE_WIDTH))
                .style(style::text_input)
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
            .style(style::text_input)
            .on_input({
                let on_input_fake = on_cancel.clone();
                move |text: String| {
                    unsafe { *name_slot = text; }
                    on_input_fake.clone()
                }
            })
            .on_submit(name_fake);

        ColorPickerOverlay {
            state,
            // The eyedropper button publishes a fake message (intercepted
            // below, submit_button pattern) that triggers the capture
            // round-trip. Without a wired buffer the button stays disabled.
            dropper_button: Button::new(
                widget::Text::new(dropper_content)
                    .align_x(Horizontal::Center)
                    .width(Length::Fill)
                    .font(dropper_font),
            )
            .width(Length::Fill)
            .on_press_maybe(
                dropper_buffer
                    .is_some()
                    .then_some(on_cancel.clone()),
            ),
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
            on_gradient_submit,
            on_gradient_change,
            on_pick,
            on_pick_submit,
            dropper_buffer,
            on_dropper_capture,
            lens_in_content_draw,
            class,
            tree,
            viewport,
        }
    }

    /// Publishes color + gradient + unified pick change messages.
    fn notify_changed(&self, shell: &mut Shell<Message>) {
        if let Some(on_color_change) = self.on_color_change {
            shell.publish(on_color_change(self.state.color));
        }
        if self.state.picker_tab == PickerTab::Gradient
            && let Some(on_gradient_change) = self.on_gradient_change
        {
            shell.publish(on_gradient_change(self.state.gradient.clone()));
        }
        if let Some(on_pick) = self.on_pick {
            shell.publish(on_pick(self.state.current_picked()));
        }
    }

    /// Switches the gradient editor tab and refreshes the field values.
    fn set_gradient_editor(&mut self, tab: GradientEditorTab, shell: &mut Shell<Message>) {
        if self.state.gradient_editor != tab {
            self.state.gradient_editor = tab;
            self.state.sync_display();
            self.state.clear_cache();
            shell.invalidate_layout();
        }
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

        // NOTE: the layout may be stale for this frame (see
        // `on_event_sliders`); bail out instead of panicking.
        let Some(sat_value_layout) = hsv_color_children.next() else {
            return event::Status::Ignored;
        };
        let sat_value_bounds = sat_value_layout.bounds();
        let Some(hue_layout) = hsv_color_children.next() else {
            return event::Status::Ignored;
        };
        let hue_bounds = hue_layout.bounds();

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
                if cursor.is_over(hue_bounds) {
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
            let t = ((cursor_position.x - hue_bounds.x) / hue_bounds.width.max(1.0)).clamp(0.0, 1.0);
            (t * 360.0).round() as u16 % 360
        };

        match self.state.color_bar_dragged {
            ColorBarDragged::SatValue => {
                // S/V-only change: keep the remembered hue instead of
                // re-deriving it from RGB, so float truncation in the
                // HSV->RGB->HSV round-trip cannot drift the hue (e.g.
                // towards red) while dragging inside the square.
                let hue = self.state.hue;
                self.state.apply_color(Color {
                    a: self.state.color.a,
                    ..Hsv {
                        hue,
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
            // Real-time updates for the square/hue editor.
            self.notify_changed(shell);
            event::Status::Captured
        } else {
            event::Status::Ignored
        }
    }

    /// The event handling for the gradient stop bar: click selects the
    /// nearest stop, drag moves it along the bar.
    fn on_event_gradient_bar(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        shell: &mut Shell<Message>,
    ) -> event::Status {
        // The Gradient picker node has a single child: the bar container.
        let mut picker_children = layout.children();
        let Some(bar_layout) = picker_children.next() else {
            return event::Status::Ignored;
        };
        let mut bar_children = bar_layout.children();
        let Some(strip_layout) = bar_children.next() else {
            return event::Status::Ignored;
        };
        let strip = strip_layout.bounds();
        if strip.width <= 0.0 || strip.height <= 0.0 {
            return event::Status::Ignored;
        }
        let section = bar_layout.bounds();
        // Pins are computed mathematically so selection works even when
        // the layout children are stale: body + pointer nub each.
        let pins: Vec<(Rectangle, Rectangle)> = (0..2)
            .map(|i| {
                let offset = self.state.gradient.stop(i).map_or(i as f32, |s| s.offset);
                (
                    gradient_handle_rect(section, offset),
                    gradient_pointer_rect(section, offset),
                )
            })
            .collect();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                let mut hit: Option<usize> = None;
                for (i, (handle, pointer)) in pins.iter().enumerate() {
                    if cursor.is_over(*handle) || cursor.is_over(*pointer) {
                        hit = Some(i);
                        break;
                    }
                }
                if hit.is_none() && cursor.is_over(strip) {
                    // Select the nearest stop to the click.
                    if let Some(pos) = cursor.land().position() {
                        let t = ((pos.x - strip.x) / strip.width).clamp(0.0, 1.0);
                        let d0 = (self
                            .state
                            .gradient
                            .stop(0)
                            .map_or(0.0, |s| s.offset)
                            - t)
                            .abs();
                        let d1 = (self
                            .state
                            .gradient
                            .stop(1)
                            .map_or(1.0, |s| s.offset)
                            - t)
                            .abs();
                        hit = Some(if d0 <= d1 { 0 } else { 1 });
                    }
                }
                if let Some(idx) = hit {
                    if self.state.selected_stop != idx {
                        self.state.select_stop(idx);
                    }
                    self.state.gradient_bar_dragged = Some(idx);
                    self.state.focus = Focus::GradientBar;
                    // Jump the dragged stop to the click position on the strip.
                    if cursor.is_over(strip)
                        && let Some(pos) = cursor.land().position()
                    {
                        let t = ((pos.x - strip.x) / strip.width).clamp(0.0, 1.0);
                        self.state.gradient.set_stop_offset(idx, t);
                        self.state.clear_cache();
                    }
                    self.notify_changed(shell);
                    return event::Status::Captured;
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. }) => {
                if self.state.gradient_bar_dragged.is_some() {
                    self.state.gradient_bar_dragged = None;
                    return event::Status::Captured;
                }
            }
            _ => {}
        }

        if let Some(idx) = self.state.gradient_bar_dragged {
            if let Some(pos) = cursor.land().position() {
                let t = ((pos.x - strip.x) / strip.width).clamp(0.0, 1.0);
                self.state.gradient.set_stop_offset(idx, t);
                self.state.clear_cache();
                self.notify_changed(shell);
                return event::Status::Captured;
            }
            return event::Status::Captured;
        }

        event::Status::Ignored
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

        // NOTE: the layout may be stale for this frame: switching the
        // editor sub-tab invalidates the layout but event handling continues
        // with the previous tree (e.g. Rect has no slider rows). Bail out
        // instead of panicking; the fresh layout arrives next frame.
        let mut row_bounds = Vec::new();
        for _ in 0..4 {
            let Some(row_layout) = slider_children.next() else {
                return event::Status::Ignored;
            };
            let mut row_children = row_layout.children();
            let _ = row_children.next();
            let Some(bar_layout) = row_children.next() else {
                return event::Status::Ignored;
            };
            row_bounds.push(bar_layout.bounds());
        }

        let channels = if self.state.picker_tab == PickerTab::Gradient {
            self.state.gradient_channels()
        } else {
            self.active_tab_channels()
        };

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
                        (calc_percentage(row_bounds[0], position) * 360.0).round() as u16 % 360
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
                let hue = self.state.hue;
                self.state.apply_color(Color {
                    a: self.state.color.a,
                    ..Hsv {
                        hue,
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
                let hue = self.state.hue;
                self.state.apply_color(Color {
                    a: self.state.color.a,
                    ..Hsv {
                        hue,
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
            // Real-time updates for the channel sliders.
            self.notify_changed(shell);
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
                        self.state.picker_tab,
                        self.state.active_tab,
                        self.state.gradient_editor,
                        self.state.naming_new_set,
                    );
                } else {
                    self.state.focus = next_focus(
                        self.state.focus,
                        self.state.picker_tab,
                        self.state.active_tab,
                        self.state.gradient_editor,
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
                    Focus::TopColor => {
                        status = match key {
                            keyboard::Key::Named(
                                keyboard::key::Named::Enter | keyboard::key::Named::Space,
                            ) => {
                                self.set_picker_tab(PickerTab::Color, shell);
                                event::Status::Captured
                            }
                            keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                                self.state.focus = Focus::TopGradient;
                                event::Status::Captured
                            }
                            _ => event::Status::Ignored,
                        };
                    }
                    Focus::TopGradient => {
                        status = match key {
                            keyboard::Key::Named(
                                keyboard::key::Named::Enter | keyboard::key::Named::Space,
                            ) => {
                                self.set_picker_tab(PickerTab::Gradient, shell);
                                event::Status::Captured
                            }
                            keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                                self.state.focus = Focus::TopColor;
                                event::Status::Captured
                            }
                            keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                                self.state.focus = Focus::TopLibrary;
                                event::Status::Captured
                            }
                            _ => event::Status::Ignored,
                        };
                    }
                    Focus::TopLibrary => {
                        status = match key {
                            keyboard::Key::Named(
                                keyboard::key::Named::Enter | keyboard::key::Named::Space,
                            ) => {
                                self.set_picker_tab(PickerTab::Library, shell);
                                event::Status::Captured
                            }
                            keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                                self.state.focus = Focus::TopGradient;
                                event::Status::Captured
                            }
                            _ => event::Status::Ignored,
                        };
                    }
                    Focus::GradientBar => {
                        status = match key {
                            keyboard::Key::Named(
                                keyboard::key::Named::ArrowLeft
                                | keyboard::key::Named::ArrowDown,
                            ) => {
                                let idx = self.state.selected_stop;
                                let cur = self
                                    .state
                                    .gradient
                                    .stop(idx)
                                    .map_or(0.0, |s| s.offset);
                                self.state.gradient.set_stop_offset(idx, cur - 0.01);
                                self.state.clear_cache();
                                event::Status::Captured
                            }
                            keyboard::Key::Named(
                                keyboard::key::Named::ArrowRight
                                | keyboard::key::Named::ArrowUp,
                            ) => {
                                let idx = self.state.selected_stop;
                                let cur = self
                                    .state
                                    .gradient
                                    .stop(idx)
                                    .map_or(0.0, |s| s.offset);
                                self.state.gradient.set_stop_offset(idx, cur + 0.01);
                                self.state.clear_cache();
                                event::Status::Captured
                            }
                            keyboard::Key::Named(keyboard::key::Named::Tab) => {
                                event::Status::Ignored
                            }
                            _ => {
                                if matches!(
                                    key,
                                    keyboard::Key::Named(
                                        keyboard::key::Named::Enter | keyboard::key::Named::Space
                                    )
                                ) {
                                    let next = (self.state.selected_stop + 1) % 2;
                                    self.state.select_stop(next);
                                    event::Status::Captured
                                } else {
                                    event::Status::Ignored
                                }
                            }
                        };
                    }
                    Focus::Square => {
                        let mut hsv = self.state.hsv();
                        hsv.hue = self.state.hue;
                        status = sat_value_handle(key, &mut self.state.color, hsv);
                    }
                    Focus::Ring => {
                        let hsv = self.state.hsv();
                        status = hue_handle(key, &mut self.state.color, hsv, &mut self.state.hue);
                    }
                    Focus::Red => {
                        status = rgba_bar_handle(key, &mut self.state.color.r);
                        let hsv: Hsv = self.state.color.into();
                        if hsv.saturation > 0.001 && hsv.value > 0.001 {
                            self.state.hue = hsv.hue;
                        }
                    }
                    Focus::Green => {
                        status = rgba_bar_handle(key, &mut self.state.color.g);
                        let hsv: Hsv = self.state.color.into();
                        if hsv.saturation > 0.001 && hsv.value > 0.001 {
                            self.state.hue = hsv.hue;
                        }
                    }
                    Focus::Blue => {
                        status = rgba_bar_handle(key, &mut self.state.color.b);
                        let hsv: Hsv = self.state.color.into();
                        if hsv.saturation > 0.001 && hsv.value > 0.001 {
                            self.state.hue = hsv.hue;
                        }
                    }
                    Focus::Alpha => status = rgba_bar_handle(key, &mut self.state.color.a),
                    Focus::HsvHue => {
                        let hsv = self.state.hsv();
                        status = hue_handle(key, &mut self.state.color, hsv, &mut self.state.hue);
                    }
                    Focus::HsvSat => {
                        let mut hsv = self.state.hsv();
                        hsv.hue = self.state.hue;
                        status = hsv_sat_handle(key, &mut self.state.color, hsv);
                    }
                    Focus::HsvVal => {
                        let mut hsv = self.state.hsv();
                        hsv.hue = self.state.hue;
                        status = hsv_val_handle(key, &mut self.state.color, hsv);
                    }
                    Focus::TabRect => {
                        status = match key {
                            keyboard::Key::Named(
                                keyboard::key::Named::Enter | keyboard::key::Named::Space,
                            ) => {
                                if self.state.picker_tab == PickerTab::Gradient {
                                    self.set_gradient_editor(GradientEditorTab::Rect, shell);
                                }
                                event::Status::Captured
                            }
                            keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                                self.state.focus = Focus::TabHsv;
                                event::Status::Captured
                            }
                            _ => event::Status::Ignored,
                        };
                    }
                    Focus::TabRgb => {
                        status = match key {
                            keyboard::Key::Named(
                                keyboard::key::Named::Enter | keyboard::key::Named::Space,
                            ) => {
                                if self.state.picker_tab == PickerTab::Gradient {
                                    self.set_gradient_editor(GradientEditorTab::Rgba, shell);
                                } else {
                                    self.set_active_tab(ActiveTab::Rgb, shell);
                                }
                                event::Status::Captured
                            }
                            keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
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
                                if self.state.picker_tab == PickerTab::Gradient {
                                    self.set_gradient_editor(GradientEditorTab::Hsv, shell);
                                } else {
                                    self.set_active_tab(ActiveTab::Hsv, shell);
                                }
                                event::Status::Captured
                            }
                            keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                                if self.state.picker_tab == PickerTab::Gradient {
                                    self.state.focus = Focus::TabRect;
                                }
                                event::Status::Captured
                            }
                            keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
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
                                self.state.reset_to_initial();
                                event::Status::Captured
                            }
                            _ => event::Status::Ignored,
                        };
                    }
                    Focus::Dropper => {
                        status = match key {
                            keyboard::Key::Named(
                                keyboard::key::Named::Enter | keyboard::key::Named::Space,
                            ) => {
                                if self.request_dropper_capture(shell) {
                                    event::Status::Captured
                                } else {
                                    event::Status::Ignored
                                }
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
                                if let Some(picked) = self
                                    .state
                                    .swatch_sets
                                    .get(self.state.active_swatch_tab)
                                    .and_then(|set| set.colors.get(idx))
                                {
                                    self.select_picked_from_swatch(picked.clone(), shell);
                                    event::Status::Captured
                                } else {
                                    event::Status::Ignored
                                }
                            }
                            // Cells flow down [`STRIP_ROWS`] rows: up/down
                            // move within a column, left/right across
                            // columns.
                            keyboard::Key::Named(
                                keyboard::key::Named::ArrowLeft
                                | keyboard::key::Named::ArrowRight
                                | keyboard::key::Named::ArrowUp
                                | keyboard::key::Named::ArrowDown,
                            ) => {
                                if set_len > 0 {
                                    let delta = match key {
                                        keyboard::Key::Named(
                                            keyboard::key::Named::ArrowLeft,
                                        ) => -(STRIP_ROWS as i32),
                                        keyboard::Key::Named(
                                            keyboard::key::Named::ArrowRight,
                                        ) => STRIP_ROWS as i32,
                                        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => -1,
                                        _ => 1,
                                    };
                                    idx = (idx as i32 + delta).clamp(0, set_len as i32 - 1) as usize;
                                    self.state.focused_swatch =
                                        Some((self.state.active_swatch_tab, idx));

                                    // Scroll the focused cell's column fully
                                    // into view.
                                    let start = (idx / STRIP_ROWS) as f32 * CELL_PITCH
                                        - self.state.swatch_scroll_x;
                                    let end = start + SWATCH_SIZE;
                                    let view = CONTENT_WIDTH - 2.0 * SWATCH_PAGE_MARGIN;
                                    if start < 0.0 {
                                        self.state.swatch_scroll_x += start;
                                    } else if end > view {
                                        self.state.swatch_scroll_x += end - view;
                                    }
                                    self.state.swatch_scroll_x = clamp_strip_scroll(
                                        self.state.swatch_scroll_x,
                                        set_len,
                                        CONTENT_WIDTH,
                                    );
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

                // If color changed via keyboard, call change callbacks.
                // Tab navigation / stop selection also publishes so live
                // previews stay in sync.
                if status == event::Status::Captured {
                    self.notify_changed(shell);
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
    /// `[H,S,V,A]`.
    fn active_tab_channels(&self) -> Vec<usize> {
        match self.state.active_tab {
            ActiveTab::Rgb => vec![0, 1, 2, 3],
            ActiveTab::Hsv => vec![4, 5, 6, 3],
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
                    hsv.hue = self.state.hue;
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
                    hsv.hue = self.state.hue;
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

    /// Switches the top-level tab (color vs. gradient vs. library).
    fn set_picker_tab(&mut self, tab: PickerTab, shell: &mut Shell<Message>) {
        if self.state.picker_tab != tab {
            self.state.picker_tab = tab;
            if tab == PickerTab::Gradient {
                // Load the selected stop into the shared editor.
                self.state.select_stop(self.state.selected_stop);
            }
            self.state.clear_cache();
            shell.invalidate_layout();
        }
    }

    /// Applies a picked value (solid or gradient) from a swatch or recent
    /// cell and publishes change callbacks.
    fn select_picked_from_swatch(&mut self, picked: PickedValue, shell: &mut Shell<Message>) {
        self.state.apply_picked(picked);
        self.state.sync_display();
        self.state.clear_cache();
        self.notify_changed(shell);
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

            // Strip cells of the active set; scrolled-out parts are not
            // clickable. `position_in` is relative, so hit-test with the
            // absolute `is_over` plus a viewport intersection guard.
            let page_bounds = page_layout.bounds();
            for (i, cell) in page_layout.children().enumerate() {
                let cell_bounds = cell.bounds();
                if cursor.is_over(cell_bounds)
                    && page_bounds.intersects(&cell_bounds)
                    && let Some(picked) = self
                        .state
                        .swatch_sets
                        .get(self.state.active_swatch_tab)
                        .and_then(|set| set.colors.get(i))
                {
                    self.select_picked_from_swatch(picked.clone(), shell);
                    return true;
                }
            }

            // The add-current-value button: stores the current tab's value
            // so gradients are kept as gradients, not flattened to solid.
            if cursor.is_over(add_btn_layout.bounds()) {
                let picked = self.state.current_picked();
                let set_idx = self.state.active_swatch_tab;
                if let Some(set) = self.state.swatch_sets.get_mut(set_idx) {
                    insert_swatch(&mut set.colors, picked);
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

impl<'a, Message, Theme> ColorPickerOverlay<'a, '_, Message, Theme>
where
    Message: 'static + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    /// Lays out the dialog content at the origin of the available `bounds`.
    ///
    /// Positioning is left to the caller: the inline widget lets it flow in
    /// the layout tree, while the [`ColorPickerWindow`] shell resolves its
    /// position strategy and applies the user drag offset.
    pub(crate) fn layout_content(&mut self, renderer: &Renderer, _bounds: Size) -> Node {
        let width = CONTENT_WIDTH;
        let spacing = CONTROLS_SPACING;
        let is_color = self.state.picker_tab == PickerTab::Color;
        let is_gradient = self.state.picker_tab == PickerTab::Gradient;
        let is_library = self.state.picker_tab == PickerTab::Library;
        let is_editor = is_color || is_gradient;
        // Inline widgets draw the top tabs in content; the floating window
        // hosts them in its draggable header instead.
        let show_top_tabs = !self.lens_in_content_draw;

        let mut children: Vec<Node> = Vec::new();
        let mut offset_y = 0.0;
        let push = |node: Node, children: &mut Vec<Node>, offset_y: &mut f32| {
            let h = node.size().height;
            children.push(node.move_to(Point::new(0.0, *offset_y)));
            if h > 0.0 {
                *offset_y += h + spacing;
            }
        };

        // [0] Top tabs (inline only).
        {
            let h = if show_top_tabs { TAB_BAR_HEIGHT } else { 0.0 };
            let node = if h > 0.0 {
                Row::<(), Theme, Renderer>::new()
                    .width(Length::Fixed(width))
                    .height(Length::Fixed(h))
                    .layout(
                        self.tree,
                        renderer,
                        &Limits::new(Size::ZERO, Size::new(width, h)),
                    )
            } else {
                Node::with_children(Size::new(width, 0.0), Vec::new())
            };
            push(node, &mut children, &mut offset_y);
        }

        // [1] Original/New previews on top (always).
        {
            let node = Node::with_children(Size::new(width, PREVIEW_AREA_HEIGHT), Vec::new());
            push(node, &mut children, &mut offset_y);
        }

        // [2] Picker: S/V square (centered) + hue slider with eyedropper
        // button on its right (Color tab: children [square, hue,
        // dropper]), or the gradient stop bar (Gradient tab: children
        // [bar] with [strip, handle0, handle1]). The Gradient Rect square
        // lives in the controls slot [4], below the editor tabs.
        {
            if is_color {
                let square_limits = Limits::new(Size::ZERO, Size::new(SQUARE_DIM, SQUARE_DIM));
                let square_node = Row::<(), Theme, Renderer>::new()
                    .width(Length::Fixed(SQUARE_DIM))
                    .height(Length::Fixed(SQUARE_DIM))
                    .layout(self.tree, renderer, &square_limits)
                    .move_to(Point::new((width - SQUARE_DIM) / 2.0, 0.0));
                let group_width = SQUARE_DIM + HUE_DROPPER_GAP + DROPPER_SIZE;
                let group_x = (width - group_width) / 2.0;
                let row_y = SQUARE_DIM + 8.0;
                let row_height = HUE_SLIDER_HEIGHT.max(DROPPER_SIZE);
                let hue_node = Node::with_children(
                    Size::new(SQUARE_DIM, HUE_SLIDER_HEIGHT),
                    Vec::new(),
                )
                .move_to(Point::new(group_x, row_y + (row_height - HUE_SLIDER_HEIGHT) / 2.0));
                let dropper_node = self
                    .dropper_button
                    .layout(
                        &mut self.tree.children[0],
                        renderer,
                        &Limits::new(Size::ZERO, Size::new(DROPPER_SIZE, DROPPER_SIZE)),
                    )
                    .move_to(Point::new(
                        group_x + SQUARE_DIM + HUE_DROPPER_GAP,
                        row_y + (row_height - DROPPER_SIZE) / 2.0,
                    ));
                let picker_node = Node::with_children(
                    Size::new(width, SQUARE_DIM + 8.0 + row_height),
                    vec![square_node, hue_node, dropper_node],
                );
                push(picker_node, &mut children, &mut offset_y);
            } else if is_gradient {
                let section = Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width,
                    height: GRADIENT_BAR_TOTAL,
                };
                let strip_rect = gradient_strip_rect(section);
                let strip = Node::with_children(strip_rect.size(), Vec::new())
                    .move_to(strip_rect.position());
                let mut bar_children = vec![strip];
                for i in 0..2 {
                    let offset = self
                        .state
                        .gradient
                        .stop(i)
                        .map_or(i as f32, |s| s.offset);
                    let hr = gradient_handle_rect(section, offset);
                    bar_children.push(
                        Node::with_children(Size::new(hr.width, hr.height), Vec::new())
                            .move_to(hr.position()),
                    );
                }
                let bar_node =
                    Node::with_children(Size::new(width, GRADIENT_BAR_TOTAL), bar_children);
                push(
                    Node::with_children(Size::new(width, GRADIENT_BAR_TOTAL), vec![bar_node]),
                    &mut children,
                    &mut offset_y,
                );
            } else {
                push(
                    Node::with_children(Size::new(width, 0.0), Vec::new()),
                    &mut children,
                    &mut offset_y,
                );
            }
        }

        // [2] RGB/HSV sub tab bar (Color + Gradient).
        {
            let h = if is_editor { TAB_BAR_HEIGHT } else { 0.0 };
            let node = if h > 0.0 {
                Row::<(), Theme, Renderer>::new()
                    .width(Length::Fill)
                    .height(Length::Fixed(h))
                    .layout(self.tree, renderer, &Limits::new(Size::ZERO, Size::new(width, h)))
            } else {
                Node::with_children(Size::new(width, 0.0), Vec::new())
            };
            push(node, &mut children, &mut offset_y);
        }

        // [4] Slider controls (Color always; Gradient for Hsv/Rgba) or
        // the S/V square + hue slider with eyedropper (Gradient + Rect,
        // below the editor tabs; children [square, hue, dropper]).
        {
            let show_sliders = is_color
                || (is_gradient && self.state.gradient_editor != GradientEditorTab::Rect);
            let show_gradient_square =
                is_gradient && self.state.gradient_editor == GradientEditorTab::Rect;
            if show_gradient_square {
                let square_limits = Limits::new(Size::ZERO, Size::new(SQUARE_DIM, SQUARE_DIM));
                let square_node = Row::<(), Theme, Renderer>::new()
                    .width(Length::Fixed(SQUARE_DIM))
                    .height(Length::Fixed(SQUARE_DIM))
                    .layout(self.tree, renderer, &square_limits)
                    .move_to(Point::new((width - SQUARE_DIM) / 2.0, 0.0));
                let group_width = SQUARE_DIM + HUE_DROPPER_GAP + DROPPER_SIZE;
                let group_x = (width - group_width) / 2.0;
                let row_y = SQUARE_DIM + 8.0;
                let row_height = HUE_SLIDER_HEIGHT.max(DROPPER_SIZE);
                let hue_node = Node::with_children(
                    Size::new(SQUARE_DIM, HUE_SLIDER_HEIGHT),
                    Vec::new(),
                )
                .move_to(Point::new(group_x, row_y + (row_height - HUE_SLIDER_HEIGHT) / 2.0));
                // NOTE: the button is always really laid out so the node
                // keeps valid button children: `Button::update` unwraps its
                // content layout.
                let dropper_node = self
                    .dropper_button
                    .layout(
                        &mut self.tree.children[0],
                        renderer,
                        &Limits::new(Size::ZERO, Size::new(DROPPER_SIZE, DROPPER_SIZE)),
                    )
                    .move_to(Point::new(
                        group_x + SQUARE_DIM + HUE_DROPPER_GAP,
                        row_y + (row_height - DROPPER_SIZE) / 2.0,
                    ));
                push(
                    Node::with_children(
                        Size::new(width, SQUARE_DIM + 8.0 + row_height),
                        vec![square_node, hue_node, dropper_node],
                    ),
                    &mut children,
                    &mut offset_y,
                );
            } else if show_sliders {
                let controls_height = 4.0 * SLIDER_HEIGHT + 3.0 * ROW_SPACING;
                let groove_width = width - LABEL_WIDTH - VALUE_WIDTH;
                let mut controls_children = Vec::new();
                for row in 0..4 {
                    let y = row as f32 * (SLIDER_HEIGHT + ROW_SPACING);
                    let label_node =
                        Node::with_children(Size::new(LABEL_WIDTH, SLIDER_HEIGHT), Vec::new())
                            .move_to(Point::new(0.0, y));
                    let groove_node =
                        Node::with_children(Size::new(groove_width, SLIDER_HEIGHT), Vec::new())
                            .move_to(Point::new(LABEL_WIDTH, y));
                    let value_input_index = if is_gradient {
                        match (self.state.gradient_editor, row) {
                            (GradientEditorTab::Rgba, i) => Some(i),
                            (GradientEditorTab::Hsv, 3) => Some(3),
                            (GradientEditorTab::Hsv, i) => Some(4 + i),
                            _ => None,
                        }
                    } else {
                        match (self.state.active_tab, row) {
                            (ActiveTab::Rgb, i) => Some(i),
                            (ActiveTab::Hsv, 3) => Some(3),
                            (ActiveTab::Hsv, i) => Some(4 + i),
                        }
                    };
                    let value_child = if let Some(value_input_index) = value_input_index {
                        let input_tree =
                            if let Some(child_tree) = self.tree.children.get_mut(VALUE_INPUTS_INDEX + value_input_index) {
                                child_tree.diff(&mut self.value_inputs[value_input_index]
                                    as &mut dyn Widget<Message, Theme, Renderer>);
                                child_tree
                            } else {
                                let child_tree = Tree::new(&self.value_inputs[value_input_index]
                                    as &dyn Widget<Message, Theme, Renderer>);
                                self.tree.children.push(child_tree);
                                self.tree.children.last_mut().unwrap()
                            };
                        self.value_inputs[value_input_index]
                            .layout(
                                input_tree,
                                renderer,
                                &Limits::new(Size::ZERO, Size::new(VALUE_WIDTH, SLIDER_HEIGHT)),
                                Some(&text_input::Value::new(
                                    &self.state.value_inputs[value_input_index],
                                )),
                            )
                            .move_to(Point::new(LABEL_WIDTH + groove_width, y))
                    } else {
                        Node::with_children(Size::new(VALUE_WIDTH, SLIDER_HEIGHT), Vec::new())
                            .move_to(Point::new(LABEL_WIDTH + groove_width, y))
                    };
                    controls_children.push(Node::with_children(
                        Size::new(width, SLIDER_HEIGHT),
                        vec![label_node, groove_node, value_child],
                    ));
                }
                push(
                    Node::with_children(Size::new(width, controls_height), controls_children),
                    &mut children,
                    &mut offset_y,
                );
            } else {
                push(
                    Node::with_children(Size::new(width, 0.0), Vec::new()),
                    &mut children,
                    &mut offset_y,
                );
            }
        }

        // [4] Hex container (Color + Gradient for the selected stop).
        {
            if is_editor {
                let hex_input_tree =
                    if let Some(child_tree) = self.tree.children.get_mut(HEX_INPUT_INDEX) {
                        child_tree.diff(
                            &mut self.hex_input as &mut dyn Widget<Message, Theme, Renderer>
                        );
                        child_tree
                    } else {
                        let child_tree =
                            Tree::new(&self.hex_input as &dyn Widget<Message, Theme, Renderer>);
                        self.tree.children.push(child_tree);
                        self.tree.children.last_mut().unwrap()
                    };
                let mut hex_input_node = self.hex_input.layout(
                    hex_input_tree,
                    renderer,
                    &Limits::new(
                        Size::ZERO,
                        Size::new(
                            width - HEX_LABEL_WIDTH - HEX_INPUT_RIGHT_INSET,
                            HEX_CONTAINER_HEIGHT,
                        ),
                    ),
                    Some(&text_input::Value::new(&self.state.hex_input)),
                );
                let hex_label_node = Node::with_children(
                    Size::new(HEX_LABEL_WIDTH, HEX_CONTAINER_HEIGHT),
                    Vec::new(),
                );
                // The TextInput sizes to its content (shorter than the 44px
                // panel); center it vertically instead of top-aligning so the
                // text lines up with the centered "Hex:" label.
                let input_y =
                    ((HEX_CONTAINER_HEIGHT - hex_input_node.size().height) / 2.0).max(0.0);
                hex_input_node =
                    hex_input_node.move_to(Point::new(HEX_LABEL_WIDTH, input_y));
                push(
                    Node::with_children(
                        Size::new(width, HEX_CONTAINER_HEIGHT),
                        vec![hex_label_node, hex_input_node],
                    ),
                    &mut children,
                    &mut offset_y,
                );
            } else {
                push(
                    Node::with_children(Size::new(width, 0.0), Vec::new()),
                    &mut children,
                    &mut offset_y,
                );
            }
        }

        // [5] Swatches heading (Library only).
        {
            let h = if is_library { LABEL_HEIGHT } else { 0.0 };
            let node = if h > 0.0 {
                Row::<(), Theme, Renderer>::new()
                    .width(Length::Fill)
                    .height(Length::Fixed(h))
                    .layout(self.tree, renderer, &Limits::new(Size::ZERO, Size::new(width, h)))
            } else {
                Node::with_children(Size::new(width, 0.0), Vec::new())
            };
            push(node, &mut children, &mut offset_y);
        }

        // [6] Swatch tab bar (Library only).
        {
            let h = if is_library { TAB_BAR_HEIGHT } else { 0.0 };
            let node = if h > 0.0 {
                Row::<(), Theme, Renderer>::new()
                    .width(Length::Fill)
                    .height(Length::Fixed(h))
                    .layout(self.tree, renderer, &Limits::new(Size::ZERO, Size::new(width, h)))
            } else {
                Node::with_children(Size::new(width, 0.0), Vec::new())
            };
            push(node, &mut children, &mut offset_y);
        }

        // [7] Swatch page (Library only).
        {
            if is_library {
                let page_height = STRIP_HEIGHT;
                let mut page_children: Vec<Node> = Vec::new();
                if self.state.naming_new_set {
                    let (input_rect, _, _) = name_prompt_rects(Rectangle {
                        x: 0.0,
                        y: 0.0,
                        width,
                        height: page_height,
                    });
                    let name_tree = if let Some(child_tree) =
                        self.tree.children.get_mut(NEW_SET_NAME_INDEX)
                    {
                        child_tree.diff(
                            &mut self.new_set_name_input
                                as &mut dyn Widget<Message, Theme, Renderer>,
                        );
                        child_tree
                    } else {
                        let child_tree = Tree::new(
                            &self.new_set_name_input as &dyn Widget<Message, Theme, Renderer>,
                        );
                        self.tree.children.push(child_tree);
                        self.tree.children.last_mut().unwrap()
                    };
                    let input_node = self
                        .new_set_name_input
                        .layout(
                            name_tree,
                            renderer,
                            &Limits::new(Size::ZERO, input_rect.size()),
                            Some(&text_input::Value::new(&self.state.pending_swatch_name)),
                        )
                        .move_to(Point::new(input_rect.x, input_rect.y));
                    page_children.push(input_node);
                } else if let Some(set) = self.state.swatch_sets.get(self.state.active_swatch_tab) {
                    let cells = set.colors.len().max(STRIP_ROWS * visible_cols(width));
                    let scroll = clamp_strip_scroll(
                        self.state.swatch_scroll_x,
                        set.colors.len(),
                        width,
                    );
                    for i in 0..cells {
                        let col = i / STRIP_ROWS;
                        let row = i % STRIP_ROWS;
                        page_children.push(
                            Node::with_children(Size::new(SWATCH_SIZE, SWATCH_SIZE), Vec::new())
                                .move_to(Point::new(
                                    SWATCH_PAGE_MARGIN + col as f32 * CELL_PITCH - scroll,
                                    SWATCH_PAGE_MARGIN + row as f32 * CELL_PITCH,
                                )),
                        );
                    }
                }
                push(
                    Node::with_children(Size::new(width, page_height), page_children),
                    &mut children,
                    &mut offset_y,
                );
            } else {
                push(
                    Node::with_children(Size::new(width, 0.0), Vec::new()),
                    &mut children,
                    &mut offset_y,
                );
            }
        }

        // [8] Add-swatch button (Library only).
        {
            let h = if is_library { ADD_BUTTON_SIZE } else { 0.0 };
            let node = if h > 0.0 {
                Row::<(), Theme, Renderer>::new()
                    .width(Length::Fixed(ADD_BUTTON_SIZE))
                    .height(Length::Fixed(h))
                    .layout(
                        self.tree,
                        renderer,
                        &Limits::new(Size::ZERO, Size::new(ADD_BUTTON_SIZE, h)),
                    )
                    .move_to(Point::new((width - ADD_BUTTON_SIZE) / 2.0, 0.0))
            } else {
                Node::with_children(Size::new(width, 0.0), Vec::new())
            };
            // Center manually: wrap in fixed-width parent.
            let wrapped = if h > 0.0 {
                Node::with_children(Size::new(width, h), vec![node])
            } else {
                Node::with_children(Size::new(width, 0.0), Vec::new())
            };
            push(wrapped, &mut children, &mut offset_y);
        }

        // [9] Divider (Library only).
        {
            let h = if is_library { DIVIDER_HEIGHT } else { 0.0 };
            let node = if h > 0.0 {
                Row::<(), Theme, Renderer>::new()
                    .width(Length::Fill)
                    .height(Length::Fixed(h))
                    .layout(self.tree, renderer, &Limits::new(Size::ZERO, Size::new(width, h)))
            } else {
                Node::with_children(Size::new(width, 0.0), Vec::new())
            };
            push(node, &mut children, &mut offset_y);
        }

        // [10] Recent heading (Library only).
        {
            let h = if is_library { LABEL_HEIGHT } else { 0.0 };
            let node = if h > 0.0 {
                Row::<(), Theme, Renderer>::new()
                    .width(Length::Fill)
                    .height(Length::Fixed(h))
                    .layout(self.tree, renderer, &Limits::new(Size::ZERO, Size::new(width, h)))
            } else {
                Node::with_children(Size::new(width, 0.0), Vec::new())
            };
            push(node, &mut children, &mut offset_y);
        }

        // [11] Recent grid: full strip in Library, mirrored single row in
        // Color below the hex input.
        {
            if is_library {
                let recent_count = self.state.recent_colors.len();
                let recent_cells = recent_count.max(STRIP_ROWS * visible_cols(width));
                let recent_scroll =
                    clamp_strip_scroll(self.state.recent_scroll_x, recent_count, width);
                let mut recent_children: Vec<Node> = Vec::new();
                for i in 0..recent_cells {
                    let col = i / STRIP_ROWS;
                    let row = i % STRIP_ROWS;
                    recent_children.push(
                        Node::with_children(Size::new(SWATCH_SIZE, SWATCH_SIZE), Vec::new())
                            .move_to(Point::new(
                                SWATCH_PAGE_MARGIN + col as f32 * CELL_PITCH - recent_scroll,
                                SWATCH_PAGE_MARGIN + row as f32 * CELL_PITCH,
                            )),
                    );
                }
                push(
                    Node::with_children(Size::new(width, STRIP_HEIGHT), recent_children),
                    &mut children,
                    &mut offset_y,
                );
            } else {
                let recent_count = self.state.recent_colors.len();
                let recent_cells = recent_count.max(visible_cols(width));
                let recent_scroll = clamp_strip_scroll_rows(
                    self.state.recent_scroll_x,
                    recent_count,
                    width,
                    RECENT_SINGLE_ROWS,
                );
                let mut recent_children: Vec<Node> = Vec::new();
                for i in 0..recent_cells {
                    recent_children.push(
                        Node::with_children(Size::new(SWATCH_SIZE, SWATCH_SIZE), Vec::new())
                            .move_to(Point::new(
                                SWATCH_PAGE_MARGIN + i as f32 * CELL_PITCH - recent_scroll,
                                RECENT_SINGLE_VERT_MARGIN,
                            )),
                    );
                }
                push(
                    Node::with_children(
                        Size::new(width, RECENT_SINGLE_ROW_HEIGHT),
                        recent_children,
                    ),
                    &mut children,
                    &mut offset_y,
                );
            }
        }

        // [13] Buttons row below hex: reset icon + OK (always).
        // The eyedropper lives beside the hue slider (see picker above).
        {
            let reset_node = Row::<(), Theme, Renderer>::new()
                .width(Length::Fixed(RESET_WIDTH))
                .height(Length::Fixed(BUTTONS_HEIGHT))
                .layout(
                    self.tree,
                    renderer,
                    &Limits::new(Size::ZERO, Size::new(RESET_WIDTH, BUTTONS_HEIGHT)),
                )
                .move_to(Point::new(0.0, 0.0));
            let button_width = width - RESET_WIDTH - 5.0;
            let submit_button = self
                .submit_button
                .layout(
                    &mut self.tree.children[1],
                    renderer,
                    &Limits::new(Size::ZERO, Size::new(button_width, BUTTONS_HEIGHT)),
                )
                .move_to(Point::new(RESET_WIDTH + 5.0, 0.0));
            push(
                Node::with_children(
                    Size::new(width, BUTTONS_HEIGHT),
                    vec![reset_node, submit_button],
                ),
                &mut children,
                &mut offset_y,
            );
        }

        if offset_y > 0.0 {
            offset_y -= spacing;
        }
        Node::with_children(Size::new(width, offset_y), children)
    }

    /// The event handling of the dialog content.
    pub(crate) fn update_content(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<Message>,
    ) {
        // --- Eye dropper ------------------------------------------------
        // Pick up a freshly captured frame and, while the dropper is
        // active, swallow every other interaction so the frozen snapshot
        // stays consistent and nothing beneath reacts.
        if self.poll_dropper() {
            shell.request_redraw();
        }
        if self.state.dropper_mode != DropperMode::Idle {
            self.on_event_dropper(event, cursor, shell);
            shell.capture_event();
            return;
        }

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
        // Single column: [0]top [1]preview [2]picker [3]subtabs [4]controls
        // [5]hex [6]swlabel [7]swtabs [8]swpage [9]add [10]div [11]reclabel
        // [12]recgrid [13]buttons
        let top_tabs_layout = children.next().expect("widget: Layout should have top tabs");
        let _preview_layout = children.next();
        let picker_layout = children.next().expect("widget: Layout should have picker");
        let tab_bar_layout = children.next().expect("widget: Layout should have tab bar");
        let controls_layout = children.next().expect("widget: Layout should have controls");
        let hex_layout = children.next().expect("widget: Layout should have hex");
        let _swatch_label_layout = children.next();
        let swatch_tab_bar_layout = children
            .next()
            .expect("widget: Layout should have swatch tabs");
        let swatch_page_layout = children.next().expect("widget: Layout should have swatch page");
        let add_btn_wrapper = children.next().expect("widget: Layout should have add button");
        let _divider_layout = children.next();
        let _recent_label_layout = children.next();
        let recent_grid_layout = children.next().expect("widget: Layout should have recent grid");
        let buttons_node = children.next().expect("widget: Layout should have buttons");
        let mut buttons_layout = buttons_node.children();
        let mut fake_messages: Vec<Message> = Vec::new();

        let reset_button_layout = buttons_layout
            .next()
            .expect("widget: Layout should have reset button");
        let is_color = self.state.picker_tab == PickerTab::Color;
        let is_gradient = self.state.picker_tab == PickerTab::Gradient;
        let is_library = self.state.picker_tab == PickerTab::Library;
        let is_editor = is_color || is_gradient;
        let show_square =
            is_color || (is_gradient && self.state.gradient_editor == GradientEditorTab::Rect);
        // Eyedropper lives beside the hue slider: third child of the
        // picker (Color) or of the controls below the editor tabs
        // (Gradient + Rect). Absent in the Library tab.
        let dropper_button_layout_opt = if is_color {
            picker_layout.children().nth(2)
        } else if show_square {
            controls_layout.children().nth(2)
        } else {
            None
        };
        let submit_button_layout = buttons_layout
            .next()
            .expect("widget: Layout should have submit button");

        if event::Status::Captured == self.on_event_keyboard(event, shell) {
            self.clear_cache();
            shell.capture_event();
            shell.request_redraw();
            return;
        }

        // Top-level tabs in content (inline mode; floating uses the header).
        if !self.lens_in_content_draw {
            match event {
                Event::Mouse(
                    mouse::Event::CursorMoved { .. }
                    | mouse::Event::ButtonPressed(_)
                    | mouse::Event::ButtonReleased(_),
                )
                | Event::Touch(touch::Event::FingerMoved { .. }) => {
                    let bounds = top_tabs_layout.bounds();
                    if bounds.height > 0.0 {
                        let gap = 6.0;
                        let w = (bounds.width - 2.0 * gap) / 3.0;
                        self.state.top_color_hovered = cursor.is_over(Rectangle {
                            x: bounds.x,
                            y: bounds.y,
                            width: w,
                            height: bounds.height,
                        });
                        self.state.top_gradient_hovered = cursor.is_over(Rectangle {
                            x: bounds.x + w + gap,
                            y: bounds.y,
                            width: w,
                            height: bounds.height,
                        });
                        self.state.top_library_hovered = cursor.is_over(Rectangle {
                            x: bounds.x + 2.0 * (w + gap),
                            y: bounds.y,
                            width: w,
                            height: bounds.height,
                        });
                    }
                }
                _ => {}
            }
            if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) = event
            {
                let bounds = top_tabs_layout.bounds();
                if bounds.height > 0.0 {
                    let gap = 6.0;
                    let w = (bounds.width - 2.0 * gap) / 3.0;
                    let color_tab = Rectangle {
                        x: bounds.x,
                        y: bounds.y,
                        width: w,
                        height: bounds.height,
                    };
                    let gradient_tab = Rectangle {
                        x: bounds.x + w + gap,
                        y: bounds.y,
                        width: w,
                        height: bounds.height,
                    };
                    let lib_tab = Rectangle {
                        x: bounds.x + 2.0 * (w + gap),
                        y: bounds.y,
                        width: w,
                        height: bounds.height,
                    };
                    if cursor.is_over(color_tab) && self.state.picker_tab != PickerTab::Color {
                        self.set_picker_tab(PickerTab::Color, shell);
                        shell.capture_event();
                        shell.request_redraw();
                        return;
                    } else if cursor.is_over(gradient_tab)
                        && self.state.picker_tab != PickerTab::Gradient
                    {
                        self.set_picker_tab(PickerTab::Gradient, shell);
                        shell.capture_event();
                        shell.request_redraw();
                        return;
                    } else if cursor.is_over(lib_tab)
                        && self.state.picker_tab != PickerTab::Library
                    {
                        self.set_picker_tab(PickerTab::Library, shell);
                        shell.capture_event();
                        shell.request_redraw();
                        return;
                    }
                }
            }
        }

        // Forward events to the hex input and the channel inputs of the
        // active tab. Every TextInput mutates its slot in `state` from its
        // `on_input` closure and pushes a fake message; a non-empty message
        // list means that this input's value changed (submit_button pattern).
        let mut hex_changed = false;
        if is_editor
            && let Some(tree_child) = self.tree.children.get_mut(HEX_INPUT_INDEX)
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
        for i in if is_color {
            self.active_tab_channels()
        } else if is_gradient {
            self.state.gradient_channels()
        } else {
            Vec::new()
        } {
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

        if color_changed {
            self.notify_changed(shell);
        }

        // Channels of the current editor (Color: active tab; Gradient:
        // gradient editor).
        let editor_channels = if is_gradient {
            self.state.gradient_channels()
        } else if is_color {
            self.active_tab_channels()
        } else {
            Vec::new()
        };
        if hex_changed || self.state.hex_focused || self.state.value_focus.is_some() {
            match event {
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                | Event::Touch(touch::Event::FingerPressed { .. }) => {
                    // Clicking outside all TextInputs blurs them.
                    let over_hex = hex_input_layout(hex_layout)
                        .is_some_and(|l| cursor.position_in(l.bounds()).is_some());
                    let over_value = editor_channels.iter().any(|i| {
                        value_cell_layout(controls_layout, self, *i)
                            .is_some_and(|l| cursor.position_in(l.bounds()).is_some())
                    });
                    if !over_hex && !over_value {
                        self.unfocus_all_text_inputs();
                        self.state.hex_focused = false;
                        self.state.value_focus = None;
                    } else if over_hex {
                        self.state.focus = Focus::Hex;
                    } else if let Some(i) = editor_channels.iter().find(|i| {
                        value_cell_layout(controls_layout, self, **i)
                            .is_some_and(|l| cursor.position_in(l.bounds()).is_some())
                    }) {
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

        // Clicking a tab bar shows that tab: [HSV | RGB(A)] in Color,
        // [Rect | HSV | RGBA] in Gradient.
        if is_editor {
            match event {
                Event::Mouse(
                    mouse::Event::CursorMoved { .. }
                    | mouse::Event::ButtonPressed(_)
                    | mouse::Event::ButtonReleased(_),
                )
                | Event::Touch(touch::Event::FingerMoved { .. }) => {
                    let bounds = tab_bar_layout.bounds();
                    if is_gradient {
                        let gap = 2.0;
                        let w = (bounds.width - 2.0 * gap) / 3.0;
                        self.state.tab_rect_hovered = cursor.is_over(Rectangle {
                            x: bounds.x,
                            y: bounds.y,
                            width: w,
                            height: bounds.height,
                        });
                        self.state.tab_hsv_hovered = cursor.is_over(Rectangle {
                            x: bounds.x + w + gap,
                            y: bounds.y,
                            width: w,
                            height: bounds.height,
                        });
                        self.state.tab_rgb_hovered = cursor.is_over(Rectangle {
                            x: bounds.x + 2.0 * (w + gap),
                            y: bounds.y,
                            width: w,
                            height: bounds.height,
                        });
                    } else {
                        let gap = 2.0;
                        let half = (bounds.width - gap) / 2.0;
                        let hsv_tab_bounds = Rectangle {
                            x: bounds.x,
                            y: bounds.y,
                            width: half,
                            height: bounds.height,
                        };
                        let rgb_tab_bounds = Rectangle {
                            x: bounds.x + half + gap,
                            y: bounds.y,
                            width: half,
                            height: bounds.height,
                        };
                        self.state.tab_rgb_hovered = cursor.is_over(rgb_tab_bounds);
                        self.state.tab_hsv_hovered = cursor.is_over(hsv_tab_bounds);
                    }
                }
                _ => {}
            }
            if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) = event
            {
                let bounds = tab_bar_layout.bounds();
                if is_gradient {
                    let gap = 2.0;
                    let w = (bounds.width - 2.0 * gap) / 3.0;
                    let rect_tab = Rectangle {
                        x: bounds.x,
                        y: bounds.y,
                        width: w,
                        height: bounds.height,
                    };
                    let hsv_tab = Rectangle {
                        x: bounds.x + w + gap,
                        y: bounds.y,
                        width: w,
                        height: bounds.height,
                    };
                    let rgba_tab = Rectangle {
                        x: bounds.x + 2.0 * (w + gap),
                        y: bounds.y,
                        width: w,
                        height: bounds.height,
                    };
                    if cursor.is_over(rect_tab)
                        && self.state.gradient_editor != GradientEditorTab::Rect
                    {
                        self.set_gradient_editor(GradientEditorTab::Rect, shell);
                        captured = true;
                    } else if cursor.is_over(hsv_tab)
                        && self.state.gradient_editor != GradientEditorTab::Hsv
                    {
                        self.set_gradient_editor(GradientEditorTab::Hsv, shell);
                        captured = true;
                    } else if cursor.is_over(rgba_tab)
                        && self.state.gradient_editor != GradientEditorTab::Rgba
                    {
                        self.set_gradient_editor(GradientEditorTab::Rgba, shell);
                        captured = true;
                    }
                } else {
                    let gap = 2.0;
                    let half = (bounds.width - gap) / 2.0;
                    let hsv_tab_bounds = Rectangle {
                        x: bounds.x,
                        y: bounds.y,
                        width: half,
                        height: bounds.height,
                    };
                    let rgb_tab_bounds = Rectangle {
                        x: bounds.x + half + gap,
                        y: bounds.y,
                        width: half,
                        height: bounds.height,
                    };
                    if cursor.is_over(rgb_tab_bounds) && self.state.active_tab != ActiveTab::Rgb {
                        self.set_active_tab(ActiveTab::Rgb, shell);
                        captured = true;
                    } else if cursor.is_over(hsv_tab_bounds)
                        && self.state.active_tab != ActiveTab::Hsv
                    {
                        self.set_active_tab(ActiveTab::Hsv, shell);
                        captured = true;
                    }
                }
            }
        }

        // Gradient stop bar: select + drag stops (Gradient tab only).
        if is_gradient
            && event::Status::Captured
                == self.on_event_gradient_bar(event, picker_layout, cursor, shell)
        {
            captured = true;
        }

        // The square lives in the picker (Color) or in the controls
        // below the editor tabs (Gradient + Rect).
        let square_layout = if is_gradient {
            controls_layout
        } else {
            picker_layout
        };
        if show_square
            && event::Status::Captured
                == self.on_event_hsv_color(event, square_layout, cursor, shell)
        {
            captured = true;
        }

        let show_slider_events = is_color
            || (is_gradient && self.state.gradient_editor != GradientEditorTab::Rect);
        if show_slider_events
            && event::Status::Captured
                == self.on_event_sliders(event, controls_layout, cursor, shell)
        {
            captured = true;
        }

        if is_library {
            let add_btn_inner = add_btn_wrapper
                .children()
                .next()
                .unwrap_or(add_btn_wrapper);
            if self.on_event_swatches(
                event,
                cursor,
                shell,
                renderer,
                clipboard,
                swatch_tab_bar_layout,
                swatch_page_layout,
                add_btn_inner,
            ) {
                captured = true;
            }
        }

        // Horizontal wheel scrolling of the swatch and recent strips. The
        // wheel is only consumed when the strip actually overflows.
        // The swatch strip only exists in Library; the recent strip is
        // mirrored as a single row in Color, so it scrolls in both tabs.
        if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event {
            let dy = match *delta {
                mouse::ScrollDelta::Lines { y, .. } => y * CELL_PITCH,
                mouse::ScrollDelta::Pixels { y, .. } => y,
            };

            if is_library {
                let page_bounds = swatch_page_layout.bounds();
                if !self.state.naming_new_set && cursor.is_over(page_bounds) {
                    let count = self
                        .state
                        .swatch_sets
                        .get(self.state.active_swatch_tab)
                        .map_or(0, |set| set.colors.len());
                    let old =
                        clamp_strip_scroll(self.state.swatch_scroll_x, count, page_bounds.width);
                    let new = clamp_strip_scroll(old + dy, count, page_bounds.width);
                    if (new - old).abs() > f32::EPSILON {
                        self.state.swatch_scroll_x = new;
                        shell.invalidate_layout();
                        captured = true;
                    }
                }
            }

            let recent_bounds = recent_grid_layout.bounds();
            if recent_bounds.height > 0.0 && cursor.is_over(recent_bounds) {
                let count = self.state.recent_colors.len();
                let rows = if is_library {
                    STRIP_ROWS
                } else {
                    RECENT_SINGLE_ROWS
                };
                let old = clamp_strip_scroll_rows(
                    self.state.recent_scroll_x,
                    count,
                    recent_bounds.width,
                    rows,
                );
                let new = clamp_strip_scroll_rows(old + dy, count, recent_bounds.width, rows);
                if (new - old).abs() > f32::EPSILON {
                    self.state.recent_scroll_x = new;
                    shell.invalidate_layout();
                    captured = true;
                }
            }
        }

        // Clicking a recent color selects it (mirrored in both tabs).
        if matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                | Event::Touch(touch::Event::FingerPressed { .. })
        )
        {
            // Scrolled-out parts of the strip are not clickable.
            let viewport = recent_grid_layout.bounds();
            for (i, cell) in recent_grid_layout.children().enumerate() {
                let cell_bounds = cell.bounds();
                if cursor.is_over(cell_bounds)
                    && viewport.intersects(&cell_bounds)
                    && let Some(picked) = self.state.recent_colors.get(i).cloned()
                {
                    self.select_picked_from_swatch(picked, shell);
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
                if show_square
                    && dropper_button_layout_opt
                        .as_ref()
                        .is_some_and(|l| cursor.is_over(l.bounds()))
                {
                    self.state.dropper_pressed = true;
                }
                if cursor.is_over(submit_button_layout.bounds()) {
                    self.state.submit_pressed = true;
                }
                if cursor.is_over(reset_button_layout.bounds()) {
                    self.state.reset_pressed = true;
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. }) => {
                self.state.dropper_pressed = false;
                self.state.submit_pressed = false;
                // Releasing inside the Reset button resets the color to the
                // initial one (Qt behavior: the dialog stays open).
                if self.state.reset_pressed && cursor.is_over(reset_button_layout.bounds()) {
                    self.state.reset_pressed = false;
                    self.state.reset_to_initial();
                    self.notify_changed(shell);
                }
                self.state.reset_pressed = false;
            }
            _ => {}
        }

        // The eyedropper button publishes a fake message (submit_button
        // pattern); a non-empty list means it was pressed and the capture
        // round-trip should start. It lives beside the hue slider, so it
        // is only interactive when the square is shown.
        if show_square && let Some(dropper_button_layout) = dropper_button_layout_opt {
            let mut dropper_messages: Vec<Message> = Vec::new();
            self.dropper_button.update(
                &mut self.tree.children[0],
                event,
                dropper_button_layout,
                cursor,
                renderer,
                clipboard,
                &mut Shell::new(&mut dropper_messages),
                &layout.bounds(),
            );

            if !dropper_messages.is_empty() && self.request_dropper_capture(shell) {
                shell.capture_event();
                shell.request_redraw();
            }
        }

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
            let picked = self.state.current_picked();
            push_recent(&mut self.state.recent_colors, picked.clone());
            // Also mirror the submitted value into the active swatch set so
            // the Library stays in sync with what was picked.
            if let Some(set) = self
                .state
                .swatch_sets
                .get_mut(self.state.active_swatch_tab)
            {
                insert_swatch(&mut set.colors, picked);
            }
            self.state.clear_cache();
            if let Some(on_pick_submit) = self.on_pick_submit {
                shell.publish(on_pick_submit(self.state.current_picked()));
            }
            if self.state.picker_tab == PickerTab::Gradient {
                if let Some(on_gradient_submit) = self.on_gradient_submit {
                    shell.publish(on_gradient_submit(self.state.gradient.clone()));
                } else if self.on_pick_submit.is_none() {
                    // Legacy compat only: the app knows solids alone, so the
                    // best we can do is the selected stop's color. When a
                    // unified or gradient callback is set it already carried
                    // the gradient above; publishing the solid here as well
                    // would overwrite it with a stale color.
                    shell.publish((self.on_submit)(self.state.color));
                }
            } else {
                shell.publish((self.on_submit)(self.state.color));
            }
            shell.capture_event();
            shell.request_redraw();
        }

        if captured {
            self.clear_cache();
            shell.capture_event();
            shell.request_redraw();
        }
    }

    /// Transitions the eye dropper from [`DropperMode::Waiting`] to
    /// [`DropperMode::Picking`] when the application has deposited a fresh
    /// frame into the shared [`DropperBuffer`].
    ///
    /// Returns `true` on the transition, so the caller can request a redraw.
    fn poll_dropper(&mut self) -> bool {
        if self.state.dropper_mode != DropperMode::Waiting {
            return false;
        }

        let Some(frame) = self.dropper_buffer.and_then(DropperBuffer::take) else {
            return false;
        };

        self.state.dropper_frame = Some(frame);
        self.state.dropper_mode = DropperMode::Picking;
        true
    }

    /// Starts the capture round-trip: clears any stale frame, enters
    /// [`DropperMode::Waiting`] and publishes the application's capture
    /// request message. Returns `false` when no buffer is wired (the button
    /// is disabled).
    fn request_dropper_capture(&mut self, shell: &mut Shell<Message>) -> bool {
        let Some(buffer) = self.dropper_buffer else {
            return false;
        };

        buffer.clear();
        self.state.dropper_mode = DropperMode::Waiting;
        self.state.dropper_frame = None;

        if let Some(on_dropper_capture) = self.on_dropper_capture {
            shell.publish(on_dropper_capture());
        }

        true
    }

    /// Leaves picking mode without changing the selection.
    fn exit_dropper(&mut self) {
        self.state.dropper_mode = DropperMode::Idle;
        self.state.dropper_frame = None;
    }

    /// Applies the sampled pixel color to the dialog and leaves picking
    /// mode.
    fn commit_dropper(&mut self, shell: &mut Shell<Message>) {
        let sampled = self.state.dropper_frame.as_ref().and_then(|frame| {
            frame.sample(self.state.dropper_cursor.x, self.state.dropper_cursor.y)
        });

        if let Some(color) = sampled {
            self.state.apply_color(color);
            self.state.sync_display();
            self.state.clear_cache();
            self.notify_changed(shell);
        }

        self.exit_dropper();
    }

    /// The event handling while the eye dropper is active. The cursor
    /// tracks the magnifier lens; left click / Enter commit the hovered
    /// pixel, right click / Escape abort without changes and the arrow keys
    /// nudge the hovered pixel by one screen pixel.
    fn on_event_dropper(
        &mut self,
        event: &Event,
        _cursor: Cursor,
        shell: &mut Shell<Message>,
    ) {
        match event {
            Event::Mouse(mouse::Event::CursorMoved { position })
            | Event::Touch(touch::Event::FingerMoved { position, .. }) => {
                self.state.dropper_cursor = *position;
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if let Event::Touch(touch::Event::FingerPressed { position, .. }) = event {
                    self.state.dropper_cursor = *position;
                }
                self.commit_dropper(shell);
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                self.exit_dropper();
                shell.request_redraw();
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) => match key {
                keyboard::Key::Named(
                    keyboard::key::Named::Enter | keyboard::key::Named::Space,
                ) => {
                    self.commit_dropper(shell);
                    shell.request_redraw();
                }
                keyboard::Key::Named(keyboard::key::Named::Escape) => {
                    self.exit_dropper();
                    shell.request_redraw();
                }
                keyboard::Key::Named(
                    keyboard::key::Named::ArrowLeft
                    | keyboard::key::Named::ArrowRight
                    | keyboard::key::Named::ArrowUp
                    | keyboard::key::Named::ArrowDown,
                ) => {
                    // Nudge by one physical pixel: convert the frame's
                    // scale factor into logical units.
                    let step = 1.0 / self
                        .state
                        .dropper_frame
                        .as_ref()
                        .map(|frame| frame.scale_factor.max(1.0))
                        .unwrap_or(1.0);
                    let hovered = &mut self.state.dropper_cursor;
                    match key {
                        keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                            hovered.x -= step;
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                            hovered.x += step;
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                            hovered.y -= step;
                        }
                        _ => {
                            hovered.y += step;
                        }
                    }
                    shell.request_redraw();
                }
                _ => {}
            },
            _ => {}
        }
    }

    /// The mouse interaction of the dialog content.
    pub(crate) fn mouse_interaction_content(
        &self,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.state.dropper_mode != DropperMode::Idle {
            return mouse::Interaction::Crosshair;
        }
        let mut children = layout.children();
        let mut interaction = mouse::Interaction::default();
        let is_color = self.state.picker_tab == PickerTab::Color;
        let is_gradient = self.state.picker_tab == PickerTab::Gradient;
        let is_library = self.state.picker_tab == PickerTab::Library;
        let is_editor = is_color || is_gradient;
        let show_sliders = is_color
            || (is_gradient && self.state.gradient_editor != GradientEditorTab::Rect);

        let top_tabs_layout = children.next().expect("Graphics: Layout should have top tabs");
        if top_tabs_layout.bounds().height > 0.0 && cursor.is_over(top_tabs_layout.bounds()) {
            interaction = interaction.max(mouse::Interaction::Pointer);
        }
        let _preview_layout = children.next();
        let picker_layout = children.next().expect("Graphics: Layout should have picker");
        let _tab_bar_layout = children.next();
        let controls_layout = children.next().expect("Graphics: Layout should have controls");
        // The square lives in the picker (Color) or in the controls below
        // the editor tabs (Gradient + Rect).
        let square_layout_opt = if is_color {
            Some(picker_layout)
        } else if is_gradient && self.state.gradient_editor == GradientEditorTab::Rect {
            Some(controls_layout)
        } else {
            None
        };
        if let Some(square_layout) = square_layout_opt {
            let mut square_children = square_layout.children();
            if let Some(square) = square_children.next()
                && cursor.is_over(square.bounds())
            {
                interaction = interaction.max(mouse::Interaction::Pointer);
            }
            if let Some(hue) = square_children.next()
                && cursor.is_over(hue.bounds())
            {
                interaction = interaction.max(mouse::Interaction::ResizingHorizontally);
            }
        }
        if is_gradient {
            // Gradient pins (bodies + pointer nubs) + strip.
            let mut picker_children = picker_layout.children();
            if let Some(bar) = picker_children.next() {
                let mut bar_children = bar.children();
                if let Some(strip_layout) = bar_children.next() {
                    let strip = strip_layout.bounds();
                    if cursor.is_over(strip) {
                        interaction =
                            interaction.max(mouse::Interaction::ResizingHorizontally);
                    }
                    if strip.width > 0.0 {
                        let section = bar.bounds();
                        for i in 0..2 {
                            let offset = self
                                .state
                                .gradient
                                .stop(i)
                                .map_or(i as f32, |s| s.offset);
                            if cursor.is_over(gradient_handle_rect(section, offset))
                                || cursor.is_over(gradient_pointer_rect(section, offset))
                            {
                                interaction = interaction.max(mouse::Interaction::Pointer);
                                break;
                            }
                        }
                    }
                }
            }
        }
        if show_sliders {
            for row_layout in controls_layout.children() {
                let mut row_children = row_layout.children();
                let _ = row_children.next();
                if let Some(bar) = row_children.next()
                    && cursor.is_over(bar.bounds())
                {
                    interaction = interaction.max(mouse::Interaction::ResizingHorizontally);
                }
            }
        }
        let hex_layout = children.next().expect("Graphics: Layout should have hex");
        if is_editor
            && let Some(tree_child) = self.tree.children.get(HEX_INPUT_INDEX)
            && let Some(input_layout) = hex_input_layout(hex_layout)
        {
            interaction = interaction.max(self.hex_input.mouse_interaction(
                tree_child,
                input_layout,
                cursor,
                &input_layout.bounds(),
                renderer,
            ));
        }
        if is_editor {
            let channels = if is_gradient {
                self.state.gradient_channels()
            } else {
                self.active_tab_channels()
            };
            for i in channels {
                if self.tree.children.len() <= VALUE_INPUTS_INDEX + i {
                    continue;
                }
                if let Some(input_layout) = value_cell_layout(controls_layout, self, i) {
                    interaction = interaction.max(self.value_inputs[i].mouse_interaction(
                        &self.tree.children[VALUE_INPUTS_INDEX + i],
                        input_layout,
                        cursor,
                        &input_layout.bounds(),
                        renderer,
                    ));
                }
            }
        }
        let _ = children.next();
        let swatch_tab_bar_layout = children.next().expect("Graphics: Layout should have swatch tabs");
        let swatch_page_layout = children.next().expect("Graphics: Layout should have swatch page");
        // Add-button wrapper has one centered child.
        let add_btn_wrapper = children.next().expect("Graphics: Layout should have add button");
        let add_btn_bounds = add_btn_wrapper
            .children()
            .next()
            .map(|l| l.bounds())
            .unwrap_or(add_btn_wrapper.bounds());
        if is_library
            && (cursor.is_over(swatch_tab_bar_layout.bounds()) || cursor.is_over(add_btn_bounds))
        {
            interaction = interaction.max(mouse::Interaction::Pointer);
        }
        if is_library {
            let active_set_cells = if self.state.naming_new_set {
                0
            } else {
                self.state
                    .swatch_sets
                    .get(self.state.active_swatch_tab)
                    .map_or(0, |set| set.colors.len())
            };
            for (i, cell) in swatch_page_layout.children().enumerate() {
                if i < active_set_cells && cursor.is_over(cell.bounds()) {
                    interaction = interaction.max(mouse::Interaction::Pointer);
                }
            }
            if self.state.naming_new_set {
                let (_, add_rect, cancel_rect) = name_prompt_rects(swatch_page_layout.bounds());
                if cursor.is_over(add_rect) || cursor.is_over(cancel_rect) {
                    interaction = interaction.max(mouse::Interaction::Pointer);
                }
            }
        }
        let _ = children.next();
        let _ = children.next();
        let recent_grid_layout = children.next().expect("Graphics: Layout should have recent grid");
        {
            let recent_cells = self.state.recent_colors.len();
            for (i, cell) in recent_grid_layout.children().enumerate() {
                if i < recent_cells && cursor.is_over(cell.bounds()) {
                    interaction = interaction.max(mouse::Interaction::Pointer);
                }
            }
        }
        // Eyedropper beside the hue slider (Color + Gradient/Rect).
        if let Some(square_layout) = square_layout_opt
            && let Some(dropper_layout) = square_layout.children().nth(2)
        {
            interaction = interaction.max(self.dropper_button.mouse_interaction(
                &self.tree.children[0],
                dropper_layout,
                cursor,
                &self.viewport,
                renderer,
            ));
        }
        let buttons_node = children.next().expect("Graphics: Layout should have buttons");
        let mut buttons_layout = buttons_node.children();
        let _ = buttons_layout.next();
        let Some(submit_button_layout) = buttons_layout.next() else {
            return interaction;
        };
        interaction.max(self.submit_button.mouse_interaction(
            &self.tree.children[1],
            submit_button_layout,
            cursor,
            &self.viewport,
            renderer,
        ))
    }

    /// The operation support of the dialog content.
    pub(crate) fn operate_content(
        &mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        // The square container is child [2] (Color picker) or child [4]
        // (Gradient + Rect controls); its third child is the eyedropper.
        let square_idx =
            if self.state.picker_tab == PickerTab::Gradient
                && self.state.gradient_editor == GradientEditorTab::Rect
            {
                4
            } else {
                2
            };
        if let Some(square_container) = layout.children().nth(square_idx)
            && let Some(dropper_layout) = square_container.children().nth(2)
        {
            Widget::operate(
                &mut self.dropper_button,
                &mut self.tree.children[0],
                dropper_layout,
                renderer,
                operation,
            );
        }
        // Buttons row is child [13] of the single column: [reset, submit].
        if let Some(buttons_layout) = layout.children().nth(13) {
            let mut button_children = buttons_layout.children();
            let _reset_layout = button_children.next();
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

    /// Draws the dialog content.
    pub(crate) fn draw_content(
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

        // Single column: [0]top [1]preview [2]picker [3]subtabs [4]controls
        // [5]hex [6]swlabel [7]swtabs [8]swpage [9]add [10]div [11]reclabel
        // [12]recgrid [13]buttons
        let is_color = self.state.picker_tab == PickerTab::Color;
        let is_gradient = self.state.picker_tab == PickerTab::Gradient;
        let is_library = self.state.picker_tab == PickerTab::Library;
        let is_editor = is_color || is_gradient;
        let show_sliders = is_color
            || (is_gradient && self.state.gradient_editor != GradientEditorTab::Rect);
        let top_tabs_layout = children.next().expect("Graphics: Layout should have top tabs");
        if top_tabs_layout.bounds().height > 0.0 {
            draw_top_tabs(renderer, self, top_tabs_layout, cursor, &style_sheet);
        }
        let preview_layout = children.next().expect("Graphics: Layout should have preview");
        preview_placeholder(renderer, self, preview_layout, cursor, &style_sheet);
        let picker_layout = children.next().expect("Graphics: Layout should have picker");
        if is_gradient {
            draw_gradient_bar(renderer, self, picker_layout, cursor, &style_sheet);
        }
        let tab_bar_layout = children.next().expect("Graphics: Layout should have tab bar");
        if is_editor {
            tab_bar_placeholder(renderer, self, tab_bar_layout, cursor, &style_sheet);
        }
        let controls_layout = children.next().expect("Graphics: Layout should have controls");
        // The square lives in the picker (Color) or in the controls below
        // the editor tabs (Gradient + Rect).
        let square_layout_opt = if is_color {
            Some(picker_layout)
        } else if is_gradient && self.state.gradient_editor == GradientEditorTab::Rect {
            Some(controls_layout)
        } else {
            None
        };
        if let Some(square_layout) = square_layout_opt {
            hsv_color(renderer, self, square_layout, cursor, &style_sheet);
            // Eyedropper icon button on the right of the hue slider.
            if let Some(dropper_layout) = square_layout.children().nth(2) {
                let disabled = self.dropper_buffer.is_none();
                draw_icon_overlay_button(
                    renderer,
                    theme,
                    EYEDROPPER_SVG,
                    dropper_layout.bounds(),
                    self.state.dropper_pressed,
                    cursor,
                    disabled,
                );
                draw_focus_border(
                    renderer,
                    self,
                    dropper_layout.bounds(),
                    Focus::Dropper,
                    &style_sheet,
                );
            }
        }
        if show_sliders {
            slider_rows(
                renderer,
                self,
                controls_layout,
                cursor,
                theme,
                style,
                &style_sheet,
                self.state.focus,
            );
        }
        let hex_layout = children.next().expect("Graphics: Layout should have hex");
        if is_editor {
            hex_input(renderer, theme, self, hex_layout, cursor, &style_sheet);
        }
        let swatch_label_layout = children.next();
        if is_library && let Some(l) = swatch_label_layout {
            draw_section_heading(
                renderer,
                l,
                "Swatches",
                style_sheet[&StyleState::Active].text_secondary,
            );
        }
        let swatch_tab_bar_layout = children.next().expect("Graphics: Layout should have swatch tabs");
        if is_library {
            swatch_tab_bar(renderer, self, swatch_tab_bar_layout, cursor, &style_sheet);
        }
        let swatch_page_layout = children.next().expect("Graphics: Layout should have swatch page");
        if is_library {
            swatch_page(renderer, theme, self, swatch_page_layout, cursor, &style_sheet);
        }
        let add_btn_wrapper = children.next().expect("Graphics: Layout should have add button");
        if is_library && let Some(add_btn_layout) = add_btn_wrapper.children().next() {
            draw_add_button(renderer, add_btn_layout, cursor, &style_sheet);
        }
        let divider_layout = children.next();
        if is_library && let Some(l) = divider_layout {
            let bounds = l.bounds();
            if (bounds.width > 0.) && (bounds.height > 0.) {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds,
                        ..renderer::Quad::default()
                    },
                    style_sheet[&StyleState::Active].panel_border_color,
                );
            }
        }
        let recent_label_layout = children.next();
        if is_library && let Some(l) = recent_label_layout {
            draw_section_heading(
                renderer,
                l,
                "Recent",
                style_sheet[&StyleState::Active].text_secondary,
            );
        }
        let recent_grid_layout = children.next().expect("Graphics: Layout should have recent grid");
        if recent_grid_layout.bounds().height > 0.0 {
            draw_recent_grid(renderer, self, recent_grid_layout, cursor, &style_sheet);
        }
        let buttons_node = children.next().expect("Graphics: Layout should have buttons");
        {
            let mut button_children = buttons_node.children();
            let Some(reset_layout) = button_children.next() else {
                return;
            };
            draw_reset_button(
                renderer,
                reset_layout.bounds(),
                self.state.reset_pressed,
                cursor,
                &style_sheet,
            );
            let Some(submit_layout) = button_children.next() else {
                return;
            };
            draw_overlay_button(
                renderer,
                theme,
                ok_icon().0,
                submit_layout.bounds(),
                self.state.submit_pressed,
                cursor,
            );
            draw_focus_border(
                renderer,
                self,
                reset_layout.bounds(),
                Focus::Reset,
                &style_sheet,
            );
            draw_focus_border(
                renderer,
                self,
                submit_layout.bounds(),
                Focus::Submit,
                &style_sheet,
            );
        }

        // Eye dropper magnifier lens for the floating window shell, drawn
        // last so it floats above every dialog element. The inline widget
        // hosts its lens in a dedicated full-window overlay instead (see
        // [`DropperLens`]), since content drawing is clipped by ancestors.
        if self.lens_in_content_draw
            && self.state.dropper_mode == DropperMode::Picking
            && let Some(frame) = &self.state.dropper_frame
        {
            // Prefer the persisted full-window viewport: the draw pass
            // rebuilds this overlay fresh without a layout call, so `self.viewport`
            // is still the ancestor-clipped construction-time rect there.
            let clamp_bounds = if self.state.window_viewport.width > 0.0
                && self.state.window_viewport.height > 0.0
            {
                self.state.window_viewport
            } else if self.viewport.width > 0.0 && self.viewport.height > 0.0 {
                self.viewport
            } else {
                bounds
            };
            // Own layer so the lens composites above the swatch/recent
            // `with_layer` strips, which would otherwise paint over anything
            // drawn into the base layer afterwards.
            renderer.with_layer(clamp_bounds, |renderer| {
                draw_dropper_lens(
                    renderer,
                    frame,
                    self.state.dropper_cursor,
                    clamp_bounds,
                    &style_sheet[&StyleState::Active],
                );
            });
        }
    }
}

/// A full-window overlay rendering the eye dropper magnifier lens for the
/// inline [`ColorPicker`](crate::color_picker::ColorPicker).
///
/// Content drawing is clipped by ancestors (scrollables, containers); this
/// overlay is laid out against the whole window, so the lens can follow the
/// cursor across the entire application window instead of being confined to
/// the picker's own bounds. It is purely visual: events are handled by the
/// dialog content as usual.
#[allow(missing_debug_implementations)]
pub(crate) struct DropperLens<'a, 'b, Theme>
where
    Theme: style::Catalog,
{
    /// The frozen snapshot being sampled.
    frame: &'a Frame,
    /// The hovered point (window coordinates, logical pixels).
    cursor: Point,
    /// The area of the window the lens is clamped to; captured during
    /// `layout`.
    clamp_bounds: Rectangle,
    /// The style class of the hosting picker.
    class: &'a <Theme as style::Catalog>::Class<'b>,
}

impl<'a, 'b, Theme> DropperLens<'a, 'b, Theme>
where
    Theme: style::Catalog + 'a,
    'b: 'a,
{
    /// Creates a new lens overlay for the given frozen snapshot and hovered
    /// point.
    pub(crate) fn new(
        frame: &'a Frame,
        cursor: Point,
        class: &'a <Theme as style::Catalog>::Class<'b>,
    ) -> Self {
        Self {
            frame,
            cursor,
            clamp_bounds: Rectangle::default(),
            class,
        }
    }

    /// Turns this lens into an overlay [`Element`](overlay::Element).
    pub(crate) fn overlay<Message>(self) -> overlay::Element<'a, Message, Theme, Renderer>
    where
        Message: Clone,
    {
        overlay::Element::new(Box::new(self))
    }
}

impl<'a, 'b, Message, Theme> overlay::Overlay<Message, Theme, Renderer>
    for DropperLens<'a, 'b, Theme>
where
    Message: Clone,
    Theme: style::Catalog + 'a,
    'b: 'a,
{
    fn layout(&mut self, _renderer: &Renderer, bounds: Size) -> Node {
        // Remember the full window extents for drawing; the anchor itself
        // is a zero-size node at the origin so it never affects layout.
        self.clamp_bounds = Rectangle::with_size(bounds);
        Node::new(Size::ZERO)
    }

    fn update(
        &mut self,
        _event: &Event,
        _layout: Layout<'_>,
        _cursor: Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        _shell: &mut Shell<Message>,
    ) {
    }

    fn mouse_interaction(
        &self,
        _layout: Layout<'_>,
        _cursor: Cursor,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        // Crosshair over the whole window while picking. Reporting a
        // non-default interaction also keeps the underlying UI unhovered.
        mouse::Interaction::Crosshair
    }

    fn draw(&self, renderer: &mut Renderer, theme: &Theme, _style: &renderer::Style, _layout: Layout<'_>, _cursor: Cursor) {
        let style = style::Catalog::style(theme, self.class, Status::Active);
        let frame = self.frame;
        let cursor = self.cursor;
        let clamp_bounds = self.clamp_bounds;
        // Own layer so the lens composites above the swatch/recent
        // `with_layer` strips, which would otherwise paint over anything
        // drawn into the base layer afterwards.
        renderer.with_layer(clamp_bounds, |renderer| {
            draw_dropper_lens(renderer, frame, cursor, clamp_bounds, &style);
        });
    }
}

/// A free-floating, window-like shell hosting the color picker dialog.
///
/// It is a regular overlay (still a widget inside the iced tree, not an OS
/// window): a draggable header strip with an empty drag area and a close
/// ("x") button on top of the dialog content. The dialog can be dragged
/// anywhere inside the viewport by its header; the dragged position is kept
/// in [`State`] and survives close/reopen. Spawning one is the job of
/// [`FloatingColorPicker`](crate::color_picker::FloatingColorPicker).
#[allow(missing_debug_implementations)]
pub struct ColorPickerWindow<'a, 'b, Message, Theme>
where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog + iced::widget::text_input::Catalog,
{
    /// The dialog content hosted below the header.
    content: ColorPickerOverlay<'a, 'b, Message, Theme>,
    /// The message published when the header close ("x") button is pressed.
    on_close: Message,
    /// The initial position strategy; dragging overrides it afterwards.
    position: Option<OverlayPosition>,
    /// The bounds of the underlay widget, for parent-relative positions.
    parent_bounds: Rectangle,
    /// The underlay center, used as the anchor point when `position` is
    /// [`None`] (the default behavior).
    fallback_center: Point,
    /// The last known cursor position, for cursor-following positions.
    cursor_position: Point,
}
impl<'a, 'b, Message, Theme> ColorPickerWindow<'a, 'b, Message, Theme>
where
    Message: 'static + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
    'b: 'a,
{
    /// Creates a new [`ColorPickerWindow`] at the given position strategy.
    ///
    /// A [`None`] position centers the window over `fallback_center` and
    /// bounces it back into the viewport; a [`Some`] position resolves like
    /// the [`OverlayManager`](crate::overlay::OverlayManager) and is clamped
    /// to the viewport. Either way the position is only used until the user
    /// drags the window by its header; afterwards the dragged spot wins and
    /// survives close/reopen.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        state: &'a mut State,
        on_cancel: Message,
        on_submit: &'a dyn Fn(Color) -> Message,
        on_color_change: Option<&'a dyn Fn(Color) -> Message>,
        on_gradient_submit: Option<&'a dyn Fn(Gradient) -> Message>,
        on_gradient_change: Option<&'a dyn Fn(Gradient) -> Message>,
        on_pick: Option<&'a dyn Fn(PickedValue) -> Message>,
        on_pick_submit: Option<&'a dyn Fn(PickedValue) -> Message>,
        dropper_buffer: Option<&'a DropperBuffer>,
        on_dropper_capture: Option<&'a dyn Fn() -> Message>,
        position: Option<OverlayPosition>,
        parent_bounds: Rectangle,
        fallback_center: Point,
        cursor_position: Point,
        class: &'a <Theme as style::Catalog>::Class<'b>,
        tree: &'a mut Tree,
        viewport: Rectangle,
    ) -> Self
    where
        for<'c> <Theme as iced::widget::text_input::Catalog>::Class<'c>:
            From<iced::widget::text_input::StyleFn<'c, Theme>>,
    {
        Self {
            content: ColorPickerOverlay::new(
                state,
                on_cancel.clone(),
                on_submit,
                on_color_change,
                on_gradient_submit,
                on_gradient_change,
                on_pick,
                on_pick_submit,
                dropper_buffer,
                on_dropper_capture,
                true,
                class,
                tree,
                viewport,
            ),
            on_close: on_cancel,
            position,
            parent_bounds,
            fallback_center,
            cursor_position,
        }
    }

    /// Turn this [`ColorPickerWindow`] into an overlay [`Element`](overlay::Element).
    #[must_use]
    pub fn overlay(self) -> overlay::Element<'a, Message, Theme, Renderer> {
        overlay::Element::new(Box::new(self))
    }

    /// The base (first-open) origin of the window for its current `size`.
    fn base_position(&self, size: Size, bounds: Size) -> Point {
        match self.position {
            Some(position) => {
                let viewport = Rectangle::with_size(bounds);
                let point = position.resolve(
                    self.parent_bounds,
                    self.cursor_position,
                    viewport,
                    Rectangle::new(Point::ORIGIN, size),
                    &[],
                );
                clamp_to_viewport(point, size, viewport)
            }
            None => centered_bounded_point(self.fallback_center, size, bounds),
        }
    }

    /// Whether the window header is currently being dragged.
    fn is_dragging(&self) -> bool {
        self.content.state.header_drag_offset.is_some()
    }
}

impl<'a, Message, Theme> Overlay<Message, Theme, Renderer>
    for ColorPickerWindow<'a, '_, Message, Theme>
where
    Message: 'static + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> Node {
        let viewport = Rectangle::with_size(bounds);
        // Overlay layout bounds are the full window; the construction-time
        // viewport from HexColorInput/FloatingColorPicker is clipped to the
        // ancestor scrollable/pane. Sync here so drag/lens/hit-test clamp to
        // the window and the picker moves freely in the viewport. The draw
        // pass rebuilds without layout, so persist it in the state as well
        // for the magnifier lens to read there.
        self.content.viewport = viewport;
        self.content.state.window_viewport = viewport;

        // Dialog content below the header strip.
        let available = Size::new(
            bounds.width,
            (bounds.height - HEADER_HEIGHT).max(0.0),
        );
        let content_node = self.content.layout_content(renderer, available);

        let width = content_node.size().width.max(CLOSE_BUTTON_SIZE + 12.0);
        let total = Size::new(width, HEADER_HEIGHT + content_node.size().height);

        // Header: empty drag strip with a reserved slot for the close
        // button (drawn and hit-tested manually from this rect).
        let close_slot =
            close_button_rect(Rectangle::new(Point::ORIGIN, Size::new(width, HEADER_HEIGHT)));
        let header_node = Node::with_children(
            Size::new(width, HEADER_HEIGHT),
            vec![Node::with_children(
                Size::new(CLOSE_BUTTON_SIZE, CLOSE_BUTTON_SIZE),
                Vec::new(),
            )
            .move_to(close_slot.position())],
        );

        let mut node = Node::with_children(
            total,
            vec![
                header_node,
                content_node.move_to(Point::new(0.0, HEADER_HEIGHT)),
            ],
        );

        // First open resolves the configured base position; afterwards the
        // persisted (user-dragged) position wins. Clamped every frame so a
        // resized viewport keeps the window fully visible.
        let base = self.base_position(total, bounds);
        let origin = self.content.state.dialog_position.unwrap_or(base);
        let clamped = clamp_to_viewport(origin, total, viewport);
        node.move_to_mut(clamped);
        self.content.state.dialog_position = Some(clamped);

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
        let mut children = layout.children();
        let _header_layout = children
            .next()
            .expect("widget: Layout should have a header layout");
        let content_layout = children
            .next()
            .expect("widget: Layout should have a content layout");

        // Forward to the dialog content first; the header sits above it
        // spatially so the two hit-test areas are disjoint.
        self.content.update_content(
            event, content_layout, cursor, renderer, clipboard, shell,
        );

        let dialog_bounds = layout.bounds();
        let header_rect = Rectangle::new(
            dialog_bounds.position(),
            Size::new(dialog_bounds.width, HEADER_HEIGHT),
        );
        let close_rect = close_button_rect(header_rect);
        let (top_color_rect, top_gradient_rect, top_library_rect) = top_tab_rects(header_rect);

        let on_close = self.on_close.clone();

        // Header hover bookkeeping for the top tabs.
        if matches!(
            event,
            Event::Mouse(
                mouse::Event::CursorMoved { .. }
                    | mouse::Event::ButtonPressed(_)
                    | mouse::Event::ButtonReleased(_),
            ) | Event::Touch(touch::Event::FingerMoved { .. })
        ) {
            self.content.state.top_color_hovered = cursor.is_over(top_color_rect);
            self.content.state.top_gradient_hovered = cursor.is_over(top_gradient_rect);
            self.content.state.top_library_hovered = cursor.is_over(top_library_rect);
        }

        // Window chrome interactions: top tabs, dragging by the header and
        // the close ("x") button.
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if cursor.is_over(close_rect) {
                    self.content.state.close_pressed = true;
                    shell.capture_event();
                } else if cursor.is_over(top_color_rect) {
                    if self.content.state.picker_tab != PickerTab::Color {
                        self.content.state.picker_tab = PickerTab::Color;
                        self.content.state.focus = Focus::TopColor;
                        shell.invalidate_layout();
                        shell.request_redraw();
                    }
                    shell.capture_event();
                } else if cursor.is_over(top_gradient_rect) {
                    if self.content.state.picker_tab != PickerTab::Gradient {
                        self.content.state.picker_tab = PickerTab::Gradient;
                        self.content.state.focus = Focus::TopGradient;
                        self.content.state.select_stop(self.content.state.selected_stop);
                        shell.invalidate_layout();
                        shell.request_redraw();
                    }
                    shell.capture_event();
                } else if cursor.is_over(top_library_rect) {
                    if self.content.state.picker_tab != PickerTab::Library {
                        self.content.state.picker_tab = PickerTab::Library;
                        self.content.state.focus = Focus::TopLibrary;
                        shell.invalidate_layout();
                        shell.request_redraw();
                    }
                    shell.capture_event();
                } else if cursor.is_over(header_rect)
                    && let Some(grab) = cursor.land().position()
                {
                    self.content.state.header_drag_offset =
                        Some(grab - dialog_bounds.position());
                    shell.capture_event();
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(
                touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. },
            ) => {
                let was_dragging = self.content.state.header_drag_offset.is_some();
                self.content.state.header_drag_offset = None;
                let was_pressed = self.content.state.close_pressed;
                self.content.state.close_pressed = false;
                if was_pressed && cursor.is_over(close_rect) {
                    shell.publish(on_close);
                    shell.capture_event();
                    shell.request_redraw();
                } else if was_dragging {
                    shell.capture_event();
                }
            }
            _ => {}
        }

        if let Some(offset) = self.content.state.header_drag_offset
            && let Some(grab) = cursor.land().position()
            && matches!(
                event,
                Event::Mouse(mouse::Event::CursorMoved { .. })
                    | Event::Touch(touch::Event::FingerMoved { .. })
            )
        {
            let desired = grab - offset;
            let clamped = clamp_to_viewport(desired, dialog_bounds.size(), self.content.viewport);
            self.content.state.dialog_position = Some(clamped);
            shell.capture_event();
            shell.request_redraw();
        }

        // Strictly forward: consume a hit only when it lands on the
        // floating window itself (`dialog_bounds`) or while a header drag
        // is in progress. Anything outside falls through to the underlay
        // so `UserInterface` forwards it to the base widgets.
        // Cursor moves stay ungated outside the dialog so
        // cursor-following `Position`s tracked in
        // `FloatingColorPicker::update` keep working.
        match event {
            Event::Mouse(
                mouse::Event::ButtonPressed(_)
                | mouse::Event::ButtonReleased(_)
                | mouse::Event::WheelScrolled { .. },
            )
            | Event::Touch(
                touch::Event::FingerPressed { .. }
                | touch::Event::FingerLifted { .. }
                | touch::Event::FingerLost { .. },
            ) => {
                if cursor.is_over(dialog_bounds) || self.is_dragging() {
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. })
            | Event::Touch(touch::Event::FingerMoved { .. }) => {
                if cursor.is_over(dialog_bounds) || self.is_dragging() {
                    shell.capture_event();
                }
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let mut children = layout.children();
        let _header_layout = children
            .next()
            .expect("Graphics: Layout should have a header layout");
        let content_layout = children
            .next()
            .expect("Graphics: Layout should have a content layout");

        let mut interaction = self
            .content
            .mouse_interaction_content(content_layout, cursor, renderer);

        let header_rect = Rectangle::new(
            layout.bounds().position(),
            Size::new(layout.bounds().width, HEADER_HEIGHT),
        );
        let close_rect = close_button_rect(header_rect);
        let (top_color_rect, top_gradient_rect, top_library_rect) = top_tab_rects(header_rect);

        if cursor.is_over(close_rect)
            || cursor.is_over(top_color_rect)
            || cursor.is_over(top_gradient_rect)
            || cursor.is_over(top_library_rect)
        {
            interaction = interaction.max(mouse::Interaction::Pointer);
        } else if cursor.is_over(header_rect) || self.is_dragging() {
            interaction = interaction.max(mouse::Interaction::Grabbing);
        }

        // Strictly forward: while the eyedropper is picking the overlay
        // stays modal (its update swallows every event). Otherwise only
        // report a hit when the cursor is over the window itself (or a
        // header drag is in progress, so the grab survives leaving the
        // window). Outside the window report `None` so the base tree keeps
        // its cursor and hover. Inside the window ensure at least `Idle`
        // so empty padding still blocks the base tree.
        if self.content.state.dropper_mode != DropperMode::Idle {
            return interaction.max(mouse::Interaction::Idle);
        }

        if cursor.is_over(layout.bounds()) || self.is_dragging() {
            interaction.max(mouse::Interaction::Idle)
        } else {
            mouse::Interaction::None
        }
    }

    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        let mut children = layout.children();
        let _header_layout = children.next();
        if let Some(content_layout) = children.next() {
            self.content.operate_content(content_layout, renderer, operation);
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
        let mut children = layout.children();
        let header_layout = children
            .next()
            .expect("Graphics: Layout should have a header layout");
        let content_layout = children
            .next()
            .expect("Graphics: Layout should have a content layout");

        let active = style::Catalog::style(theme, self.content.class, Status::Active);

        // Header background with rounded top corners matching the dialog.
        let header_bounds = header_layout.bounds();
        renderer.fill_quad(
            renderer::Quad {
                bounds: header_bounds,
                border: Border {
                    radius: Radius {
                        top_left: active.border_radius,
                        top_right: active.border_radius,
                        bottom_left: 0.0,
                        bottom_right: 0.0,
                    },
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                ..renderer::Quad::default()
            },
            active.header_background,
        );

        // Divider line under the header.
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle::new(
                    Point::new(header_bounds.x, header_bounds.y + header_bounds.height - 1.0),
                    Size::new(header_bounds.width, 1.0),
                ),
                ..renderer::Quad::default()
            },
            active.header_border_color,
        );

        // Top-level tabs on the left of the draggable header.
        draw_header_tabs(
            renderer,
            header_bounds,
            self.content.state.picker_tab,
            self.content.state.top_color_hovered,
            self.content.state.top_gradient_hovered,
            self.content.state.top_library_hovered,
            cursor,
            &active,
            active.tab_selected_background,
        );

        // Close ("x") button.
        let close_rect = close_button_rect(header_bounds);
        let hovered = cursor.is_over(close_rect);
        let pressed = self.content.state.close_pressed;
        let close_background = if pressed {
            active.close_button_hover_background
        } else if hovered {
            lerp(active.close_button_background, active.close_button_hover_background, 0.6)
        } else {
            active.close_button_background
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds: close_rect,
                border: Border {
                    radius: 4.0.into(),
                    width: 1.0,
                    color: active.close_button_border_color,
                },
                ..renderer::Quad::default()
            },
            close_background,
        );
        draw_svg_icon(
            renderer,
            CANCEL_SVG,
            close_rect,
            header_bounds,
            if hovered || pressed {
                active.text_primary
            } else {
                active.close_symbol_color
            },
        );

        self.content.draw_content(
            renderer, theme, style, content_layout, cursor,
        );
    }
}


/// Defines the layout of the left pane: picker (ring + sat/value square),
/// tab bar, slider controls column and the hex container.
/// Legacy two-pane helper, kept but unused by the single-column layout.
#[allow(dead_code)]
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

        // The value cells host the channel [`TextInput`]s of the active tab.
        let value_input_index = match (color_picker.state.active_tab, row) {
            (ActiveTab::Rgb, i) => Some(i),
            (ActiveTab::Hsv, 3) => Some(3),
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
        Size::new(HEX_LABEL_WIDTH, HEX_CONTAINER_HEIGHT),
        Vec::new(),
    )
    .move_to(Point::new(0.0, 0.0));
    let legacy_input_y = ((HEX_CONTAINER_HEIGHT - hex_input_node.size().height) / 2.0).max(0.0);
    hex_input_node = hex_input_node.move_to(Point::new(HEX_LABEL_WIDTH, legacy_input_y));
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

/// Height of the hex container. Just enough for a single-line input plus a
/// slim panel padding.
const HEX_CONTAINER_HEIGHT: f32 = 32.0;
/// Width of the "Hex:" label cell. Wide enough for the 4-char label at the
/// default text size without touching the container/input edges.
const HEX_LABEL_WIDTH: f32 = 40.0;
/// Right inset of the hex `TextInput` from the container border.
const HEX_INPUT_RIGHT_INSET: f32 = 4.0;
/// Height of the preview area (panels + labels) in the right pane.
const PREVIEW_AREA_HEIGHT: f32 = PREVIEW_HEIGHT + 18.0 + 2.0;
/// Margin of the swatch strips inside the tab page.
const SWATCH_PAGE_MARGIN: f32 = 5.0;
/// The fixed height of the swatch/recent strips: [`STRIP_ROWS`] rows of
/// cells plus the page margin above and below.
const STRIP_HEIGHT: f32 = STRIP_ROWS as f32 * SWATCH_SIZE
    + (STRIP_ROWS - 1) as f32 * GRID_SPACING
    + 2.0 * SWATCH_PAGE_MARGIN;
/// Vertical margin above/below the cells of the mirrored single-row recent
/// strip. Slimmer than [`SWATCH_PAGE_MARGIN`] so the Color-tab mirror stays
/// compact.
const RECENT_SINGLE_VERT_MARGIN: f32 = 2.0;
/// The fixed height of the mirrored single-row recent strip shown in the
/// Color tab below the hex input.
const RECENT_SINGLE_ROW_HEIGHT: f32 = SWATCH_SIZE + 2.0 * RECENT_SINGLE_VERT_MARGIN;
/// The row count of the mirrored single-row recent strip.
const RECENT_SINGLE_ROWS: usize = 1;
/// Height of the "new swatch set" name prompt band, centered vertically
/// inside the tab page.
const NAME_PROMPT_HEIGHT: f32 = 32.0;
/// Height of the section headings ("Swatches", "Recent").
const LABEL_HEIGHT: f32 = 18.0;
/// Height of the divider.
const DIVIDER_HEIGHT: f32 = 2.0;
/// Height of the buttons row in the right pane.
const BUTTONS_HEIGHT: f32 = 32.0;
/// Width of the Reset icon button (square).
const RESET_WIDTH: f32 = 32.0;
/// Spacing between the right pane children.
#[allow(dead_code)]
const RIGHT_PANE_SPACING: f32 = 10.0;

/// Defines the layout of the right pane: previews, swatches, recent colors
/// and the Reset/Eyedropper/OK buttons.
/// Legacy two-pane helper, kept but unused by the single-column layout.
#[allow(dead_code)]
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

    // [3] Swatch tab page: either the active set's strip or the "new swatch
    // set" name prompt. Both share the fixed strip height so toggling the
    // prompt never reflows the dialog.
    let page_height = STRIP_HEIGHT;

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
        // Fixed strip: cells flow down [`STRIP_ROWS`] rows and then into
        // further columns; placeholder wells pad out the viewport so the
        // layout stays constant until the strip overflows and scrolls.
        let cells = set.colors.len().max(STRIP_ROWS * visible_cols(width));
        let scroll =
            clamp_strip_scroll(color_picker.state.swatch_scroll_x, set.colors.len(), width);
        for i in 0..cells {
            let col = i / STRIP_ROWS;
            let row = i % STRIP_ROWS;
            let cell = Node::with_children(Size::new(SWATCH_SIZE, SWATCH_SIZE), Vec::new())
                .move_to(Point::new(
                    SWATCH_PAGE_MARGIN + col as f32 * CELL_PITCH - scroll,
                    SWATCH_PAGE_MARGIN + row as f32 * CELL_PITCH,
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

    // [7] Recent strip: [`STRIP_ROWS`] fixed rows, cells flowing down and
    // then into further columns, padded with placeholder wells up to the
    // visible capacity so the layout stays constant; it scrolls
    // horizontally only once more colors arrive than fit.
    let recent_count = color_picker.state.recent_colors.len();
    let recent_cells = recent_count.max(STRIP_ROWS * visible_cols(width));
    let recent_scroll =
        clamp_strip_scroll(color_picker.state.recent_scroll_x, recent_count, width);
    let mut recent_children: Vec<Node> = Vec::new();
    for i in 0..recent_cells {
        let col = i / STRIP_ROWS;
        let row = i % STRIP_ROWS;
        let cell = Node::with_children(Size::new(SWATCH_SIZE, SWATCH_SIZE), Vec::new())
            .move_to(Point::new(
                SWATCH_PAGE_MARGIN + col as f32 * CELL_PITCH - recent_scroll,
                SWATCH_PAGE_MARGIN + row as f32 * CELL_PITCH,
            ));
        recent_children.push(cell);
    }
    let recent_grid_node = Node::with_children(
        Size::new(width, STRIP_HEIGHT),
        recent_children,
    )
    .move_to(Point::new(0.0, offset_y));
    children.push(recent_grid_node);
    offset_y += STRIP_HEIGHT + spacing;

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

    let dropper_button = color_picker
        .dropper_button
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
            dropper_button,
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

/// Draws the top-level `[Color | Library]` tabs of the single column
/// (inline mode) or the draggable header (floating mode calls this with the
/// header rects).
fn draw_top_tabs<Message, Theme>(
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
    let gap = 6.0;
    let w = (bounds.width - 2.0 * gap) / 3.0;
    let tabs = [
        ("Color", PickerTab::Color, bounds.x, color_picker.state.top_color_hovered),
        (
            "Gradient",
            PickerTab::Gradient,
            bounds.x + w + gap,
            color_picker.state.top_gradient_hovered,
        ),
        (
            "Library",
            PickerTab::Library,
            bounds.x + 2.0 * (w + gap),
            color_picker.state.top_library_hovered,
        ),
    ];
    for (label, tab, x, hovered_flag) in tabs {
        let tab_bounds = Rectangle {
            x,
            y: bounds.y,
            width: w,
            height: bounds.height,
        };
        let selected = color_picker.state.picker_tab == tab;
        let background = if selected {
            style_sheet[&StyleState::Selected].tab_selected_background
        } else if cursor.is_over(tab_bounds) || hovered_flag {
            active_style.tab_hover_background
        } else {
            active_style.tab_background
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
            if selected {
                active_style.text_primary
            } else {
                active_style.text_secondary
            },
            tab_bounds,
        );
    }
    let focus_target = match color_picker.state.focus {
        Focus::TopColor | Focus::TopGradient | Focus::TopLibrary => {
            Some(color_picker.state.focus)
        }
        _ => None,
    };
    if let Some(target) = focus_target {
        let gap = 6.0;
        let w = (bounds.width - 2.0 * gap) / 3.0;
        let fb = if target == Focus::TopColor {
            Rectangle {
                x: bounds.x,
                y: bounds.y,
                width: w,
                height: bounds.height,
            }
        } else if target == Focus::TopGradient {
            Rectangle {
                x: bounds.x + w + gap,
                y: bounds.y,
                width: w,
                height: bounds.height,
            }
        } else {
            Rectangle {
                x: bounds.x + 2.0 * (w + gap),
                y: bounds.y,
                width: w,
                height: bounds.height,
            }
        };
        draw_focus_border(renderer, color_picker, fb, target, style_sheet);
    }
}

/// Draws header tabs of the floating window shell.
fn draw_header_tabs(
    renderer: &mut Renderer,
    header: Rectangle,
    picker_tab: PickerTab,
    top_color_hovered: bool,
    top_gradient_hovered: bool,
    top_library_hovered: bool,
    cursor: Cursor,
    style: &Style,
    selected_tab_bg: Background,
) {
    let (color_rect, gradient_rect, library_rect) = top_tab_rects(header);
    for (rect, label, selected, hovered) in [
        (color_rect, "Color", picker_tab == PickerTab::Color, top_color_hovered),
        (
            gradient_rect,
            "Gradient",
            picker_tab == PickerTab::Gradient,
            top_gradient_hovered,
        ),
        (
            library_rect,
            "Library",
            picker_tab == PickerTab::Library,
            top_library_hovered,
        ),
    ] {
        let background = if selected {
            selected_tab_bg
        } else if cursor.is_over(rect) || hovered {
            style.tab_hover_background
        } else {
            style.tab_background
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds: rect,
                border: Border {
                    radius: 5.0.into(),
                    width: 1.0,
                    color: style.tab_border_color,
                },
                ..renderer::Quad::default()
            },
            background,
        );
        renderer.fill_text(
            Text {
                content: label.to_owned(),
                bounds: rect.size(),
                size: Pixels(12.0),
                font: Font::default(),
                align_x: text::Alignment::Center,
                align_y: Vertical::Center,
                line_height: text::LineHeight::Relative(1.0),
                shaping: text::Shaping::Basic,
                wrapping: text::Wrapping::None,
            },
            rect.center(),
            if selected {
                style.text_primary
            } else {
                style.text_secondary
            },
            rect,
        );
    }
}

/// Draws the left pane: picker (ring + sat/value square), tab bar
/// placeholder, slider controls and the hex container.
/// Legacy two-pane helper, kept but unused by the single-column layout.
#[allow(dead_code, clippy::too_many_arguments)]
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

/// Draws one editor sub-tab cell.
fn draw_editor_tab(
    renderer: &mut Renderer,
    tab_bounds: Rectangle,
    label: &str,
    background: Background,
    text_color: Color,
    active_style: &Style,
) {
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

/// Draws the keyboard focus border of the gradient editor tabs.
fn draw_gradient_editor_focus<Message, Theme>(
    renderer: &mut Renderer,
    color_picker: &ColorPickerOverlay<'_, '_, Message, Theme>,
    bounds: Rectangle,
    style_sheet: &HashMap<StyleState, Style>,
) where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    let gap = 2.0;
    let w = (bounds.width - 2.0 * gap) / 3.0;
    let target = match color_picker.state.focus {
        Focus::TabRect | Focus::TabHsv | Focus::TabRgb => Some(color_picker.state.focus),
        _ => None,
    };
    if let Some(target) = target {
        let x = match target {
            Focus::TabRect => bounds.x,
            Focus::TabHsv => bounds.x + w + gap,
            _ => bounds.x + 2.0 * (w + gap),
        };
        draw_focus_border(
            renderer,
            color_picker,
            Rectangle {
                x,
                y: bounds.y,
                width: w,
                height: bounds.height,
            },
            target,
            style_sheet,
        );
    }
}

/// Draws the gradient stop bar (strip + two stop handles). The bar
/// container is the gradient picker node's only child.
fn draw_gradient_bar<Message, Theme>(
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
    let mut picker_children = layout.children();
    let Some(bar) = picker_children.next() else {
        return;
    };
    let mut bar_children = bar.children();
    let Some(strip_layout) = bar_children.next() else {
        return;
    };
    let strip = strip_layout.bounds();
    if strip.width <= 0.0 || strip.height <= 0.0 {
        return;
    }
    let active_style = &style_sheet[&StyleState::Active];
    // Checkerboard underlay for alpha, then the interpolated strip.
    draw_checkerboard(
        renderer,
        strip,
        6.0,
        active_style.checker_color_1,
        active_style.checker_color_2,
    );
    let steps = strip.width as i32;
    for x in 0..steps {
        let t = if strip.width > 1.0 {
            x as f32 / (strip.width - 1.0)
        } else {
            0.0
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle::new(
                    Point::new(strip.x + x as f32, strip.y),
                    Size::new(1.0, strip.height),
                ),
                ..renderer::Quad::default()
            },
            color_picker.state.gradient.sample(t),
        );
    }
    renderer.fill_quad(
        renderer::Quad {
            bounds: strip,
            border: Border {
                radius: active_style.bar_border_radius.into(),
                width: active_style.bar_border_width,
                color: active_style.slider_groove_border_color,
            },
            ..renderer::Quad::default()
        },
        Color::TRANSPARENT,
    );
    // Figma-style stop pins riding on top of the strip: a rounded body
    // with the stop color swatch inside and a pointer nub stabbed down
    // into the exact bar position. Triangles need canvas geometry; quads
    // cannot do them.
    let section = bar.bounds();
    let pins: Vec<(Rectangle, Color, bool, bool)> = (0..2)
        .filter_map(|i| {
            let stop = color_picker.state.gradient.stops.get(i)?;
            let handle = gradient_handle_rect(section, stop.offset);
            if handle.width <= 0.0 || handle.height <= 0.0 {
                return None;
            }
            Some((
                handle,
                stop.color,
                color_picker.state.selected_stop == i,
                cursor.is_over(handle)
                    || cursor.is_over(gradient_pointer_rect(section, stop.offset)),
            ))
        })
        .collect();
    let section_origin = Vector::new(section.x, section.y);
    let pin_geometry = color_picker.state.gradient_handles_cache.draw(
        renderer,
        section.size(),
        |frame| {
            for (handle, stop_color, selected, hovered) in &pins {
                // Work in section-local coordinates.
                let body = Rectangle {
                    x: handle.x - section.x,
                    y: handle.y - section.y,
                    width: handle.width,
                    height: handle.height,
                };
                let cx = body.center_x();
                let border_color = if *selected {
                    active_style.text_primary
                } else {
                    active_style.slider_handle_border_color
                };
                // Unselected pins are greyed out entirely (body + pointer),
                // not just the pointer nub below the rectangle.
                let background = if *selected {
                    if *hovered {
                        active_style.slider_handle_hover_background
                    } else {
                        active_style.slider_handle_background
                    }
                } else {
                    active_style.slider_handle_border_color
                };
                // Pointer nub: base merged into the body bottom, apex
                // stabbed into the bar at the stop's exact position.
                let base_y = body.y + body.height - 2.0;
                let apex = Point::new(
                    cx,
                    GRADIENT_PIN_ROW + GRADIENT_PIN_APEX_DEPTH,
                );
                let pointer = Path::new(|b| {
                    b.move_to(Point::new(cx - GRADIENT_PIN_POINTER_HALF, base_y));
                    b.line_to(Point::new(cx + GRADIENT_PIN_POINTER_HALF, base_y));
                    b.line_to(apex);
                    b.close();
                });
                frame.fill(&pointer, border_color);
                // Pin body with the stop color swatch inside.
                let body_path = Path::rounded_rectangle(
                    body.position(),
                    body.size(),
                    5.0.into(),
                );
                frame.fill(&body_path, background);
                frame.stroke(
                    &body_path,
                    Stroke {
                        style: canvas::Style::Solid(border_color),
                        width: if *selected { 2.0 } else { 1.0 },
                        ..Stroke::default()
                    },
                );
                frame.fill(
                    &Path::rounded_rectangle(
                        Point::new(body.x + 3.0, body.y + 3.0),
                        Size::new((body.width - 6.0).max(1.0), (body.height - 6.0).max(1.0)),
                        3.0.into(),
                    ),
                    *stop_color,
                );
            }
        },
    );
    renderer.with_translation(section_origin, |renderer| {
        renderer.draw_geometry(pin_geometry);
    });
    for (handle, _, selected, _) in &pins {
        if color_picker.state.focus == Focus::GradientBar && *selected {
            draw_focus_border(
                renderer,
                color_picker,
                *handle,
                Focus::GradientBar,
                style_sheet,
            );
        }
    }
}

/// Draws a placeholder for the HSV/RGB(A) tab bar (Color) or the
/// Rect/HSV/RGBA tab bar (Gradient).
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
    if color_picker.state.picker_tab == PickerTab::Gradient {
        let gap = 2.0;
        let w = (bounds.width - 2.0 * gap) / 3.0;
        let tabs = [
            (
                "Rect",
                GradientEditorTab::Rect,
                0.0,
                color_picker.state.tab_rect_hovered,
            ),
            (
                "HSV",
                GradientEditorTab::Hsv,
                w + gap,
                color_picker.state.tab_hsv_hovered,
            ),
            (
                "RGBA",
                GradientEditorTab::Rgba,
                2.0 * (w + gap),
                color_picker.state.tab_rgb_hovered,
            ),
        ];
        for (label, tab, x, hovered) in tabs {
            let tab_bounds = Rectangle {
                x: bounds.x + x,
                y: bounds.y,
                width: w,
                height: bounds.height,
            };
            let selected = color_picker.state.gradient_editor == tab;
            let background = if selected {
                style_sheet[&StyleState::Selected].tab_selected_background
            } else if cursor.is_over(tab_bounds) || hovered {
                active_style.tab_hover_background
            } else {
                active_style.tab_background
            };
            let text_color = if selected {
                active_style.text_primary
            } else {
                active_style.text_secondary
            };
            draw_editor_tab(renderer, tab_bounds, label, background, text_color, &active_style);
        }
        draw_gradient_editor_focus(renderer, color_picker, bounds, style_sheet);
        return;
    }
    let gap = 2.0;
    let half = (bounds.width - gap) / 2.0;

    let tabs = [("HSV", ActiveTab::Hsv, 0.0), ("RGB(A)", ActiveTab::Rgb, half + gap)];

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
/// Legacy two-pane helper, kept but unused by the single-column layout.
#[allow(dead_code, clippy::too_many_arguments)]
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

    let dropper_button_layout = buttons_layout
        .next()
        .expect("Graphics: Layout should have an eyedropper button layout for a ColorPicker");

    draw_overlay_button(
        renderer,
        theme,
        dropper_icon().0,
        dropper_button_layout.bounds(),
        color_picker.state.dropper_pressed,
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
        dropper_button_layout.bounds(),
        Focus::Dropper,
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

/// Draws the swatch tab page: the active set's strip (checkerboard + color
/// fill + hover/focus border per cell, placeholder wells beyond the set's
/// length), or the "new swatch set" name prompt when it is open.
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
    let viewport = layout.bounds();

    // The strip is clipped to its viewport so scrolled-out cells stay
    // hidden.
    renderer.with_layer(viewport, |renderer| {
        for (i, cell_layout) in layout.children().enumerate() {
            let cell = cell_layout.bounds();
            let Some(picked) = set.colors.get(i) else {
                draw_placeholder_well(
                    renderer,
                    cell,
                    tile,
                    checker_1,
                    checker_2,
                    active_style.swatch_border_color,
                );
                continue;
            };

            // Checkerboard behind the fill; tiles are drawn as solid quads so
            // they stack below the fill (the renderer batches quads and
            // meshes separately, and a mesh would always draw on top of a quad).
            draw_checkerboard(renderer, cell, tile, checker_1, checker_2);

            // Picked fill on top (alpha-composited over the checkerboard).
            draw_picked_fill(renderer, cell, picked);

            // Border: hover / keyboard focus highlight.
            let focused = color_picker.state.focused_swatch
                == Some((color_picker.state.active_swatch_tab, i));
            let border_color =
                if (cursor.is_over(cell) && viewport.intersects(&cell)) || focused {
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
    });
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

/// Draws the recent colors strip: checkerboard + color fill + hover border
/// per cell, placeholder wells beyond the list length, clipped to the
/// strip's viewport (same drawing as the swatch strip).
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
    let viewport = layout.bounds();

    // The strip is clipped to its viewport so scrolled-out cells stay
    // hidden.
    renderer.with_layer(viewport, |renderer| {
        for (i, cell_layout) in layout.children().enumerate() {
            let cell = cell_layout.bounds();
            let Some(picked) = color_picker.state.recent_colors.get(i) else {
                draw_placeholder_well(
                    renderer,
                    cell,
                    tile,
                    checker_1,
                    checker_2,
                    active_style.swatch_border_color,
                );
                continue;
            };

            draw_checkerboard(renderer, cell, tile, checker_1, checker_2);

            draw_picked_fill(renderer, cell, picked);

            renderer.fill_quad(
                renderer::Quad {
                    bounds: cell,
                    border: Border {
                        radius: 2.0.into(),
                        width: 1.0,
                        color: if cursor.is_over(cell) && viewport.intersects(&cell) {
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
    });
}

/// Draws an empty placeholder well of the swatch/recent strips: a plain
/// checkerboard with the idle swatch border; placeholders are not
/// interactive.
fn draw_placeholder_well(
    renderer: &mut Renderer,
    bounds: Rectangle,
    tile: f32,
    checker_1: Color,
    checker_2: Color,
    border_color: Color,
) {
    draw_checkerboard(renderer, bounds, tile, checker_1, checker_2);

    renderer.fill_quad(
        renderer::Quad {
            bounds,
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

/// Fills `bounds` with a picked value: a single quad for solids, a sampled
/// horizontal gradient strip for gradients (same technique as the gradient
/// bar, so stop offsets are honored and alpha shows the checkerboard).
fn draw_picked_fill(renderer: &mut Renderer, bounds: Rectangle, picked: &PickedValue) {
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return;
    }
    match picked {
        PickedValue::Solid(color) => {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    ..renderer::Quad::default()
                },
                *color,
            );
        }
        PickedValue::Gradient(gradient) => {
            draw_gradient_fill(renderer, bounds, gradient);
        }
    }
}

/// Fills `bounds` with a horizontal sampled gradient.
fn draw_gradient_fill(renderer: &mut Renderer, bounds: Rectangle, gradient: &Gradient) {
    let steps = bounds.width.ceil() as i32;
    if steps <= 0 {
        return;
    }
    // Fast path for solid gradients (two identical stops): one quad.
    if gradient.stops.len() == 2
        && same_rgba(gradient.stops[0].color, gradient.stops[1].color)
        && (gradient.stops[0].offset - 0.0).abs() < f32::EPSILON
        && (gradient.stops[1].offset - 1.0).abs() < f32::EPSILON
    {
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                ..renderer::Quad::default()
            },
            gradient.stops[0].color,
        );
        return;
    }
    for x in 0..steps {
        let t = if bounds.width > 1.0 {
            x as f32 / (bounds.width - 1.0)
        } else {
            0.0
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle::new(
                    Point::new(bounds.x + x as f32, bounds.y),
                    Size::new(1.0, bounds.height),
                ),
                ..renderer::Quad::default()
            },
            gradient.sample(t),
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
    // Original = snapshot from before open (solid or gradient).
    // New = live current value (solid or gradient).
    let panels = [
        (
            "Original",
            color_picker.state.initial_picked(),
            Rectangle {
                x: bounds.x,
                y: bounds.y,
                width: panel_width,
                height: PREVIEW_HEIGHT,
            },
        ),
        (
            "New",
            color_picker.state.current_picked(),
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

    for (label, picked, panel_bounds) in panels {
        // Checkerboard behind the fill; tiles are drawn as solid quads so
        // they stack below the fill (the renderer batches quads and
        // meshes separately, and a mesh would always draw on top of a quad).
        draw_checkerboard(renderer, panel_bounds, tile, checker_1, checker_2);

        // Picked fill on top (alpha-composited over the checkerboard).
        draw_picked_fill(renderer, panel_bounds, &picked);

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

/// Draws the Reset icon button with its danger palette colors.
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

    draw_svg_icon(renderer, RESET_SVG, bounds, bounds, active_style.text_primary);
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

/// Draws an icon button (e.g. the eyedropper) with the button-catalog
/// background and an SVG glyph tinted with the button text color.
/// `disabled` forces the Disabled catalog status and dims the glyph.
fn draw_icon_overlay_button<Theme>(
    renderer: &mut Renderer,
    theme: &Theme,
    icon: &'static [u8],
    bounds: Rectangle,
    pressed: bool,
    cursor: Cursor,
    disabled: bool,
) where
    Theme: iced::widget::button::Catalog,
{
    let status = if disabled {
        button::Status::Disabled
    } else if pressed && cursor.is_over(bounds) {
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

    let mut icon_color = style.text_color;
    if disabled {
        icon_color.a *= 0.4;
    }
    draw_svg_icon(renderer, icon, bounds, bounds, icon_color);
}
/// The total span of the magnifier pixel grid (logical pixels).
fn lens_grid_span() -> f32 {
    let cells = (2 * LENS_SRC_RADIUS + 1) as f32;
    cells * LENS_CELL + (cells - 1.0) * LENS_GAP
}

/// The full size of the magnifier backdrop: pixel grid plus padding and the
/// hex readout pill below it.
fn lens_backdrop_size() -> Size {
    let span = lens_grid_span();
    Size::new(
        span + 2.0 * LENS_PAD,
        LENS_PAD + span + LENS_PAD + LENS_PILL_HEIGHT,
    )
}

/// Positions the magnifier backdrop relative to the cursor, flipping to the
/// opposite quadrant near the right/bottom edges of `bounds` and clamping
/// so it stays fully inside them (never leaves the window).
fn lens_backdrop_rect(cursor: Point, bounds: Rectangle) -> Rectangle {
    let size = lens_backdrop_size();

    let mut x = cursor.x + LENS_CURSOR_MARGIN;
    let mut y = cursor.y + LENS_CURSOR_MARGIN;

    if x + size.width > bounds.x + bounds.width {
        x = cursor.x - LENS_CURSOR_MARGIN - size.width;
    }
    if y + size.height > bounds.y + bounds.height {
        y = cursor.y - LENS_CURSOR_MARGIN - size.height;
    }

    x = x.max(bounds.x).min((bounds.x + bounds.width - size.width).max(bounds.x));
    y = y.max(bounds.y).min((bounds.y + bounds.height - size.height).max(bounds.y));

    Rectangle::new(Point::new(x, y), size)
}

/// Formats a [`Color`] as an opaque `#RRGGBB` string for the lens readout.
fn rgb_hex_string(color: Color) -> String {
    fn byte(v: f32) -> u8 {
        (v * 255.0).round().clamp(0.0, 255.0) as u8
    }
    format!("#{:02X}{:02X}{:02X}", byte(color.r), byte(color.g), byte(color.b))
}

/// Draws the eye dropper magnifier lens: a zoomed pixel grid around the
/// hovered source pixel, a crosshair marking the exact pixel and a pill
/// with its `#RRGGBB` value. Pixels outside the captured frame render as
/// checkerboard.
fn draw_dropper_lens(
    renderer: &mut Renderer,
    frame: &Frame,
    hovered: Point,
    clamp_bounds: Rectangle,
    style: &Style,
) {
    let backdrop = lens_backdrop_rect(hovered, clamp_bounds);

    // Backdrop panel.
    renderer.fill_quad(
        renderer::Quad {
            bounds: backdrop,
            border: Border {
                radius: 6.0.into(),
                width: 1.0,
                color: style.lens_border_color,
            },
            ..renderer::Quad::default()
        },
        Background::Color(style.lens_backdrop),
    );

    // Zoomed pixel grid. Quantize the cursor to its physical source pixel
    // first, then offset in whole physical pixels: offsetting in logical
    // pixels (`hovered + dx`) skips pixels at scale != 1 and lets the
    // sub-pixel fraction of the cursor reshuffle the grid while moving
    // inside a single source pixel.
    let center = LENS_SRC_RADIUS;
    let pitch = LENS_CELL + LENS_GAP;
    let grid_origin = Point::new(backdrop.x + LENS_PAD, backdrop.y + LENS_PAD);
    let radius = 2.0;
    let center_px = frame.to_physical(hovered.x, hovered.y);

    for dy in -center..=center {
        for dx in -center..=center {
            let cell = Rectangle::new(
                Point::new(
                    grid_origin.x + (dx + center) as f32 * pitch,
                    grid_origin.y + (dy + center) as f32 * pitch,
                ),
                Size::new(LENS_CELL, LENS_CELL),
            );
            let color = center_px
                .and_then(|(pcx, pcy)| frame.sample_pixel(pcx + dx, pcy + dy))
                .unwrap_or(style.checker_color_2);

            renderer.fill_quad(
                renderer::Quad {
                    bounds: cell,
                    border: Border {
                        radius: radius.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..renderer::Quad::default()
                },
                Background::Color(color),
            );
        }
    }

    // Crosshair ring around the exact hovered pixel.
    let center_cell = Rectangle::new(
        Point::new(
            grid_origin.x + center as f32 * pitch - 2.0,
            grid_origin.y + center as f32 * pitch - 2.0,
        ),
        Size::new(LENS_CELL + 4.0, LENS_CELL + 4.0),
    );
    renderer.fill_quad(
        renderer::Quad {
            bounds: center_cell,
            border: Border {
                radius: (radius + 2.0).into(),
                width: 2.0,
                color: style.lens_crosshair_color,
            },
            ..renderer::Quad::default()
        },
        Background::Color(Color::TRANSPARENT),
    );

    // Hex readout pill.
    let pill = Rectangle::new(
        Point::new(backdrop.x + LENS_PAD, backdrop.y + LENS_PAD + lens_grid_span() + LENS_PAD),
        Size::new(lens_grid_span(), LENS_PILL_HEIGHT),
    );
    renderer.fill_quad(
        renderer::Quad {
            bounds: pill,
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: style.panel_border_color,
            },
            ..renderer::Quad::default()
        },
        Background::Color(style.lens_pill_background),
    );

    if let Some((pcx, pcy)) = center_px
        && let Some(color) = frame.sample_pixel(pcx, pcy)
    {
        renderer.fill_text(
            Text {
                content: rgb_hex_string(color),
                bounds: pill.size(),
                size: Pixels(13.0),
                font: Font::default(),
                align_x: text::Alignment::Center,
                align_y: Vertical::Center,
                line_height: text::LineHeight::Relative(1.0),
                shaping: text::Shaping::Basic,
                wrapping: text::Wrapping::None,
            },
            pill.center(),
            style.lens_pill_text,
            pill,
        );
    }
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
    // Normal horizontal hue slider below the square, same gradient-bar look
    // as the channel rows.
    let hue_bounds = hue_layout.bounds();
    if (hue_bounds.width > 0.) && (hue_bounds.height > 0.) {
        let active_style = &style_sheet[&StyleState::Active];
        for x in 0..hue_bounds.width as i32 {
            let t = if hue_bounds.width > 1.0 {
                x as f32 / (hue_bounds.width - 1.0)
            } else {
                0.0
            };
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle::new(
                        Point::new(hue_bounds.x + x as f32, hue_bounds.y),
                        Size::new(1.0, hue_bounds.height),
                    ),
                    ..renderer::Quad::default()
                },
                Color::from(Hsv::from_hsv((t * 360.0) as u16 % 360, 1.0, 1.0)),
            );
        }
        let fraction = f32::from(hsv_color.hue) / 360.0;
        let handle_center = Point::new(
            hue_bounds.x + hue_bounds.width * fraction,
            hue_bounds.y + hue_bounds.height / 2.0,
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
        renderer.fill_quad(
            renderer::Quad {
                bounds: hue_bounds,
                border: Border {
                    radius: active_style.bar_border_radius.into(),
                    width: active_style.bar_border_width,
                    color: active_style.slider_groove_border_color,
                },
                ..renderer::Quad::default()
            },
            Color::TRANSPARENT,
        );
        draw_focus_border(renderer, color_picker, hue_bounds, Focus::Ring, style_sheet);
    }
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
    let row = if color_picker.state.picker_tab == PickerTab::Gradient {
        match (color_picker.state.gradient_editor, channel) {
            (GradientEditorTab::Rgba, 0..=3) => channel,
            (GradientEditorTab::Hsv, 4..=6) => channel - 4,
            (GradientEditorTab::Hsv, 3) => 3,
            _ => return None,
        }
    } else {
        match (color_picker.state.active_tab, channel) {
            (ActiveTab::Rgb, 0..=3) => channel,
            (ActiveTab::Hsv, 4..=6) => channel - 4,
            (ActiveTab::Hsv, 3) => 3,
            _ => return None,
        }
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

    let labels = if color_picker.state.picker_tab == PickerTab::Gradient {
        match color_picker.state.gradient_editor {
            GradientEditorTab::Rgba => ["R", "G", "B", "A"],
            GradientEditorTab::Hsv => ["H", "S", "V", "A"],
            GradientEditorTab::Rect => ["R", "G", "B", "A"],
        }
    } else {
        match color_picker.state.active_tab {
            ActiveTab::Rgb => ["R", "G", "B", "A"],
            ActiveTab::Hsv => ["H", "S", "V", "A"],
        }
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

        let channel = if color_picker.state.picker_tab == PickerTab::Gradient {
            match (color_picker.state.gradient_editor, row) {
                (GradientEditorTab::Rgba, 0) => 0,
                (GradientEditorTab::Rgba, 1) => 1,
                (GradientEditorTab::Rgba, 2) => 2,
                (GradientEditorTab::Rgba, 3) => 3,
                (GradientEditorTab::Hsv, 0) => 4,
                (GradientEditorTab::Hsv, 1) => 5,
                (GradientEditorTab::Hsv, 2) => 6,
                (GradientEditorTab::Hsv, 3) => 3,
                _ => usize::MAX,
            }
        } else {
            match (color_picker.state.active_tab, row) {
                (ActiveTab::Rgb, 0) => 0,
                (ActiveTab::Rgb, 1) => 1,
                (ActiveTab::Rgb, 2) => 2,
                (ActiveTab::Rgb, 3) => 3,
                (ActiveTab::Hsv, 0) => 4,
                (ActiveTab::Hsv, 1) => 5,
                (ActiveTab::Hsv, 2) => 6,
                (ActiveTab::Hsv, 3) => 3,
                _ => usize::MAX,
            }
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

        // Value field: channel TextInput of the active tab.
        let value_input_index = if color_picker.state.picker_tab == PickerTab::Gradient {
            match (color_picker.state.gradient_editor, row) {
                (GradientEditorTab::Rgba, i) => i,
                (GradientEditorTab::Hsv, 0) => 4,
                (GradientEditorTab::Hsv, 1) => 5,
                (GradientEditorTab::Hsv, 2) => 6,
                (GradientEditorTab::Hsv, 3) => 3,
                _ => usize::MAX,
            }
        } else {
            match (color_picker.state.active_tab, row) {
                (ActiveTab::Rgb, i) => i,
                (ActiveTab::Hsv, 0) => 4,
                (ActiveTab::Hsv, 1) => 5,
                (ActiveTab::Hsv, 2) => 6,
                (ActiveTab::Hsv, 3) => 3,
                _ => usize::MAX,
            }
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

    // Keyboard focus outline around the panel so tabbing to Hex is visible.
    draw_focus_border(renderer, color_picker, bounds, Focus::Hex, style_sheet);
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
    /// The cache of the gradient stop pins of the [`ColorPickerOverlay`].
    pub(crate) gradient_handles_cache: canvas::Cache,
    /// The dragged color bar of the [`ColorPickerOverlay`].
    pub(crate) color_bar_dragged: ColorBarDragged,
    /// the focus of the [`ColorPickerOverlay`].
    pub(crate) focus: Focus,
    /// The previously pressed keyboard modifiers.
    pub(crate) keyboard_modifiers: keyboard::Modifiers,
    /// Whether the eyedropper button is currently pressed.
    pub(crate) dropper_pressed: bool,
    /// Whether the submit button is currently pressed.
    pub(crate) submit_pressed: bool,
    /// Whether the reset button is currently pressed.
    pub(crate) reset_pressed: bool,
    /// The active controls tab of the left pane.
    pub(crate) active_tab: ActiveTab,
    /// The top-level tab: raw color picking or the swatch library.
    pub(crate) picker_tab: PickerTab,
    /// The editor shown below the gradient stop bar.
    pub(crate) gradient_editor: GradientEditorTab,
    /// The two-stop gradient edited in the Gradient tab.
    pub(crate) gradient: Gradient,
    /// The gradient used to initialize the dialog.
    pub(crate) initial_gradient: Gradient,
    /// Whether the pre-open set value was a gradient (`true`) or a solid
    /// (`false`). Fixed at construction / open-time synchronize; Original
    /// always renders this snapshot and never follows tab switches.
    pub(crate) initial_is_gradient: bool,
    /// The selected gradient stop (`0` or `1`).
    pub(crate) selected_stop: usize,
    /// The dragged gradient stop, if any.
    pub(crate) gradient_bar_dragged: Option<usize>,
    /// Whether the header/content "Color" top tab is hovered.
    pub(crate) top_color_hovered: bool,
    /// Whether the header/content "Gradient" top tab is hovered.
    pub(crate) top_gradient_hovered: bool,
    /// Whether the header/content "Library" top tab is hovered.
    pub(crate) top_library_hovered: bool,
    /// Whether the "Rect" gradient editor tab is hovered.
    pub(crate) tab_rect_hovered: bool,
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
    /// The recently submitted picked values (solids or gradients).
    pub(crate) recent_colors: Vec<PickedValue>,
    /// The horizontal scroll offset of the recent colors strip.
    pub(crate) recent_scroll_x: f32,
    /// The horizontal scroll offset of the active swatch set's strip.
    pub(crate) swatch_scroll_x: f32,
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
    /// The persisted top-left position of the [`ColorPickerWindow`] inside
    /// the viewport. `None` until the first open computes a base position;
    /// afterwards it survives close/reopen so the window reappears where
    /// the user dragged it.
    pub(crate) dialog_position: Option<Point>,
    /// The grab offset while the window header is being dragged:
    /// cursor position minus the dialog origin.
    pub(crate) header_drag_offset: Option<Vector>,
    /// Whether the header close ("x") button is currently pressed.
    pub(crate) close_pressed: bool,
    /// The runtime mode of the eye dropper.
    pub(crate) dropper_mode: DropperMode,
    /// The frozen window snapshot owned while the eye dropper picks a
    /// pixel. `None` outside of [`DropperMode::Picking`].
    pub(crate) dropper_frame: Option<Frame>,
    /// The last hovered point during picking (window coordinates, logical
    /// pixels).
    pub(crate) dropper_cursor: Point,
    /// The full-window viewport captured during [`ColorPickerWindow`] layout.
    /// The draw pass rebuilds the overlay fresh without a layout call, so the
    /// construction-time `viewport` is still clipped to the ancestor
    /// scrollable/pane; this persisted copy lets the magnifier lens clamp to
    /// the window and move freely in the viewport.
    pub(crate) window_viewport: Rectangle,
}

impl State {
    /// Creates a new State with the given color.
    #[must_use]
    pub fn new(color: Color) -> Self {
        let hue = Hsv::from(color).hue;
        let gradient = Gradient::two(color, color);
        Self {
            color,
            initial_color: color,
            hue,
            gradient: gradient.clone(),
            initial_gradient: gradient,
            hex_input: color_to_hex_argb(color),
            value_inputs: value_inputs_from_color(color),
            ..Self::default()
        }
    }

    /// Creates a new State with the given color and initial gradient.
    /// The pre-open set value is the gradient.
    #[must_use]
    pub fn with_gradient(color: Color, gradient: Gradient) -> Self {
        let mut state = Self::new(color);
        state.gradient = gradient.clone();
        state.initial_gradient = gradient;
        state.initial_is_gradient = true;
        state.selected_stop = 0;
        state
    }

    /// Reset cached canvas when internal state is modified.
    ///
    /// If the color has changed, empty all canvas caches
    /// as they (unfortunately) do not depend on the picker state.
    fn clear_cache(&self) {
        self.sat_value_canvas_cache.clear();
        self.hue_canvas_cache.clear();
        self.gradient_handles_cache.clear();
    }

    /// Refresh the hex input and value field texts to match `self.color`.
    pub(crate) fn sync_display(&mut self) {
        self.hex_input = color_to_hex_argb(self.color);
        self.value_inputs = value_inputs_from_color(self.color);
    }

    /// Returns the current value as a unified picked value.
    #[must_use]
    pub(crate) fn current_picked(&self) -> PickedValue {
        if self.picker_tab == PickerTab::Gradient {
            PickedValue::Gradient(self.gradient.clone())
        } else {
            PickedValue::Solid(self.color)
        }
    }

    /// Returns the pre-open set value: fixed at construction / open-time
    /// synchronize, never follows later tab switches or live edits.
    #[must_use]
    pub(crate) fn initial_picked(&self) -> PickedValue {
        if self.initial_is_gradient {
            PickedValue::Gradient(self.initial_gradient.clone())
        } else {
            PickedValue::Solid(self.initial_color)
        }
    }

    /// Applies a picked value: solids update the color (and keep the
    /// gradient solid so Always-gradient previews stay in sync), gradients
    /// load the stops and select the first stop.
    pub(crate) fn apply_picked(&mut self, picked: PickedValue) {
        match picked {
            PickedValue::Solid(color) => {
                self.apply_color(color);
            }
            PickedValue::Gradient(gradient) => {
                self.gradient = gradient;
                self.selected_stop = 0;
                if let Some(stop) = self.gradient.stop(0) {
                    let hsv: Hsv = stop.color.into();
                    if hsv.saturation > 0.001 && hsv.value > 0.001 {
                        self.hue = hsv.hue;
                    }
                    self.color = stop.color;
                }
                self.sync_display();
                self.clear_cache();
            }
        }
    }

    /// Sets the current color, remembering its hue when it has one.
    ///
    /// Near-achromatic colors (tiny saturation or value) carry no reliable
    /// hue: re-deriving it from RGB amplifies float noise and would corrupt
    /// the remembered hue. They do not overwrite it; see [`Self::hue`].
    pub(crate) fn apply_color(&mut self, color: Color) {
        let hsv: Hsv = color.into();
        if hsv.saturation > 0.001 && hsv.value > 0.001 {
            self.hue = hsv.hue;
        }
        self.color = color;
        // Mirror every color edit into the selected gradient stop so the
        // square, sliders, hex, keyboard, dropper and swatches all edit the
        // stop while the Gradient tab is active.
        if self.picker_tab == PickerTab::Gradient {
            self.gradient.set_stop_color(self.selected_stop, color);
        }
    }

    /// Selects a gradient stop and loads its color into the shared editor.
    pub(crate) fn select_stop(&mut self, index: usize) {
        let index = index.min(self.gradient.stops.len().saturating_sub(1));
        self.selected_stop = index;
        if let Some(stop) = self.gradient.stop(index) {
            let hsv: Hsv = stop.color.into();
            if hsv.saturation > 0.001 && hsv.value > 0.001 {
                self.hue = hsv.hue;
            }
            self.color = stop.color;
            self.sync_display();
            self.clear_cache();
        }
    }

    /// Resets the active tab's value to its initial value.
    pub(crate) fn reset_to_initial(&mut self) {
        if self.picker_tab == PickerTab::Gradient {
            self.gradient = self.initial_gradient.clone();
            self.selected_stop = 0;
            if let Some(stop) = self.gradient.stop(0) {
                self.color = stop.color;
            }
        } else {
            self.color = self.initial_color;
        }
        self.sync_display();
        self.clear_cache();
    }

    /// The channel indices edited in the Gradient tab for the current
    /// editor: none for Rect (square), HSV or RGBA rows otherwise.
    pub(crate) fn gradient_channels(&self) -> Vec<usize> {
        match self.gradient_editor {
            GradientEditorTab::Rect => Vec::new(),
            GradientEditorTab::Hsv => vec![4, 5, 6, 3],
            GradientEditorTab::Rgba => vec![0, 1, 2, 3],
        }
    }

    /// The HSV of the current color for display purposes.
    ///
    /// Near-achromatic colors have no reliable hue; the last meaningful hue
    /// is substituted so the ring/square do not snap to red.
    pub(crate) fn hsv(&self) -> Hsv {
        let hsv: Hsv = self.color.into();
        if hsv.saturation > 0.001 && hsv.value > 0.001 {
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
        self.initial_is_gradient = false;
        self.color = color;
        let hsv: Hsv = color.into();
        if hsv.saturation > 0.001 && hsv.value > 0.001 {
            self.hue = hsv.hue;
        }
        self.sync_display();
        self.clear_cache();
    }

    /// Synchronize the gradient with an externally provided value.
    /// The pre-open set value becomes the gradient.
    pub(crate) fn force_synchronize_gradient(&mut self, gradient: Gradient) {
        self.initial_gradient = gradient.clone();
        self.initial_is_gradient = true;
        self.gradient = gradient;
        self.selected_stop = 0;
        if let Some(stop) = self.gradient.stop(0) {
            self.color = stop.color;
        }
        self.sync_display();
        self.clear_cache();
    }
}

impl Default for State {
    fn default() -> Self {
        let default_color = Color::from_rgb(0.5, 0.25, 0.25);
        let default_gradient = Gradient::two(default_color, default_color);
        Self {
            color: default_color,
            initial_color: default_color,
            hue: Hsv::from(default_color).hue,
            sat_value_canvas_cache: canvas::Cache::default(),
            hue_canvas_cache: canvas::Cache::default(),
            gradient_handles_cache: canvas::Cache::default(),
            color_bar_dragged: ColorBarDragged::None,
            focus: Focus::default(),
            keyboard_modifiers: keyboard::Modifiers::default(),
            dropper_pressed: false,
            submit_pressed: false,
            reset_pressed: false,
            active_tab: ActiveTab::Hsv,
            picker_tab: PickerTab::Color,
            gradient_editor: GradientEditorTab::Rect,
            gradient: default_gradient.clone(),
            initial_gradient: default_gradient,
            initial_is_gradient: false,
            selected_stop: 0,
            gradient_bar_dragged: None,
            top_color_hovered: false,
            top_gradient_hovered: false,
            top_library_hovered: false,
            tab_rect_hovered: false,
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
            recent_scroll_x: 0.0,
            swatch_scroll_x: 0.0,
            swatch_hover: SwatchHover::default(),
            tab_rgb_hovered: false,
            tab_hsv_hovered: false,
            plus_tab_hovered: false,
            focused_swatch: None,
            dialog_position: None,
            header_drag_offset: None,
            close_pressed: false,
            dropper_mode: DropperMode::Idle,
            dropper_frame: None,
            dropper_cursor: Point::ORIGIN,
            window_viewport: Rectangle::default(),
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
    /// The eyedropper button of the [`ColorPickerOverlay`].
    dropper_button: Element<'a, Message, Theme, Renderer>,
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
        let (dropper_content, dropper_font) = dropper_icon();
        let (submit_content, submit_font) = ok_icon();

        Self {
            dropper_button: Button::new(
                widget::Text::new(dropper_content).font(dropper_font),
            )
            .into(),
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
            Tree::new(&self.dropper_button),
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

    /// The top-level "Color" tab is in focus.
    TopColor,

    /// The top-level "Gradient" tab is in focus.
    TopGradient,

    /// The top-level "Library" tab is in focus.
    TopLibrary,

    /// The gradient stop bar is in focus.
    GradientBar,

    /// The hue slider below the square is in focus.
    Ring,

    /// The saturation/value square is in focus.
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

    /// The Rect tab of the gradient editor is in focus.
    TabRect,

    /// The swatch section is in focus.
    Swatches,

    /// The "new swatch set" name input is in focus.
    NewSetName,

    /// The reset button is in focus.
    Reset,

    /// The eyedropper button is in focus.
    Dropper,

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
/// tab are skipped, the swatch grid is only reachable in the Library tab,
/// and the "new swatch set" input is only reachable while the naming prompt
/// is active.
fn focus_cycle(
    picker_tab: PickerTab,
    active_tab: ActiveTab,
    gradient_editor: GradientEditorTab,
    naming_new_set: bool,
) -> Vec<Focus> {
    let mut cycle = vec![
        Focus::Overlay,
        Focus::TopColor,
        Focus::TopGradient,
        Focus::TopLibrary,
    ];
    match picker_tab {
        PickerTab::Color => {
            let (first, second, third) = match active_tab {
                ActiveTab::Rgb => (Focus::Red, Focus::Green, Focus::Blue),
                ActiveTab::Hsv => (Focus::HsvHue, Focus::HsvSat, Focus::HsvVal),
            };
            cycle.extend([
                Focus::Square,
                Focus::Ring,
                first,
                second,
                third,
                Focus::Alpha,
                Focus::Hex,
                Focus::TabHsv,
                Focus::TabRgb,
            ]);
        }
        PickerTab::Gradient => {
            cycle.push(Focus::GradientBar);
            match gradient_editor {
                GradientEditorTab::Rect => {
                    cycle.extend([Focus::Square, Focus::Ring]);
                }
                GradientEditorTab::Hsv => {
                    cycle.extend([Focus::HsvHue, Focus::HsvSat, Focus::HsvVal, Focus::Alpha]);
                }
                GradientEditorTab::Rgba => {
                    cycle.extend([Focus::Red, Focus::Green, Focus::Blue, Focus::Alpha]);
                }
            }
            cycle.extend([Focus::Hex, Focus::TabRect, Focus::TabHsv, Focus::TabRgb]);
        }
        PickerTab::Library => {
            cycle.push(Focus::Swatches);
            if naming_new_set {
                cycle.push(Focus::NewSetName);
            }
        }
    }
    cycle.extend([Focus::Reset, Focus::Dropper, Focus::Submit]);
    cycle
}

/// Gets the next focusable element.
#[must_use]
fn next_focus(
    focus: Focus,
    picker_tab: PickerTab,
    active_tab: ActiveTab,
    gradient_editor: GradientEditorTab,
    naming_new_set: bool,
) -> Focus {
    let cycle = focus_cycle(picker_tab, active_tab, gradient_editor, naming_new_set);
    let Some(position) = cycle.iter().position(|f| *f == focus) else {
        // Not part of the cycle (e.g. `None` or a channel focus of the
        // inactive tab): jump to the first element.
        return Focus::Overlay;
    };
    cycle[(position + 1) % cycle.len()]
}

/// Gets the previous focusable element.
#[must_use]
fn previous_focus(
    focus: Focus,
    picker_tab: PickerTab,
    active_tab: ActiveTab,
    gradient_editor: GradientEditorTab,
    naming_new_set: bool,
) -> Focus {
    let cycle = focus_cycle(picker_tab, active_tab, gradient_editor, naming_new_set);
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
            Focus::TopColor,
            Focus::TopGradient,
            Focus::TopLibrary,
            Focus::Square,
            Focus::Ring,
            Focus::Red,
            Focus::Green,
            Focus::Blue,
            Focus::Alpha,
            Focus::Hex,
            Focus::TabHsv,
            Focus::TabRgb,
            Focus::Reset,
            Focus::Dropper,
            Focus::Submit,
        ] {
            focus = next_focus(
                focus,
                PickerTab::Color,
                ActiveTab::Rgb,
                GradientEditorTab::Rect,
                false,
            );
            assert_eq!(focus, expected);
        }
        assert_eq!(
            next_focus(
                focus,
                PickerTab::Color,
                ActiveTab::Rgb,
                GradientEditorTab::Rect,
                false
            ),
            Focus::Overlay
        );
    }

    #[test]
    fn focus_cycle_hsv_forwards() {
        let mut focus = Focus::None;
        for expected in [
            Focus::Overlay,
            Focus::TopColor,
            Focus::TopGradient,
            Focus::TopLibrary,
            Focus::Square,
            Focus::Ring,
            Focus::HsvHue,
            Focus::HsvSat,
            Focus::HsvVal,
            Focus::Alpha,
            Focus::Hex,
            Focus::TabHsv,
            Focus::TabRgb,
            Focus::Reset,
            Focus::Dropper,
            Focus::Submit,
        ] {
            focus = next_focus(
                focus,
                PickerTab::Color,
                ActiveTab::Hsv,
                GradientEditorTab::Rect,
                false,
            );
            assert_eq!(focus, expected);
        }
        assert_eq!(
            next_focus(
                focus,
                PickerTab::Color,
                ActiveTab::Hsv,
                GradientEditorTab::Rect,
                false
            ),
            Focus::Overlay
        );
    }

    #[test]
    fn focus_cycle_rgb_backwards() {
        let mut focus = Focus::Overlay;
        for expected in [
            Focus::Submit,
            Focus::Dropper,
            Focus::Reset,
            Focus::TabRgb,
            Focus::TabHsv,
            Focus::Hex,
            Focus::Alpha,
            Focus::Blue,
            Focus::Green,
            Focus::Red,
            Focus::Ring,
            Focus::Square,
            Focus::TopLibrary,
            Focus::TopGradient,
            Focus::TopColor,
        ] {
            focus = previous_focus(
                focus,
                PickerTab::Color,
                ActiveTab::Rgb,
                GradientEditorTab::Rect,
                false,
            );
            assert_eq!(focus, expected);
        }
        assert_eq!(
            previous_focus(
                focus,
                PickerTab::Color,
                ActiveTab::Rgb,
                GradientEditorTab::Rect,
                false
            ),
            Focus::Overlay
        );
    }

    #[test]
    fn focus_cycle_hsv_backwards() {
        let mut focus = Focus::Overlay;
        for expected in [
            Focus::Submit,
            Focus::Dropper,
            Focus::Reset,
            Focus::TabRgb,
            Focus::TabHsv,
            Focus::Hex,
            Focus::Alpha,
            Focus::HsvVal,
            Focus::HsvSat,
            Focus::HsvHue,
            Focus::Ring,
            Focus::Square,
            Focus::TopLibrary,
            Focus::TopGradient,
            Focus::TopColor,
        ] {
            focus = previous_focus(
                focus,
                PickerTab::Color,
                ActiveTab::Hsv,
                GradientEditorTab::Rect,
                false,
            );
            assert_eq!(focus, expected);
        }
        assert_eq!(
            previous_focus(
                focus,
                PickerTab::Color,
                ActiveTab::Hsv,
                GradientEditorTab::Rect,
                false
            ),
            Focus::Overlay
        );
    }

    #[test]
    fn focus_cycle_gradient_rect() {
        let mut focus = Focus::None;
        for expected in [
            Focus::Overlay,
            Focus::TopColor,
            Focus::TopGradient,
            Focus::TopLibrary,
            Focus::GradientBar,
            Focus::Square,
            Focus::Ring,
            Focus::Hex,
            Focus::TabRect,
            Focus::TabHsv,
            Focus::TabRgb,
            Focus::Reset,
            Focus::Dropper,
            Focus::Submit,
        ] {
            focus = next_focus(
                focus,
                PickerTab::Gradient,
                ActiveTab::Rgb,
                GradientEditorTab::Rect,
                false,
            );
            assert_eq!(focus, expected);
        }
    }

    #[test]
    fn focus_cycle_library() {
        let mut focus = Focus::None;
        for expected in [
            Focus::Overlay,
            Focus::TopColor,
            Focus::TopGradient,
            Focus::TopLibrary,
            Focus::Swatches,
            Focus::Reset,
            Focus::Dropper,
            Focus::Submit,
        ] {
            focus = next_focus(
                focus,
                PickerTab::Library,
                ActiveTab::Rgb,
                GradientEditorTab::Rect,
                false,
            );
            assert_eq!(focus, expected);
        }
        assert_eq!(
            next_focus(
                Focus::Swatches,
                PickerTab::Library,
                ActiveTab::Rgb,
                GradientEditorTab::Rect,
                true
            ),
            Focus::NewSetName
        );
        assert_eq!(
            next_focus(
                Focus::NewSetName,
                PickerTab::Library,
                ActiveTab::Rgb,
                GradientEditorTab::Rect,
                true
            ),
            Focus::Reset
        );
    }

    #[test]
    fn focus_cycle_naming_new_set() {
        assert_eq!(
            next_focus(
                Focus::Swatches,
                PickerTab::Library,
                ActiveTab::Rgb,
                GradientEditorTab::Rect,
                true
            ),
            Focus::NewSetName
        );
        assert_eq!(
            next_focus(
                Focus::NewSetName,
                PickerTab::Library,
                ActiveTab::Rgb,
                GradientEditorTab::Rect,
                true
            ),
            Focus::Reset
        );
        assert_eq!(
            previous_focus(
                Focus::Reset,
                PickerTab::Library,
                ActiveTab::Rgb,
                GradientEditorTab::Rect,
                true
            ),
            Focus::NewSetName
        );
        assert_eq!(
            next_focus(
                Focus::Swatches,
                PickerTab::Library,
                ActiveTab::Rgb,
                GradientEditorTab::Rect,
                false
            ),
            Focus::Reset
        );
    }

    #[test]
    fn focus_cycle_stray_and_unfocused() {
        // A channel of the inactive tab is normalized: it is not part of
        // the cycle.
        assert_eq!(
            next_focus(
                Focus::Red,
                PickerTab::Color,
                ActiveTab::Hsv,
                GradientEditorTab::Rect,
                false
            ),
            Focus::Overlay
        );
        assert_eq!(
            previous_focus(
                Focus::Red,
                PickerTab::Color,
                ActiveTab::Hsv,
                GradientEditorTab::Rect,
                false
            ),
            Focus::None
        );
        // Unfocused elements enter/leave the cycle at its start.
        assert_eq!(
            next_focus(
                Focus::None,
                PickerTab::Color,
                ActiveTab::Hsv,
                GradientEditorTab::Rect,
                false
            ),
            Focus::Overlay
        );
        assert_eq!(
            previous_focus(
                Focus::None,
                PickerTab::Color,
                ActiveTab::Rgb,
                GradientEditorTab::Rect,
                false
            ),
            Focus::None
        );
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
        let red = PickedValue::Solid(Color::from_rgb8(255, 0, 0));
        let green = PickedValue::Solid(Color::from_rgb8(0, 255, 0));
        let blue = PickedValue::Solid(Color::from_rgb8(0, 0, 255));

        let mut colors = vec![red.clone(), green.clone()];
        // Byte-exact duplicate is removed and the value moves to the front.
        insert_swatch(&mut colors, red.clone());
        assert_eq!(colors, vec![red.clone(), green.clone()]);
        // A solid whose floats differ but whose RGBA bytes match is a duplicate:
        // the old value is removed and the new instance inserted at the front.
        let green_alias = PickedValue::Solid(Color {
            r: 0.001,
            ..Color::from_rgb8(0, 255, 0)
        });
        insert_swatch(&mut colors, green_alias.clone());
        assert_eq!(colors, vec![green_alias.clone(), red.clone()]);
        insert_swatch(&mut colors, blue.clone());
        assert_eq!(colors, vec![blue.clone(), green_alias.clone(), red.clone()]);
        // Gradients are a distinct picked type and do not dedupe solids.
        let gradient = PickedValue::Gradient(Gradient::two(
            Color::from_rgb8(255, 0, 0),
            Color::from_rgb8(0, 0, 255),
        ));
        insert_swatch(&mut colors, gradient.clone());
        assert_eq!(colors[0], gradient);

        // Truncation at MAX_SWATCHES_PER_SET: the oldest entries fall off the
        // end. The list was [0..=26] oldest-first; after inserting the new
        // value at the front the tail [23..=26] is dropped.
        let mut many = (0..MAX_SWATCHES_PER_SET + 3)
            .map(|i| PickedValue::Solid(Color::from_rgb8(i as u8, 0, 0)))
            .collect::<Vec<_>>();
        let new_color = PickedValue::Solid(Color::from_rgb8(200, 200, 200));
        insert_swatch(&mut many, new_color.clone());
        assert_eq!(many.len(), MAX_SWATCHES_PER_SET);
        assert_eq!(many[0], new_color);
        assert_eq!(many[1], PickedValue::Solid(Color::from_rgb8(0, 0, 0)));
        assert_eq!(
            many.last(),
            Some(&PickedValue::Solid(Color::from_rgb8(22, 0, 0)))
        );
    }

    #[test]
    fn push_recent_dedupes_front_and_truncates() {
        let red = PickedValue::Solid(Color::from_rgb8(255, 0, 0));
        let green = PickedValue::Solid(Color::from_rgb8(0, 255, 0));

        let mut recent = Vec::new();
        push_recent(&mut recent, red.clone());
        push_recent(&mut recent, green.clone());
        assert_eq!(recent, vec![green.clone(), red.clone()]);
        // Re-submitting the same value only moves it to the front.
        push_recent(&mut recent, red.clone());
        assert_eq!(recent, vec![red.clone(), green.clone()]);

        // Truncation at MAX_RECENT.
        let mut many = (0..MAX_RECENT + 2)
            .map(|i| PickedValue::Solid(Color::from_rgb8(i as u8, 0, 0)))
            .collect::<Vec<_>>();
        push_recent(
            &mut many,
            PickedValue::Solid(Color::from_rgb8(9, 9, 9)),
        );
        assert_eq!(many.len(), MAX_RECENT);
        assert_eq!(
            many[0],
            PickedValue::Solid(Color::from_rgb8(9, 9, 9))
        );
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
    fn visible_cols_fits_the_pane() {
        // The 230px pane fits six 30px columns plus margins and spacing.
        assert_eq!(visible_cols(RIGHT_PANE_WIDTH), 6);
        // Narrow viewports still show at least one placeholder cell.
        assert_eq!(visible_cols(0.0), 1);
        assert_eq!(visible_cols(SWATCH_SIZE), 1);
    }

    #[test]
    fn strip_content_cols_fill_the_viewport_first() {
        // Empty: exactly one viewport worth of placeholder columns.
        assert_eq!(strip_content_cols(0, RIGHT_PANE_WIDTH), 6);
        // A partial trailing column counts once.
        assert_eq!(strip_content_cols(7, RIGHT_PANE_WIDTH), 6);
        // Overflow grows horizontally.
        assert_eq!(strip_content_cols(19, RIGHT_PANE_WIDTH), 7);
    }

    #[test]
    fn strip_scroll_clamps_to_content() {
        // One viewport worth fits exactly: no scrolling.
        assert_eq!(strip_max_scroll(0, RIGHT_PANE_WIDTH), 0.0);
        assert_eq!(strip_max_scroll(18, RIGHT_PANE_WIDTH), 0.0);
        // Nineteen cells need seven columns; scrolling stops at the margin.
        let expected = strip_content_width(19, RIGHT_PANE_WIDTH)
            + 2.0 * SWATCH_PAGE_MARGIN
            - RIGHT_PANE_WIDTH;
        assert_eq!(strip_max_scroll(19, RIGHT_PANE_WIDTH), expected.max(0.0));
        assert_eq!(clamp_strip_scroll(-5.0, 19, RIGHT_PANE_WIDTH), 0.0,);
        assert_eq!(
            clamp_strip_scroll(expected + 100.0, 19, RIGHT_PANE_WIDTH),
            expected
        );
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
