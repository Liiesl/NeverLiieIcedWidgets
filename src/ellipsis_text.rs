//! Line-clamped text with ellipsis truncation.
//!
//! [`EllipsisText`] behaves like `iced::widget::text` but never exceeds a
//! given number of lines: if the content does not fit the available width
//! within `max_lines`, it is truncated with a trailing `…`. Truncation uses
//! the real paragraph measurement of the renderer, so it works with any font
//! and glyph widths (CJK, emoji, etc.).
//!
//! # Performance
//!
//! `iced` re-lays-out the whole tree every frame, and a truncation search
//! costs several full paragraph (re)builds. To keep that off the hot path:
//!
//! * Each widget keeps a steady-state memo in its [`Tree`] state, so frames
//!   where nothing changed are a single cheap paragraph no-op.
//! * The laid-out *paragraph* for a given `(content, size, max_lines, width)`
//!   is deterministic, so it is memoized in a shared cache keyed by content
//!   (the same pattern as `lazy_icon`'s handle into the renderer's image
//!   cache). Virtualized lists shift items between tree slots while
//!   scrolling, which busts every per-slot memo at once; the shared cache
//!   turns those frames into a cheap paragraph clone (an `Arc` bump for the
//!   wgpu renderer) instead of a full re-shape.
//!
//! # Example
//!
//! ```no_run
//! use iced::Element;
//! use neverliie_iced_widgets::ellipsis_text::{ellipsis_text, EllipsisText};
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
use iced::{Color, Element, Length, Pixels, Point, Rectangle, Size, alignment};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use unicode_segmentation::UnicodeSegmentation;

const ELLIPSIS: &str = "…";

/// Maximum number of verification steps in the hit-test fast path before
/// falling back to the measurement binary search.
const MAX_FAST_PATH_VERIFY_STEPS: usize = 8;

/// Upper bound on the number of entries in the shared paragraph cache.
/// Each entry holds one laid-out paragraph, so a few thousand are cheap;
/// when the cap is reached the least recently *inserted* entry is evicted.
const PARAGRAPH_CACHE_CAP: usize = 2048;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

type CacheKey = (u64, u32, u32, u32, u8, u8);

/// A text widget clamped to a maximum number of lines, truncating with `…`.
pub struct EllipsisText {
    content: String,
    size: f32,
    color: Option<Color>,
    max_lines: usize,
    align_x: text::Alignment,
    shaping: Shaping,
}

/// Per-widget tree state: the laid-out paragraph plus a steady-state memo.
///
/// `iced` re-lays-out the whole tree every frame, so the memo skips the
/// (expensive, paragraph-rebuilding) truncation search on frames where
/// nothing relevant changed. It is keyed on everything that affects
/// measurement (content, size, line count and available width).
///
/// The memo only survives while an item stays in the same tree slot; virtual
/// scroll lists shift items between slots every frame, which is why the laid
/// out paragraph itself is additionally memoized in a shared, content-keyed
/// cache (see [`ParagraphCache`]).
struct State<P: Paragraph> {
    paragraph: paragraph::Plain<P>,
    memo: Option<Memo>,
}

struct Memo {
    content: String,
    size: f32,
    max_lines: usize,
    width: f32,
    truncated: Option<String>,
}

/// One memoized laid-out paragraph plus the decision that produced it.
struct CachedParagraph<P: Paragraph> {
    content: String,
    /// `None` means the full content fits within `max_lines`.
    truncated: Option<String>,
    paragraph: paragraph::Plain<P>,
}

/// Shared, bounded cache of laid-out paragraphs, keyed by a content hash plus
/// the exact measurement parameters (size, line count, width, shaping and
/// horizontal alignment).
///
/// The content is stored alongside the hash and compared on lookup, so a hash
/// collision can never yield a wrong paragraph. The horizontal alignment is
/// part of the key because a paragraph carries its alignment and is drawn
/// anchored with it, so sharing an entry between widgets with different
/// alignment would misplace the text. `P` is the renderer's paragraph type;
/// each monomorphization gets its own entry in a thread-local cache (the UI
/// thread lays out and draws the whole tree, so thread locality is enough).
struct ParagraphCache<P: Paragraph> {
    entries: HashMap<CacheKey, CachedParagraph<P>>,
    /// Insertion order of the keys, used to evict the oldest entry.
    order: VecDeque<CacheKey>,
}

