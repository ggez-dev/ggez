//! Timing and measurement functions.
//!
//! ggez does not try to do any framerate limitation by default. If
//! you want to run at anything other than full-bore max speed all the
//! time, call [`thread::yield_now()`](https://doc.rust-lang.org/std/thread/fn.yield_now.html)
//! (or [`timer::yield_now()`](fn.yield_now.html) which does the same
//! thing) to yield to the OS so it has a chance to breathe before continuing
//! with your game. This should prevent it from using 100% CPU as much unless it
//! really needs to.  Enabling vsync by setting
//! [`conf.window_setup.vsync`](../conf/struct.WindowSetup.html#structfield.vsync)
//! in your [`Conf`](../conf/struct.Conf.html) object is generally the best
//! way to cap your displayed framerate.
//!
//! For a more detailed tutorial in how to handle frame timings in games,
//! see <https://gafferongames.com/post/fix_your_timestep/>

use std::{cmp, convert::TryFrom, f64, thread, time};

use crate::Context;

/// A simple buffer that fills
/// up to a limit and then holds the last
/// N items that have been inserted into it,
/// overwriting old ones in a round-robin fashion.
///
/// It's not quite a ring buffer 'cause you can't
/// remove items from it, it just holds the last N
/// things.
#[derive(Debug, Clone)]
struct LogBuffer<T>
where
    T: Clone,
{
    head: usize,
    size: usize,
    /// The number of actual samples inserted, used for
    /// smarter averaging.
    samples: usize,
    contents: Vec<T>,
}

impl<T> LogBuffer<T>
where
    T: Clone + Copy,
{
    fn new(size: usize, init_val: T) -> LogBuffer<T> {
        LogBuffer {
            head: 0,
            size,
            contents: vec![init_val; size],
            // Never divide by 0
            samples: 1,
        }
    }

    /// Pushes a new item into the `LogBuffer`, overwriting
    /// the oldest item in it.
    fn push(&mut self, item: T) {
        self.head = (self.head + 1) % self.contents.len();
        self.contents[self.head] = item;
        self.size = cmp::min(self.size + 1, self.contents.len());
        self.samples += 1;
    }

    /// Returns a slice pointing at the contents of the buffer.
    /// They are in *no particular order*, and if not all the
    /// slots are filled, the empty slots will be present but
    /// contain the initial value given to [`new()`](#method.new).
    ///
    /// We're only using this to log FPS for a short time,
    /// so we don't care for the second or so when it's inaccurate.
    fn contents(&self) -> &[T] {
        if self.samples > self.size {
            &self.contents
        } else {
            &self.contents[..self.samples]
        }
    }

    /// Returns the most recent value in the buffer.
    fn latest(&self) -> T {
        self.contents[self.head]
    }
}

/// A structure that contains our time-tracking state.
#[derive(Debug)]
pub struct TimeContext {
    init_instant: time::Instant,
    last_instant: time::Instant,
    frame_durations: LogBuffer<time::Duration>,
    residual_update_dt: time::Duration,
    frame_count: usize,
}

/// How many frames we log update times for.
const TIME_LOG_FRAMES: usize = 200;

impl TimeContext {
    /// Creates a new `TimeContext` and initializes the start to this instant.
    pub fn new() -> TimeContext {
        let initial_dt = time::Duration::from_millis(16);
        TimeContext {
            init_instant: time::Instant::now(),
            last_instant: time::Instant::now(),
            frame_durations: LogBuffer::new(TIME_LOG_FRAMES, initial_dt),
            residual_update_dt: time::Duration::from_secs(0),
            frame_count: 0,
        }
    }

    /// Get the time between the start of the last frame and the current one;
    /// in other words, the length of the last frame.
    pub fn delta(&self) -> time::Duration {
        self.frame_durations.latest()
    }

