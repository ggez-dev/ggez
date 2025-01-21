use super::{
    context::GraphicsContext, Canvas, Color, Draw, DrawMode, DrawParam, Drawable, LinearColor,
    Rect, WgpuContext,
};
use crate::{context::Has, GameError, GameResult};
use lyon::{math::Point as LPoint, path::Polygon, tessellation as tess};
use wgpu::util::DeviceExt;

/// Vertex format uploaded to vertex buffers.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct Vertex {
    /// `vec2` position.
    pub position: [f32; 2],
    /// `vec2` UV/texture coordinates.
    pub uv: [f32; 2],
    /// `vec4` color.
    pub color: [f32; 4],
}

impl Vertex {
    pub(crate) const fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 3] = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 8,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 2,
            },
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }
}

/// Mesh data stored on the GPU as a vertex and index buffer. Cheap to clone.
#[derive(Debug, Clone)]
pub struct Mesh {
    pub(crate) verts: wgpu::Buffer,
    pub(crate) inds: wgpu::Buffer,
    pub(crate) vertex_count: usize,
    pub(crate) index_count: usize,
    pub(crate) bounds: Rect,
}

impl Mesh {
    /// Create a new mesh from [`MeshData`].
    pub fn from_data(gfx: &impl Has<GraphicsContext>, raw: MeshData) -> Self {
        let gfx = gfx.retrieve();
        Self::from_data_wgpu(&gfx.wgpu, raw)
    }

    pub(crate) fn from_data_wgpu(wgpu: &WgpuContext, raw: MeshData) -> Self {
        let [minx, miny, maxx, maxy] = raw.vertices.iter().fold(
            [f32::MAX, f32::MAX, f32::MIN, f32::MIN],
            |[minx, miny, maxx, maxy], vert| {
                let [x, y] = vert.position;
                [minx.min(x), miny.min(y), maxx.max(x), maxy.max(y)]
            },
        );

        Mesh {
            verts: Self::create_verts(wgpu, raw.vertices),
            inds: Self::create_inds(wgpu, raw.indices),
            vertex_count: raw.vertices.len(),
            index_count: raw.indices.len(),
            bounds: Rect {
                x: minx,
                y: miny,
                w: maxx - minx,
                h: maxy - miny,
            },
        }
    }

    /// Create a new mesh for a line of one or more connected segments.
    pub fn new_line(
        gfx: &impl Has<GraphicsContext>,
        points: &[impl Into<mint::Point2<f32>> + Clone],
        width: f32,
        color: Color,
    ) -> GameResult<Self> {
        Ok(Mesh::from_data(
            gfx,
            MeshBuilder::new()
                .polyline(DrawMode::stroke(width), points, color)?
                .build(),
        ))
    }

    /// Create a new mesh for a circle.
    pub fn new_circle(
        gfx: &impl Has<GraphicsContext>,
        mode: DrawMode,
        point: impl Into<mint::Point2<f32>>,
        radius: f32,
        tolerance: f32,
        color: Color,
    ) -> GameResult<Self> {
        Ok(Mesh::from_data(
            gfx,
            MeshBuilder::new()
                .circle(mode, point, radius, tolerance, color)?
                .build(),
        ))
    }

    /// Create a new mesh for an ellipse.
    pub fn new_ellipse(
        gfx: &impl Has<GraphicsContext>,
        mode: DrawMode,
        point: impl Into<mint::Point2<f32>>,
        radius1: f32,
        radius2: f32,
        tolerance: f32,
        color: Color,
    ) -> GameResult<Self> {
        Ok(Mesh::from_data(
            gfx,
            MeshBuilder::new()
                .ellipse(mode, point, radius1, radius2, tolerance, color)?
                .build(),
        ))
    }

    /// Create a new mesh for a series of connected lines.
    pub fn new_polyline(
        gfx: &impl Has<GraphicsContext>,
        mode: DrawMode,
        points: &[impl Into<mint::Point2<f32>> + Clone],
        color: Color,
    ) -> GameResult<Self> {
        Ok(Mesh::from_data(
            gfx,
            MeshBuilder::new().polyline(mode, points, color)?.build(),
        ))
    }

