//! A more sophisticated example of how to use shaders
//! and canvas's to do 2D GPU shadows.

#[macro_use]
extern crate gfx;
extern crate ggez;
#[macro_use]
extern crate log;

use ggez::*;
use ggez::event::*;
use ggez::graphics::*;
use std::env;
use std::path;

gfx_defines!{
    /// Constants used by the shaders to calculate stuff
    constant Light {
        light_color: [f32; 4] = "u_LightColor",
        shadow_color: [f32; 4] = "u_ShadowColor",
        pos: [f32; 2] = "u_Pos",
        screen_size: [f32; 2] = "u_ScreenSize",
        glow: f32 = "u_Glow",
        strength: f32 = "u_Strength",
    }
}

/// Shader source for calculating a 1D shadow map that encodes half distances
/// in the red channel. The idea is that we scan X rays (X is the horizontal
/// size of the output) and calculate the distance to the nearest pixel at that
/// angle that has transparency above a threshold. The distance gets halved
/// and encoded in the red channel (it is halved because if the distance can be
/// greater than 1.0 - think bottom left to top right corner, that sqrt(1) and
/// will not get properly encoded).
const OCCLUSIONS_SHADER_SOURCE: &[u8] = b"#version 150 core

uniform sampler2D t_Texture;
in vec2 v_Uv;
out vec4 Target0;

layout (std140) uniform Light {
    vec4 u_LightColor;
    vec4 u_ShadowColor;
    vec2 u_Pos;
    vec2 u_ScreenSize;
    float u_Glow;
    float u_Strength;
};

void main() {
    float dist = 1.0;
    float theta = radians(v_Uv.x * 360.0);
    vec2 dir = vec2(cos(theta), sin(theta));
    for(int i = 0; i < 1024; i++) {
        float fi = i;
        float r = fi / 1024.0;
        vec2 rel = r * dir;
        vec2 p = clamp(u_Pos+rel, 0.0, 1.0);
        if (texture(t_Texture, p).a > 0.8) {
            dist = distance(u_Pos, p) * 0.5;
            break;
        }
    }

    float others = dist == 1.0 ? 0.0 : dist;
    Target0 = vec4(dist, others, others, 1.0);
}
";

const VERTEX_SHADER_SOURCE: &[u8] = include_bytes!("../resources/basic_150.glslv");

/// Shader for drawing shadows based on a 1D shadow map. It takes current
/// fragment coordinates and converts them to polar coordinates centered
/// around the light source, using the angle to sample from the 1D shadow map.
/// If the distance from the light source is greater than the distance of the
/// closest reported shadow, then the output is the shadow color, else it calculates some
/// shadow based on the distance from light source based on strength and glow
/// uniform parameters.
const SHADOWS_SHADER_SOURCE: &[u8] = b"#version 150 core

uniform sampler2D t_Texture;
in vec2 v_Uv;
out vec4 Target0;

layout (std140) uniform Light {
    vec4 u_LightColor;
    vec4 u_ShadowColor;
    vec2 u_Pos;
    vec2 u_ScreenSize;
    float u_Glow;
    float u_Strength;
};

void main() {
    vec2 coord = gl_FragCoord.xy / u_ScreenSize;
    vec2 rel = coord - u_Pos;
    float theta = atan(rel.y, rel.x);
    float ox = degrees(theta) / 360.0;
    if (ox < 0) {
        ox += 1.0;
    }
    float r = length(rel);
    float occl = texture(t_Texture, vec2(ox, 0.5)).r * 2.0;

    float intensity = 1.0;
    if (r < occl) {
        vec2 g = u_ScreenSize / u_ScreenSize.y;
        float p = u_Strength + u_Glow;
        float d = distance(g * coord, g * u_Pos);
        intensity = 1.0 - clamp(p/(d*d), 0.0, 1.0);
    }

    Target0 = mix(vec4(1.0, 1.0, 1.0, 1.0), vec4(u_ShadowColor.rgb, 1.0), intensity);
}
";