/// Runs `f` with the shared paragraph cache for the given paragraph type.
///
/// `thread_local!` expands to a named item that cannot capture outer generic
/// parameters, so the cache is stored type-erased and downcast on access.
fn with_paragraph_cache<P, R>(f: impl FnOnce(&mut ParagraphCache<P>) -> R) -> R
where
    P: Paragraph + 'static,
{
    thread_local! {
        static CACHE: RefCell<Option<Box<dyn std::any::Any>>> = RefCell::new(None);
    }

    CACHE.with(|cell| {
        let mut borrowed = cell.borrow_mut();
        let cache: &mut ParagraphCache<P> = borrowed
            .get_or_insert_with(|| {
                Box::new(ParagraphCache::<P> {
                    entries: HashMap::new(),
                    order: VecDeque::new(),
                })
            })
            .downcast_mut()
            .expect("paragraph cache holds an incompatible paragraph type");

        f(cache)
    })
}

fn fnv1a(content: &str) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in content.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn shaping_discriminant(shaping: Shaping) -> u8 {
    match shaping {
        Shaping::Advanced => 1,
        Shaping::Basic => 0,
        _ => 2,
    }
}

fn align_discriminant(align_x: text::Alignment) -> u8 {
    match align_x {
        text::Alignment::Default => 0,
        text::Alignment::Left => 1,
        text::Alignment::Center => 2,
        text::Alignment::Right => 3,
        text::Alignment::Justified => 4,
    }
}

fn truncation_key(
    content: &str,
    size: f32,
    max_lines: usize,
    width: f32,
    shaping: Shaping,
    align_x: text::Alignment,
) -> CacheKey {
    (
        fnv1a(content),
        size.to_bits(),
        max_lines as u32,
        width.to_bits(),
        shaping_discriminant(shaping),
        align_discriminant(align_x),
    )
}

