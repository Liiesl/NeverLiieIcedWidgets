use iced::widget::{column, container, text};
use iced::{Element, Length};

use neverliie_iced_widgets::slider_tooltip::{SliderTooltip, TooltipPosition};

fn main() -> iced::Result {
    iced::run(App::update, App::view)
}

#[derive(Default)]
struct App {
    basic_value: f32,
    bottom_value: f32,
    delay_value: f32,
    custom_value: f32,
}

#[derive(Debug, Clone)]
enum Message {
    BasicChanged(f32),
    BottomChanged(f32),
    DelayChanged(f32),
    CustomChanged(f32),
}

impl App {
    fn update(&mut self, message: Message) {
        match message {
            Message::BasicChanged(v) => self.basic_value = v,
            Message::BottomChanged(v) => self.bottom_value = v,
            Message::DelayChanged(v) => self.delay_value = v,
            Message::CustomChanged(v) => self.custom_value = v,
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let basic_slider = SliderTooltip::new(
            0.0..=100.0,
            self.basic_value,
            Message::BasicChanged,
        )
        .tooltip_position(TooltipPosition::Top)
        .tooltip_gap(10.0);

        let bottom_slider = SliderTooltip::new(
            0.0..=100.0,
            self.bottom_value,
            Message::BottomChanged,
        )
        .tooltip_position(TooltipPosition::Bottom);

        let delay_slider = SliderTooltip::new(
            0.0..=100.0,
            self.delay_value,
            Message::DelayChanged,
        )
        .tooltip_delay(std::time::Duration::from_millis(500))
        .tooltip_format(|v| format!("Volume: {:.0}%", v));

        let custom_style_slider = SliderTooltip::new(
            0.0..=10.0,
            self.custom_value,
            Message::CustomChanged,
        )
        .tooltip_format(|v| format!("{:.2}", v))
        .tooltip_style(|_theme| container::Style {
            background: Some(
                iced::Color::from_rgba(0.0, 0.5, 0.8, 0.95).into(),
            ),
            border: iced::Border {
                radius: 8.0.into(),
                width: 0.0,
                color: iced::Color::TRANSPARENT,
            },
            text_color: Some(iced::Color::WHITE),
            ..container::Style::default()
        });

        column![
            text("Slider Tooltip Demo").size(24),
            text(format!("Basic: {:.1}", self.basic_value)),
            text("Basic (above):"),
            basic_slider,
            text(format!("Bottom: {:.1}", self.bottom_value)),
            text("Below:"),
            bottom_slider,
            text(format!("Delay: {:.1}", self.delay_value)),
            text("With delay + custom format:"),
            delay_slider,
            text(format!("Custom: {:.2}", self.custom_value)),
            text("Custom style (blue theme):"),
            custom_style_slider,
        ]
        .spacing(10)
        .padding(20)
        .width(Length::Fill)
        .into()
    }
}