/// Shader for drawing lights based on a 1D shadow map. It takes current
/// fragment coordinates and converts them to polar coordinates centered
/// around the light source, using the angle to sample from the 1D shadow map.
/// If the distance from the light source is greater than the distance of the
/// closest reported shadow, then the output is black, else it calculates some
/// light based on the distance from light source based on strength and glow
/// uniform parameters. It is meant to be used additively for drawing multiple
/// lights.
const LIGHTS_SHADER_SOURCE: &[u8] = b"#version 150 core

uniform sampler2D t_Texture;
in vec2 v_Uv;
out vec4 Target0;

layout (std140) uniform Light {
    vec4 u_LightColor;
    vec4 u_ShadowColor;
    vec2 u_Pos;
    vec2 u_ScreenSize;
    float u_Glow;
    float u_Strength;
};

void main() {
    vec2 coord = gl_FragCoord.xy / u_ScreenSize;
    vec2 rel = coord - u_Pos;
    float theta = atan(rel.y, rel.x);
    float ox = degrees(theta) / 360.0;
    if (ox < 0) {
        ox += 1.0;
    }
    float r = length(rel);
    float occl = texture(t_Texture, vec2(ox, 0.5)).r * 2.0;

    float intensity = 0.0;
    if (r < occl) {
        vec2 g = u_ScreenSize / u_ScreenSize.y;
        float p = u_Strength + u_Glow;
        float d = distance(g * coord, g * u_Pos);
        intensity = clamp(p/(d*d), 0.0, 0.6);
    }

    Target0 = mix(vec4(0.0, 0.0, 0.0, 1.0), vec4(u_LightColor.rgb, 1.0), intensity);
}
";

struct MainState {
    background: Image,
    tile: Image,
    text: Text,
    torch: Light,
    static_light: Light,
    foreground: Canvas,
    occlusions: Canvas,
    shadows: Canvas,
    lights: Canvas,
    occlusions_shader: Shader<Light>,
    shadows_shader: Shader<Light>,
    lights_shader: Shader<Light>,
}

/// The color cast things take when not illuminated
const AMBIENT_COLOR: [f32; 4] = [0.25, 0.22, 0.34, 1.0];
/// The default color for the static light
const STATIC_LIGHT_COLOR: [f32; 4] = [0.37, 0.69, 0.75, 1.0];
/// The default color for the mouse-controlled torch
const TORCH_COLOR: [f32; 4] = [0.80, 0.73, 0.44, 1.0];
/// The number of rays to cast to. Increasing this number will result in better
/// quality shadows. If you increase too much you might hit some GPU shader
/// hardware limits.
const LIGHT_RAY_COUNT: u32 = 1440;
/// The strength of the light - how far it shines
const LIGHT_STRENGTH: f32 = 0.0035;
/// The factor at which the light glows - just for fun
const LIGHT_GLOW_FACTOR: f32 = 0.0001;
/// The rate at which the glow effect oscillates
const LIGHT_GLOW_RATE: f32 = 50.0;

