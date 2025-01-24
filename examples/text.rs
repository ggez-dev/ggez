//! This example demonstrates how to use `Text` to draw TrueType font texts efficiently.

use ggez::conf::{WindowMode, WindowSetup};
use ggez::glam::Vec2;
use ggez::graphics::{self, Color, PxScale, Text, TextAlign, TextFragment};
use ggez::timer;
use ggez::{event, graphics::TextLayout};
use ggez::{Context, ContextBuilder, GameResult};
use std::collections::BTreeMap;
use std::env;
use std::path;

/// Creates a random RGB color.
fn random_color(rng: &mut oorandom::Rand32) -> Color {
    Color::new(rng.rand_float(), rng.rand_float(), rng.rand_float(), 1.0)
}

struct App {
    // Doesn't have to be a `BTreeMap`; it's handy if you care about specific elements,
    // want to retrieve them by trivial handles, and have to preserve ordering.
    texts: BTreeMap<&'static str, Text>,
    rng: oorandom::Rand32,
}

impl App {
    #[allow(clippy::needless_update)]
    fn new(ctx: &mut Context) -> GameResult<App> {
        let mut texts = BTreeMap::new();

        // We just use a fixed RNG seed for simplicity.
        let mut rng = oorandom::Rand32::new(314159);

        // This is the simplest way to create a drawable text;
        // the color, font, and scale will be default: white, LiberationMono-Regular, 16px unform.
        // Note that you don't even have to load a font: LiberationMono-Regular is baked into `ggez` itself.
        let text = Text::new("Hello, World!");
        // Store the text in `App`s map, for drawing in main loop.
        texts.insert("0_hello", text);

        // This is what actually happens in `Text::new()`: the `&str` gets
        // automatically converted into a `TextFragment`.
        let mut text = Text::new(TextFragment {
            // `TextFragment` stores a string, and optional parameters which will override those
            // of `Text` itself. This allows inlining differently formatted lines, words,
            // or even individual letters, into the same block of text.
            text: "Small red fragment".to_string(),
            color: Some(Color::new(1.0, 0.0, 0.0, 1.0)),
            // The font name refers to a loaded TTF, stored inside the `GraphicsContext`.
            // A default font always exists and maps to LiberationMono-Regular.
            font: Some("LiberationMono-Regular".into()),
            scale: Some(PxScale::from(10.0)),
            // This doesn't do anything at this point; can be used to omit fields in declarations.
            ..Default::default()
        });

        // More fragments can be appended at any time.
        text.add(" default fragment, should be long enough to showcase everything")
            // `add()` can be chained, along with most `Text` methods.
            .add(TextFragment::new(" magenta fragment").color(Color::new(1.0, 0.0, 1.0, 1.0)))
            .add(" another default fragment, to really drive the point home");

        // This loads a new TrueType font into the context named "Fancy font".
        ctx.gfx.add_font(
            "Fancy font",
            graphics::FontData::from_path(ctx, "/Tangerine_Regular.ttf")?,
        );

        // `Font` is really only an integer handle, and can be copied around.
        text.add(
            TextFragment::new(" fancy fragment")
                .font("Fancy font")
                .scale(PxScale::from(25.0)),
        )
        .add(" and a default one, for symmetry");
        // Store a copy of the built text, retain original for further modifications.
        texts.insert("1_demo_text_1", text.clone());

        // Text can be wrapped by setting it's bounds, in screen coordinates;
        // vertical bound will cut off the extra off the bottom.
        // Alignment within the bounds can be set by `Align` enum.
        text.set_bounds(Vec2::new(400.0, f32::INFINITY))
            .set_layout(TextLayout {
                h_align: TextAlign::Begin,
                v_align: TextAlign::Begin,
            });
        texts.insert("1_demo_text_2", text.clone());

        text.set_bounds(Vec2::new(500.0, f32::INFINITY))
            .set_layout(TextLayout {
                h_align: TextAlign::End,
                v_align: TextAlign::Begin,
            });
        texts.insert("1_demo_text_3", text.clone());

        // This can be used to set the font and scale unformatted fragments will use.
        // Color is specified when drawing, via `DrawParam`.
        // Side note: TrueType fonts aren't very consistent between themselves in terms
        // of apparent scale - this font with default scale will appear too small.
        text.set_font("Fancy font")
            .set_scale(16.0)
            .set_bounds(Vec2::new(300.0, f32::INFINITY))
            .set_layout(TextLayout {
                h_align: TextAlign::Middle,
                v_align: TextAlign::Begin,
            });
        texts.insert("1_demo_text_4", text);

        // These methods can be combined to easily create a variety of simple effects.
        let chroma_string = "Not quite a rainbow.";
        // `default()` exists pretty much specifically for this usecase.
        let mut chroma_text = Text::default();
        for ch in chroma_string.chars() {
            chroma_text.add(TextFragment::new(ch).color(random_color(&mut rng)));
        }
        texts.insert("2_rainbow", chroma_text);

        let wonky_string = "So, so wonky.";
        let mut wonky_text = Text::default();
        for ch in wonky_string.chars() {
            wonky_text
                .add(TextFragment::new(ch).scale(PxScale::from(10.0 + 24.0 * rng.rand_float())));
        }
        texts.insert("3_wonky", wonky_text);

        Ok(App { texts, rng })
    }
}

