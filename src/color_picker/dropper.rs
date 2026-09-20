//! The eye dropper of the [`ColorPicker`](crate::color_picker::ColorPicker).
//!
//! The dropper samples colors from a frozen snapshot of the *window
//! contents only* — it never captures anything outside the application
//! window. Because iced widgets cannot spawn [`Task`]s themselves, the
//! capture round-trip is plumbed through the application:
//!
//! 1. The user activates the eyedropper button; the widget publishes the
//!    message produced by
//!    [`on_dropper_capture`](crate::color_picker::ColorPicker::on_dropper_capture).
//! 2. The application reacts by running `window::latest()` followed by
//!    `window::screenshot` and mapping the captured
//!    [`Screenshot`](iced::window::screenshot::Screenshot) into one of its
//!    messages.
//! 3. The application stores the screenshot into the shared
//!    [`DropperBuffer`] handed to the picker via
//!    [`dropper_buffer`](crate::color_picker::ColorPicker::dropper_buffer).
//! 4. The widget picks the fresh frame up on its next update pass and
//!    enters picking mode: a magnifier lens follows the cursor and left
//!    click (or Enter) commits the sampled color.
//!
//! ```no_run
//! # use neverliie_iced_widgets::color_picker::{floating_color_picker, DropperBuffer};
//! # use iced::{Color, Element, Task, Theme, window};
//! # #[derive(Clone, Debug)]
//! # enum Message { Capture, Shot(window::screenshot::Screenshot), Cancel, Submit(Color) }
//! let buffer = DropperBuffer::new();
//!
//! // View: hand the picker a clone of the buffer and the request callback.
//! fn view(buffer: &DropperBuffer) -> Element<'_, Message, Theme> {
//!     floating_color_picker(
//!         true,
//!         Color::BLACK,
//!         iced::widget::button("Pick color").on_press(Message::Cancel),
//!         Message::Cancel,
//!         Message::Submit,
//!     )
//!     .dropper_buffer(buffer.clone())
//!     .on_dropper_capture(|| Message::Capture)
//!     .into()
//! }
//!
//! // Update: fulfill capture requests with a window screenshot.
//! fn update(message: Message, buffer: &DropperBuffer) -> Task<Message> {
//!     match message {
//!         Message::Capture => window::latest()
//!             .and_then(window::screenshot)
//!             .map(Message::Shot),
//!         Message::Shot(screenshot) => {
//!             buffer.store(&screenshot);
//!             Task::none()
//!         }
//!         _ => Task::none(),
//!     }
//! }
//! # let _ = (&view, &update);
//! ```

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use bytes::Bytes;
use iced::window::screenshot::Screenshot;
use iced::Color;

/// A frame captured from the window: RGBA8 bytes of the whole client area.
#[derive(Clone)]
pub(crate) struct Frame {
    /// The RGBA8 bytes of the window client area (physical pixels, sRGB).
    pub(crate) rgba: Bytes,
    /// The width of the frame in physical pixels.
    pub(crate) width: u32,
    /// The height of the frame in physical pixels.
    pub(crate) height: u32,
    /// The scale factor mapping logical window coordinates onto physical
    /// frame pixels.
    pub(crate) scale_factor: f32,
}

impl Frame {
    /// Builds a frame from a window screenshot.
    pub(crate) fn from_screenshot(screenshot: &Screenshot) -> Self {
        Self {
            rgba: screenshot.rgba.clone(),
            width: screenshot.size.width,
            height: screenshot.size.height,
            scale_factor: screenshot.scale_factor,
        }
    }

    /// Maps logical window coordinates onto physical frame pixel indices.
    ///
    /// Returns `None` only when the frame has no usable scale factor. The
    /// returned indices may lie outside the captured area; use
    /// [`sample_pixel`](Self::sample_pixel) for bounds-checked sampling.
    #[must_use]
    pub fn to_physical(&self, x: f32, y: f32) -> Option<(i32, i32)> {
        if !(self.scale_factor > 0.0 && self.scale_factor.is_finite()) {
            return None;
        }
        let scale = self.scale_factor;

        Some(((x * scale).floor() as i32, (y * scale).floor() as i32))
    }

    /// Samples the sRGB color of the physical frame pixel `(px, py)`.
    /// The returned color is always opaque.
    ///
    /// Returns `None` when the pixel lies outside the captured area.
    #[must_use]
    pub fn sample_pixel(&self, px: i32, py: i32) -> Option<Color> {
        if px < 0 || py < 0 || px >= self.width as i32 || py >= self.height as i32 {
            return None;
        }

        let index = ((py as u32 * self.width + px as u32) * 4) as usize;
        let bytes = self.rgba.get(index..index + 4)?;

        Some(Color {
            r: f32::from(bytes[0]) / 255.0,
            g: f32::from(bytes[1]) / 255.0,
            b: f32::from(bytes[2]) / 255.0,
            a: 1.0,
        })
    }

    /// Samples the sRGB color of the pixel under the given logical window
    /// coordinates. The returned color is always opaque: the alpha channel
    /// of a rendered framebuffer is not visually meaningful.
    ///
    /// Returns `None` when the point lies outside the captured area or the
    /// frame has no usable scale factor.
    #[must_use]
    pub fn sample(&self, x: f32, y: f32) -> Option<Color> {
        let (px, py) = self.to_physical(x, y)?;
        self.sample_pixel(px, py)
    }
}

impl fmt::Debug for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Frame")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("scale_factor", &self.scale_factor)
            .finish_non_exhaustive()
    }
}