    /// Gets the average time of a frame, averaged
    /// over the last 200 frames.
    pub fn average_delta(&self) -> time::Duration {
        let sum: time::Duration = self.frame_durations.contents().iter().sum();

        // If our buffer is actually full, divide by its size.
        // Otherwise divide by the number of samples we've added
        // Both these unwraps should be fine as frame_durations should always fit in a u32
        if self.frame_durations.samples > self.frame_durations.size {
            sum / u32::try_from(self.frame_durations.size).unwrap()
        } else {
            sum / u32::try_from(self.frame_durations.samples).unwrap()
        }
    }

    /// Gets the FPS of the game, averaged over the last
    /// 200 frames.
    pub fn fps(&self) -> f64 {
        let duration_per_frame = self.average_delta();
        let seconds_per_frame = duration_per_frame.as_secs_f64();
        1.0 / seconds_per_frame
    }

    /// Gets the number of times the game has gone through its event loop.
    ///
    /// Specifically, the number of times that [`TimeContext::tick()`](struct.TimeContext.html#method.tick)
    /// has been called by it.
    pub fn ticks(&self) -> usize {
        self.frame_count
    }

    /// Returns the time since the game was initialized,
    /// as reported by the system clock.
    pub fn time_since_start(&self) -> time::Duration {
        self.init_instant.elapsed()
    }

    /// Check whether or not the desired amount of time has elapsed
    /// since the last frame.
    ///
    /// The intention is to use this in your `update` call to control
    /// how often game logic is updated per frame (see [the astroblasto example](https://github.com/ggez/ggez/blob/30ea4a4ead67557d2ebb39550e17339323fc9c58/examples/05_astroblasto.rs#L438-L442)).
    ///
    /// Calling this decreases a timer inside the context if the function returns true.
    /// If called in a loop it may therefore return true once, twice or not at all, depending on
    /// how much time elapsed since the last frame.
    ///
    /// For more info on the idea behind this see <https://gafferongames.com/post/fix_your_timestep/>.
    ///
    /// Due to the global nature of this timer it's desirable to only use this function at one point
    /// of your code. If you want to limit the frame rate in both game logic and drawing consider writing
    /// your own event loop, or using a dirty bit for when to redraw graphics, which is set whenever the game
    /// logic runs.
    pub fn check_update_time(&mut self, target_fps: u32) -> bool {
        let target_dt = fps_as_duration(target_fps);
        if self.residual_update_dt > target_dt {
            self.residual_update_dt -= target_dt;
            true
        } else {
            false
        }
    }

    /// Returns the fractional amount of a frame not consumed
    /// by  [`check_update_time()`](fn.check_update_time.html).
    /// For example, if the desired
    /// update frame time is 40 ms (25 fps), and 45 ms have
    /// passed since the last frame, [`check_update_time()`](fn.check_update_time.html)
    /// will return `true` and `remaining_update_time()` will
    /// return 5 ms -- the amount of time "overflowing" from one
    /// frame to the next.
    ///
    /// The intention is for it to be called in your
    /// [`draw()`](../event/trait.EventHandler.html#tymethod.draw) callback
    /// to interpolate physics states for smooth rendering.
    /// (see <https://gafferongames.com/post/fix_your_timestep/>)
    pub fn remaining_update_time(&self) -> time::Duration {
        self.residual_update_dt
    }

    /// Update the state of the `TimeContext` to record that
    /// another frame has taken place.  Necessary for the FPS
    /// tracking and [`check_update_time()`](fn.check_update_time.html)
    /// functions to work.
    ///
    /// It's usually not necessary to call this function yourself,
    /// [`event::run()`](../event/fn.run.html) will do it for you.
    /// You only need to call this function if you're writing your
    /// own custom event loop.
    pub fn tick(&mut self) {
        let now = time::Instant::now();
        let time_since_last = now - self.last_instant;
        self.frame_durations.push(time_since_last);
        self.last_instant = now;
        self.frame_count += 1;

        self.residual_update_dt += time_since_last;
    }
}

impl Default for TimeContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the time between the start of the last frame and the current one;
/// in other words, the length of the last frame.
#[deprecated(note = "Use `ctx.time.delta` instead")]
pub fn delta(ctx: &Context) -> time::Duration {
    ctx.time.delta()
}