    /// Create a new mesh for closed polygon.
    /// The points given must be in clockwise order,
    /// otherwise at best the polygon will not draw.
    pub fn new_polygon(
        gfx: &impl Has<GraphicsContext>,
        mode: DrawMode,
        points: &[impl Into<mint::Point2<f32>> + Clone],
        color: Color,
    ) -> GameResult<Self> {
        Ok(Mesh::from_data(
            gfx,
            MeshBuilder::new().polygon(mode, points, color)?.build(),
        ))
    }

    /// Create a new mesh for a rectangle.
    pub fn new_rectangle(
        gfx: &impl Has<GraphicsContext>,
        mode: DrawMode,
        bounds: Rect,
        color: Color,
    ) -> GameResult<Self> {
        Ok(Mesh::from_data(
            gfx,
            MeshBuilder::new().rectangle(mode, bounds, color)?.build(),
        ))
    }

    /// Create a new mesh for a rounded rectangle.
    pub fn new_rounded_rectangle(
        gfx: &impl Has<GraphicsContext>,
        mode: DrawMode,
        bounds: Rect,
        radius: f32,
        color: Color,
    ) -> GameResult<Self> {
        Ok(Mesh::from_data(
            gfx,
            MeshBuilder::new()
                .rounded_rectangle(mode, bounds, radius, color)?
                .build(),
        ))
    }

    /// Create a new `Mesh` from a raw list of triangle points.
    pub fn from_triangles(
        gfx: &impl Has<GraphicsContext>,
        triangles: &[impl Into<mint::Point2<f32>> + Clone],
        color: Color,
    ) -> GameResult<Self> {
        Ok(Mesh::from_data(
            gfx,
            MeshBuilder::new().triangles(triangles, color)?.build(),
        ))
    }

    /// Returns the WGPU vertex and index buffers of this mesh, in that order.
    #[inline]
    pub fn wgpu(&self) -> (&wgpu::Buffer, &wgpu::Buffer) {
        (&self.verts, &self.inds)
    }

    /// Returns the number of vertices in this mesh.
    #[inline]
    pub fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    /// Returns the number of indices in this mesh.
    #[inline]
    pub fn index_count(&self) -> usize {
        self.index_count
    }

    #[allow(unsafe_code)]
    fn create_verts(wgpu: &WgpuContext, vertices: &[Vertex]) -> wgpu::Buffer {
        wgpu.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            })
    }

    #[allow(unsafe_code)]
    fn create_inds(wgpu: &WgpuContext, indices: &[u32]) -> wgpu::Buffer {
        wgpu.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            })
    }
}

impl Drawable for Mesh {
    fn draw(&self, canvas: &mut Canvas, param: impl Into<DrawParam>) {
        canvas.push_draw(
            Draw::Mesh {
                mesh: self.clone(),
                image: canvas.default_resources().image.clone(),
                scale: false,
            },
            param.into(),
        );
    }

    fn dimensions(&self, _gfx: &impl Has<GraphicsContext>) -> Rect {
        self.bounds
    }
}

/// A `Drawable` unit type that maps to a simple 1x1 quad mesh.
///
/// Draw like any other mesh;
/// ```rust
/// # use ggez::graphics::*;
/// # fn f(mut canvas: Canvas) {
/// // draw a 50x70 red rectangle at (200, 250)
/// canvas.draw(&Quad, DrawParam::default().color(Color::RED).scale([50., 70.]).dest([200., 250.]));
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Quad;

// draw quad
impl Drawable for Quad {
    fn draw(&self, canvas: &mut Canvas, param: impl Into<DrawParam>) {
        canvas.default_resources().mesh.clone().draw(canvas, param);
    }

    fn dimensions(&self, _gfx: &impl Has<GraphicsContext>) -> Rect {
        Rect::one()
    }
}

/// Borrowed mesh data.
#[derive(Debug, Clone)]
pub struct MeshData<'a> {
    /// List of vertices.
    pub vertices: &'a [Vertex],
    /// List of indices (indices into `vertices`).
    pub indices: &'a [u32],
}

/// Builder pattern for constructing meshes.
#[derive(Debug, Clone)]
pub struct MeshBuilder {
    buffer: tess::geometry_builder::VertexBuffers<Vertex, u32>,
}

