//! Example that just prints out all the input events.

extern crate ggez;
#[macro_use]
extern crate log;

use ggez::*;
use ggez::event::*;
use ggez::graphics::{DrawMode, Point2};
use std::env;
use std::path;

struct MainState {
    pos_x: i32,
    pos_y: i32,
    mouse_down: bool,
}

impl MainState {
    fn new() -> MainState {
        MainState {
            pos_x: 100,
            pos_y: 100,
            mouse_down: false,
        }
    }
}

impl event::EventHandler for MainState {
    fn update(&mut self, _ctx: &mut Context) -> GameResult<()> {
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::clear(ctx);
        graphics::circle(
            ctx,
            DrawMode::Fill,
            Point2::new(self.pos_x as f32, self.pos_y as f32),
            100.0,
            1.0,
        )?;
        graphics::present(ctx);
        Ok(())
    }

    fn mouse_button_down_event(&mut self, _ctx: &mut Context, button: MouseButton, x: i32, y: i32) {
        self.mouse_down = true;
        info!("Mouse button pressed: {:?}, x: {}, y: {}", button, x, y);
    }

    fn mouse_button_up_event(&mut self, _ctx: &mut Context, button: MouseButton, x: i32, y: i32) {
        self.mouse_down = false;
        info!("Mouse button released: {:?}, x: {}, y: {}", button, x, y);
    }

    fn mouse_motion_event(
        &mut self,
        _ctx: &mut Context,
        _state: MouseState,
        x: i32,
        y: i32,
        xrel: i32,
        yrel: i32,
    ) {
        if self.mouse_down {
            self.pos_x = x;
            self.pos_y = y;
        }
        info!(
            "Mouse motion, x: {}, y: {}, relative x: {}, relative y: {}",
            x, y, xrel, yrel
        );
    }

    fn mouse_wheel_event(&mut self, _ctx: &mut Context, x: i32, y: i32) {
        info!("Mousewheel event, x: {}, y: {}", x, y);
    }

    fn key_down_event(&mut self, _ctx: &mut Context, keycode: Keycode, keymod: Mod, repeat: bool) {
        info!(
            "Key pressed: {:?}, modifier {:?}, repeat: {}",
            keycode, keymod, repeat
        );
    }
    fn key_up_event(&mut self, _ctx: &mut Context, keycode: Keycode, keymod: Mod, repeat: bool) {
        info!(
            "Key released: {:?}, modifier {:?}, repeat: {}",
            keycode, keymod, repeat
        );
    }

    fn text_editing_event(&mut self, _ctx: &mut Context, text: String, start: i32, length: i32) {
        info!(
            "Text editing: {}, start {}, length: {}",
            text, start, length
        );
    }

    fn text_input_event(&mut self, _ctx: &mut Context, text: String) {
        info!("Text input: {}", text);
    }

    fn controller_button_down_event(&mut self, _ctx: &mut Context, btn: Button, instance_id: i32) {
        info!(
            "Controller button pressed: {:?} Controller_Id: {}",
            btn, instance_id
        );
    }

    fn controller_button_up_event(&mut self, _ctx: &mut Context, btn: Button, instance_id: i32) {
        info!(
            "Controller button released: {:?} Controller_Id: {}",
            btn, instance_id
        );
    }

    fn controller_axis_event(
        &mut self,
        _ctx: &mut Context,
        axis: Axis,
        value: i16,
        instance_id: i32,
    ) {
        info!(
            "Axis Event: {:?} Value: {} Controller_Id: {}",
            axis, value, instance_id
        );
    }

    fn focus_event(&mut self, _ctx: &mut Context, gained: bool) {
        if gained {
            info!("Focus gained");
        } else {
            info!("Focus lost");
        }
    }
}

pub fn main() {
    let c = conf::Conf::new();
    let ctx = &mut Context::load_from_conf("input_test", "ggez", c).unwrap();

    // We add the CARGO_MANIFEST_DIR/resources do the filesystems paths so
    // we we look in the cargo project for files.
    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        let mut path = path::PathBuf::from(manifest_dir);
        path.push("resources");
        ctx.filesystem.mount(&path, true);
    }

    let state = &mut MainState::new();
    event::run(ctx, state).unwrap();
}
