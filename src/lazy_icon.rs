//! Lazy-loading icon widget with placeholder support.
//!
//! This module provides [`LazyIcon`], a widget that displays an image or SVG
//! once its decoder has finished, showing a colored placeholder rectangle
//! until then. Also includes [`Placeholder`], a standalone colored rounded
//! rectangle widget useful for skeleton loading states.
//!
//! # Overview
//!
//! - [`LazyIcon`] — renders an icon when decoded, placeholder until then
//! - [`Placeholder`] — standalone colored rounded rectangle
//! - [`IconHandle`] — enum wrapping image or SVG handles
//!
//! # Example
//!
//! ```no_run
//! use iced::widget::text;
//! use iced::{Color, Element};
//! use neverliie_iced_widgets::lazy_icon::{IconHandle, LazyIcon};
//!
//! enum Message {}
//!
//! fn view() -> Element<'_, Message> {
//!     LazyIcon::new(IconHandle::Image(
//!         iced::widget::image::Handle::from_path("icon.png"),
//!     ))
//!     .size(48.0)
//!     .placeholder_color(Color::from_rgb(0.2, 0.2, 0.2))
//!     .placeholder_radius(8.0)
//!     .into()
//! }
//! ```
//!
//! # Custom Icons (extracted from files)
//!
//! For icons extracted from `.exe`, `.dll`, `.ico`, or other sources, use
//! [`iced::widget::image::Handle::from_rgba`] to pass decoded pixels:
//!
//! ```no_run
//! use iced::{Color, Element};
//! use neverliie_iced_widgets::lazy_icon::{IconHandle, LazyIcon};
//!
//! enum Message {}
//!
//! fn custom_icon(rgba: Vec<u8>, width: u32, height: u32) -> Element<'_, Message> {
//!     LazyIcon::new(IconHandle::Image(
//!         iced::widget::image::Handle::from_rgba(width, height, rgba),
//!     ))
//!     .size(48.0)
//!     .into()
//! }
//! ```
//!
//! [`LazyIcon`]: struct.LazyIcon
//! [`Placeholder`]: struct.Placeholder
//! [`IconHandle`]: enum.IconHandle

use iced::advanced::image::Image;
use iced::advanced::layout::{self, Layout};
use iced::advanced::mouse;
use iced::advanced::renderer::{self, Quad};
use iced::advanced::svg::Svg;
use iced::advanced::widget::{Tree, Widget};
use iced::{Color, Element, Length, Rectangle, Shadow, Size};

/// A handle wrapping either an image or SVG icon.
///
/// Use this to provide icon data to [`LazyIcon`].
#[derive(Clone)]
pub enum IconHandle {
    /// A raster image handle.
    Image(iced::widget::image::Handle),
    /// An SVG handle.
    Svg(iced::widget::svg::Handle),
}

/// A widget that displays an icon when decoded, or a placeholder until then.
///
/// For raster images (`IconHandle::Image`), the widget checks whether the
/// decoder has finished via [`Renderer::measure_image`]. If not ready, a
/// colored placeholder rectangle is drawn instead.
///
/// For SVGs (`IconHandle::Svg`), the widget checks
/// [`Renderer::measure_svg`] for valid dimensions (>1×1). SVGs are
/// typically parsed synchronously, so the placeholder is rarely visible.
///
/// [`Renderer::measure_image`]: iced::advanced::image::Renderer::measure_image
/// [`Renderer::measure_svg`]: iced::advanced::svg::Renderer::measure_svg
pub struct LazyIcon {
    handle: IconHandle,
    size: f32,
    color: Color,
    radius: f32,
}

impl LazyIcon {
    /// Creates a new [`LazyIcon`] with the given handle.
    ///
    /// Use [`size`](Self::size) to set the icon dimensions, and
    /// [`placeholder_color`](Self::placeholder_color) /
    /// [`placeholder_radius`](Self::placeholder_radius) to configure the
    /// loading placeholder.
    pub fn new(handle: IconHandle) -> Self {
        Self {
            handle,
            size: 16.0,
            color: Color::TRANSPARENT,
            radius: 0.0,
        }
    }

