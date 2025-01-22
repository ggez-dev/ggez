//! This example demonstrates how to roll your own event loop,
//! if for some reason you want to do that instead of using the `EventHandler`
//! trait to do that for you.
//!
//! This is how `ggez::event::run()` works, mostly, (if you want to see which parts were left out
//! of this example, check [event.rs](https://github.com/ggez/ggez/blob/master/src/event.rs),
//! it really is not doing anything magical.  But, if you want a bit more power over
//! the control flow of your game, this is how you get it.
//!
//! It is functionally identical to the `super_simple.rs` example apart from that.

use ggez::graphics::{self, Color, DrawMode};
use ggez::Context;
use ggez::GameResult;
use ggez::{event, GameError};

use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::keyboard::{Key, NamedKey};
use winit::window::WindowId;

struct CustomApplicationHandler {
    ctx: Context,
    position: f32,
}

impl ApplicationHandler<()> for CustomApplicationHandler {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, _: StartCause) {
        if self.ctx.fields.quit_requested {
            self.ctx.fields.continuing = false;
        }
        if !self.ctx.fields.continuing {
            event_loop.exit();
            return;
        }

        event_loop.set_control_flow(ControlFlow::Poll);
    }

    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        mut window_id: WindowId,
        mut event: WindowEvent,
    ) {
        // This tells `ggez` to update it's internal states, should the event require that.
        // These include cursor position, view updating on resize, etc.
        event::process_window_event(&mut self.ctx, &mut window_id, &mut event);

        match event {
            WindowEvent::CloseRequested => {
                self.ctx.request_quit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if Key::Named(NamedKey::Escape) == event.logical_key {
                    self.ctx.request_quit();
                }
            }
            // `CloseRequested` and `KeyboardInput` events won't appear here.
            x => println!("Other window event fired: {x:?}"),
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        mut device_id: DeviceId,
        mut event: DeviceEvent,
    ) {
        // This tells `ggez` to update it's internal states, should the event require that.
        // These include cursor position, view updating on resize, etc.
        event::process_device_event(&mut self.ctx, &mut device_id, &mut event);

        println!("Device event fired: {event:?}");
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Tell the timer stuff a frame has happened.
        // Without this the FPS timer functions and such won't work.
        self.ctx.time.tick();

        // Update
        self.position += 1.0;

        // Draw
        self.ctx.gfx.begin_frame().unwrap();

        let mut canvas =
            graphics::Canvas::from_frame(&self.ctx, graphics::Color::from([0.1, 0.2, 0.3, 1.0]));

        let circle = graphics::Mesh::new_circle(
            &self.ctx,
            DrawMode::fill(),
            ggez::glam::Vec2::new(0.0, 0.0),
            100.0,
            2.0,
            Color::WHITE,
        )
        .unwrap();
        canvas.draw(&circle, ggez::glam::Vec2::new(self.position, 380.0));

        canvas.finish(&mut self.ctx).unwrap();
        self.ctx.gfx.end_frame().unwrap();

        // reset the mouse delta for the next frame
        // necessary because it's calculated cumulatively each cycle
        self.ctx.mouse.reset_delta();

        // Copy the state of the keyboard into the KeyboardContext and
        // the mouse into the MouseContext.
        // Not required for this example but important if you want to
        // use the functions keyboard::is_key_just_pressed/released and
        // mouse::is_button_just_pressed/released.
        self.ctx.keyboard.save_keyboard_state();
        self.ctx.mouse.save_mouse_state();

        ggez::timer::yield_now();
    }
}

pub fn main() -> GameResult {
    let cb = ggez::ContextBuilder::new("eventloop", "ggez");
    let (ctx, events_loop) = cb.build()?;

    let mut app = CustomApplicationHandler { ctx, position: 1.0 };

    // Handle events. Refer to `winit` docs for more information.
    events_loop
        .run_app(&mut app)
        .map_err(GameError::EventLoopError)
}
