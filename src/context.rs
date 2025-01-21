//! The `context` module contains functions and traits related to using the `Context` type.

use std::fmt;
/// We re-export winit so it's easy for people to use the same version as we are
/// without having to mess around figuring it out.
pub use winit;

#[cfg(feature = "audio")]
use crate::audio;
use crate::conf;
use crate::error::GameResult;
use crate::filesystem::Filesystem;
use crate::graphics;
use crate::graphics::GraphicsContext;
use crate::input;
use crate::timer;

/// A `Context` is an object that holds on to global resources.
/// It basically tracks hardware state such as the screen, audio
/// system, timers, and so on.  Generally this type can **not**
/// be shared/sent between threads and only one `Context` can exist at a time.  Trying
/// to create a second one will fail.  It is fine to drop a `Context`
/// and create a new one, but this will also close and re-open your
/// game's window.
///
/// Most functions that interact with the hardware, for instance
/// drawing things, playing sounds, or loading resources will need
/// to access one, or rarely even two, of its sub-contexts.
/// It is an error to create some type that
/// relies upon a `Context`, such as `Image`, and then drop the `Context`
/// and try to draw the old `Image` with the new `Context`.
///
/// The fields in this struct used to be basically undocumented features,
/// only here to make it easier to debug, or to let advanced users
/// hook into the guts of ggez and make it do things it normally
/// can't. Now that `ggez`'s module-level functions, taking the whole `Context`
/// have been deprecated, calling their methods directly is recommended.
pub struct Context {
    /// Filesystem state.
    pub fs: Filesystem,
    /// Graphics state.
    pub gfx: GraphicsContext,
    /// Timer state.
    pub time: timer::TimeContext,
    /// Audio context.
    #[cfg(feature = "audio")]
    pub audio: audio::AudioContext,
    /// Keyboard input context.
    pub keyboard: input::keyboard::KeyboardContext,
    /// Mouse input context.
    pub mouse: input::mouse::MouseContext,
    /// Gamepad input context.
    pub gamepad: input::gamepad::GamepadContext,
    /// Fields used by all contexts
    pub fields: ContextFields,
}

impl Context {
    /// Attempts to terminate the [`ggez::event::run()`](crate::event::run) loop by requesting a
    /// [`quit_event`](crate::event::EventHandler::quit_event) at the very start of the next frame. If this event
    /// returns `Ok(false)`, then [`Context.continuing`](struct.Context.html#structfield.continuing)
    /// is set to `false` and the loop breaks.
    pub fn request_quit(&mut self) {
        HasMut::<ContextFields>::retrieve_mut(self).quit_requested = true;
    }
}

/// Common fields that any context require
#[derive(Clone, Debug)]
pub struct ContextFields {
    /// The Conf object the Context was created with.
    /// It's here just so that we can see the original settings,
    /// updating it will have no effect.
    pub conf: conf::Conf,
    /// Controls whether or not the event loop should be running.
    /// This is internally controlled by the outcome of [`quit_event`](crate::event::EventHandler::quit_event),
    /// requested through [`event::request_quit()`](crate::Context::request_quit).
    pub continuing: bool,
    /// Whether or not a `quit_event` has been requested.
    /// Set this with [`ggez::event::request_quit()`](crate::Context::request_quit).
    ///
    /// It's exposed here for people who want to roll their own event loop.
    pub quit_requested: bool,
}

// This is ugly and hacky but greatly improves ergonomics.

/// Used to represent types that can provide a certain context type.
///
/// If you don't know what this is, you most likely want to pass `ctx`.
///
/// This trait is basically syntactical sugar, saving you from having
/// to split contexts when you don't need to and also shortening calls like
/// ```rust
/// # use ggez::GameResult;
/// # fn t(ctx: &mut ggez::Context, canvas: ggez::graphics::Canvas) -> GameResult {
/// canvas.finish(&mut ctx.gfx)?;
/// # Ok(())
/// # }
/// ```
/// into just
/// ```rust
/// # use ggez::GameResult;
/// # fn t(ctx: &mut ggez::Context, canvas: ggez::graphics::Canvas) -> GameResult {
/// canvas.finish(ctx)?;
/// # Ok(())
/// # }
/// ```
pub trait Has<T> {
    /// Method to retrieve the context type.
    fn retrieve(&self) -> &T;
}

impl<T> Has<T> for T {
    #[inline]
    fn retrieve(&self) -> &T {
        self
    }
}

impl Has<Filesystem> for Context {
    #[inline]
    fn retrieve(&self) -> &Filesystem {
        &self.fs
    }
}

impl Has<GraphicsContext> for Context {
    #[inline]
    fn retrieve(&self) -> &GraphicsContext {
        &self.gfx
    }
}