impl Default for MeshBuilder {
    fn default() -> Self {
        Self {
            buffer: tess::VertexBuffers::new(),
        }
    }
}

impl MeshBuilder {
    /// Create a new [`MeshBuilder`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new mesh for a line of one or more connected segments.
    pub fn line<P>(&mut self, points: &[P], width: f32, color: Color) -> GameResult<&mut Self>
    where
        P: Into<mint::Point2<f32>> + Clone,
    {
        self.polyline(DrawMode::stroke(width), points, color)
    }

    /// Create a new mesh for a circle.
    ///
    /// For the meaning of the `tolerance` parameter, [see here](https://docs.rs/lyon_geom/0.11.0/lyon_geom/#flattening).
    pub fn circle<P>(
        &mut self,
        mode: DrawMode,
        point: P,
        radius: f32,
        tolerance: f32,
        color: Color,
    ) -> GameResult<&mut Self>
    where
        P: Into<mint::Point2<f32>>,
    {
        assert!(
            tolerance > 0.0,
            "Tolerances <= 0 are invalid, see https://github.com/ggez/ggez/issues/892"
        );
        {
            let point = point.into();
            let buffers = &mut self.buffer;
            let vb = VertexBuilder {
                color: LinearColor::from(color),
            };
            match mode {
                DrawMode::Fill(fill_options) => {
                    let mut tessellator = tess::FillTessellator::new();
                    tessellator.tessellate_circle(
                        tess::math::point(point.x, point.y),
                        radius,
                        &fill_options.with_tolerance(tolerance),
                        &mut tess::BuffersBuilder::new(buffers, vb),
                    )?;
                }
                DrawMode::Stroke(options) => {
                    let mut tessellator = tess::StrokeTessellator::new();
                    tessellator.tessellate_circle(
                        tess::math::point(point.x, point.y),
                        radius,
                        &options.with_tolerance(tolerance),
                        &mut tess::BuffersBuilder::new(buffers, vb),
                    )?;
                }
            };
        }
        Ok(self)
    }

    /// Create a new mesh for an ellipse.
    ///
    /// For the meaning of the `tolerance` parameter, [see here](https://docs.rs/lyon_geom/0.11.0/lyon_geom/#flattening).
    pub fn ellipse<P>(
        &mut self,
        mode: DrawMode,
        point: P,
        radius1: f32,
        radius2: f32,
        tolerance: f32,
        color: Color,
    ) -> GameResult<&mut Self>
    where
        P: Into<mint::Point2<f32>>,
    {
        assert!(
            tolerance > 0.0,
            "Tolerances <= 0 are invalid, see https://github.com/ggez/ggez/issues/892"
        );
        {
            let buffers = &mut self.buffer;
            let point = point.into();
            let vb = VertexBuilder {
                color: LinearColor::from(color),
            };
            match mode {
                DrawMode::Fill(fill_options) => {
                    let builder = &mut tess::BuffersBuilder::new(buffers, vb);
                    let mut tessellator = tess::FillTessellator::new();
                    tessellator.tessellate_ellipse(
                        tess::math::point(point.x, point.y),
                        tess::math::vector(radius1, radius2),
                        tess::math::Angle { radians: 0.0 },
                        tess::path::Winding::Positive,
                        &fill_options.with_tolerance(tolerance),
                        builder,
                    )?;
                }
                DrawMode::Stroke(options) => {
                    let builder = &mut tess::BuffersBuilder::new(buffers, vb);
                    let mut tessellator = tess::StrokeTessellator::new();
                    tessellator.tessellate_ellipse(
                        tess::math::point(point.x, point.y),
                        tess::math::vector(radius1, radius2),
                        tess::math::Angle { radians: 0.0 },
                        tess::path::Winding::Positive,
                        &options.with_tolerance(tolerance),
                        builder,
                    )?;
                }
            };
        }
        Ok(self)
    }

