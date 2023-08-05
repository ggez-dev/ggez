//! The `event` module contains traits and structs to actually run your game mainloop
//! and handle top-level state, as well as handle input events such as keyboard
//! and mouse.
//!
//! If you don't want to use `ggez`'s built in event loop, you can
//! write your own mainloop and check for events on your own.  This is
//! not particularly hard, there's nothing special about the
//! `EventHandler` trait.  It just tries to simplify the process a
//! little.  For examples of how to write your own main loop, see the
//! source code for this module, or the [`eventloop`
//! example](https://github.com/ggez/ggez/blob/master/examples/eventloop.rs).

use winit::{self, dpi};

/// A mouse button.
pub use winit::event::{MouseButton, ScanCode};

/// An analog axis of some device (gamepad thumbstick, joystick...).
#[cfg(feature = "gamepad")]
pub use gilrs::Axis;
/// A button of some device (gamepad, joystick...).
#[cfg(feature = "gamepad")]
pub use gilrs::Button;

/// `winit` events; nested in a module for re-export neatness.
pub mod winit_event {
    pub use super::winit::event::{
        DeviceEvent, ElementState, Event, KeyboardInput, ModifiersState, MouseScrollDelta,
        TouchPhase, WindowEvent,
    };
}
use crate::context::ContextFields;
#[cfg(not(feature = "gamepad"))]
use crate::context::GamepadContext;
use crate::context::HasMut;
use crate::graphics::GraphicsContext;
use crate::input;
#[cfg(feature = "gamepad")]
use crate::input::gamepad::GamepadContext;
#[cfg(feature = "gamepad")]
pub use crate::input::gamepad::GamepadId;
use crate::input::keyboard::{KeyCode, KeyInput, KeyMods};
use crate::Context;
use crate::GameError;

use self::winit_event::{
    ElementState, Event, KeyboardInput, MouseScrollDelta, TouchPhase, WindowEvent,
};
/// `winit` event loop.
pub use winit::event_loop::{ControlFlow, EventLoop};

/// Used in [`EventHandler::on_error()`](trait.EventHandler.html#method.on_error)
/// to specify where an error originated
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ErrorOrigin {
    /// error originated in `update()`
    Update,
    /// error originated in `draw()`
    Draw,
    /// error originated in `mouse_button_down_event()`
    MouseButtonDownEvent,
    /// error originated in `mouse_button_up_event()`
    MouseButtonUpEvent,
    /// error originated in `mouse_motion_event()`
    MouseMotionEvent,
    /// error originated in `raw_mouse_motion_event()`
    RawMouseMotionEvent,
    /// error originated in `mouse_enter_or_leave()`
    MouseEnterOrLeave,
    /// error originated in `mouse_wheel_event()`
    MouseWheelEvent,
    /// error originated in `key_down_event()`
    KeyDownEvent,
    /// error originated in `key_up_event()`
    KeyUpEvent,
    /// error originated in `text_input_event()`
    TextInputEvent,
    /// error originated in `touch_event()`
    TouchEvent,
    /// error originated in `gamepad_button_down_event()`
    GamepadButtonDownEvent,
    /// error originated in `gamepad_button_up_event()`
    GamepadButtonUpEvent,
    /// error originated in `gamepad_axis_event()`
    GamepadAxisEvent,
    /// error originated in `focus_event()`
    FocusEvent,
    /// error originated in `quit_event()`
    QuitEvent,
    /// error originated in `resize_event()`
    ResizeEvent,
}

