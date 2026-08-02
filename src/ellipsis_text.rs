//! Line-clamped text with ellipsis truncation.
//!
//! [`EllipsisText`] behaves like `iced::widget::text` but never exceeds a
//! given number of lines: if the content does not fit the available width
//! within `max_lines`, it is truncated with a trailing `…`. Truncation uses
//! the real paragraph measurement of the renderer, so it works with any font
//! and glyph widths (CJK, emoji, etc.).
//!
//! # Example
//!
//! ```no_run
//! use iced::Element;
//! use neverlie_iced_widgets::ellipsis_text::{ellipsis_text, EllipsisText};
//!
//! enum Message {}
//!
//! fn view() -> Element<'_, Message> {
//!     EllipsisText::new("A very long file name that must be clamped")
//!         .size(12)
//!         .max_lines(2)
//!         .into()
//! }
//!
//! fn view_helper() -> Element<'_, Message> {
//!     ellipsis_text("Short label").max_lines(2).into()
//! }
//! ```

use iced::advanced::layout::{self, Layout};
use iced::advanced::mouse;
use iced::advanced::renderer;
use iced::advanced::text::{paragraph, Paragraph};
use iced::advanced::widget::text::{self, Format, LineHeight, Shaping, Style, Wrapping};
use iced::advanced::widget::{tree, Tree, Widget};
use iced::{Color, Element, Length, Pixels, Rectangle, Size, alignment};
use unicode_segmentation::UnicodeSegmentation;

const ELLIPSIS: &str = "…";

/// A text widget clamped to a maximum number of lines, truncating with `…`.
pub struct EllipsisText {
    content: String,
    size: f32,
    color: Option<Color>,
    max_lines: usize,
    align_x: text::Alignment,
    shaping: Shaping,
}

impl EllipsisText {
    /// Creates a new [`EllipsisText`] with the given content.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            size: 16.0,
            color: None,
            max_lines: 2,
            align_x: text::Alignment::Default,
            shaping: Shaping::default(),
        }
    }

    /// Sets the size of the [`EllipsisText`].
    #[must_use]
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = size.into().0;
        self
    }

    /// Sets the [`Color`] of the [`EllipsisText`].
    #[must_use]
    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// Sets the maximum number of lines before truncating with `…`.
    #[must_use]
    pub fn max_lines(mut self, max_lines: usize) -> Self {
        self.max_lines = max_lines.max(1);
        self
    }

    /// Sets the horizontal alignment of the [`EllipsisText`].
    #[must_use]
    pub fn align_x(mut self, alignment: impl Into<text::Alignment>) -> Self {
        self.align_x = alignment.into();
        self
    }

    /// Sets the [`Shaping`] strategy of the [`EllipsisText`].
    #[must_use]
    pub fn shaping(mut self, shaping: Shaping) -> Self {
        self.shaping = shaping;
        self
    }

    fn format<Font>(&self) -> Format<Font> {
        Format {
            width: Length::Fill,
            height: Length::Shrink,
            size: Some(Pixels(self.size)),
            font: None,
            line_height: LineHeight::Relative(1.0),
            align_x: self.align_x,
            align_y: alignment::Vertical::Top,
            shaping: self.shaping,
            wrapping: Wrapping::WordOrGlyph,
        }
    }

    fn measure<Renderer>(
        state: &mut paragraph::Plain<Renderer::Paragraph>,
        renderer: &Renderer,
        limits: &layout::Limits,
        content: &str,
        format: Format<Renderer::Font>,
    ) -> Size
    where
        Renderer: iced::advanced::text::Renderer,
    {
        let _ = text::layout(state, renderer, limits, content, format);
        state.min_bounds()
    }

    fn truncated<Renderer>(
        &self,
        state: &mut paragraph::Plain<Renderer::Paragraph>,
        renderer: &Renderer,
        limits: &layout::Limits,
        format: Format<Renderer::Font>,
        max_height: f32,
    ) -> Option<String>
    where
        Renderer: iced::advanced::text::Renderer,
    {
        let mut offsets: Vec<usize> = self
            .content
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .collect();
        if offsets.is_empty() {
            return None;
        }
        offsets.push(self.content.len());

        let fits = |state: &mut paragraph::Plain<Renderer::Paragraph>, prefix: &str| {
            let candidate = format!("{}{}", prefix, ELLIPSIS);
            Self::measure(state, renderer, limits, &candidate, format).height <= max_height
        };

        let mut low = 0;
        let mut high = offsets.len() - 1;
        while low < high {
            let mid = (low + high + 1) / 2;
            if fits(state, &self.content[..offsets[mid]]) {
                low = mid;
            } else {
                high = mid - 1;
            }
        }

        Some(format!("{}{}", &self.content[..offsets[low]], ELLIPSIS))
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for EllipsisText
where
    Renderer: iced::advanced::text::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<paragraph::Plain<Renderer::Paragraph>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(paragraph::Plain::<Renderer::Paragraph>::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree.state.downcast_mut::<paragraph::Plain<Renderer::Paragraph>>();
        let format = self.format::<Renderer::Font>();

        let node = text::layout(state, renderer, limits, &self.content, format);

        let max_width = limits.max().width;
        if !max_width.is_finite() || max_width <= 0.0 {
            return node;
        }

        // Plain paragraphs lay out every line at exactly the requested line
        // height, so the paragraph itself reports the single-line height.
        let paragraph = state.raw();
        let line_height = paragraph.line_height().to_absolute(paragraph.size()).0;
        let max_height = line_height * self.max_lines as f32;

        if node.size().height <= max_height {
            return node;
        }

        if let Some(truncated) =
            self.truncated(state, renderer, limits, format, max_height)
        {
            return text::layout(state, renderer, limits, &truncated, format);
        }

        node
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<paragraph::Plain<Renderer::Paragraph>>();

        text::draw(
            renderer,
            style,
            layout.bounds(),
            state.raw(),
            Style { color: self.color },
            viewport,
        );
    }
}

impl<'a, Message, Theme, Renderer> From<EllipsisText>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::text::Renderer + 'a,
{
    fn from(text: EllipsisText) -> Self {
        Element::new(text)
    }
}

/// Creates a new [`EllipsisText`] with the given content.
pub fn ellipsis_text(content: impl Into<String>) -> EllipsisText {
    EllipsisText::new(content)
}
