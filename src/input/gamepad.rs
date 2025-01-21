//! Gamepad utility functions.
//!
//! This is going to be a bit of a work-in-progress as gamepad input
//! gets fleshed out.  The `gilrs` crate needs help to add better
//! cross-platform support.  Why not give it a hand?

use crate::GameResult;
pub use gamepad_context::*;

#[cfg(not(feature = "gamepad"))]
mod gamepad_context {
    #![allow(missing_copy_implementations)]
    use super::*;
    /// Dummy gamepad context, for if the gamepad feature is disabled, to allow trait bounds
    /// to still resolve properly
    #[derive(Debug)]
    pub struct GamepadContext;

    impl GamepadContext {
        /// Create a new dummy GamepadContext
        pub fn new() -> GameResult<Self> {
            Ok(Self)
        }
    }
}

#[cfg(feature = "gamepad")]
mod gamepad_context {
    use super::*;
    use gilrs::ConnectedGamepadsIterator;
    use std::fmt;

    pub use gilrs::{self, Event, Gamepad, Gilrs};

    /// A unique identifier for a particular gamepad
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct GamepadId(pub(crate) gilrs::GamepadId);

    use crate::context::Context;

    /// A structure that contains gamepad state using `gilrs`.
    pub struct GamepadContext {
        pub(crate) gilrs: Gilrs,
    }

    impl fmt::Debug for GamepadContext {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "<GilrsGamepadContext: {self:p}>")
        }
    }

    impl GamepadContext {
        /// Create a new GamepadContext
        pub fn new() -> GameResult<Self> {
            let gilrs = Gilrs::new()?;
            Ok(GamepadContext { gilrs })
        }
    }

    impl From<Gilrs> for GamepadContext {
        /// Converts from a `Gilrs` custom instance to a `GilrsGamepadContext`
        fn from(gilrs: Gilrs) -> Self {
            Self { gilrs }
        }
    }

    impl GamepadContext {
        /// Returns a gamepad event.
        pub fn next_event(&mut self) -> Option<Event> {
            self.gilrs.next_event()
        }

        /// Returns the `Gamepad` associated with an `id`.
        pub fn gamepad(&self, id: GamepadId) -> Gamepad {
            self.gilrs.gamepad(id.0)
        }

        /// Return an iterator of all the `Gamepads` that are connected.
        pub fn gamepads(&self) -> GamepadsIterator {
            GamepadsIterator {
                wrapped: self.gilrs.gamepads(),
            }
        }
    }

    /// An iterator of the connected gamepads
    pub struct GamepadsIterator<'a> {
        wrapped: ConnectedGamepadsIterator<'a>,
    }

    impl fmt::Debug for GamepadsIterator<'_> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "<GamepadsIterator: {self:p}>")
        }
    }

    impl<'a> Iterator for GamepadsIterator<'a> {
        type Item = (GamepadId, Gamepad<'a>);

        fn next(&mut self) -> Option<(GamepadId, Gamepad<'a>)> {
            self.wrapped.next().map(|(id, gp)| (GamepadId(id), gp))
        }
    }

    /// Returns the `Gamepad` associated with an `id`.
    #[deprecated(since = "0.8.0", note = "Use `ctx.gamepad.gamepad` instead")]
    pub fn gamepad(ctx: &Context, id: GamepadId) -> Gamepad {
        ctx.gamepad.gamepad(id)
    }

    /// Return an iterator of all the `Gamepads` that are connected.
    #[deprecated(since = "0.8.0", note = "Use `ctx.gamepad.gamepads` instead")]
    pub fn gamepads(ctx: &Context) -> GamepadsIterator {
        ctx.gamepad.gamepads()
    }
}
// Properties gamepads might want:
// Number of buttons
// Number of axes
// Name/ID
// Is it connected?  (For consoles?)
// Whether or not they support vibration

/*
/// Lists all gamepads.  With metainfo, maybe?
pub fn list_gamepads() {
    unimplemented!()
}

/// Returns the state of the given axis on a gamepad.
pub fn axis() {
    unimplemented!()
}

/// Returns the state of the given button on a gamepad.
pub fn button_pressed() {
    unimplemented!()
}
*/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gilrs_init() {
        assert!(GamepadContext::new().is_ok());
    }
}