/// A trait defining event callbacks.  This is your primary interface with
/// `ggez`'s event loop.  Implement this trait for a type and
/// override at least the [`update()`](#tymethod.update) and
/// [`draw()`](#tymethod.draw) methods, then pass it to
/// [`event::run()`](fn.run.html) to run the game's mainloop.
///
/// The default event handlers do nothing, apart from
/// [`key_down_event()`](#method.key_down_event), which will by
/// default exit the game if the escape key is pressed.  Just
/// override the methods you want to use.
///
/// For the error type simply choose the default [`GameError`](../error/enum.GameError.html),
/// or something more generic, if your situation requires it.
pub trait EventHandler<C = Context, E = GameError>
where
    E: std::fmt::Debug,
    C: HasMut<ContextFields> + HasMut<input::mouse::MouseContext>,
{
    /// Called upon each logic update to the game.
    /// This should be where the game's logic takes place.
    fn update(&mut self, _ctx: &mut C) -> Result<(), E>;

    /// Called to do the drawing of your game.
    /// You probably want to start this with
    /// [`Canvas::from_frame`](../graphics/struct.Canvas.html#method.from_frame) and end it
    /// with [`Canvas::finish`](../graphics/struct.Canvas.html#method.finish).
    fn draw(&mut self, _ctx: &mut C) -> Result<(), E>;

    /// A mouse button was pressed
    fn mouse_button_down_event(
        &mut self,
        _ctx: &mut C,
        _button: MouseButton,
        _x: f32,
        _y: f32,
    ) -> Result<(), E> {
        Ok(())
    }

    /// A mouse button was released
    fn mouse_button_up_event(
        &mut self,
        _ctx: &mut C,
        _button: MouseButton,
        _x: f32,
        _y: f32,
    ) -> Result<(), E> {
        Ok(())
    }

    /// The mouse was moved; it provides both absolute x and y coordinates in the window,
    /// and relative x and y coordinates compared to its last position.
    fn mouse_motion_event(
        &mut self,
        _ctx: &mut C,
        _x: f32,
        _y: f32,
        _dx: f32,
        _dy: f32,
    ) -> Result<(), E> {
        Ok(())
    }

    /// Returns the raw mouse emotion from DeviceEvent::MouseMotion. This is just the raw device movement of the mouse
    fn raw_mouse_motion_event(&mut self, _ctx: &mut C, _dx: f64, _dy: f64) -> Result<(), E> {
        Ok(())
    }

    /// mouse entered or left window area
    fn mouse_enter_or_leave(&mut self, _ctx: &mut C, _entered: bool) -> Result<(), E> {
        Ok(())
    }

    /// The mousewheel was scrolled, vertically (y, positive away from and negative toward the user)
    /// or horizontally (x, positive to the right and negative to the left).
    fn mouse_wheel_event(&mut self, _ctx: &mut C, _x: f32, _y: f32) -> Result<(), E> {
        Ok(())
    }

    /// A keyboard button was pressed.
    ///
    /// The default implementation of this will call [`ctx.request_quit()`](crate::Context::request_quit)
    /// when the escape key is pressed. If you override this with your own
    /// event handler you have to re-implement that functionality yourself.
    fn key_down_event(&mut self, ctx: &mut C, input: KeyInput, _repeated: bool) -> Result<(), E> {
        if input.keycode == Some(KeyCode::Escape) {
            HasMut::<ContextFields>::retrieve_mut(ctx).quit_requested = true;
        }
        Ok(())
    }

    /// A keyboard button was released.
    fn key_up_event(&mut self, _ctx: &mut C, _input: KeyInput) -> Result<(), E> {
        Ok(())
    }

    /// A unicode character was received, usually from keyboard input.
    /// This is the intended way of facilitating text input.
    fn text_input_event(&mut self, _ctx: &mut C, _character: char) -> Result<(), E> {
        Ok(())
    }

    /// An event from a touchscreen has been triggered; it provides the x and y location
    /// inside the window as well as the state of the tap (such as Started, Moved, Ended, etc)
    /// By default, touch events will trigger mouse behavior
    fn touch_event(&mut self, ctx: &mut C, phase: TouchPhase, x: f64, y: f64) -> Result<(), E> {
        let mouse = HasMut::<input::mouse::MouseContext>::retrieve_mut(ctx);
        mouse.handle_move(x as f32, y as f32);

        match phase {
            TouchPhase::Started => {
                mouse.set_button(MouseButton::Left, true);
                self.mouse_button_down_event(ctx, MouseButton::Left, x as f32, y as f32)?;
            }
            TouchPhase::Moved => {
                let diff = mouse.last_delta();
                self.mouse_motion_event(ctx, x as f32, y as f32, diff.x, diff.y)?;
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                mouse.set_button(MouseButton::Left, false);
                self.mouse_button_up_event(ctx, MouseButton::Left, x as f32, y as f32)?;
            }
        }

        Ok(())
    }

    /// A gamepad button was pressed; `id` identifies which gamepad.
    #[cfg(feature = "gamepad")]
    fn gamepad_button_down_event(
        &mut self,
        _ctx: &mut C,
        _btn: gilrs::Button,
        _id: GamepadId,
    ) -> Result<(), E> {
        Ok(())
    }

    /// A gamepad button was released; `id` identifies which gamepad.
    #[cfg(feature = "gamepad")]
    fn gamepad_button_up_event(
        &mut self,
        _ctx: &mut C,
        _btn: gilrs::Button,
        _id: GamepadId,
    ) -> Result<(), E> {
        Ok(())
    }

    /// A gamepad axis moved; `id` identifies which gamepad.
    #[cfg(feature = "gamepad")]
    fn gamepad_axis_event(
        &mut self,
        _ctx: &mut C,
        _axis: gilrs::Axis,
        _value: f32,
        _id: GamepadId,
    ) -> Result<(), E> {
        Ok(())
    }

    /// Called when the window is shown or hidden.
    fn focus_event(&mut self, _ctx: &mut C, _gained: bool) -> Result<(), E> {
        Ok(())
    }

    /// Called upon a quit event.  If it returns true,
    /// the game does not exit (the quit event is cancelled).
    fn quit_event(&mut self, _ctx: &mut C) -> Result<bool, E> {
        debug!("quit_event() callback called, quitting...");
        Ok(false)
    }

    /// Called when the user resizes the window, or when it is resized
    /// via [`GraphicsContext::set_mode()`](../graphics/struct.GraphicsContext.html#method.set_mode).
    fn resize_event(&mut self, _ctx: &mut C, _width: f32, _height: f32) -> Result<(), E> {
        Ok(())
    }

    /// Something went wrong, causing a `GameError` (or some other kind of error, depending on what you specified).
    /// If this returns true, the error was fatal, so the event loop ends, aborting the game.
    fn on_error(&mut self, _ctx: &mut C, _origin: ErrorOrigin, _e: E) -> bool {
        true
    }
}

