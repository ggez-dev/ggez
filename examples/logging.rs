//! This example shows two ways to set up logging in apps, using `log` crate macros with `fern`
//! frontend, to display neatly formatted console output and write same output to a file.
//!
//! Output in question is a trace of app's initialization, and keyboard presses when it's running.
//!
//! `fern` provides a way to write log output to a `std::sync::mpsc::Sender`, so we can use a
//! matching `std::sync::mpsc::Receiver` to get formatted log strings for file output.

extern crate chrono;
extern crate fern;
extern crate ggez;
#[macro_use]
extern crate log;

use ggez::{Context, ContextBuilder, GameResult};
use ggez::conf::{WindowMode, WindowSetup};
use ggez::event::{EventHandler, Keycode, Mod};
use ggez::filesystem::{File};
use ggez::graphics;
use ggez::timer;
use std::io::Write;
use std::path;
use std::sync::mpsc;

/// A basic file writer.
/// Hogs it's log file until dropped, writes to it whenever `update()` is called.
struct FileLogger {
    /// `ggez`' virtual file representation to write log messages to.
    file: File,
    /// Channel to get log messages from.
    receiver: mpsc::Receiver<String>,
}

impl FileLogger {
    /// Initializes a file writer. Needs an initialized `ggez::Context`, to use it's filesystem.
    fn new(
        ctx: &mut Context,
        path: &str,
        receiver: mpsc::Receiver<String>,
    ) -> GameResult<FileLogger> {
        // This (re)creates a file and opens it for appending.
        let file = ctx.filesystem.create(path::Path::new(path))?;
        debug!("Created log file {:?} in {:?}", path, ctx.filesystem.get_user_config_dir());
        Ok(FileLogger { file, receiver })
    }

    /// Reads pending messages from the channel and writes them to the file.
    /// Intended to be called in `EventHandler::update()`, to avoid using threads.
    /// (which you totally shouldn't actively avoid, Rust is perfect for concurrency)
    fn update(&mut self) -> GameResult<()> {
        // try_recv() doesn't block, it returns Err if there's no message to pop.
        while let Ok(msg) = self.receiver.try_recv() {
            // std::io::Write::write_all() takes a byte array.
            self.file.write_all(msg.as_bytes())?;
        }
        Ok(())
    }
}

/// Main state struct. In an actual application, this is where your asset handles, etc go.
struct App {
    /// Owned FileLogger instance; there are multiple ways of going about this, but since we
    /// are not interested in logging to a file anything that happens while app
    /// logic isn't running, this will do.
    file_logger: FileLogger,
}

impl App {
    /// Creates an instance, takes ownership of passed FileLogger.
    fn new(_ctx: &mut Context, logger: FileLogger) -> GameResult<App> {
        Ok(App { file_logger: logger })
    }
}

/// Where the app meets the `ggez`.
impl EventHandler for App {
    /// This is where the logic should happen.
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        const DESIRED_FPS: u32 = 60;
        // This tries to throttle updates to desired value.
        while timer::check_update_time(ctx, DESIRED_FPS) {
            // Since we don't have any non-callback logic, all we do is append our logs.
            self.file_logger.update()?;
        }
        Ok(())
    }

    /// Draws the screen. We don't really have anything to draw.
    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::clear(ctx);
        graphics::present(ctx);
        timer::yield_now();
        Ok(())
    }

    /// Called when `ggez` catches a keyboard key being pressed.
    fn key_down_event(&mut self, ctx: &mut Context, keycode: Keycode, keymod: Mod, repeat: bool) {
        // Log the keypress to info channel!
        info!("Key down event: {}, modifiers: {:?}, repeat: {}", keycode, keymod, repeat);
        if keycode == Keycode::Escape {
            // Escape key closes the app.
            if let Err(e) = ctx.quit() {
                error!("Context::quit() failed, somehow: {}", e);
            }
        }
    }

    /// Called when window is resized.
    fn resize_event(&mut self, ctx: &mut Context, width: u32, height: u32) {
        match graphics::set_screen_coordinates(
            ctx,
            graphics::Rect::new(0.0, 0.0, width as f32, height as f32),
        ) {
            Ok(()) => info!("Resized window to {} x {}", width, height),
            Err(e) => error!("Couldn't resize window: {}", e),
        }
    }
}

pub fn main() {
    // This creates a channel that can be used to asynchronously pass things between parts of the
    // app. There's some overhead, so using it somewhere that doesn't need async (read: threads)
    // is suboptimal. But, `fern`'s arbitrary logging requires a channel.
    let (log_tx, log_rx) = mpsc::channel();

    // `log` is not initialized yet.
    debug!("I will not be logged!");

    // This sets up a `fern` logger and initializes `log`.
    fern::Dispatch::new()
        // Formats logs.
        .format(|out, msg, rec| {
            out.finish(format_args!(
                "[{}][{}][{}] {}",
                chrono::Local::now().format("%H:%M:%S"),
                rec.target(),
                rec.level(),
                msg,
            ))
        })
        // Sets global log level.
        .level(log::LevelFilter::Trace)
        // `gfx_device_gl` is very verbose, on info channel too, so let's filter most of that out.
        .level_for("gfx_device_gl", log::LevelFilter::Warn)
        // Hooks up console output.
        .chain(std::io::stdout())
        // Hooks up the channel.
        .chain(log_tx)
        .apply()
        .unwrap();

    // Note, even though our file logger hasn't been initialized in any way yet, logs starting
    // from here will still appear in the log file.
    debug!("I am logged!");

    trace!("Creating ggez context.");

    // This sets up `ggez` guts (including filesystem) and creates a window.
    let ctx = &mut ContextBuilder::new("logging", "ggez")
        .window_setup(
            WindowSetup::default()
                .title("Pretty console output!")
                .resizable(true),
        )
        .window_mode(WindowMode::default().dimensions(640, 480))
        .build()
        .unwrap();

    trace!("Context created, creating a file logger.");

    let file_logger = FileLogger::new(ctx, "/out.log", log_rx).unwrap();

    trace!("File logger created, starting loop.");

    // Creates our state, and starts `ggez`' loop.
    match App::new(ctx, file_logger) {
        Err(e) => {
            error!("Could not initialize: {}", e);
        }
        Ok(ref mut app) => {
            match ggez::event::run(ctx, app) {
                Err(e) => {
                    error!("Error occurred: {}", e);
                }
                Ok(_) => {
                    debug!("Exited cleanly.");
                }
            }
        }
    }

    trace!("Since file logger is dropped with App, this line will cause an error in fern!");
}
