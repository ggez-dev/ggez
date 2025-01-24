use super::{Canvas, Color, LinearColor, Rect};
use glam::{Mat4, Vec4};

/// A struct that represents where to put a drawable object.
///
/// This can either be a set of individual components, or
/// a single `Matrix4` transform.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Transform {
    /// Transform made of individual values
    Values {
        /// The position to draw the graphic expressed as a `Point2`.
        dest: mint::Point2<f32>,
        /// The orientation of the graphic in radians.
        rotation: f32,
        /// The x/y scale factors expressed as a `Vector2`.
        scale: mint::Vector2<f32>,
        /// An offset, which is applied before scaling and rotation happen.
        ///
        /// For most objects this works as a relative offset (meaning `[0.5,0.5]` is an offset which
        /// centers the object on the destination). These objects are:
        /// * `Image`, `Canvas`, `Text` and the sprites inside an `InstanceArray` (as long as you're
        ///   not making an instanced mesh-draw)
        offset: mint::Point2<f32>,
    },
    /// Transform made of an arbitrary matrix.
    ///
    /// It should represent the final model matrix of the given drawable.  This is useful for
    /// situations where, for example, you build your own hierarchy system, where you calculate
    /// matrices of each hierarchy item and store a calculated world-space model matrix of an item.
    /// This lets you implement transform stacks, skeletal animations, etc.
    Matrix(mint::ColumnMatrix4<f32>),
}

impl Default for Transform {
    fn default() -> Self {
        Transform::Values {
            dest: mint::Point2 { x: 0.0, y: 0.0 },
            rotation: 0.0,
            scale: mint::Vector2 { x: 1.0, y: 1.0 },
            offset: mint::Point2 { x: 0.0, y: 0.0 },
        }
    }
}

fn transform_to_matrix(
    dest: mint::Point2<f32>,
    rotation: f32,
    scale: mint::Vector2<f32>,
    offset: mint::Point2<f32>,
) -> Mat4 {
    // Calculate a matrix equivalent to doing this:
    // type Vec3 = na::Vector3<f32>;
    // let o = offset;
    // let translate = na::Matrix4::new_translation(&Vec3::new(dest.x, dest.y, 0.0));
    // let offset = na::Matrix4::new_translation(&Vec3::new(offset.x, offset.y, 0.0));
    // let offset_inverse =
    //     na::Matrix4::new_translation(&Vec3::new(-o.x, -o.y, 0.0));
    // let axis_angle = Vec3::z() * *rotation;
    // let rotation = na::Matrix4::new_rotation(axis_angle);
    // let scale = na::Matrix4::new_nonuniform_scaling(&Vec3::new(scale.x, scale.y, 1.0));
    // translate * rotation * scale * offset_inverse
    //
    // Doing the bits manually is faster though, or at least was last I checked.
    let (sinr, cosr) = rotation.sin_cos();
    let m00 = cosr * scale.x;
    let m01 = -sinr * scale.y;
    let m10 = sinr * scale.x;
    let m11 = cosr * scale.y;
    let m03 = offset.x * (-m00) - offset.y * m01 + dest.x;
    let m13 = offset.y * (-m11) - offset.x * m10 + dest.y;
    // Welp, this transpose fixes some bug that makes nothing draw,
    // that was introduced in commit 2c6b3cc03f34fb240f4246f5a68c75bd85b60eae.
    // The best part is, I don't know if this code is wrong, or whether there's
    // some reversed matrix multiply or such somewhere else that this cancel
    // out.  Probably the former though.
    glam::Mat4::from_cols_array(&[
        m00, m01, 0.0, m03, // oh rustfmt you so fine
        m10, m11, 0.0, m13, // you so fine you blow my mind
        0.0, 0.0, 1.0, 0.0, // but leave my matrix formatting alone
        0.0, 0.0, 0.0, 1.0, // plz
    ])
    .transpose()
}