/// Runs the game's main loop, calling event callbacks on the given state
/// object as events occur.
///
/// It does not try to do any type of framerate limiting.  See the
/// documentation for the [`timer`](../timer/index.html) module for more info.
#[allow(clippy::needless_return)] // necessary as the returns used here are actually necessary to break early from the event loop
pub fn run<S: 'static, C, E>(mut ctx: C, event_loop: EventLoop<()>, mut state: S) -> !
where
    S: EventHandler<C, E>,
    E: std::fmt::Debug,
    C: 'static
        + HasMut<ContextFields>
        + HasMut<GraphicsContext>
        + HasMut<input::keyboard::KeyboardContext>
        + HasMut<input::mouse::MouseContext>
        + HasMut<GamepadContext>
        + HasMut<crate::timer::TimeContext>,
{
    event_loop.run(move |mut event, _, control_flow| {
        let ctx = &mut ctx;
        let state = &mut state;

        // let mut fields = HasMut::<ContextFields>::retrieve_mut(ctx).clone();
        // let mut mouse =
        //     HasMut::<input::mouse::MouseContext>::retrieve_mut(ctx)
        // let mut keyboard = HasMut::<input::keyboard::KeyboardContext>::retrieve_mut(ctx).clone();

        if HasMut::<ContextFields>::retrieve_mut(ctx).quit_requested {
            let res = state.quit_event(ctx);
            HasMut::<ContextFields>::retrieve_mut(ctx).quit_requested = false;
            if let Ok(false) = res {
                HasMut::<ContextFields>::retrieve_mut(ctx).continuing = false;
            } else if catch_error(ctx, res, state, control_flow, ErrorOrigin::QuitEvent) {
                return;
            }
        }
        if !HasMut::<ContextFields>::retrieve_mut(ctx).continuing {
            *control_flow = ControlFlow::Exit;
            return;
        }

        *control_flow = ControlFlow::Poll;

        process_event(ctx, &mut event);
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Resized(logical_size) => {
                    // let actual_size = logical_size;
                    let res = state.resize_event(
                        ctx,
                        logical_size.width as f32,
                        logical_size.height as f32,
                    );
                    if catch_error(ctx, res, state, control_flow, ErrorOrigin::ResizeEvent) {
                        return;
                    };
                }
                WindowEvent::CloseRequested => {
                    let res = state.quit_event(ctx);
                    if let Ok(false) = res {
                        HasMut::<ContextFields>::retrieve_mut(ctx).continuing = false;
                    } else if catch_error(ctx, res, state, control_flow, ErrorOrigin::QuitEvent) {
                        return;
                    }
                }
                WindowEvent::Focused(gained) => {
                    let res = state.focus_event(ctx, gained);
                    if catch_error(ctx, res, state, control_flow, ErrorOrigin::FocusEvent) {
                        return;
                    };
                }
                WindowEvent::ReceivedCharacter(ch) => {
                    let res = state.text_input_event(ctx, ch);
                    if catch_error(ctx, res, state, control_flow, ErrorOrigin::TextInputEvent) {
                        return;
                    };
                }
                WindowEvent::ModifiersChanged(mods) => {
                    HasMut::<input::keyboard::KeyboardContext>::retrieve_mut(ctx)
                        .set_modifiers(KeyMods::from(mods))
                }
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: keycode,
                            scancode,
                            ..
                        },
                    ..
                } => {
                    let active_mods =
                        HasMut::<input::keyboard::KeyboardContext>::retrieve_mut(ctx).active_mods();

                    let repeat = HasMut::<input::keyboard::KeyboardContext>::retrieve_mut(ctx)
                        .is_key_repeated();
                    let res = state.key_down_event(
                        ctx,
                        KeyInput {
                            scancode,
                            keycode,
                            mods: active_mods,
                        },
                        repeat,
                    );
                    if catch_error(ctx, res, state, control_flow, ErrorOrigin::KeyDownEvent) {
                        return;
                    };
                }
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state: ElementState::Released,
                            virtual_keycode: keycode,
                            scancode,
                            ..
                        },
                    ..
                } => {
                    let active_mods =
                        HasMut::<input::keyboard::KeyboardContext>::retrieve_mut(ctx).active_mods();
                    let res = state.key_up_event(
                        ctx,
                        KeyInput {
                            scancode,
                            keycode,
                            mods: active_mods,
                        },
                    );
                    if catch_error(ctx, res, state, control_flow, ErrorOrigin::KeyUpEvent) {
                        return;
                    };
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    let gfx = HasMut::<GraphicsContext>::retrieve_mut(ctx);
                    let (x, y) = match delta {
                        MouseScrollDelta::LineDelta(x, y) => (x, y),
                        MouseScrollDelta::PixelDelta(pos) => {
                            let scale_factor = gfx.window.scale_factor();
                            let dpi::LogicalPosition { x, y } = pos.to_logical::<f32>(scale_factor);
                            (x, y)
                        }
                    };
                    let res = state.mouse_wheel_event(ctx, x, y);
                    if catch_error(ctx, res, state, control_flow, ErrorOrigin::MouseWheelEvent) {
                        return;
                    };
                }
                WindowEvent::MouseInput {
                    state: element_state,
                    button,
                    ..
                } => {
                    let position =
                        HasMut::<input::mouse::MouseContext>::retrieve_mut(ctx).position();
                    match element_state {
                        ElementState::Pressed => {
                            let res =
                                state.mouse_button_down_event(ctx, button, position.x, position.y);
                            if catch_error(
                                ctx,
                                res,
                                state,
                                control_flow,
                                ErrorOrigin::MouseButtonDownEvent,
                            ) {
                                return;
                            };
                        }
                        ElementState::Released => {
                            let res =
                                state.mouse_button_up_event(ctx, button, position.x, position.y);
                            if catch_error(
                                ctx,
                                res,
                                state,
                                control_flow,
                                ErrorOrigin::MouseButtonUpEvent,
                            ) {
                                return;
                            };
                        }
                    }
                }
                WindowEvent::CursorMoved { .. } => {
                    let position =
                        HasMut::<input::mouse::MouseContext>::retrieve_mut(ctx).position();
                    let delta =
                        HasMut::<input::mouse::MouseContext>::retrieve_mut(ctx).last_delta();
                    let res =
                        state.mouse_motion_event(ctx, position.x, position.y, delta.x, delta.y);
                    if catch_error(ctx, res, state, control_flow, ErrorOrigin::MouseMotionEvent) {
                        return;
                    };
                }
                WindowEvent::Touch(touch) => {
                    let res =
                        state.touch_event(ctx, touch.phase, touch.location.x, touch.location.y);
                    if catch_error(ctx, res, state, control_flow, ErrorOrigin::TouchEvent) {
                        return;
                    };
                }
                WindowEvent::CursorEntered { device_id: _ } => {
                    let res = state.mouse_enter_or_leave(ctx, true);
                    if catch_error(
                        ctx,
                        res,
                        state,
                        control_flow,
                        ErrorOrigin::MouseEnterOrLeave,
                    ) {
                        return;
                    }
                }
                WindowEvent::CursorLeft { device_id: _ } => {
                    let res = state.mouse_enter_or_leave(ctx, false);
                    if catch_error(
                        ctx,
                        res,
                        state,
                        control_flow,
                        ErrorOrigin::MouseEnterOrLeave,
                    ) {
                        return;
                    }
                }
                _x => {
                    // trace!("ignoring window event {:?}", x);
                }
            },
            Event::DeviceEvent {
                device_id: _,
                event,
            } => {
                if let winit::event::DeviceEvent::MouseMotion { delta } = event {
                    let res = state.raw_mouse_motion_event(ctx, delta.0, delta.0);
                    if catch_error(
                        ctx,
                        res,
                        state,
                        control_flow,
                        ErrorOrigin::RawMouseMotionEvent,
                    ) {
                        return;
                    }
                }
            }
            Event::Resumed => (),
            Event::Suspended => (),
            Event::NewEvents(_) => (),
            Event::UserEvent(_) => (),
            Event::MainEventsCleared => {
                // If you are writing your own event loop, make sure
                // you include `timer_context.tick()` and
                // `ctx.process_event()` calls.  These update ggez's
                // internal state however necessary.
                let time = HasMut::<crate::timer::TimeContext>::retrieve_mut(ctx);
                time.tick();

                // Handle gamepad events if necessary.
                #[cfg(feature = "gamepad")]
                while let Some(gilrs::Event { id, event, .. }) =
                    HasMut::<input::gamepad::GamepadContext>::retrieve_mut(ctx).next_event()
                {
                    match event {
                        gilrs::EventType::ButtonPressed(button, _) => {
                            let res = state.gamepad_button_down_event(ctx, button, GamepadId(id));
                            if catch_error(
                                ctx,
                                res,
                                state,
                                control_flow,
                                ErrorOrigin::GamepadButtonDownEvent,
                            ) {
                                return;
                            };
                        }
                        gilrs::EventType::ButtonReleased(button, _) => {
                            let res = state.gamepad_button_up_event(ctx, button, GamepadId(id));
                            if catch_error(
                                ctx,
                                res,
                                state,
                                control_flow,
                                ErrorOrigin::GamepadButtonUpEvent,
                            ) {
                                return;
                            };
                        }
                        gilrs::EventType::AxisChanged(axis, value, _) => {
                            let res = state.gamepad_axis_event(ctx, axis, value, GamepadId(id));
                            if catch_error(
                                ctx,
                                res,
                                state,
                                control_flow,
                                ErrorOrigin::GamepadAxisEvent,
                            ) {
                                return;
                            };
                        }
                        _ => {}
                    }
                }

                let res = state.update(ctx);
                if catch_error(ctx, res, state, control_flow, ErrorOrigin::Update) {
                    return;
                };

                if let Err(e) = HasMut::<GraphicsContext>::retrieve_mut(ctx).begin_frame() {
                    error!("Error on GraphicsContext::begin_frame(): {e:?}");
                    eprintln!("Error on GraphicsContext::begin_frame(): {e:#?}");
                    *control_flow = ControlFlow::Exit;
                }

                if let Err(e) = state.draw(ctx) {
                    error!("Error on EventHandler::draw(): {e:?}");
                    eprintln!("Error on EventHandler::draw(): {e:#?}");
                    if state.on_error(ctx, ErrorOrigin::Draw, e) {
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                }

                if let Err(e) = HasMut::<GraphicsContext>::retrieve_mut(ctx).end_frame() {
                    error!("Error on GraphicsContext::end_frame(): {e:?}");
                    eprintln!("Error on GraphicsContext::end_frame(): {e:#?}");
                    *control_flow = ControlFlow::Exit;
                }

                // reset the mouse delta for the next frame
                // necessary because it's calculated cumulatively each cycle
                HasMut::<input::mouse::MouseContext>::retrieve_mut(ctx).reset_delta();

                // Copy the state of the keyboard into the KeyboardContext
                // and the mouse into the MouseContext
                HasMut::<input::keyboard::KeyboardContext>::retrieve_mut(ctx).save_keyboard_state();
                HasMut::<input::mouse::MouseContext>::retrieve_mut(ctx).save_mouse_state();
            }
            Event::RedrawRequested(_) => (),
            Event::RedrawEventsCleared => (),
            Event::LoopDestroyed => (),
        }
    })
}

