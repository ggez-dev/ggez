use glam::{Mat4, Vec3, Vec4};

use crate::graphics::{Canvas3d, Color};

use super::{LinearColor, ZIndex};

/// A 3d version of `DrawParam` used for transformation of 3d meshes
#[derive(Clone, Copy, Debug)]
pub struct DrawParam3d {
    /// The transform of the mesh to draw see `Transform3d`
    pub transform: Transform3d,
    /// The alpha component is used for intensity of blending instead of actual alpha
    pub color: Color,
    /// This is used for the order of rendering
    pub z: ZIndex,
}

impl DrawParam3d {
    /// Change the scale of the `Transform3d`
    ///
    /// # Panics
    ///
    /// Panics if `Transform3d` is of the `Matrix` variant.
    #[must_use]
    pub fn scale<V>(mut self, scale_: V) -> Self
    where
        V: Into<mint::Vector3<f32>>,
    {
        let p: mint::Vector3<f32> = scale_.into();
        self.transform = self.transform.scale(p);
        self
    }

    /// Change the position of the `Transform3d`
    ///
    /// # Panics
    ///
    /// Panics if `Transform3d` is of the `Matrix` variant.
    #[must_use]
    pub fn position<P>(mut self, position_: P) -> Self
    where
        P: Into<mint::Point3<f32>>,
    {
        let p: mint::Point3<f32> = position_.into();
        self.transform = self.transform.position(p);
        self
    }

    /// Change the rotation of the `Transform3d`
    ///
    /// # Panics
    ///
    /// Panics if `Transform3d` is of the `Matrix` variant.
    #[must_use]
    pub fn rotation<R>(mut self, rotation_: R) -> Self
    where
        R: Into<mint::Quaternion<f32>>,
    {
        let p: mint::Quaternion<f32> = rotation_.into();
        self.transform = self.transform.rotation(p);
        self
    }

    /// Move the position by given amount
    ///
    /// # Panics
    ///
    /// Panics if `Transform3d` is of the `Matrix` variant.
    #[must_use]
    pub fn translate<T>(mut self, translate_: T) -> Self
    where
        T: Into<mint::Vector3<f32>>,
    {
        let t: mint::Vector3<f32> = translate_.into();
        self.transform = self.transform.translate(t);
        self
    }
    /// Change the pivot of the `DrawParam3d`
    ///
    /// # Panics
    ///
    /// Panics if `Transform3d` is of the `Matrix` variant.
    #[must_use]
    pub fn pivot<P>(mut self, pivot_: P) -> Self
    where
        P: Into<mint::Point3<f32>>,
    {
        let p: mint::Point3<f32> = pivot_.into();
        self.transform = self.transform.pivot(p);
        self
    }

    /// Change the offset of the `DrawParam3d`
    ///
    /// # Panics
    ///
    /// Panics if `Transform3d` is of the `Matrix` variant.
    #[must_use]
    pub fn offset<O>(mut self, offset_: O) -> Self
    where
        O: Into<mint::Point3<f32>>,
    {
        let o: mint::Point3<f32> = offset_.into();
        self.transform = self.transform.offset(o);
        self
    }
    /// Change the color of the `DrawParam3d`
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Change the transform of the `DrawParam3d`
    #[must_use]
    pub fn transform(mut self, transform: Transform3d) -> Self {
        self.transform = transform;
        self
    }
}

impl Default for DrawParam3d {
    fn default() -> Self {
        Self {
            transform: Transform3d::default(),
            color: Color::new(1.0, 1.0, 1.0, 0.0),
            z: 0,
        }
    }
}

/// Represents transformations for 3d objects
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Transform3d {
    /// Transform made of individual values
    Values {
        /// The position to draw the graphic expressed as a `Point3`.
        pos: mint::Point3<f32>,
        /// The orientation of the graphic as a Quaternion.
        rotation: mint::Quaternion<f32>,
        /// The x/y scale factors expressed as a `Vector3`.
        scale: mint::Vector3<f32>,
        /// An offset, which is applied before scaling and rotation happen. By default this will be the distance to the center of the given mesh or model
        offset: Option<mint::Point3<f32>>,
        /// The pivot point or origin of the transform3d. By default this will be the center of the mesh/model in world space.
        pivot: Option<mint::Point3<f32>>,
    },
    /// Transform made of an arbitrary matrix.
    ///
    /// It should represent the final model matrix of the given drawable.  This is useful for
    /// situations where, for example, you build your own hierarchy system, where you calculate
    /// matrices of each hierarchy item and store a calculated world-space model matrix of an item.
    /// This lets you implement transform stacks, skeletal animations, etc.
    Matrix(mint::ColumnMatrix4<f32>),
}

fn transform_to_matrix(
    pos: mint::Point3<f32>,
    rotation: mint::Quaternion<f32>,
    scale: mint::Vector3<f32>,
    offset: Option<mint::Point3<f32>>,
    pivot: Option<mint::Point3<f32>>,
) -> Mat4 {
    let offset = if let Some(offset) = offset {
        offset
    } else {
        Vec3::ZERO.into()
    };
    let pivot = if let Some(piv) = pivot {
        Vec3::from(piv) + Vec3::from(offset)
    } else {
        Vec3::from(pos) + Vec3::from(offset)
    };
    Mat4::from_translation(pivot)
        * Mat4::from_scale(scale.into())
        * Mat4::from_quat(rotation.into())
        * Mat4::from_translation(-(pivot))
        * Mat4::from_translation(Vec3::from(pos))
}

impl Default for Transform3d {
    fn default() -> Self {
        Transform3d::Values {
            pos: mint::Point3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            rotation: glam::Quat::IDENTITY.into(),
            scale: mint::Vector3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
            offset: None,
            pivot: None,
        }
    }
}

