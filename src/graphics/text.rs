use super::{
    gpu::text::{Extra, TextRenderer},
    Canvas, Color, Draw, DrawParam, Drawable, GraphicsContext,
};
use crate::{context::Has, filesystem::Filesystem, GameError, GameResult};
use glyph_brush::{ab_glyph, FontId, GlyphCruncher};
use std::{collections::HashMap, io::Read, path::Path};

/// Font data that can be used to create a new font in [`GraphicsContext`].
#[derive(Debug)]
pub struct FontData {
    pub(crate) font: ab_glyph::FontArc,
}

impl FontData {
    /// Loads font data from a given path in the filesystem.
    #[allow(unused_results)]
    pub fn from_path(fs: &impl Has<Filesystem>, path: impl AsRef<Path>) -> GameResult<Self> {
        let fs = fs.retrieve();

        let mut bytes = vec![];
        fs.open(path)?.read_to_end(&mut bytes)?;
        Ok(FontData {
            font: ab_glyph::FontArc::try_from_vec(bytes)?,
        })
    }

    /// Loads font data from owned bytes.
    pub fn from_vec(data: Vec<u8>) -> GameResult<Self> {
        Ok(FontData {
            font: ab_glyph::FontArc::try_from_vec(data)?,
        })
    }

    /// Loads font data from static bytes.
    pub fn from_slice(data: &'static [u8]) -> GameResult<Self> {
        Ok(FontData {
            font: ab_glyph::FontArc::try_from_slice(data)?,
        })
    }
}

pub use glyph_brush::ab_glyph::PxScale;

/// Parameters of a single piece ("fragment") of text, including font, color, and size.
#[derive(Debug, Default, Clone)]
pub struct TextFragment {
    /// The text itself.
    pub text: String,
    /// Font name of the text framgnet, defaults to text's font.
    pub font: Option<String>,
    /// Pixel scale of the text framgent, defaults to text's scale.
    pub scale: Option<PxScale>,
    /// Color of the text fragment, defaults to the text's color.
    pub color: Option<Color>,
}

impl TextFragment {
    /// Creates a new fragment with text set to a string.
    pub fn new(text: impl Into<String>) -> Self {
        TextFragment {
            text: text.into(),
            ..Default::default()
        }
    }

    /// Sets the `font` field, overriding the text's font.
    pub fn font(self, font: impl Into<String>) -> Self {
        TextFragment {
            font: Some(font.into()),
            ..self
        }
    }

    /// Sets the `scale` field, overriding the text's scale.
    pub fn scale(self, scale: impl Into<PxScale>) -> Self {
        TextFragment {
            scale: Some(scale.into()),
            ..self
        }
    }

    /// Sets the `color` field, overriding the text's color.
    pub fn color(self, color: impl Into<Color>) -> Self {
        TextFragment {
            color: Some(color.into()),
            ..self
        }
    }
}

impl<S: Into<String>> From<S> for TextFragment {
    fn from(text: S) -> Self {
        TextFragment::new(text)
    }
}

/// Drawable text object.  Essentially a list of [`TextFragment`].
/// and some cached size information.
///
/// It implements [`Drawable`] so it can be drawn immediately with [`Canvas::draw()`].
#[derive(Debug, Clone)]
pub struct Text {
    fragments: Vec<TextFragment>,
    layout: TextLayout,
    wrap: bool,
    bounds: mint::Vector2<f32>,
    scale: PxScale,
    font: String,
}

impl Default for Text {
    fn default() -> Self {
        Self {
            fragments: Vec::new(),
            layout: TextLayout::top_left(),
            wrap: true,
            bounds: mint::Vector2::<f32> {
                x: f32::INFINITY,
                y: f32::INFINITY,
            },
            scale: 16.0.into(),
            font: "LiberationMono-Regular".into(),
        }
    }
}

impl Text {
    /// Creates a `Text` from a `TextFragment`.
    ///
    /// ```rust
    /// # use ggez::graphics::Text;
    /// # fn main() {
    /// let text = Text::new("foo");
    /// # }
    /// ```
    pub fn new(fragment: impl Into<TextFragment>) -> Self {
        let mut text = Text::default();
        let _ = text.add(fragment);
        text
    }

    /// Appends a `TextFragment` to the `Text`.
    pub fn add(&mut self, fragment: impl Into<TextFragment>) -> &mut Self {
        self.fragments.push(fragment.into());
        self
    }

    /// Clear all `TextFragment` from the `Text`
    pub fn clear(&mut self) {
        self.fragments.clear();
    }

    /// Returns an immutable slice of all `TextFragment`s.
    #[inline]
    pub fn fragments(&self) -> &[TextFragment] {
        &self.fragments
    }

    /// Returns a mutable slice of all `TextFragment`s.
    #[inline]
    pub fn fragments_mut(&mut self) -> &mut [TextFragment] {
        &mut self.fragments
    }

    /// Specifies rectangular dimensions to fit text inside of,
    /// wrapping where necessary. Within these bounds is also where
    /// text alignment occurs.
    pub fn set_bounds(&mut self, bounds: impl Into<mint::Vector2<f32>>) -> &mut Self {
        self.bounds = bounds.into();
        self
    }

    /// Specifies how the text will be layed out.
    pub fn set_layout(&mut self, layout: TextLayout) -> &mut Self {
        self.layout = layout;
        self
    }

