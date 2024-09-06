//! Mouse utility functions.

use crate::context::Context;
use crate::error::GameError;
use crate::error::GameResult;
use std::collections::HashSet;
use winit::dpi;
pub use winit::event::MouseButton;
use winit::window::CursorGrabMode;
pub use winit::window::CursorIcon;

/// Stores state information for the mouse input.
// TODO: Add "differences with window cursor" notice
#[derive(Clone, Debug)]
pub struct MouseContext {
    last_position: glam::Vec2,
    last_delta: glam::Vec2,
    delta: glam::Vec2,
    raw_delta: glam::DVec2,
    buttons_pressed: HashSet<MouseButton>,
    cursor_type: CursorIcon,
    cursor_grabbed: bool,
    cursor_hidden: bool,
    previous_buttons_pressed: HashSet<MouseButton>,
}

impl MouseContext {
    /// Create a new MouseContext
    pub fn new() -> Self {
        Self {
            last_position: glam::Vec2::ZERO,
            last_delta: glam::Vec2::ZERO,
            delta: glam::Vec2::ZERO,
            raw_delta: glam::DVec2::ZERO,
            cursor_type: CursorIcon::Default,
            buttons_pressed: HashSet::new(),
            cursor_grabbed: false,
            cursor_hidden: false,
            previous_buttons_pressed: HashSet::new(),
        }
    }

    /// Returns the current mouse cursor type of the window.
    pub fn cursor_type(&self) -> CursorIcon {
        self.cursor_type
    }

    /// Set whether or not the mouse is hidden (invisible)
    pub fn cursor_hidden(&self) -> bool {
        self.cursor_hidden
    }

    /// Get the current position of the mouse cursor, in pixels.
    /// Complement to [`set_position()`](fn.set_position.html).
    /// Uses strictly window-only coordinates.
    pub fn position(&self) -> mint::Point2<f32> {
        self.last_position.into()
    }

    /// Get the distance the cursor was moved during the current frame, in pixels.
    pub fn delta(&self) -> mint::Point2<f32> {
        self.delta.into()
    }

    /// Returns whether or not the given mouse button is pressed.
    pub fn button_pressed(&self, button: MouseButton) -> bool {
        self.buttons_pressed.contains(&button)
    }

    /// Returns whether or not the given mouse button has been pressed this frame.
    pub fn button_just_pressed(&self, button: MouseButton) -> bool {
        self.buttons_pressed.contains(&button) && !self.previous_buttons_pressed.contains(&button)
    }

    /// Returns whether or not the given mouse button has been released this frame.
    pub fn button_just_released(&self, button: MouseButton) -> bool {
        !self.buttons_pressed.contains(&button) && self.previous_buttons_pressed.contains(&button)
    }

    /// Updates delta and position values.
    /// The inputs are interpreted as pixel coordinates inside the window.
    ///
    /// This function is called internally whenever the mouse moves to a new location.
    /// It can also be used to simulate mouse input.
    /// (It gets called inside the default implementation of the
    /// [`touch_event`](../../event/trait.EventHandler.html#method.touch_event), for example.)
    /// Calling this function alone won't trigger a
    /// [`mouse_motion_event`](../../event/trait.EventHandler.html#method.mouse_motion_event) though.
    /// (Note that the default implementation of
    /// [`touch_event`](../../event/trait.EventHandler.html#method.touch_event) DOES trigger one, but
    /// it does so by invoking it on the `EventHandler` manually.)
    pub fn handle_move(&mut self, new_x: f32, new_y: f32) {
        let current_delta = self.delta();
        let current_pos = self.position();
        let diff = glam::Vec2::new(new_x - current_pos.x, new_y - current_pos.y);
        // Sum up the cumulative mouse change for this frame in `delta`:
        self.set_delta(glam::Vec2::new(
            current_delta.x + diff.x,
            current_delta.y + diff.y,
        ));
        // `last_delta` is not cumulative.
        // It represents only the change between the last mouse event and the current one.
        self.set_last_delta(diff);
        self.set_last_position(glam::Vec2::new(new_x, new_y));
    }

    /// Handles the raw motion of the mouse to be able to provide the raw delta
    pub fn handle_motion(&mut self, x: f64, y: f64) {
        self.raw_delta = glam::DVec2::new(x, y);
    }

    /// Returns the raw delta or mouse motion of the device moving the cursor
    pub fn raw_delta(&self) -> mint::Vector2<f64> {
        self.raw_delta.into()
    }

    /// Resets the value returned by [`mouse::delta`](fn.delta.html) to zero.
    /// You shouldn't need to call this, except when you're running your own event loop.
    /// In this case call it right at the end, after `draw` and `update` have finished.
    pub fn reset_delta(&mut self) {
        self.delta = glam::Vec2::ZERO;
        self.raw_delta = glam::DVec2::ZERO;
    }

    /// Copies the current state of the mouse buttons into the context. If you are writing your own event loop
    /// you need to call this at the end of every update in order to use the functions `is_button_just_pressed`
    /// and `is_button_just_released`. Otherwise this is handled for you.
    pub fn save_mouse_state(&mut self) {
        self.previous_buttons_pressed
            .clone_from(&self.buttons_pressed);
    }

    pub(crate) fn set_last_position(&mut self, p: glam::Vec2) {
        self.last_position = p;
    }

    pub(crate) fn set_last_delta(&mut self, p: glam::Vec2) {
        self.last_delta = p;
    }

    pub(crate) fn set_delta(&mut self, p: glam::Vec2) {
        self.delta = p;
    }