impl event::EventHandler for App {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        const DESIRED_FPS: u32 = 60;
        while ctx.time.check_update_time(DESIRED_FPS) {}
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        let mut canvas = graphics::Canvas::from_frame(ctx, Color::from([0.1, 0.2, 0.3, 1.0]));

        let fps = ctx.time.fps();
        let fps_display = Text::new(format!("FPS: {fps}"));
        // When drawing through these calls, `DrawParam` will work as they are documented.
        canvas.draw(
            &fps_display,
            graphics::DrawParam::from([200.0, 0.0]).color(Color::WHITE),
        );

        let mut height = 0.0;
        for (key, text) in &self.texts {
            let x = match *key {
                // (bounds position) + 20
                "1_demo_text_3" => 500.0 + 20.0,
                "1_demo_text_4" => (300.0 / 2.0) + 20.0,
                _ => 20.0,
            };
            canvas.draw(text, Vec2::new(x, 20.0 + height));
            //height += 20.0 + text.height(ctx) as f32;
            height += 20.0 + text.measure(ctx)?.y
        }

        // Individual fragments within the `Text` can be replaced;
        // this can be used for inlining animated sentences, words, etc.
        if let Some(text) = self.texts.get_mut("1_demo_text_3") {
            // `.fragments_mut()` returns a mutable slice of contained fragments.
            // Fragments are indexed in order of their addition, starting at 0 (of course).
            text.fragments_mut()[3].color = Some(random_color(&mut self.rng));
        }

        // Another animation example. Note, this is relatively inefficient as-is.
        let wobble_string = "WOBBLE";
        let mut wobble = Text::default();
        for ch in wobble_string.chars() {
            wobble.add(
                TextFragment::new(ch).scale(PxScale::from(10.0 + 6.0 * self.rng.rand_float())),
            );
        }
        let wobble_bounds = wobble.measure(ctx)?;
        canvas.draw(
            &wobble,
            graphics::DrawParam::new()
                .color((0.0, 1.0, 1.0, 1.0))
                .dest([500.0, 300.0])
                .rotation(-0.5),
        );
        let t = Text::new(format!(
            "width: {}\nheight: {}",
            wobble_bounds.x, wobble_bounds.y
        ));
        canvas.draw(&t, graphics::DrawParam::from([500.0, 320.0]).rotation(-0.5));

        canvas.finish(ctx)?;
        timer::yield_now();
        Ok(())
    }
}

pub fn main() -> GameResult {
    if cfg!(debug_assertions) && env::var("yes_i_really_want_debug_mode").is_err() {
        eprintln!(
            "Note: Release mode will improve performance greatly.\n    \
             e.g. use `cargo run --example text --release`"
        );
    }
    let resource_dir = if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        let mut path = path::PathBuf::from(manifest_dir);
        path.push("resources");
        path
    } else {
        path::PathBuf::from("./resources")
    };

    let (mut ctx, events_loop) = ContextBuilder::new("text_cached", "ggez")
        .window_setup(WindowSetup::default().title("Cached text example!"))
        .window_mode(
            WindowMode::default()
                .dimensions(640.0, 480.0)
                .resizable(true),
        )
        .add_resource_path(resource_dir)
        .build()?;

    let state = App::new(&mut ctx)?;
    event::run(ctx, events_loop, state)
}