#[cfg(feature = "audio")]
impl Has<audio::AudioContext> for Context {
    #[inline]
    fn retrieve(&self) -> &audio::AudioContext {
        &self.audio
    }
}

/// Used to represent types that can provide a certain context type in a mutable form.
/// See also [`Has<T>`].
///
/// If you don't know what this is, you most likely want to pass `ctx`.
pub trait HasMut<T> {
    /// Method to retrieve the context type as mutable.
    fn retrieve_mut(&mut self) -> &mut T;
}

impl<T> HasMut<T> for T {
    #[inline]
    fn retrieve_mut(&mut self) -> &mut T {
        self
    }
}

impl HasMut<ContextFields> for Context {
    #[inline]
    fn retrieve_mut(&mut self) -> &mut ContextFields {
        &mut self.fields
    }
}

impl HasMut<GraphicsContext> for Context {
    #[inline]
    fn retrieve_mut(&mut self) -> &mut GraphicsContext {
        &mut self.gfx
    }
}

impl HasMut<timer::TimeContext> for Context {
    #[inline]
    fn retrieve_mut(&mut self) -> &mut timer::TimeContext {
        &mut self.time
    }
}

impl HasMut<input::keyboard::KeyboardContext> for Context {
    #[inline]
    fn retrieve_mut(&mut self) -> &mut input::keyboard::KeyboardContext {
        &mut self.keyboard
    }
}

impl HasMut<input::mouse::MouseContext> for Context {
    #[inline]
    fn retrieve_mut(&mut self) -> &mut input::mouse::MouseContext {
        &mut self.mouse
    }
}

impl HasMut<input::gamepad::GamepadContext> for Context {
    #[inline]
    fn retrieve_mut(&mut self) -> &mut input::gamepad::GamepadContext {
        &mut self.gamepad
    }
}

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<Context: {self:p}>")
    }
}

impl Context {
    /// Tries to create a new Context using settings from the given [`Conf`](../conf/struct.Conf.html) object.
    /// Usually called by [`ContextBuilder::build()`](struct.ContextBuilder.html#method.build).
    fn from_conf(
        game_id: &str,
        conf: conf::Conf,
        fs: Filesystem,
    ) -> GameResult<(Context, winit::event_loop::EventLoop<()>)> {
        #[cfg(feature = "audio")]
        let audio_context = audio::AudioContext::new(&fs)?;
        let events_loop = winit::event_loop::EventLoop::new()?;
        let timer_context = timer::TimeContext::new();
        let graphics_context =
            graphics::context::GraphicsContext::new(game_id, &events_loop, &conf, &fs)?;

        let ctx = Context {
            fs,
            gfx: graphics_context,
            time: timer_context,
            #[cfg(feature = "audio")]
            audio: audio_context,
            keyboard: input::keyboard::KeyboardContext::new(),
            mouse: input::mouse::MouseContext::new(),
            gamepad: input::gamepad::GamepadContext::new()?,
            fields: ContextFields {
                conf,
                continuing: true,
                quit_requested: false,
            },
        };

        Ok((ctx, events_loop))
    }
}

use std::borrow::Cow;
use std::path;

/// A builder object for creating a [`Context`](struct.Context.html).
#[derive(Debug, Clone, PartialEq)]
pub struct ContextBuilder {
    pub(crate) game_id: String,
    pub(crate) author: String,
    pub(crate) conf: conf::Conf,
    pub(crate) resources_dir_name: path::PathBuf,
    pub(crate) resources_zip_name: path::PathBuf,
    pub(crate) paths: Vec<path::PathBuf>,
    pub(crate) memory_zip_files: Vec<Cow<'static, [u8]>>,
    pub(crate) load_conf_file: bool,
}

impl ContextBuilder {
    /// Create a new `ContextBuilder` with default settings.
    pub fn new(game_id: &str, author: &str) -> Self {
        Self {
            game_id: game_id.to_string(),
            author: author.to_string(),
            conf: conf::Conf::default(),
            resources_dir_name: "resources".into(),
            resources_zip_name: "resources.zip".into(),
            paths: vec![],
            memory_zip_files: vec![],
            load_conf_file: true,
        }
    }

    /// Sets the window setup settings.
    #[must_use]
    pub fn window_setup(mut self, setup: conf::WindowSetup) -> Self {
        self.conf.window_setup = setup;
        self
    }

    /// Sets the window mode settings.
    #[must_use]
    pub fn window_mode(mut self, mode: conf::WindowMode) -> Self {
        self.conf.window_mode = mode;
        self
    }

    /// Sets the graphics backend.
    #[must_use]
    pub fn backend(mut self, backend: conf::Backend) -> Self {
        self.conf.backend = backend;
        self
    }