    pub(crate) fn set_button(&mut self, button: MouseButton, pressed: bool) {
        if pressed {
            let _ = self.buttons_pressed.insert(button);
        } else {
            let _ = self.buttons_pressed.remove(&button);
        }
    }

    /// Get the distance the cursor was moved between the latest two `mouse_motion_events`.
    /// Really useful only if you are writing your own event loop
    pub fn last_delta(&self) -> mint::Point2<f32> {
        self.last_delta.into()
    }
}

impl Default for MouseContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the current mouse cursor type of the window.
#[deprecated(since = "0.8.0", note = "Use `ctx.mouse.cursor_type` instead")]
pub fn cursor_type(ctx: &Context) -> CursorIcon {
    ctx.mouse.cursor_type()
}

/// Set whether or not the mouse is hidden (invisible)
#[deprecated(since = "0.8.0", note = "Use `ctx.mouse.cursor_hidden` instead")]
pub fn cursor_hidden(ctx: &Context) -> bool {
    ctx.mouse.cursor_hidden()
}

/// Get the current position of the mouse cursor, in pixels.
/// Complement to [`set_position()`](fn.set_position.html).
/// Uses strictly window-only coordinates.
#[deprecated(since = "0.8.0", note = "Use `ctx.mouse.position` instead")]
pub fn position(ctx: &Context) -> mint::Point2<f32> {
    ctx.mouse.position()
}

/// Get the distance the cursor was moved during the current frame, in pixels.
#[deprecated(since = "0.8.0", note = "Use `ctx.mouse.delta` instead")]
pub fn delta(ctx: &Context) -> mint::Point2<f32> {
    ctx.mouse.delta()
}

/// Returns whether or not the given mouse button is pressed.
#[deprecated(since = "0.8.0", note = "Use `ctx.mouse.button_pressed` instead")]
pub fn button_pressed(ctx: &Context, button: MouseButton) -> bool {
    ctx.mouse.button_pressed(button)
}

/// Returns whether or not the given mouse button has been pressed this frame.
#[deprecated(since = "0.8.0", note = "Use `ctx.mouse.button_just_pressed` instead")]
pub fn button_just_pressed(ctx: &Context, button: MouseButton) -> bool {
    ctx.mouse.button_just_pressed(button)
}

/// Returns whether or not the given mouse button has been released this frame.
#[deprecated(since = "0.8.0", note = "Use `ctx.mouse.button_just_released` instead")]
pub fn button_just_released(ctx: &Context, button: MouseButton) -> bool {
    ctx.mouse.button_just_released(button)
}

/// Updates delta and position values.
/// The inputs are interpreted as pixel coordinates inside the window.
///
/// This function is called internally whenever the mouse moves to a new location.
/// It can also be used to simulate mouse input.
/// (It gets called inside the default implementation of the
/// [`touch_event`](../../event/trait.EventHandler.html#method.touch_event), for example.)
/// Calling this function alone won't trigger a
/// [`mouse_motion_event`](../../event/trait.EventHandler.html#method.mouse_motion_event) though.
/// (Note that the default implementation of
/// [`touch_event`](../../event/trait.EventHandler.html#method.touch_event) DOES trigger one, but
/// it does so by invoking it on the `EventHandler` manually.)
#[deprecated(since = "0.8.0", note = "Use `ctx.mouse.handle_move` instead")]
pub fn handle_move(ctx: &mut Context, new_x: f32, new_y: f32) {
    ctx.mouse.handle_move(new_x, new_y);
}

/// Set whether or not the mouse is hidden (invisible).
// TODO: Move to graphics context (This isn't input)
pub fn set_cursor_hidden(ctx: &mut Context, hidden: bool) {
    ctx.mouse.cursor_hidden = hidden;
    ctx.gfx.window.set_cursor_visible(!hidden);
}

/// Modifies the mouse cursor type of the window.
// TODO: Move to graphics context (This isn't input)
pub fn set_cursor_type(ctx: &mut Context, cursor_type: CursorIcon) {
    ctx.mouse.cursor_type = cursor_type;
    ctx.gfx.window.set_cursor(cursor_type);
}

/// Get whether or not the mouse is grabbed.
// TODO: Move to graphics context (This isn't input)
pub fn cursor_grabbed(ctx: &Context) -> bool {
    ctx.mouse.cursor_grabbed
}

/// Set whether or not the mouse is grabbed (confined to the window)
///
/// **Note**: macOS locks the cursor rather than confining it.
// TODO: Move to graphics context (This isn't input)
#[allow(clippy::missing_errors_doc)]
pub fn set_cursor_grabbed(ctx: &mut Context, grabbed: bool) -> GameResult {
    ctx.mouse.cursor_grabbed = grabbed;
    ctx.gfx
        .window
        .set_cursor_grab(if grabbed {
            if cfg!(target_os = "macos") {
                CursorGrabMode::Locked
            } else {
                CursorGrabMode::Confined
            }
        } else {
            CursorGrabMode::None
        })
        .map_err(|e| GameError::WindowError(e.to_string()))
}

/// Set the current position of the mouse cursor, in pixels.
/// Uses strictly window-only coordinates.
/// ### Errors
///
/// Will return `GameError::WindowError` if platform doesn't support this.
// TODO: Move to graphics context (This isn't input)
pub fn set_position<P>(ctx: &mut Context, point: P) -> GameResult
where
    P: Into<mint::Point2<f32>>,
{
    let point = glam::Vec2::from(point.into());
    ctx.mouse.last_position = point;
    ctx.gfx
        .window
        .set_cursor_position(dpi::LogicalPosition {
            x: f64::from(point.x),
            y: f64::from(point.y),
        })
        .map_err(|_| GameError::WindowError("Couldn't set mouse cursor position!".to_owned()))
}