fn catch_error<T, C, E, S: 'static>(
    ctx: &mut C,
    event_result: Result<T, E>,
    state: &mut S,
    control_flow: &mut ControlFlow,
    origin: ErrorOrigin,
) -> bool
where
    S: EventHandler<C, E>,
    E: std::fmt::Debug,
    C: HasMut<ContextFields> + HasMut<input::mouse::MouseContext>,
{
    if let Err(e) = event_result {
        error!("Error on EventHandler {origin:?}: {e:?}");
        eprintln!("Error on EventHandler {origin:?}: {e:#?}");
        if state.on_error(ctx, origin, e) {
            *control_flow = ControlFlow::Exit;
            return true;
        }
    }
    false
}

/// Feeds an `Event` into the `Context` so it can update any internal
/// state it needs to, such as detecting window resizes.  If you are
/// rolling your own event loop, you should call this on the events
/// you receive before processing them yourself.
pub fn process_event<C>(ctx: &mut C, event: &mut winit::event::Event<()>)
where
    C: HasMut<ContextFields>
        + HasMut<GraphicsContext>
        + HasMut<input::keyboard::KeyboardContext>
        + HasMut<input::mouse::MouseContext>,
{
    if let winit_event::Event::DeviceEvent {
        event: winit::event::DeviceEvent::MouseMotion { delta },
        ..
    } = event
    {
        let mouse = HasMut::<input::mouse::MouseContext>::retrieve_mut(ctx);
        mouse.handle_motion(delta.0, delta.1);
    }
    if let winit_event::Event::WindowEvent { event, .. } = event {
        match event {
            winit_event::WindowEvent::Resized(physical_size) => {
                let gfx = HasMut::<GraphicsContext>::retrieve_mut(ctx);
                gfx.resize(*physical_size);
            }
            winit_event::WindowEvent::CursorMoved {
                position: physical_position,
                ..
            } => {
                let mouse = HasMut::<input::mouse::MouseContext>::retrieve_mut(ctx);
                mouse.handle_move(physical_position.x as f32, physical_position.y as f32);
            }
            winit_event::WindowEvent::MouseInput { button, state, .. } => {
                let mouse = HasMut::<input::mouse::MouseContext>::retrieve_mut(ctx);
                let pressed = match state {
                    winit_event::ElementState::Pressed => true,
                    winit_event::ElementState::Released => false,
                };
                mouse.set_button(*button, pressed);
            }
            winit_event::WindowEvent::ModifiersChanged(mods) => {
                let keyboard = HasMut::<input::keyboard::KeyboardContext>::retrieve_mut(ctx);
                keyboard.set_modifiers(KeyMods::from(*mods))
            }
            winit_event::WindowEvent::KeyboardInput {
                input:
                    winit::event::KeyboardInput {
                        state,
                        scancode,
                        virtual_keycode: keycode,
                        ..
                    },
                ..
            } => {
                let keyboard = HasMut::<input::keyboard::KeyboardContext>::retrieve_mut(ctx);
                let pressed = match state {
                    winit_event::ElementState::Pressed => true,
                    winit_event::ElementState::Released => false,
                };
                keyboard.set_scancode(*scancode, pressed);
                if let Some(key) = keycode {
                    keyboard.set_key(*key, pressed);
                }
            }
            winit_event::WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                let fields = HasMut::<ContextFields>::retrieve_mut(ctx);
                if !fields.conf.window_mode.resize_on_scale_factor_change {
                    // actively set the new_inner_size to be the desired size
                    // to stop winit from resizing our window
                    **new_inner_size = winit::dpi::PhysicalSize::<u32>::from([
                        fields.conf.window_mode.width,
                        fields.conf.window_mode.height,
                    ]);
                }
            }
            _ => (),
        }
    };
}