impl MainState {
    fn new(ctx: &mut Context) -> GameResult<MainState> {
        let background = Image::new(ctx, "/bg_top.png")?;
        let tile = Image::new(ctx, "/tile.png")?;
        let text = {
            let font = Font::new(ctx, "/DejaVuSerif.ttf", 48)?;
            Text::new(ctx, "SHADOWS...", &font)?
        };
        let screen_size = {
            let size = graphics::get_drawable_size(ctx);
            [size.0 as f32, size.1 as f32]
        };
        let torch = Light {
            pos: [0.0, 0.0],
            light_color: TORCH_COLOR,
            shadow_color: AMBIENT_COLOR,
            screen_size,
            glow: 0.0,
            strength: LIGHT_STRENGTH,
        };
        let (w, h) = graphics::get_size(ctx);
        let (x, y) = (100.0 / w as f32, 1.0 - 75.0 / h as f32);
        let static_light = Light {
            pos: [x, y],
            light_color: STATIC_LIGHT_COLOR,
            shadow_color: AMBIENT_COLOR,
            screen_size,
            glow: 0.0,
            strength: LIGHT_STRENGTH,
        };
        let foreground = Canvas::with_window_size(ctx)?;
        let occlusions = Canvas::new(ctx, LIGHT_RAY_COUNT, 1, conf::NumSamples::One)?;
        let mut shadows = Canvas::with_window_size(ctx)?;
        // The shadow map will be drawn on top using the multiply blend mode
        shadows.set_blend_mode(Some(BlendMode::Multiply));
        let mut lights = Canvas::with_window_size(ctx)?;
        // The light map will be drawn on top using the add blend mode
        lights.set_blend_mode(Some(BlendMode::Add));
        let occlusions_shader = Shader::from_u8(
            ctx,
            VERTEX_SHADER_SOURCE,
            OCCLUSIONS_SHADER_SOURCE,
            torch,
            "Light",
            None,
        ).unwrap();
        let shadows_shader = Shader::from_u8(
            ctx,
            VERTEX_SHADER_SOURCE,
            SHADOWS_SHADER_SOURCE,
            torch,
            "Light",
            None,
        ).unwrap();
        let lights_shader = Shader::from_u8(
            ctx,
            VERTEX_SHADER_SOURCE,
            LIGHTS_SHADER_SOURCE,
            torch,
            "Light",
            Some(&[BlendMode::Add]),
        ).unwrap();

        Ok(MainState {
            background,
            tile,
            text,
            torch,
            static_light,
            foreground,
            occlusions,
            shadows,
            lights,
            occlusions_shader,
            shadows_shader,
            lights_shader,
        })
    }
    fn render_light(
        &mut self,
        ctx: &mut Context,
        light: Light,
        origin: DrawParam,
        canvas_origin: DrawParam,
    ) -> GameResult<()> {
        let size = graphics::get_size(ctx);
        // Now we want to run the occlusions shader to calculate our 1D shadow
        // distances into the `occlusions` canvas.
        graphics::set_canvas(ctx, Some(&self.occlusions));
        {
            let _shader_lock = graphics::use_shader(ctx, &self.occlusions_shader);

            self.occlusions_shader.send(ctx, light)?;
            graphics::draw_ex(ctx, &self.foreground, canvas_origin)?;
        }

        // Now we render our shadow map and light map into their respective
        // canvases based on the occlusion map. These will then be drawn onto
        // the final render target using appropriate blending modes.
        graphics::set_canvas(ctx, Some(&self.shadows));
        {
            let _shader_lock = graphics::use_shader(ctx, &self.shadows_shader);

            let param = DrawParam {
                scale: Point2::new((size.0 as f32) / (LIGHT_RAY_COUNT as f32), size.1 as f32),
                ..origin
            };
            self.shadows_shader.send(ctx, light)?;
            graphics::draw_ex(ctx, &self.occlusions, param)?;
        }
        graphics::set_canvas(ctx, Some(&self.lights));
        {
            let _shader_lock = graphics::use_shader(ctx, &self.lights_shader);

            let param = DrawParam {
                scale: Point2::new((size.0 as f32) / (LIGHT_RAY_COUNT as f32), size.1 as f32),
                ..origin
            };
            self.lights_shader.send(ctx, light)?;
            graphics::draw_ex(ctx, &self.occlusions, param)?;
        }
        Ok(())
    }
}

impl event::EventHandler for MainState {
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        if timer::get_ticks(ctx) % 100 == 0 {
            info!("Average FPS: {}", timer::get_fps(ctx));
        }

        self.torch.glow =
            LIGHT_GLOW_FACTOR * ((timer::get_ticks(ctx) as f32) / LIGHT_GLOW_RATE).cos();
        self.static_light.glow =
            LIGHT_GLOW_FACTOR * ((timer::get_ticks(ctx) as f32) / LIGHT_GLOW_RATE * 0.75).sin();
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        let size = graphics::get_size(ctx);
        let origin = DrawParam {
            dest: Point2::new(0.0, 0.0),
            ..Default::default()
        };
        // for re-rendering canvases, we need to take the DPI into account
        let dpiscale = {
            let dsize = graphics::get_drawable_size(ctx);
            Point2::new(
                size.0 as f32 / dsize.0 as f32,
                size.1 as f32 / dsize.1 as f32,
            )
        };
        let canvas_origin = DrawParam {
            scale: dpiscale,
            ..origin
        };

