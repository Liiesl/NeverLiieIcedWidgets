use iced::widget::{column, container, row, scrollable, text};
use iced::{Color, Element, Length};

use neverliie_iced_widgets::lazy_icon::{placeholder, IconHandle, LazyIcon};

fn main() -> iced::Result {
    iced::run(App::update, App::view)
}

#[derive(Default)]
struct App;

#[derive(Debug, Clone)]
enum Message {}

impl App {
    fn update(&mut self, _message: Message) {}

    fn view(&self) -> Element<'_, Message> {
        // 1. Placeholder while image loads from path (decoder not done yet)
        let loading_icon = LazyIcon::new(IconHandle::Image(
            iced::widget::image::Handle::from_path("nonexistent.png"),
        ))
        .size(64.0)
        .placeholder_color(Color::from_rgb(0.25, 0.25, 0.3))
        .placeholder_radius(12.0);

        // 2. Decoded RGBA pixels — shows immediately, no placeholder
        let mut rgba_pixels = Vec::new();
        for y in 0..32u32 {
            for x in 0..32u32 {
                let r = (x as f32 / 32.0 * 255.0) as u8;
                let g = (y as f32 / 32.0 * 255.0) as u8;
                let b = 128u8;
                rgba_pixels.extend_from_slice(&[r, g, b, 255]);
            }
        }
        let decoded_icon =
            LazyIcon::new(IconHandle::Image(iced::widget::image::Handle::from_rgba(
                32, 32, rgba_pixels,
            )))
            .size(64.0);

        // 3. SVG from bytes
        let svg_data = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
            <circle cx="12" cy="12" r="10" fill="rgb(59,130,246)"/>
            <text x="12" y="17" text-anchor="middle" fill="white" font-size="14" font-family="sans-serif">S</text>
        </svg>"#;
        let svg_icon = LazyIcon::new(IconHandle::Svg(
            iced::widget::svg::Handle::from_memory(svg_data.as_slice()),
        ))
        .size(64.0);

        // 4. Standalone placeholders (skeleton loading)
        let skeleton_a = placeholder(Color::from_rgb(0.2, 0.2, 0.25), 8.0, 64.0);
        let skeleton_b = placeholder(Color::from_rgb(0.2, 0.2, 0.25), 16.0, 64.0);
        let skeleton_c = placeholder(Color::from_rgb(0.2, 0.2, 0.25), 32.0, 64.0);

        let content = column![
            text("LazyIcon Demo").size(24),
            text("Image from path (placeholder while loading):").size(14),
            row![loading_icon].spacing(12),
            text("Decoded RGBA pixels (immediate):").size(14),
            row![decoded_icon].spacing(12),
            text("SVG from bytes (immediate):").size(14),
            row![svg_icon].spacing(12),
            text("Standalone placeholders (different radii):").size(14),
            row![skeleton_a, skeleton_b, skeleton_c].spacing(12),
        ]
        .spacing(16)
        .padding(24);

        container(scrollable(content))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