    /// Sets the icon size in pixels (square).
    #[must_use]
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Sets the placeholder fill color shown while the icon is loading.
    #[must_use]
    pub fn placeholder_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Sets the placeholder corner radius.
    #[must_use]
    pub fn placeholder_radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for LazyIcon
where
    Renderer: iced::advanced::image::Renderer<Handle = iced::widget::image::Handle>
        + iced::advanced::svg::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.size), Length::Fixed(self.size))
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        _limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(Size::new(self.size, self.size))
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        match &self.handle {
            IconHandle::Image(handle) => {
                let measured = renderer.measure_image(handle);
                if let Some(size) = measured {
                    if size.width > 1 || size.height > 1 {
                        renderer.draw_image(
                            Image {
                                handle: handle.clone(),
                                border_radius: Default::default(),
                                filter_method: Default::default(),
                                rotation: iced::Radians(0.0),
                                opacity: 1.0,
                                snap: true,
                            },
                            bounds,
                            bounds,
                        );
                        return;
                    }
                }
                fill_placeholder(renderer, bounds, self.color, self.radius);
            }
            IconHandle::Svg(handle) => {
                let svg_size = renderer.measure_svg(handle);
                if svg_size.width > 1 || svg_size.height > 1 {
                    renderer.draw_svg(
                        Svg {
                            handle: handle.clone(),
                            color: None,
                            rotation: iced::Radians(0.0),
                            opacity: 1.0,
                        },
                        bounds,
                        bounds,
                    );
                    return;
                }
                fill_placeholder(renderer, bounds, self.color, self.radius);
            }
        }
    }
}

fn fill_placeholder<Renderer>(renderer: &mut Renderer, bounds: Rectangle, color: Color, radius: f32)
where
    Renderer: iced::advanced::Renderer,
{
    if color.a == 0.0 {
        return;
    }
    renderer.fill_quad(
        Quad {
            bounds,
            border: iced::Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: radius.into(),
            },
            shadow: Shadow::default(),
            snap: true,
        },
        color,
    );
}

/// A standalone colored rounded rectangle widget.
///
/// Useful as a skeleton loading indicator or placeholder independent of
/// [`LazyIcon`].
///
/// # Example
///
/// ```no_run
/// use iced::{Color, Element};
/// use neverliie_iced_widgets::lazy_icon::placeholder;
///
/// enum Message {}
///
/// fn view() -> Element<'_, Message> {
///     placeholder(Color::from_rgb(0.2, 0.2, 0.2), 8.0, 48.0).into()
/// }
/// ```
pub fn placeholder<'a, Message>(
    color: Color,
    radius: f32,
    size: f32,
) -> Element<'a, Message> {
    Element::new(Placeholder { color, radius, size })
}

struct Placeholder {
    color: Color,
    radius: f32,
    size: f32,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Placeholder
where
    Renderer: iced::advanced::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.size), Length::Fixed(self.size))
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        _limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(Size::new(self.size, self.size))
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        fill_placeholder(renderer, layout.bounds(), self.color, self.radius);
    }
}

impl<'a, Message, Theme, Renderer> From<LazyIcon>
    for Element<'a, Message, Theme, Renderer>
where
    Theme: 'a,
    Renderer: iced::advanced::image::Renderer<Handle = iced::widget::image::Handle>
        + iced::advanced::svg::Renderer
        + 'a,
{
    fn from(icon: LazyIcon) -> Self {
        Element::new(icon)
    }
}

impl<'a, Message, Theme, Renderer> From<Placeholder>
    for Element<'a, Message, Theme, Renderer>
where
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(p: Placeholder) -> Self {
        Element::new(p)
    }
}