        // First thing we want to do it to render all the foreground items (that
        // will have shadows) onto their own Canvas (off-screen render). We will
        // use this canvas to:
        //  - run the occlusions shader to determine where the shadows are
        //  - render to screen once all the shadows are calculated and rendered
        graphics::set_canvas(ctx, Some(&self.foreground));
        graphics::set_background_color(ctx, [0.0; 4].into());
        graphics::clear(ctx);
        graphics::draw_ex(
            ctx,
            &self.tile,
            DrawParam {
                dest: Point2::new(598.0, 124.0),
                ..Default::default()
            },
        )?;
        graphics::draw_ex(
            ctx,
            &self.tile,
            DrawParam {
                dest: Point2::new(92.0, 350.0),
                ..Default::default()
            },
        )?;
        graphics::draw_ex(
            ctx,
            &self.tile,
            DrawParam {
                dest: Point2::new(442.0, 468.0),
                rotation: 0.5,
                ..Default::default()
            },
        )?;
        graphics::draw(ctx, &self.text, Point2::new(50.0, 200.0), 0.0)?;

        // First we draw our light and shadow maps
        let torch = self.torch;
        let light = self.static_light;
        graphics::set_canvas(ctx, Some(&self.lights));
        graphics::clear(ctx);
        graphics::set_canvas(ctx, Some(&self.shadows));
        graphics::clear(ctx);
        self.render_light(ctx, torch, origin, canvas_origin)?;
        self.render_light(ctx, light, origin, canvas_origin)?;

        // Now lets finally render to screen starting with out background, then
        // the shadows and lights overtop and finally our foreground.
        graphics::set_canvas(ctx, None);
        graphics::set_color(ctx, graphics::WHITE)?;
        graphics::draw_ex(ctx, &self.background, origin)?;
        graphics::draw_ex(ctx, &self.shadows, origin)?;
        graphics::draw_ex(ctx, &self.lights, origin)?;
        // We switch the color to the shadow color before drawing the foreground objects
        // this has the same effect as applying this color in a multiply blend mode with
        // full opacity. We also reset the blend mode back to the default Alpha blend mode.
        graphics::set_color(ctx, AMBIENT_COLOR.into())?;
        graphics::draw_ex(ctx, &self.foreground, origin)?;

        // Uncomment following two lines to visualize the 1D occlusions canvas,
        // red pixels represent angles at which no shadows were found, and then
        // the greyscale pixels are the half distances of the nearest shadows to
        // the mouse position (equally encoded in all color channels).
        // graphics::set_color(ctx, [1.0; 4].into())?;
        // graphics::draw_ex(ctx, &self.occlusions, origin)?;

        graphics::present(ctx);
        Ok(())
    }

    fn mouse_motion_event(
        &mut self,
        ctx: &mut Context,
        _state: MouseState,
        x: i32,
        y: i32,
        _xrel: i32,
        _yrel: i32,
    ) {
        let (w, h) = graphics::get_size(ctx);
        let (x, y) = (x as f32 / w as f32, 1.0 - y as f32 / h as f32);
        self.torch.pos = [x, y];
    }
}

pub fn main() {
    let c = conf::Conf::new();
    let ctx = &mut Context::load_from_conf("shadows", "ggez", c).unwrap();

    // We add the CARGO_MANIFEST_DIR/resources do the filesystems paths so
    // we we look in the cargo project for files.
    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        let mut path = path::PathBuf::from(manifest_dir);
        path.push("resources");
        ctx.filesystem.mount(&path, true);
    }

    let state = &mut MainState::new(ctx).unwrap();
    event::run(ctx, state).unwrap();
}