/// A shared slot where the application deposits window screenshots for the
/// eye dropper of a [`ColorPicker`](crate::color_picker::ColorPicker).
///
/// Clone it freely: every clone points at the same slot. Hand one clone to
/// [`dropper_buffer`](crate::color_picker::ColorPicker::dropper_buffer) and
/// keep another to [`store`](Self::store) screenshots produced by the
/// `window::screenshot` task (see the [module docs](self) for the full
/// round-trip).
///
/// The widget consumes each stored frame exactly once — when it enters
/// picking mode after an eyedropper activation. Storing a new frame
/// replaces any previous one.
#[derive(Clone, Default)]
pub struct DropperBuffer(Rc<RefCell<Option<Frame>>>);

impl DropperBuffer {
    /// Creates a new empty [`DropperBuffer`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores a window screenshot in the buffer, replacing any previous
    /// frame.
    pub fn store(&self, screenshot: &Screenshot) {
        *self.0.borrow_mut() = Some(Frame::from_screenshot(screenshot));
    }

    /// Takes the stored frame out of the buffer, leaving it empty.
    pub(crate) fn take(&self) -> Option<Frame> {
        self.0.borrow_mut().take()
    }

    /// Discards any stored frame without taking it.
    pub(crate) fn clear(&self) {
        *self.0.borrow_mut() = None;
    }
}

impl fmt::Debug for DropperBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DropperBuffer").finish_non_exhaustive()
    }
}

/// The runtime mode of the eye dropper inside the dialog [`State`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum DropperMode {
    /// The dropper is inactive; the dialog behaves normally.
    #[default]
    Idle,
    /// A capture was requested; waiting for the application to store a
    /// fresh frame into the [`DropperBuffer`].
    Waiting,
    /// A frame is available and the user is picking a pixel with the
    /// magnifier lens.
    Picking,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a 2x2 physical-pixel frame at scale factor 2.0.
    ///
    /// Layout (RGBA): red, green / blue, white.
    fn test_frame(scale: f32) -> Frame {
        Frame {
            rgba: Bytes::from(vec![
                255, 0, 0, 255, //
                0, 255, 0, 255, //
                0, 0, 255, 255, //
                255, 255, 255, 255,
            ]),
            width: 2,
            height: 2,
            scale_factor: scale,
        }
    }

    #[test]
    fn samples_logical_points_through_scale_factor() {
        let frame = test_frame(2.0);

        // A 2x2 physical frame at scale 2.0 covers the logical unit square
        // [0,1)x[0,1); each source pixel spans 0.5 logical units.
        assert_eq!(frame.sample(0.0, 0.0), Some(Color::from_rgb(1.0, 0.0, 0.0)));
        assert_eq!(frame.sample(0.4, 0.4), Some(Color::from_rgb(1.0, 0.0, 0.0)));
        assert_eq!(frame.sample(0.6, 0.4), Some(Color::from_rgb(0.0, 1.0, 0.0)));
        assert_eq!(
            frame.sample(0.25, 0.75),
            Some(Color::from_rgb(0.0, 0.0, 1.0))
        );
        assert_eq!(
            frame.sample(0.75, 0.9),
            Some(Color::from_rgb(1.0, 1.0, 1.0))
        );
    }

    #[test]
    fn sampling_is_always_opaque() {
        let mut frame = test_frame(2.0);
        // Overwrite the red pixel's alpha byte.
        let mut bytes = frame.rgba.to_vec();
        bytes[3] = 42;
        frame.rgba = Bytes::from(bytes);

        let sampled = frame.sample(0.0, 0.0).unwrap();
        assert_eq!(sampled.a, 1.0);
        assert_eq!(sampled.r, 1.0);
    }

    #[test]
    fn out_of_bounds_samples_are_none() {
        let frame = test_frame(2.0);

        // The logical extent of the frame is [0,1)x[0,1).
        assert_eq!(frame.sample(-0.1, 0.5), None);
        assert_eq!(frame.sample(0.5, -0.1), None);
        assert_eq!(frame.sample(1.01, 0.5), None);
        assert_eq!(frame.sample(0.5, 1.01), None);
    }

    #[test]
    fn invalid_scale_factors_disable_sampling() {
        for scale in [0.0, -1.0, f32::NAN] {
            let mut frame = test_frame(scale);
            frame.scale_factor = scale;
            assert_eq!(frame.sample(0.5, 0.5), None);
        }
    }

    #[test]
    fn buffer_store_take_and_clear_roundtrip() {
        let buffer = DropperBuffer::new();
        assert!(buffer.take().is_none());

        let mut rgba_bytes = vec![0u8; 16];
        rgba_bytes[0] = 1;
        let screenshot = Screenshot::new(
            Bytes::from(rgba_bytes),
            iced::Size::new(2, 2),
            1.0,
        );

        buffer.store(&screenshot);
        let frame = buffer.take().expect("stored frame");
        assert_eq!(frame.width, 2);
        assert_eq!(frame.height, 2);
        assert_eq!(frame.scale_factor, 1.0);

        // Taking consumes the slot...
        assert!(buffer.take().is_none());

        // ...and clear also empties it.
        buffer.store(&screenshot);
        buffer.clear();
        assert!(buffer.take().is_none());
    }

    #[test]
    fn buffer_clones_share_the_slot() {
        let buffer = DropperBuffer::new();
        let clone = buffer.clone();

        let screenshot = Screenshot::new(Bytes::from(vec![0u8; 64]), iced::Size::new(4, 4), 1.0);
        buffer.store(&screenshot);

        assert!(clone.take().is_some());
        assert!(buffer.take().is_none());
    }
}