impl Transform {
    /// Crunches the transform down to a single matrix, if it's not one already.
    #[must_use]
    pub fn to_matrix(self) -> Self {
        Transform::Matrix(self.to_bare_matrix())
    }

    /// Same as `to_matrix()` but just returns a bare `mint` matrix.
    #[must_use]
    pub fn to_bare_matrix(self) -> mint::ColumnMatrix4<f32> {
        match self {
            Transform::Matrix(m) => m,
            Transform::Values {
                dest,
                rotation,
                scale,
                offset,
            } => transform_to_matrix(dest, rotation, scale, offset).into(),
        }
    }

    /// Same as `to_matrix()` but just returns a bare `glam` matrix.
    #[must_use]
    pub(crate) fn to_bare_matrix_glam(self) -> Mat4 {
        match self {
            Transform::Matrix(m) => m.into(),
            Transform::Values {
                dest,
                rotation,
                scale,
                offset,
            } => transform_to_matrix(dest, rotation, scale, offset),
        }
    }
}

/// Value describing the Z "coordinate" of a draw.
///
/// Greater values correspond to the foreground, and lower values
/// correspond to the background.
///
/// [`InstanceArray`](crate::graphics::InstanceArray)s internally uphold this order for their instances, _if_ they're created with
/// `ordered` set to `true`.
pub type ZIndex = i32;

/// A struct containing all the necessary info for drawing with parameters.
///
/// This struct implements the `Default` trait, so to set only some parameter
/// you can just do:
///
/// ```rust
/// # use ggez::*;
/// # use ggez::graphics::*;
/// # fn t(canvas: &mut Canvas, image: Image) {
/// let my_dest = glam::vec2(13.0, 37.0);
/// canvas.draw(&image, DrawParam::default().dest(my_dest));
/// # }
/// ```
///
/// As a shortcut, it also implements [`From` for `Into<Point2<f32>>`](#impl-From<P>).
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct DrawParam {
    /// A portion of the drawable to clip, as a fraction of the whole image.
    /// Defaults to the whole image (\[0.0, 0.0\] to \[1.0, 1.0\]) if omitted.
    pub src: Rect,
    /// Default: white.
    pub color: Color,
    /// Where to put the object.
    pub transform: Transform,
    /// The Z coordinate of the draw.
    pub z: ZIndex,
}

impl Default for DrawParam {
    fn default() -> Self {
        DrawParam {
            src: Rect::one(),
            color: Color::WHITE,
            transform: Transform::default(),
            z: 0,
        }
    }
}

impl DrawParam {
    /// Create a new `DrawParam` with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the source rect.
    #[must_use]
    pub fn src(mut self, src: Rect) -> Self {
        self.src = src;
        self
    }

    pub(crate) fn get_dest_mut(&mut self) -> &mut mint::Point2<f32> {
        if let Transform::Values { dest, .. } = &mut self.transform {
            dest
        } else {
            panic!("Cannot calculate destination value for a DrawParam matrix")
        }
    }

    /// Set the dest point.
    ///
    /// # Panics
    ///
    /// Panics if `Transform` is of the `Matrix` variant.
    pub fn dest<P>(mut self, dest: P) -> Self
    where
        P: Into<mint::Point2<f32>>,
    {
        *self.get_dest_mut() = dest.into();
        self
    }

    /// Set the `dest` and `scale` together.
    ///
    /// # Panics
    ///
    /// Panics if `Transform` is of the `Matrix` variant.
    #[must_use]
    pub fn dest_rect(self, rect: Rect) -> Self {
        self.dest(rect.point()).scale(rect.size())
    }