impl Transform3d {
    /// Change the scale of the `Transform3d`
    ///
    /// # Panics
    ///
    /// Panics if `Transform3d` is of the `Matrix` variant.
    #[must_use]
    pub fn scale<V>(mut self, scale_: V) -> Self
    where
        V: Into<mint::Vector3<f32>>,
    {
        let p: mint::Vector3<f32> = scale_.into();
        if let Self::Values { ref mut scale, .. } = self {
            *scale = p;
        } else {
            panic!("Setting values of a Transform3d when it is a Matrix doesn't work!");
        }
        self
    }

    /// Change the position of the `Transform3d`
    ///
    /// # Panics
    ///
    /// Panics if `Transform3d` is of the `Matrix` variant.
    #[must_use]
    pub fn position<P>(mut self, position_: P) -> Self
    where
        P: Into<mint::Point3<f32>>,
    {
        let p: mint::Point3<f32> = position_.into();
        if let Self::Values { ref mut pos, .. } = self {
            *pos = p;
        } else {
            panic!("Setting values of a Transform3d when it is a Matrix doesn't work!");
        }
        self
    }

    /// Change the rotation of the `Transform3d`
    ///
    /// # Panics
    ///
    /// Panics if `Transform3d` is of the `Matrix` variant.
    #[must_use]
    pub fn rotation<R>(mut self, rotation_: R) -> Self
    where
        R: Into<mint::Quaternion<f32>>,
    {
        let p: mint::Quaternion<f32> = rotation_.into();
        if let Self::Values {
            ref mut rotation, ..
        } = self
        {
            *rotation = p;
        } else {
            panic!("Setting values of a Transform3d when it is a Matrix doesn't work!");
        }
        self
    }

    /// Move the position by given amount
    ///
    /// # Panics
    ///
    /// Panics if `Transform3d` is of the `Matrix` variant.
    #[must_use]
    pub fn translate<T>(mut self, translate_: T) -> Self
    where
        T: Into<mint::Vector3<f32>>,
    {
        let t: mint::Vector3<f32> = translate_.into();
        if let Self::Values { ref mut pos, .. } = self {
            *pos = (glam::Vec3::from(*pos) + glam::Vec3::from(t)).into();
        } else {
            panic!("Setting values of a Transform3d when it is a Matrix doesn't work!");
        }
        self
    }

    /// Sets the pivot point basically the origin of the mesh
    ///
    /// # Panics
    ///
    /// Panics if `Transform3d` is of the `Matrix` variant.
    #[must_use]
    pub fn pivot<P>(mut self, pivot_: P) -> Self
    where
        P: Into<mint::Point3<f32>>,
    {
        let p: mint::Point3<f32> = pivot_.into();
        if let Self::Values { ref mut pivot, .. } = self {
            *pivot = Some(p)
        } else {
            panic!("Setting values of a Transform3d when it is a Matrix doesn't work!");
        }
        self
    }

    /// Change the offset of the `DrawParam3d`
    ///
    /// # Panics
    ///
    /// Panics if `Transform3d` is of the `Matrix` variant.
    #[must_use]
    pub fn offset<O>(mut self, offset_: O) -> Self
    where
        O: Into<mint::Point3<f32>>,
    {
        let o: mint::Point3<f32> = offset_.into();
        if let Self::Values { ref mut offset, .. } = self {
            *offset = Some(o);
        } else {
            panic!("Setting values of a Transform3d when it is a Matrix doesn't work!");
        }
        self
    }

    /// Crunches the transform down to a single matrix, if it's not one already.
    #[must_use]
    pub fn to_matrix(self) -> Self {
        Transform3d::Matrix(self.to_bare_matrix())
    }

    /// Same as `to_matrix()` but just returns a bare `mint` matrix.
    #[must_use]
    pub fn to_bare_matrix(self) -> mint::ColumnMatrix4<f32> {
        match self {
            Transform3d::Matrix(m) => m,
            Transform3d::Values {
                pos,
                rotation,
                scale,
                offset,
                pivot,
            } => transform_to_matrix(pos, rotation, scale, offset, pivot).into(),
        }
    }

    /// Same as `to_matrix()` but just returns a bare `glam` matrix.
    #[must_use]
    pub(crate) fn to_bare_matrix_glam(self) -> Mat4 {
        match self {
            Transform3d::Matrix(m) => m.into(),
            Transform3d::Values {
                pos,
                rotation,
                scale,
                offset,
                pivot,
            } => transform_to_matrix(pos, rotation, scale, offset, pivot),
        }
    }
}

/// All types that can be drawn onto a canvas3d implement the `Drawable3d` trait.
pub trait Drawable3d {
    /// Draws the drawable3d onto the canvas3d.
    fn draw(&self, canvas: &mut Canvas3d, param: impl Into<DrawParam3d>);
}

#[derive(Debug, Copy, Clone, crevice::std140::AsStd140, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
pub(crate) struct DrawUniforms3d {
    pub color: Vec4,
    pub model_transform: Mat4,
    pub camera_transform: Mat4,
}

impl DrawUniforms3d {
    pub fn from_param(param: &DrawParam3d) -> Self {
        let param = match param.transform {
            Transform3d::Values { .. } => *param,
            Transform3d::Matrix(m) => param.transform(Transform3d::Matrix(m)),
        };

        let color = LinearColor::from(param.color);

        DrawUniforms3d {
            color: Vec4::from_array(color.into()),
            model_transform: param.transform.to_bare_matrix_glam(),
            camera_transform: Mat4::IDENTITY,
        }
    }

    pub fn projection(mut self, projection: impl Into<mint::ColumnMatrix4<f32>>) -> Self {
        self.camera_transform = projection.into().into();
        self
    }
}