    /// Create a new mesh for a series of connected lines.
    pub fn polyline<P>(
        &mut self,
        mode: DrawMode,
        points: &[P],
        color: Color,
    ) -> GameResult<&mut Self>
    where
        P: Into<mint::Point2<f32>> + Clone,
    {
        if points.len() < 2 {
            return Err(GameError::LyonError(
                "MeshBuilder::polyline() got a list of < 2 points".to_string(),
            ));
        }

        self.polyline_inner(mode, points, false, color)
    }

    /// Create a new mesh for a closed polygon.
    /// The points given must be in clockwise order,
    /// otherwise at best the polygon will not draw.
    pub fn polygon<P>(
        &mut self,
        mode: DrawMode,
        points: &[P],
        color: Color,
    ) -> GameResult<&mut Self>
    where
        P: Into<mint::Point2<f32>> + Clone,
    {
        if points.len() < 3 {
            return Err(GameError::LyonError(
                "MeshBuilder::polygon() got a list of < 3 points".to_string(),
            ));
        }

        self.polyline_inner(mode, points, true, color)
    }

    fn polyline_inner<P>(
        &mut self,
        mode: DrawMode,
        points: &[P],
        is_closed: bool,
        color: Color,
    ) -> GameResult<&mut Self>
    where
        P: Into<mint::Point2<f32>> + Clone,
    {
        let vb = VertexBuilder {
            color: LinearColor::from(color),
        };
        self.polyline_with_vertex_builder(mode, points, is_closed, vb)
    }

    /// Create a new mesh for a given polyline using a custom vertex builder.
    /// The points given must be in clockwise order.
    pub fn polyline_with_vertex_builder<P, V>(
        &mut self,
        mode: DrawMode,
        points: &[P],
        is_closed: bool,
        vb: V,
    ) -> GameResult<&mut Self>
    where
        P: Into<mint::Point2<f32>> + Clone,
        V: tess::StrokeVertexConstructor<Vertex> + tess::FillVertexConstructor<Vertex>,
    {
        {
            assert!(points.len() > 1);
            let buffers = &mut self.buffer;
            let points: Vec<LPoint> = points
                .iter()
                .cloned()
                .map(|p| {
                    let mint_point: mint::Point2<f32> = p.into();
                    tess::math::point(mint_point.x, mint_point.y)
                })
                .collect();
            let polygon = Polygon {
                points: &points,
                closed: is_closed,
            };
            match mode {
                DrawMode::Fill(options) => {
                    let builder = &mut tess::BuffersBuilder::new(buffers, vb);
                    let tessellator = &mut tess::FillTessellator::new();
                    tessellator.tessellate_polygon(polygon, &options, builder)?;
                }
                DrawMode::Stroke(options) => {
                    let builder = &mut tess::BuffersBuilder::new(buffers, vb);
                    let tessellator = &mut tess::StrokeTessellator::new();
                    tessellator.tessellate_polygon(polygon, &options, builder)?;
                }
            };
        }
        Ok(self)
    }

    /// Create a new mesh for a rectangle.
    pub fn rectangle(
        &mut self,
        mode: DrawMode,
        bounds: Rect,
        color: Color,
    ) -> GameResult<&mut Self> {
        {
            let buffers = &mut self.buffer;
            let rect = tess::math::Box2D::from_origin_and_size(
                tess::math::point(bounds.x, bounds.y),
                tess::math::size(bounds.w, bounds.h),
            );
            let vb = VertexBuilder {
                color: LinearColor::from(color),
            };
            match mode {
                DrawMode::Fill(fill_options) => {
                    let builder = &mut tess::BuffersBuilder::new(buffers, vb);
                    let mut tessellator = tess::FillTessellator::new();
                    tessellator.tessellate_rectangle(&rect, &fill_options, builder)?;
                }
                DrawMode::Stroke(options) => {
                    let builder = &mut tess::BuffersBuilder::new(buffers, vb);
                    let mut tessellator = tess::StrokeTessellator::new();
                    tessellator.tessellate_rectangle(&rect, &options, builder)?;
                }
            };
        }
        Ok(self)
    }