/// Largest grapheme boundary not greater than `offset` (a byte offset).
fn truncation_cut(content: &str, offset: usize) -> usize {
    if content.is_empty() {
        return 0;
    }

    let offset = offset.min(content.len());
    let mut cut = 0;

    for (index, _) in content.grapheme_indices(true) {
        if index > offset {
            break;
        }
        cut = index;
    }

    cut
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

    /// Finds the longest prefix of `self.content` that, with a trailing `…`,
    /// fits within `max_height`, rebuilding the paragraph for each candidate.
    ///
    /// First a fast path is attempted: the full paragraph has already been
    /// laid out, so a single hit test on its last allowed line locates the cut
    /// without any re-shaping. The candidate is then verified with one real
    /// measurement (usually no extra shaping at all, since the paragraph is
    /// re-used via `update`). The measurement binary search remains
    /// as the fallback for multi-line content and fast-path verification
    /// failures. The search *result* — including the laid-out paragraph — is
    /// what gets stored in the shared cache.
    fn truncated<Renderer>(
        &self,
        state: &mut State<Renderer::Paragraph>,
        renderer: &Renderer,
        limits: &layout::Limits,
        format: Format<Renderer::Font>,
        max_height: f32,
        max_width: f32,
    ) -> Option<String>
    where
        Renderer: iced::advanced::text::Renderer,
    {
        // Width of the ellipsis glyph, measured once per invalidation in a
        // throwaway paragraph so the full paragraph above stays available for
        // the hit test.
        let mut ellipsis_paragraph = paragraph::Plain::<Renderer::Paragraph>::default();
        let ellipsis_width =
            Self::measure(&mut ellipsis_paragraph, renderer, limits, ELLIPSIS, format).width;

        let target_x = (max_width - ellipsis_width).max(0.0);

        // Fast path: probe the last line that may remain for the cut offset.
        //
        // `hit_test` reports the byte offset of the nearest character, but
        // relative to its line. For single-line content (no line break) that
        // offset is also relative to the whole content, which is exactly the
        // cut we need. Multi-line content falls back to the measurement
        // search below.
        let probe = {
            let paragraph = state.paragraph.raw();
            let line_height = paragraph.line_height().to_absolute(paragraph.size()).0;
            let y = line_height * (self.max_lines as f32 - 1.0) + line_height * 0.5;

            if self.content.contains(['\n', '\r']) {
                None
            } else {
                paragraph
                    .hit_test(Point::new(target_x, y))
                    .map(|hit| hit.cursor())
            }
        };

        if let Some(offset) = probe {
            let mut cut = truncation_cut(&self.content, offset);

            for _ in 0..MAX_FAST_PATH_VERIFY_STEPS {
                let candidate = format!("{}{}", &self.content[..cut], ELLIPSIS);
                let fits = Self::measure(&mut state.paragraph, renderer, limits, &candidate, format)
                    .height
                    <= max_height;

                if fits {
                    return Some(candidate);
                }

                cut = truncation_cut(&self.content, cut.saturating_sub(1));
                if cut == 0 {
                    break;
                }
            }
        }

        // Fallback: binary search over grapheme boundaries with a real
        // paragraph measurement for each candidate.
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
            if fits(&mut state.paragraph, &self.content[..offsets[mid]]) {
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
    Renderer::Paragraph: Clone,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<Renderer::Paragraph>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State {
            paragraph: paragraph::Plain::<Renderer::Paragraph>::default(),
            memo: None,
        })
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
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();
        let format = self.format::<Renderer::Font>();
        let max_width = limits.max().width;

        if !max_width.is_finite() || max_width <= 0.0 {
            return text::layout(&mut state.paragraph, renderer, limits, &self.content, format);
        }

        // Steady state: nothing relevant changed since the previous frame, so
        // laying out the memoized content is a no-op for the underlying
        // paragraph.
        if state.memo.as_ref().is_some_and(|memo| {
            memo.content == self.content
                && memo.size == self.size
                && memo.max_lines == self.max_lines
                && memo.width == max_width
        }) {
            let memo = state.memo.as_ref().expect("checked above");
            let content = memo.truncated.as_deref().unwrap_or(&self.content);

            return text::layout(&mut state.paragraph, renderer, limits, content, format);
        }

        let key = truncation_key(
            &self.content,
            self.size,
            self.max_lines,
            max_width,
            self.shaping,
            self.align_x,
        );

        // Shared paragraph cache: this content was laid out before, possibly
        // by a different tree slot (virtualized lists move items between
        // slots while scrolling). On a hit the laid-out paragraph is cloned
        // into this slot — an `Arc` bump plus a string copy for the wgpu
        // renderer, no re-shaping.
        let cached = with_paragraph_cache::<Renderer::Paragraph, _>(|cache| {
            cache
                .entries
                .get(&key)
                .filter(|entry| entry.content == self.content)
                .map(|entry| (entry.paragraph.clone(), entry.truncated.clone()))
        });

        if let Some((paragraph, truncated)) = cached {
            state.paragraph = paragraph;

            let min_bounds = state.paragraph.min_bounds();
            let node = layout::sized(limits, Length::Fill, Length::Shrink, |_| min_bounds);

            state.memo = Some(Memo {
                content: self.content.clone(),
                size: self.size,
                max_lines: self.max_lines,
                width: max_width,
                truncated,
            });

            return node;
        }

        // Cache miss: lay out the full content and decide whether it fits,
        // searching for the cut when it does not. The result — including the
        // laid-out paragraph — is stored in the shared cache.
        let node = text::layout(&mut state.paragraph, renderer, limits, &self.content, format);

        let paragraph = state.paragraph.raw();
        let line_height = paragraph.line_height().to_absolute(paragraph.size()).0;
        let max_height = line_height * self.max_lines as f32;

        let truncated = if node.size().height <= max_height {
            None
        } else {
            self.truncated(state, renderer, limits, format, max_height, max_width)
        };

        let node = match &truncated {
            Some(truncated) => {
                text::layout(&mut state.paragraph, renderer, limits, truncated, format)
            }
            None => node,
        };

        let cache_truncated = truncated.clone();
        let cache_paragraph = state.paragraph.clone();

        with_paragraph_cache::<Renderer::Paragraph, _>(|cache| {
            if !cache.entries.contains_key(&key) {
                if cache.entries.len() >= PARAGRAPH_CACHE_CAP {
                    if let Some(evicted) = cache.order.pop_front() {
                        cache.entries.remove(&evicted);
                    }
                }
                cache.order.push_back(key);
            }

            cache.entries.insert(
                key,
                CachedParagraph {
                    content: self.content.clone(),
                    truncated: cache_truncated,
                    paragraph: cache_paragraph,
                },
            );
        });

        state.memo = Some(Memo {
            content: self.content.clone(),
            size: self.size,
            max_lines: self.max_lines,
            width: max_width,
            truncated,
        });

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
        let state = tree.state.downcast_ref::<State<Renderer::Paragraph>>();

        text::draw(
            renderer,
            style,
            layout.bounds(),
            state.paragraph.raw(),
            Style { color: self.color },
            viewport,
        );
    }
}

impl<'a, Message, Theme, Renderer> From<EllipsisText>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::text::Renderer + 'a,
    Renderer::Paragraph: Clone,
{
    fn from(text: EllipsisText) -> Self {
        Element::new(text)
    }
}

/// Creates a new [`EllipsisText`] with the given content.
pub fn ellipsis_text(content: impl Into<String>) -> EllipsisText {
    EllipsisText::new(content)
}