    /// Sets all the config options, overriding any previous
    /// ones from [`window_setup()`](#method.window_setup),
    /// [`window_mode()`](#method.window_mode), and
    /// [`backend()`](#method.backend).  These are used as
    /// defaults and are overridden by any external config
    /// file found.
    #[must_use]
    pub fn default_conf(mut self, conf: conf::Conf) -> Self {
        self.conf = conf;
        self
    }

    /// Sets resources dir name.
    /// Default resources dir name is `resources`.
    #[must_use]
    pub fn resources_dir_name(mut self, new_name: impl Into<path::PathBuf>) -> Self {
        self.resources_dir_name = new_name.into();
        self
    }

    /// Sets resources zip name.
    /// Default resources dir name is `resources.zip`.
    #[must_use]
    pub fn resources_zip_name(mut self, new_name: impl Into<path::PathBuf>) -> Self {
        self.resources_zip_name = new_name.into();
        self
    }

    /// Add a new read-only filesystem path to the places to search
    /// for resources.
    #[must_use]
    pub fn add_resource_path<T>(mut self, path: T) -> Self
    where
        T: Into<path::PathBuf>,
    {
        self.paths.push(path.into());
        self
    }

    /// Add a new zip file from bytes whose contents will be searched
    /// for resources. The zip file will be stored in-memory.
    /// You can pass it a static slice, a `Vec` of bytes, etc.
    ///
    /// ```ignore
    /// use ggez::context::ContextBuilder;
    /// let _ = ContextBuilder::new()
    ///     .add_zipfile_bytes(include_bytes!("../resources.zip").to_vec())
    ///     .build();
    /// ```
    #[must_use]
    pub fn add_zipfile_bytes<B>(mut self, bytes: B) -> Self
    where
        B: Into<Cow<'static, [u8]>>,
    {
        let cow = bytes.into();
        self.memory_zip_files.push(cow);
        self
    }

    /// Specifies whether or not to load the `conf.toml` file if it
    /// exists and use its settings to override the provided values.
    /// Defaults to `true` which is usually what you want, but being
    /// able to fiddle with it is sometimes useful for debugging.
    #[must_use]
    pub fn with_conf_file(mut self, load_conf_file: bool) -> Self {
        self.load_conf_file = load_conf_file;
        self
    }

    /// Build a `Context`
    pub fn build(self) -> GameResult<(Context, winit::event_loop::EventLoop<()>)> {
        let fs = Filesystem::new(
            self.game_id.as_ref(),
            self.author.as_ref(),
            &self.resources_dir_name,
            &self.resources_zip_name,
        )?;

        for path in &self.paths {
            fs.mount(path, true);
        }

        for zipfile_bytes in self.memory_zip_files {
            fs.add_zip_file(std::io::Cursor::new(zipfile_bytes))?;
        }

        let config = if self.load_conf_file {
            fs.read_config().unwrap_or(self.conf)
        } else {
            self.conf
        };

        Context::from_conf(self.game_id.as_ref(), config, fs)
    }

    /// Build a Custom `Context`.
    pub fn custom_build<C>(
        self,
        from_conf: impl Fn(
            String,
            conf::Conf,
            Filesystem,
        ) -> GameResult<(C, winit::event_loop::EventLoop<()>)>,
    ) -> GameResult<(C, winit::event_loop::EventLoop<()>)>
    where
        C: HasMut<ContextFields>
            + HasMut<timer::TimeContext>
            + HasMut<input::keyboard::KeyboardContext>
            + HasMut<input::mouse::MouseContext>
            + HasMut<input::gamepad::GamepadContext>,
    {
        let fs = Filesystem::new(
            self.game_id.as_ref(),
            self.author.as_ref(),
            &self.resources_dir_name,
            &self.resources_zip_name,
        )?;

        for path in &self.paths {
            fs.mount(path, true);
        }

        for zipfile_bytes in self.memory_zip_files {
            fs.add_zip_file(std::io::Cursor::new(zipfile_bytes))?;
        }

        let config = if self.load_conf_file {
            fs.read_config().unwrap_or(self.conf)
        } else {
            self.conf
        };

        from_conf(self.game_id, config, fs)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        context::{Has, HasMut},
        graphics::GraphicsContext,
        ContextBuilder,
    };

    // This will fail when testing if not running using one thread but is actually fine
    #[test]
    fn has_traits() {
        let (mut ctx, _event_loop) = ContextBuilder::new("test", "ggez").build().unwrap();

        fn takes_gfx(_gfx: &impl Has<GraphicsContext>) {}
        takes_gfx(&ctx);
        takes_gfx(&ctx.gfx);

        fn takes_mut_gfx(_gfx: &mut impl HasMut<GraphicsContext>) {}
        takes_mut_gfx(&mut ctx);
        takes_mut_gfx(&mut ctx.gfx);
    }
}