    /// Specifies whether or not the text will be wrapped within the bounds bounds specified by [`Text::set_bounds`].
    pub fn set_wrap(&mut self, wrap: bool) -> &mut Self {
        self.wrap = wrap;
        self
    }

    /// Specifies the text's font for fragments that don't specify their own font.
    ///
    /// Note: [`Canvas::finish`] will return a [`GameError::FontSelectError`] if the given font is not loaded.
    /// See [`GraphicsContext::add_font`] and [`GraphicsContext::has_font`]
    pub fn set_font(&mut self, font: impl Into<String>) -> &mut Self {
        self.font = font.into();
        self
    }

    /// Specifies the text's font scale for fragments that don't specify their own scale.
    pub fn set_scale(&mut self, scale: impl Into<PxScale>) -> &mut Self {
        self.scale = scale.into();
        self
    }

    /// Returns the string that the text represents.
    pub fn contents(&self) -> String {
        self.fragments.iter().map(|f| f.text.as_str()).collect()
    }

    /// Returns a `Vec` containing the coordinates of the formatted and wrapped text.
    pub fn glyph_positions(
        &self,
        gfx: &impl Has<GraphicsContext>,
    ) -> GameResult<Vec<mint::Point2<f32>>> {
        let gfx = gfx.retrieve();
        Ok(gfx
            .text
            .glyph_brush
            .borrow_mut()
            .glyphs(self.as_section(&gfx.fonts, DrawParam::default())?)
            .map(|glyph| mint::Point2::<f32> {
                x: glyph.glyph.position.x,
                y: glyph.glyph.position.y,
            })
            .collect())
    }

    /// Measures the glyph boundaries for the text.
    #[inline]
    pub fn measure(&self, gfx: &impl Has<GraphicsContext>) -> GameResult<mint::Vector2<f32>> {
        let gfx = gfx.retrieve();
        self.measure_raw(&gfx.text, &gfx.fonts)
    }

    pub(crate) fn measure_raw(
        &self,
        text: &TextRenderer,
        fonts: &HashMap<String, FontId>,
    ) -> GameResult<mint::Vector2<f32>> {
        Ok(text
            .glyph_brush
            .borrow_mut()
            .glyph_bounds(self.as_section(fonts, DrawParam::default())?)
            .map(|rect| mint::Vector2::<f32> {
                x: rect.width(),
                y: rect.height(),
            })
            .unwrap_or_else(|| mint::Vector2::<f32> { x: 0., y: 0. }))
    }

    pub(crate) fn as_section<'a>(
        &'a self,
        fonts: &HashMap<String, FontId>,
        param: DrawParam,
    ) -> GameResult<glyph_brush::Section<'a, Extra>> {
        Ok(glyph_brush::Section {
            screen_position: (0., 0.),

            bounds: (self.bounds.x, self.bounds.y),
            layout: if self.wrap {
                glyph_brush::Layout::default_wrap()
            } else {
                glyph_brush::Layout::default_single_line()
            }
            .h_align(self.layout.h_align.into())
            .v_align(self.layout.v_align.into()),

            text: self
                .fragments
                .iter()
                .map(|text| {
                    let font = text.font.as_ref().unwrap_or(&self.font);
                    Ok(glyph_brush::Text {
                        text: &text.text,
                        scale: text.scale.unwrap_or(self.scale),
                        font_id: *fonts
                            .get(font)
                            .ok_or_else(|| GameError::FontSelectError(font.clone()))?,
                        extra: Extra {
                            color: text.color.unwrap_or(param.color).into(),
                            transform: param.transform.to_bare_matrix_glam(),
                        },
                    })
                })
                .collect::<GameResult<Vec<_>>>()?,
        })
    }
}

impl Drawable for Text {
    fn draw(&self, canvas: &mut Canvas, param: impl Into<DrawParam>) {
        canvas.push_draw(Draw::BoundedText { text: self.clone() }, param.into());
    }
}

/// Describes text alignment along a single axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TextAlign {
    /// Text is aligned to the beginning of the axis (left, top).
    Begin,
    /// Text is aligned to the center of the axis.
    Middle,
    /// Text is aligned to the end of the axis (right, bottom).
    End,
}

impl From<TextAlign> for glyph_brush::HorizontalAlign {
    fn from(align: TextAlign) -> Self {
        match align {
            TextAlign::Begin => glyph_brush::HorizontalAlign::Left,
            TextAlign::Middle => glyph_brush::HorizontalAlign::Center,
            TextAlign::End => glyph_brush::HorizontalAlign::Right,
        }
    }
}

impl From<TextAlign> for glyph_brush::VerticalAlign {
    fn from(align: TextAlign) -> Self {
        match align {
            TextAlign::Begin => glyph_brush::VerticalAlign::Top,
            TextAlign::Middle => glyph_brush::VerticalAlign::Center,
            TextAlign::End => glyph_brush::VerticalAlign::Bottom,
        }
    }
}

/// Describes text alignment along both axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextLayout {
    /// Horizontal alignment.
    pub h_align: TextAlign,
    /// Vertical alignment.
    pub v_align: TextAlign,
}

impl TextLayout {
    /// Text aligned to the top-left.
    pub fn top_left() -> Self {
        TextLayout {
            h_align: TextAlign::Begin,
            v_align: TextAlign::Begin,
        }
    }

    /// Text aligned to the center.
    pub fn center() -> Self {
        TextLayout {
            h_align: TextAlign::Middle,
            v_align: TextAlign::Middle,
        }
    }
}