/// Gets the average time of a frame, averaged
/// over the last 200 frames.
#[deprecated(note = "Use `ctx.time.average_delta` instead")]
pub fn average_delta(ctx: &Context) -> time::Duration {
    ctx.time.average_delta()
}

/// Returns a `Duration` representing how long each
/// frame should be to match the given fps.
///
/// Approximately.
fn fps_as_duration(fps: u32) -> time::Duration {
    let target_dt_seconds = 1.0 / f64::from(fps);
    time::Duration::from_secs_f64(target_dt_seconds)
}

/// Gets the FPS of the game, averaged over the last
/// 200 frames.
#[deprecated(note = "Use `ctx.time.fps` instead")]
pub fn fps(ctx: &Context) -> f64 {
    ctx.time.fps()
}

/// Returns the time since the game was initialized,
/// as reported by the system clock.
#[deprecated(note = "Use `ctx.time.time_since_start` instead")]
pub fn time_since_start(ctx: &Context) -> time::Duration {
    let tc = &ctx.time;
    tc.init_instant.elapsed()
}

/// Check whether or not the desired amount of time has elapsed
/// since the last frame.
///
/// The intention is to use this in your `update` call to control
/// how often game logic is updated per frame (see [the astroblasto example](https://github.com/ggez/ggez/blob/30ea4a4ead67557d2ebb39550e17339323fc9c58/examples/05_astroblasto.rs#L438-L442)).
///
/// Calling this decreases a timer inside the context if the function returns true.
/// If called in a loop it may therefore return true once, twice or not at all, depending on
/// how much time elapsed since the last frame.
///
/// For more info on the idea behind this see <https://gafferongames.com/post/fix_your_timestep/>.
///
/// Due to the global nature of this timer it's desirable to only use this function at one point
/// of your code. If you want to limit the frame rate in both game logic and drawing consider writing
/// your own event loop, or using a dirty bit for when to redraw graphics, which is set whenever the game
/// logic runs.
#[deprecated(note = "Use `ctx.time.check_update_time` instead")]
pub fn check_update_time(ctx: &mut Context, target_fps: u32) -> bool {
    let timedata = &mut ctx.time;

    let target_dt = fps_as_duration(target_fps);
    if timedata.residual_update_dt > target_dt {
        timedata.residual_update_dt -= target_dt;
        true
    } else {
        false
    }
}

/// Returns the fractional amount of a frame not consumed
/// by  [`check_update_time()`](fn.check_update_time.html).
/// For example, if the desired
/// update frame time is 40 ms (25 fps), and 45 ms have
/// passed since the last frame, [`check_update_time()`](fn.check_update_time.html)
/// will return `true` and `remaining_update_time()` will
/// return 5 ms -- the amount of time "overflowing" from one
/// frame to the next.
///
/// The intention is for it to be called in your
/// [`draw()`](../event/trait.EventHandler.html#tymethod.draw) callback
/// to interpolate physics states for smooth rendering.
/// (see <https://gafferongames.com/post/fix_your_timestep/>)
#[deprecated(note = "Use `ctx.time.remaining_update_time` instead")]
pub fn remaining_update_time(ctx: &Context) -> time::Duration {
    ctx.time.residual_update_dt
}

/// Pauses the current thread for the target duration.
/// Just calls [`std::thread::sleep()`](https://doc.rust-lang.org/std/thread/fn.sleep.html)
/// so it's as accurate as that is (which is usually not very).
pub fn sleep(duration: time::Duration) {
    thread::sleep(duration);
}

/// Yields the current timeslice to the OS.
///
/// This just calls [`std::thread::yield_now()`](https://doc.rust-lang.org/std/thread/fn.yield_now.html)
/// but it's handy to have here.
pub fn yield_now() {
    thread::yield_now();
}

/// Gets the number of times the game has gone through its event loop.
///
/// Specifically, the number of times that [`TimeContext::tick()`](struct.TimeContext.html#method.tick)
/// has been called by it.
#[deprecated(note = "Use `ctx.time.ticks` instead")]
pub fn ticks(ctx: &Context) -> usize {
    ctx.time.frame_count
}
