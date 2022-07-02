//! The graphics module performs the perhaps most important task of ggez, which is
//! **drawing things onto the screen**.
//!
//! The rough workflow for this is usually as follows:
//! 1. Create something that you want to render ([`Mesh`]es, [`Image`]s, [`InstanceArray`]s or [`Text`]s).
//! 2. Create a [`Canvas`] to render them onto (usually by calling [`Canvas::from_frame`], to draw directly onto the screen).
//! 3. (Select a [custom shader] and/or [blend mode] if you desire.)
//! 4. Queue draw calls by calling the appropriate draw method on [`Canvas`] ([`Canvas::draw`], `Canvas::draw_<whatever>`).
//! 5. (Go back to step 3 if you want to add more draw calls with different shaders or blend modes.)
//! 6. Submit the draw queue by calling [`Canvas::finish`].
//!
//! A [`Canvas`] represents a single render pass, operating on a certain render target.
//! You can create [`Canvas`]es that render to an image, instead of directly to the screen, with
//! [`Canvas::from_image`] and other related functions. With these you can, for example, render your
//! scene onto an [`Image`] held by a [`Canvas`] first and then render that [`Image`] onto the screen,
//! using a different shader, to do some post-processing.
//!
//! The module also handles the creation of [`Image`]s and other drawable objects and the screen
//! coordinate system / projection matrix through [`Canvas`].
//!
//! [custom shader]:Canvas::set_shader
//! [blend mode]:Canvas::set_blend_mode

pub(crate) mod canvas;
pub(crate) mod context;
pub(crate) mod draw;
pub(crate) mod gpu;
pub(crate) mod image;
pub(crate) mod instance;
pub(crate) mod internal_canvas;
pub(crate) mod mesh;
pub(crate) mod sampler;
pub(crate) mod shader;
pub(crate) mod text;
mod types;

pub use lyon::tessellation::{FillOptions, FillRule, LineCap, LineJoin, StrokeOptions};
pub use {
    self::image::*, canvas::*, context::*, draw::*, instance::*, mesh::*, sampler::*, shader::*,
    text::*, types::*,
};

/// Applies `DrawParam` to `Rect`.
pub fn transform_rect(rect: Rect, param: DrawParam) -> Rect {
    match param.transform {
        Transform::Values {
            scale,
            offset,
            dest,
            rotation,
        } => {
            // first apply the offset
            let mut r = Rect {
                w: rect.w,
                h: rect.h,
                x: rect.x - offset.x * rect.w,
                y: rect.y - offset.y * rect.h,
            };
            // apply the scale
            let real_scale = (param.src.w * scale.x, param.src.h * scale.y);
            r.w = real_scale.0 * rect.w;
            r.h = real_scale.1 * rect.h;
            r.x *= real_scale.0;
            r.y *= real_scale.1;
            // apply the rotation
            r.rotate(rotation);
            // apply the destination translation
            r.x += dest.x;
            r.y += dest.y;

            r
        }
        Transform::Matrix(_m) => todo!("Fix me"),
    }
}

use crate::{
    context::{Has, HasTwo},
    filesystem::Filesystem,
    GameResult,
};
use mint::Point2;
use std::path::Path;

/// Draws the given Drawable object to the screen by calling its draw() method.
#[deprecated(
    since = "0.8.0",
    note = "Use `drawable.draw` or `canvas.draw` instead."
)]
pub fn draw(canvas: &mut Canvas, drawable: &impl Drawable, param: impl Into<DrawParam>) {
    drawable.draw(canvas, param);
}

/// Sets the window icon. `None` for path removes the icon.
#[deprecated(since = "0.8.0", note = "Use `ctx.gfx.set_window_icon` instead.")]
pub fn set_window_icon<P: AsRef<Path>>(
    ctx: &impl HasTwo<GraphicsContext, Filesystem>,
    path: impl Into<Option<P>>,
) -> GameResult {
    let gfx: &GraphicsContext = ctx.retrieve_first();
    let fs: &Filesystem = ctx.retrieve_second();
    gfx.set_window_icon(fs, path)
}

/// Sets the window position.
#[deprecated(since = "0.8.0", note = "Use `ctx.gfx.set_window_position` instead.")]
pub fn set_window_position(
    ctx: &impl Has<GraphicsContext>,
    position: impl Into<winit::dpi::Position>,
) -> GameResult {
    let gfx: &GraphicsContext = ctx.retrieve();
    gfx.set_window_position(position)
}

/// Returns a reference to the Winit window.
#[deprecated(since = "0.8.0", note = "Use `ctx.gfx.window` instead.")]
pub fn window(ctx: &impl Has<GraphicsContext>) -> &winit::window::Window {
    let gfx: &GraphicsContext = ctx.retrieve();
    gfx.window()
}

/// Sets the window title.
#[deprecated(since = "0.8.0", note = "Use `ctx.gfx.set_window_title` instead.")]
pub fn set_window_title(ctx: &impl Has<GraphicsContext>, title: &str) {
    let gfx: &GraphicsContext = ctx.retrieve();
    gfx.set_window_title(title);
}

/// Draws text.
#[deprecated(
    since = "0.8.0",
    note = "Don't use the `queue_text` and `draw_queued_text` system. Instead draw the texts directly."
)]
pub fn queue_text(
    canvas: &mut Canvas,
    text: &Text,
    relative_dest: impl Into<Point2<f32>>,
    color: Option<Color>,
) {
    canvas
        .queued_texts
        .push((text.clone(), relative_dest.into(), color));
}

/// Draws all of the Texts added via queue_text().
#[deprecated(
    since = "0.8.0",
    note = "Don't use the `queue_text` and `draw_queued_text` system. Instead draw the texts directly."
)]
pub fn draw_queued_text(
    canvas: &mut Canvas,
    param: impl Into<DrawParam>,
    blend: Option<BlendMode>,
    filter: FilterMode,
) -> GameResult {
    let mut param = param.into();
    let param_dest = *param.get_dest_mut()?;

    canvas.set_sampler(filter);
    if let Some(blend) = blend {
        canvas.set_blend_mode(blend);
    }

    for queued_text in std::mem::take(&mut canvas.queued_texts) {
        queued_text.0.draw(
            canvas,
            param.dest(mint::Point2 {
                x: param_dest.x + queued_text.1.x,
                y: param_dest.y + queued_text.1.y,
            }),
        )
    }

    Ok(())
}