    /// Create a new mesh for a rounded rectangle.
    pub fn rounded_rectangle(
        &mut self,
        mode: DrawMode,
        bounds: Rect,
        radius: f32,
        color: Color,
    ) -> GameResult<&mut Self> {
        {
            let buffers = &mut self.buffer;
            let rect = tess::math::Box2D::from_origin_and_size(
                tess::math::point(bounds.x, bounds.y),
                tess::math::size(bounds.w, bounds.h),
            );
            let radii = tess::path::builder::BorderRadii::new(radius);
            let vb = VertexBuilder {
                color: LinearColor::from(color),
            };
            let mut path_builder = tess::path::Path::builder();
            path_builder.add_rounded_rectangle(&rect, &radii, tess::path::Winding::Positive);
            let path = path_builder.build();

            match mode {
                DrawMode::Fill(fill_options) => {
                    let builder = &mut tess::BuffersBuilder::new(buffers, vb);
                    let mut tessellator = tess::FillTessellator::new();
                    tessellator.tessellate_path(&path, &fill_options, builder)?;
                }
                DrawMode::Stroke(options) => {
                    let builder = &mut tess::BuffersBuilder::new(buffers, vb);
                    let mut tessellator = tess::StrokeTessellator::new();
                    tessellator.tessellate_path(&path, &options, builder)?;
                }
            };
        }
        Ok(self)
    }

    /// Create a new [`Mesh`](struct.Mesh.html) from a raw list of triangles.
    /// The length of the list must be a multiple of 3.
    ///
    /// Currently does not support UV's or indices.
    pub fn triangles<P>(&mut self, triangles: &[P], color: Color) -> GameResult<&mut Self>
    where
        P: Into<mint::Point2<f32>> + Clone,
    {
        {
            if (triangles.len() % 3) != 0 {
                return Err(GameError::LyonError(String::from(
                    "Called Mesh::triangles() with points that have a length not a multiple of 3.",
                )));
            }
            let tris = triangles
                .iter()
                .cloned()
                .map(|p| {
                    // Gotta turn ggez Point2's into lyon points
                    let mint_point = p.into();
                    lyon::math::point(mint_point.x, mint_point.y)
                })
                // Removing this collect might be nice, but is not easy.
                // We can chunk a slice, but can't chunk an arbitrary
                // iterator.
                // Using the itertools crate doesn't really make anything
                // nicer, so we'll just live with it.
                .collect::<Vec<_>>();
            let tris = tris.chunks(3);
            let vb = VertexBuilder {
                color: LinearColor::from(color),
            };
            for tri in tris {
                // Ideally this assert makes bounds-checks only happen once.
                assert!(tri.len() == 3);
                let first_index: u32 = self.buffer.vertices.len().try_into().unwrap(); // This shouldn't fail as the amount of vertices shouldn't ever be high enough to not fit into a u32
                self.buffer.vertices.push(vb.new_vertex(tri[0]));
                self.buffer.vertices.push(vb.new_vertex(tri[1]));
                self.buffer.vertices.push(vb.new_vertex(tri[2]));
                self.buffer.indices.push(first_index);
                self.buffer.indices.push(first_index + 1);
                self.buffer.indices.push(first_index + 2);
            }
        }
        Ok(self)
    }

    /// Takes the accumulated geometry and return it as [`MeshData`].
    pub fn build(&self) -> MeshData {
        MeshData {
            vertices: &self.buffer.vertices,
            indices: &self.buffer.indices,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
struct VertexBuilder {
    color: LinearColor,
}

impl VertexBuilder {
    fn new_vertex(self, position: LPoint) -> Vertex {
        Vertex {
            position: [position.x, position.y],
            uv: [position.x, position.y],
            color: self.color.into(),
        }
    }
}

impl tess::StrokeVertexConstructor<Vertex> for VertexBuilder {
    fn new_vertex(&mut self, vertex: tess::StrokeVertex) -> Vertex {
        let position = vertex.position();
        Vertex {
            position: [position.x, position.y],
            uv: [0.0, 0.0],
            color: self.color.into(),
        }
    }
}

impl tess::FillVertexConstructor<Vertex> for VertexBuilder {
    fn new_vertex(&mut self, vertex: tess::FillVertex) -> Vertex {
        let position = vertex.position();
        Vertex {
            position: [position.x, position.y],
            uv: [0.0, 0.0],
            color: self.color.into(),
        }
    }
}