    /// Set the color. This will be blended with whatever
    /// color the drawn object already is.
    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.color = color.into();
        self
    }

    /// Set the rotation.
    ///
    /// # Panics
    ///
    /// Panics if `Transform` is of the `Matrix` variant.
    #[must_use]
    pub fn rotation(mut self, rot: f32) -> Self {
        if let Transform::Values {
            ref mut rotation, ..
        } = self.transform
        {
            *rotation = rot;
            self
        } else {
            panic!("Cannot set values for a DrawParam matrix")
        }
    }

    /// Set the scaling factors.
    ///
    /// # Panics
    ///
    /// Panics if `Transform` is of the `Matrix` variant.
    pub fn scale<V>(mut self, scale_: V) -> Self
    where
        V: Into<mint::Vector2<f32>>,
    {
        if let Transform::Values { ref mut scale, .. } = self.transform {
            let p: mint::Vector2<f32> = scale_.into();
            *scale = p;
            self
        } else {
            panic!("Cannot set values for a DrawParam matrix")
        }
    }

    /// Set the transformation offset.
    ///
    /// # Panics
    ///
    /// Panics if `Transform` is of the `Matrix` variant.
    pub fn offset<P>(mut self, offset_: P) -> Self
    where
        P: Into<mint::Point2<f32>>,
    {
        if let Transform::Values { ref mut offset, .. } = self.transform {
            let p: mint::Point2<f32> = offset_.into();
            *offset = p;
            self
        } else {
            panic!("Cannot set values for a DrawParam matrix")
        }
    }

    /// Set the transformation matrix.
    pub fn transform<M>(mut self, transform: M) -> Self
    where
        M: Into<mint::ColumnMatrix4<f32>>,
    {
        self.transform = Transform::Matrix(transform.into());
        self
    }

    /// Set the Z coordinate.
    pub fn z(mut self, z: ZIndex) -> Self {
        self.z = z;
        self
    }
}

/// Create a `DrawParam` from a location, like this:
///
/// ```rust
/// # use ggez::*;
/// # use ggez::graphics::*;
/// # fn t(canvas: &mut Canvas, image: Image) {
/// let my_dest = glam::vec2(13.0, 37.0);
/// canvas.draw(&image, my_dest);
/// # }
/// ```
impl<P> From<P> for DrawParam
where
    P: Into<mint::Point2<f32>>,
{
    fn from(location: P) -> Self {
        DrawParam::new().dest(location)
    }
}

/// All types that can be drawn onto a canvas implement the `Drawable` trait.
pub trait Drawable {
    /// Draws the drawable onto the canvas.
    fn draw(&self, canvas: &mut Canvas, param: impl Into<DrawParam>);
}

#[derive(Debug, Copy, Clone, crevice::std140::AsStd140)]
pub(crate) struct DrawUniforms {
    pub color: Vec4,
    pub src_rect: Vec4,
    pub transform: Mat4,
}

#[allow(unsafe_code)]
unsafe impl bytemuck::Zeroable for DrawUniforms {}
#[allow(unsafe_code)]
unsafe impl bytemuck::Pod for DrawUniforms {}

impl DrawUniforms {
    pub fn from_param(param: &DrawParam, image_scale: Option<mint::Vector2<f32>>) -> Self {
        let (scale_x, scale_y) = if let Some(image_scale) = image_scale {
            (image_scale.x * param.src.w, image_scale.y * param.src.h)
        } else {
            (1., 1.)
        };

        let param = match param.transform {
            Transform::Values { scale, .. } => param.scale(mint::Vector2 {
                x: scale.x * scale_x,
                y: scale.y * scale_y,
            }),
            Transform::Matrix(m) => param.transform(
                glam::Mat4::from(m) * glam::Mat4::from_scale(glam::vec3(scale_x, scale_y, 1.)),
            ),
        };

        let color = LinearColor::from(param.color);

        DrawUniforms {
            color: Vec4::from_array(color.into()),
            src_rect: Vec4::new(
                param.src.x,
                param.src.y,
                param.src.x + param.src.w,
                param.src.y + param.src.h,
            ),
            transform: param.transform.to_bare_matrix_glam(),
        }
    }
}
