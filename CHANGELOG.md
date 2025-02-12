# 0.10.0

## Added

You are now able to add your own custom contexts! Using the new `ContextFields` type and implementing `HasMut<KeyboardContext>, HasMut<MouseContext>, HasMut<TimeContext>, HasMut<GraphicsContext>, and HasMut<GamepadContext>` if gamepad is enabled

### Coroutines

You can create a new coroutine by doing
```rust
slow_coroutine: Coroutine::new(async move {
    // wait 100 frames
    for _ in 0..100 {
        yield_now().await
    }

    String::from("I came from a coroutine!")
})
```
and then in the update handler you can poll it and wait for the value
```rust
fn update(&mut self, _ctx: &mut Context) -> GameResult {
    if let Some(val) = self.slow_coroutine.poll() {
        println!("Coroutine says: \"{val}\"");
    }
    Ok(())
}
```
This will print `Coroutine says: "I came from a coroutine!"` after 100 frames.

### 3d

ggez is capable of basic 3d rendering. The APIs and types differ from 2d in the following ways.
`Canvas3d` and `Drawable3d` are the 3d equivalents of Canvas and Drawable. A simple example of drawing might look like this:
```rust
fn draw(&mut self, ctx: &mut Context) -> GameResult {
  let mut canvas3d = Canvas3d::from_frame(ctx, Color::BLACK);
  canvas3d.set_projection(self.camera.calc_matrix());
  canvas3d.draw(&drawable, draw_param);
  canvas3d.finish(ctx)?;
}
```

Some types that implement `Drawable3d` by default are `Mesh3d` and `Model`. `Mesh3d` is used for a singular mesh.
`Model` is used for more complicated objects that may be made up of multiple meshes. `Model` also provides a way to load basic gltf/glb files and obj files.
(Note: not all of gltf is supported. obj models must be triangulated in your software of choice, and no material files will be read).

There is a huge list of gltf features right now we only support the very basics of loading mesh data with any transforms applied directly to the vertices.
Here are some limitations:

Gltf Caveats:
Complicated scenes are loaded as a singular `Model` even if there is multiple nodes in the gltf scene. It will load all of them but just as a `Model`
- No animation support
- No PBR
- No tangents
- And more

Caveats:
This is a new system so not everything is 100% complete. The base features needed to make games are available, but there is more coming soon:

- Custom Vertex Formats
- More examples on how to do common 3d task.
- API may change 

If you need advanced fast 3d rendering out of the box this isn't for you. We don't have plans to implement any fancy rendering features, that is up to you (or 3rd party crates).

ggez provides a simple 3d API that is good enough for simple to medium sized games. Keep in mind that larger games will require extra work, compared to e.g. godot or bevy.

### New event / Mouse data

`MouseContext` now has a function called raw_delta() this returns device motion, rather than cursor motion. This is great for implementing controls that aren't cursor based, such as a 3d camera.
A new event callback exists for this called `raw_mouse_motion_event()`

### Shader

`Shader::vs_module` and `Shader::fs_module` now exist to get the underlying wgpu shader modules.

### Examples

General examples:

- `coroutine.rs`: How to use Coroutines
- `cpu_image.rs`: Copies an image to the cpu and back to the gpu


3d related examples:

- `3d.rs`: Various 3d related things
- `3dtexture.rs`: Renders a 3d scene to an image that can be used on another 3d object, or rendered to the screen
- `3dshapes.rs`: Simple primitives supported by the new `Mesh3dBuilder`
- `3dinstance.rs`: Shows off how to use instancing in 3d

## Changed

- `EventHandler` now takes another generic in form of whatever context implementation you are using whether that be the default one of a custom one
- Dependencies updates (including public ones)
- `InstanceArray::dimensions_meshed` has been renamed to `bounding_box` and no longer requires a context as parameter

## Removed
- `Drawable::dimensions`, as its usage wasn't clear enough and it was not used internally either. For meshes, use `Mesh::bounding_box()` instead, and for texts use `Text::measure()`.

## Fixed
- `Image::to_pixels` no longer crashes and works properly
- `Image` can now be loaded from jpg/jpeg files 

# 0.9.3

## Fixed
- `Image` can now be loaded from jpg/jpeg files
- Fixed memory leak caused by resizing

# 0.9.2

## Fixed
- Fixed memory leak caused by previous `Image` fix.
- Fixed memory leak caused by `Text` rendering and creation

# 0.9.1

## Fixed
- Fixed memory leak caused by drawing `Image`

# 0.9.0

## Added

None, just minor but breaking changes.

## Changed

`Image::new_blank_canvas_image` has been renamed to `Image::from_color`

`InstanceArray::resize` takes a `usize` instead of a `u32`.

`Canvas::set_shader`, `Canvas::set_shader_params`, and `Canvas::set_text_shader_params` now take a reference.

`ShaderParams::set_uniforms` now takes self and ctx mutably.

`ShaderParams::set_uniforms` and `ShaderParamsBuilder::build` now take the full context.

`ShaderBuilder::new_wgsl` is now called `ShaderBuilder::new`.

`ShaderBuilder::combined_code` and `ShaderBuilder::combined_path` have become associated functions called `ShaderBuilder::from_code` and `ShaderBuilder::from_path` respectively.

Some `ContextBuilder` options now take paths instead of strings.

## Removed

`ShaderParams` no longer implements Clone, PartialEq, Eq, or ToOwned.

`Image::from_solid` has been removed use `Image::from_color`

## Fixed

Things not rendering.

# 0.8.1

## Fixed
- Fixed broken `InstanceArray::resize` and improved its documentation

# 0.8.0 (wgpu)

The biggest change in this version is the long awaited redo of our graphics stack, which used to be based on `gfx-rs`
and is now using `wgpu`. This gives us more reliability going into the future and fixes many bugs, albeit costing us
some portability to low-level hardware (looking at you Pi 3; EDIT: and... [Pi 4 as well?](https://github.com/ggez/ggez/issues/1093) o_o ).

Credit goes out to our wonderful contributors, with special thanks to [@jazzfool](https://github.com/jazzfool)
and [@aleokdev](https://github.com/aleokdev), for putting so much work and patience into the graphics stack.

As there are too many changes to simply list them in the usual fashion, let's look at them topic by topic:

## Changes in the graphics API

With the redo of the graphics stack some parts of the API changes with it, most notably canvases and the shader API.

### Canvas

First of all, each draw call is now explicitly bound to a `Canvas`. This means instead of "setting" the active canvas and
then drawing implicitly on that canvas you now call `canvas.draw(...)` or `drawable.draw(canvas, ...)`. And then, once
you're done drawing on it, you call `canvas.finish(ctx)`.
This helps to keep track of the active canvas and gives you more explicit renderpasses to work with, as `Canvas` is now
no longer a special image that you can draw to, but a wrapped `wgpu` renderpass, operating naturally on whatever image
you pass it, or on the screen buffer itself.

The downside of this is that it's a bit more verbose and that you have to pass around your canvas to be able to draw.

### Shader

There's a new struct `ShaderParams`allowing you to pass images, samplers and uniforms to shaders.
Both `ShaderParam`s and `Shader`s are now set per `Canvas` (as well as blend modes and projection matrices).

`Shader`s and `ShaderParam`s are created through `ShaderBuilder` and `ShaderParamBuilder` respectively, allowing you to
only set the parameters you're interested in, without worrying about the rest.

Uniforms are now no longer created using the `gfx!` macro. No need to include `gfx-rs` in your own project, just to be
able to create shader uniforms. Now, simply deriving `AsStd140` is all you usually need (see the shader examples).
At the time of writing you're sadly also required to depend on `crevice 0.11` directly, as `AsStd140` needs to have it
visible globally (and re-exporting it on our side doesn't seem to be enough). If you know a way around this, let us know!

### InstanceArray

`SpriteBatch` and `MeshBatch` have been replaced by `InstanceArray`, a more generic "batch" that also features internal z-ordering.

### Z-Order

Before, the order of draws had been determined solely by order of execution. Now `DrawParam` features an additional
field `z`, to give you control over the order in which draw calls are placed. This works on the global level, but also
inside of `InstanceArray`, when requested.

## Sub-contexts

Another field that has seen a bit of love is the modularization of contexts. Sub-contexts are now public and can be borrowed
and handed around freely. Most module functions used to require `Context` as a whole. These have, for this reason, now been
deprecated and directly replaced by methods on the sub-contexts.

In situations where multiple sub-contexts are needed (one is the creation of audio sources and one the creation of images
from paths) you can pass the necessary sub-contexts, or instead just pass `Context` as a whole, just like before, thanks to a little trait-workaround.

The latter applies to all situations in which you'd need one specific sub-context as well.
If you, for some reason, needed or wanted to split the context, then you can pass only the required sub-context.
If you didn't split it then you can comfortably hand around and pass the context as a whole, like before.

## Added

* Added touch event to `EventHandler`
* Added access to scancodes in both keyboard events and keyboard context methods, allowing you to make your game
 portable across the different keyboard letter layouts of different countries
* Added `Canvas::set_scissor_rect` allowing you to restrict drawing to a part of your surface
* Added `is_key_just_pressed` and `is_key_just_released` to keyboard context
* Added an option for transparent windows
* Added the ability to build your own `BlendMode`s built from the components offered through wgpu's `BlendComponent` struct
* Exposed rodio API for skipping the first part of a sample
* Added `audio` and `gamepad` as crate features, allowing you to disable them if not necessary
* Added the `zip-compression` feature (as part of the default features), now allowing the use of zip-files with compression
* Added `Rect::overlaps_circle`
* Added `Context::request_quit` as a replacement for `event::quit`
  * `Context::request_quit` works like `event::quit` did before, except that instead of directly breaking the game loop it
  now triggers a `quit_event`, which allows you to handle all attempts to quit the game in one place.
* Added a re-export for `glam`, as ggez is aimed at beginners for whom it's convenient to just have it at hand directly; most people will want/need to use it anyway
* Added `logical_size` as optional argument in `WindowMode` which overrides width/height with a `LogicalSize` which supports high DPI systems.

## Changed

The following list doesn't repeat the changes already mentioned above.

* Relaxed the error type of `EventHandler` from `std::error::Error` into `std::fmt::Debug`, allowing you to use
 things like `anyhow::error` as error types as well
* Made offset on `Text` relative (I know, I know, we've been changing this around a lot lately, but I hope we're finally
  done now), as it makes things like centering text on positions easier (see the blend modes example)
* Also `Text` is now a first class citizen and can be drawn normally with `DrawParam`, implementing things like rotation
 that weren't possible in batched text rendering before
* Changed how bounds on Text work as well as layouting
  * `Text::set_bounds` now expects width and height of the bounds, but not the destination point, as that's handled through the `DrawParam`
  * additionally to horizontal alignment vertical alignment is now possible as well
* Improved `Text` performance through better glyph re-use
* Changed the `Drawable` trait; this will downstream require changes in projects like `ggez-egui`
* Version bumped `zip` to 0.6, `directories` to 4.0.1, `winit` to 0.27.3, image to `0.24` and `rodio` to 0.16
* As each `Canvas` now keeps track of its own projection matrix the `screen_coordinates` of each `Canvas` now start out
 with the same dimensions as the `Canvas` surface

## Deprecated

* Most of the module level functions, which have been  replaced by sub-context methods

## Removed

* Removed `duration_to_f64` and `f64_to_duration` as the std library now already contains this
 functionality itself
* Removed `From<tuple>` implementations for `DrawParam`, as they're non-transparent and weird
* Removed `event::quit`, as it was replaced by `Context::request_quit`
* Removed the ability to update only parts of the `DrawParams` inside a `MeshBatch` (now `InstanceArray`)
  * If you want that ability back let us know! Atm it's staged as "maybe in `0.8.1`"

## Fixed
Many graphics bugs that were caused by the use of the discontinued `gfx-rs` were fixed by the switch to `wgpu`. The
following list is very probably not complete.

* Multisampling on canvases is now no longer based on dirty workarounds, but on the inner workings of `wgpu`,
 supporting it naturally
* Fixed zip `read_dir` not working deeper than one level on Windows
* Fixed a memory leak on `set_screen_coordinates` on Windows 11
* Fixed not being able to take screenshots of anti-aliased targets

# 0.7.0

## Added

* Added `filesystem::zip_dir`
* Expanded/improved documentation

## Changed

* Switched `DrawParam::offset` behavior back to how it was pre-ggez 0.6; more details in the [FAQ](https://github.com/ggez/ggez/blob/devel/docs/FAQ.md#offsets)
* Moved some generic functionality from `Image` to `ImageGeneric` and from `Canvas` to `CanvasGeneric`
* Also moved some `Canvas` specific functionality from `CanvasGeneric` to `Canvas`
* Made `GameError` the implicit default error type for the event handler
* Made `TextFragment` functions take `Into<T>` for better usability
* Changed Rust edition to 2021
* Version bumped `bytemuck` to 1.7
* Version bumped `glam` to 0.20

## Deprecated

Nothing

## Removed

* Multi-sampled canvases (which didn't work at all before) can no longer be created when using the GLES backend.
  The reason for this is that we finally fixed them via a fragment shader workaround which isn't supported on GLES.

## Fixed

* Finally fixed/implemented MSAA on canvases. As `gfx` doesn't provide us with the necessary tools to do so directly,
  the implementation is internally based upon a fragment shader workaround, which doesn't work on GLES.
* Made sure that the bounding box of `Mesh` is actually updated when `Mesh::set_vertices` is called

## Broken

Nothing we're aware of yet

# 0.6.1

## Added

 * Allowed `ContextBuilder` to rename resources folder and resources.zip
 * Added `winit` re-export
 * Added `get_window_position`
 * Added an example showcasing animation using keyframe
 * Added support for the TGA image file format (and possibly some others by accident as well)
 * Added methods to access sprites inside of a `SpriteBatch` directly

## Changed

 * `MeshBatch::dimensions` now returns a rectangle containing all of its mesh instances
   (instead of simply returning the dimensions of the underlying single mesh, as before)
    * Drawing a `MeshBatch` with an offset != (0,0) results in such dimensions being calculated (just like in `SpriteBatch`),
      which can be expensive, but leads to the offset being interpreted as a relative offset, instead of an absolute one
 * Changed mouse move callback a little: it now returns the difference in movement relative to the last callback,
   not the mouse position at the end of the previous frame
 * Most of the filesystem functions now take `&Context` instead of a mutable one
 * Version bumped `old_school_gfx_glutin_ext` to 0.27
 * Version bumped `glutin` to 0.27
 * Version bumped `winit` to 0.25
 * Version bumped `glam` to 0.17.3

## Deprecated

Nothing

## Removed

Nothing

## Fixed

 * fixed color transformation from linear color back to sRGB
 * internal folder structure of the resources.zip file is now resolved correctly on Windows
 * fixed `mouse::delta`: it now actually returns the difference in mouse position relative to the previous frame
   instead of the raw mouse feedback it returned until now

## Broken

 * bumping our dependencies on a patch release is technically a breaking change, sry for that

# 0.6.0

## Added

 * Added `MeshBatch`
 * Added a `Premultiplied` blend mode, [which greatly improves `Canvas` usability](https://github.com/ggez/ggez/issues/301#issuecomment-854603057)
 * Added a `CustomError` variant to `GameError`.
 * Added function to allow custom gamepad definitions
 * Added function to fetch raw window
 * Added function to set window position on the screen
 * Added function to get supported resolutions of the current monitor
 * Added generators for rounded rectangle meshes
 * Tried to make more error types conveniently comply with
   `std::error::Error`
 * Added functions to fetch positions of text glyphs
 * Added `visible` to `WindowMode` to allow ggez to run without a visible window
 * Added `on_error` function to `EventHandler`, making error handling more convenient
 * Added a download buffer handle to the gfx context, to avoid possibly recreating it all the time,
   which means things like taking multiple screenshots should work more smoothly now, as long as the target size doesn't change

## Changed

 * `EventHandler` now takes an error type as a parameter, which allows you to use your own error types
 * `FullscreenType::True` now causes the game to be rendered exclusively on the current monitor, which also allows
   to set different resolutions
 * Changed blend modes massively in the hope that they're either more "correct" or helpful now
 * Changed the way `SpriteBatch` reacts to `DrawParam`s with an offset != (0,0): It now calculates its own dimensions
   (a rectangle containing all sprites) and interprets the offset as a fraction of that
 * Switched `rand` in the examples to `oorandom`, for basically
   aesthetic reasons.  (Not advertising at all, honest.)
 * Version bumped `rodio` to 0.13
 * Version bumped `lyon` to 0.16
 * Version bumped `glyph_brush` to 0.7
 * Version bumped `winit` to 0.23, which brings many fixes, better
   Wayland handling, and a slightly different style of event loop
   with some different constraints and type names.
 * `winit` update has also changed and smoothed over some of the issues
   with high-DPI handling.
 * Updated just about every other dependency under the sun
 * Minimum rustc version is now 1.42
 * Audio API in general changed a little for `rodio` API differences.

## Deprecated

Nothing

## Removed

 * removed `ggez::nalgebra` crate re-export.  All math in the public API
   should now be `mint` types, and it is a bug if they are not.

## Fixed

 * Fixed a mistake in the matrices created from `DrawParams` leading to them being slightly wrong when an offset was used
   (this might fix a lot of very subtle rendering bugs)
 * ggez no longer creates empty directories (for resources and other things), unless necessary
 * Setting `DrawParam`s now results in consistent behaviour <del>everywhere</del> (ok, no, we missed `MeshBatch`,
   which received this fix in 0.6.1), including `SpriteBatch` and `Canvas`
 * Fixed a memory leak in `screenshot` and `to_rgba8`
 * Fixed `transfrom_rect` (and added some more tests for it)
 * Too many things to count

## Broken

Nothing (yet)


# 0.5.1

## Added

Nothing

## Changed

 * version bumped `image`
 * Tiny doc cleanups and futzing around with readme


## Deprecated

Nothing

## Removed

Nothing

## Fixed

Nothing

## Broken

Nothing

# 0.5.0

## Added

 * Added line cap and join options
 * Added spatial sources for audio
 * Added `From` implementations for `Color` to convert from various tuples of `f32`'s.  Redundant but it annoyed me they don't exist.
 * Add OpenGL ES 3.0 support
 * Add optional textures to `Mesh`es.
 * Added lots of tests and doctests.
 * Added a `c_dependencies` feature.  It's on by default, but
   disabling it will build ggez without unnecessary C dependencies
   (currently `bzip2` and `minimp3`). [#549](https://github.com/ggez/ggez/issues/549)
 * Added (basic) spatial sound support.
 * Added loading of resource zip files from in-memory bytes

## Changed

 * Updated versions of lots of dependencies.
 * Minimum rustc version is now 1.33, rust 2018 edition.
 * We now use `winit` instead of `sdl2` for window creation and events!  This removes the last major C dependency from ggez.  It also involves lots of minor changes, the full extent of which is still going to evolve.
 * `DrawParam` now uses the builder pattern instead of being a bare struct, which allows easier conversion from generics (such as `mint` types) as well as simplifying the internal math.
 * All public-facing API's that take `Point2`, `Vector2` or `Matrix4` should now take
   `Into<mint::...>` for the appropriate type from the `mint` crate.  This should let users use
   whatever math library they care to that supports `mint`; currently `nalgebra`, `cgmath` and
   `euclid` are all options.
 * Moved all the `FilesystemContext` methods into top-level functions in the `filesystem` module,
   to be consistent with the rest of the API.
 * What used to be the `text_cached` module is now the `text` module, replacing all the old text stuff with cached text drawing using the `glyph_brush` crate.  This *dramatically* changes the text API, as well as being faster and more powerful.
 * Various dimension parameters have changed to fit the underlying implementations more closely.  `Image` dimensions have changed from `u32` to `u16`, which they always were but now it's exposed to the API.  Various screen size dimensions have changed from `u32` to `f64`, which allows `winit` to do smoother scaling.
 * Similarly, `Mesh`'s now have `u32` indices. [#574](https://github.com/ggez/ggez/issues/574)
 * Various getters have been renamed from `get_<field>()` to `<field>`(). Of particular note are changes to Drawable and ShaderHandle traits.
 * Some minor modularization has taken place; at least, gamepad and audio module scan be disabled with settings in your `conf.toml`.  Doing the same for filesystem, graphics, and input is a liiiiiittle more involved.
 * `MeshBuilder` `DrawMode`'s now can take parameters, and have some shortcut functions to make default parameters.  This simplifies things somewhat by not needing separate args to specify things like a stroke width for `DrawMode::Stroke`.
 * HiDPI support removed [since it doesn't do anything useful](https://github.com/rust-windowing/winit/issues/837#issuecomment-485864175). Any problems with your window not being the size you asked for are `winit`'s problem and will be solved once they fix it. [#587](https://github.com/ggez/ggez/issues/587)
 * Moved `ggez::quit()` to `ggez::event::quit()`.  [This commit](https://github.com/ggez/ggez/commit/66f21b3d03aea482001d60d23032354d7876446b)
 * Probably tons of other things I've forgotten.

## Deprecated

 * Nothing, it's a breaking change so things just got removed.

## Removed

 * Apple products are no longer officially supported.  They may work fine anyway, and I'll accept PR's for them, but handlin it all myself is too large an investment of time and energy.  Sorry.  :-(  [this commit](https://github.com/ggez/ggez/commit/2f02c72cf31401a1e6ab55edc745f6227c99fb67)
 * The foreground and background colors and associated functions have beeen removed; all colors are now specified purely where they are used for drawing.
 * Removed deprecated `BoundSpriteBatch` type.
 * Removed `Context::print_resource_stats()` in favor of `filesystem::print_all()`.
 * Removed `graphics::rectangle()` and friends in favor of just
   building and drawing the meshes explicitly.  Shortcut functions for
   this have been added to `Mesh`. [#466](https://github.com/ggez/ggez/issues/466)
 * Removed `TTFFont` font type in favor of `GlyphBrush`. [#132](https://github.com/ggez/ggez/issues/132)
 * Removed `Context::from_conf()` for `ContextBuilder` which is strictly more powerful.  [#429](https://github.com/ggez/ggez/issues/429)
 * Removed bitmap fonts; better support deserves to exist than what ggez currently provides, and there's no reason it can't be its own crate.
 * Removed the `cargo-resource-root` feature flag; just use `filesystem::mount()` instead or add the directories to your `ContextBuilder`.

## Fixed

 * Minor things beyond counting.  Don't worry, we added plenty of new
   bugs too.

## Broken

 * Does not work on Windows 7 or below, again due to `gilrs`.
   [#588](https://github.com/ggez/ggez/issues/588)

# 0.4.4

## Added

 * Added functions to get and set mouse cursor visibility.
 * Derived `PartialEq` for `Image` and `SpriteBatch`.

## Changed

Nothing

## Deprecated

Nothing

## Removed

Nothing

## Fixed

 * Myriad small documentation and example typos.
 * Fixed a rounding error in `Font::get_width()`.

# 0.4.3

## Added

 * Added a feature flag to build nalgebra with the `mint` math library inter-operability layer [#344](https://github.com/ggez/ggez/issues/344)
 * Updated `image` to 0.19 which lets us add another feature flag selecting whether or not to use multithreaded libraries when loading images.  [#377](https://github.com/ggez/ggez/issues/377)
 * We got more awesome logos!  Thanks ozkriff and termhn! [#327](https://github.com/ggez/ggez/issues/327)
 * Added hooks to the `log` crate, so we will now output some logging data via it that clients may use.  [#311](https://github.com/ggez/ggez/pull/331)
 * There's now a functional and reasonably ergonomic [game template](https://github.com/ggez/game-template) repo that demonstrates how to use `ggez` with `specs`, `warmy`, `failure`, `log` and other useful tools.
 * Added `Font::new_px()` and `Font::from_bytes_px()` functions to create fonts that are specific pixel sizes  [#268](https://github.com/ggez/ggez/issues/268)
 * Added Ratysz's glyph cache implementation integrating the awesome `gfx_glyph` crate!  This gives us faster text drawing as well as more features; if it works out well it should replace all text rendering in another version or two.  [#132](https://github.com/ggez/ggez/issues/132)

## Changed

 * Made it so that the configuration directories are only created on-demand, not whenever the Context is created: [#356](https://github.com/ggez/ggez/issues/356)
 * Updated rodio to 0.7, which fixes a sample rate bug on Linux: [#359](https://github.com/ggez/ggez/issues/359)
 * Documented which version of rustc we require, and added unit tests for that specific version: it is currently >=1.23.0,
   primarily driven by features required by dependencies.
 * Moved `Context::quit()` to `ggez::quit()` 'cause all our other non-object-related functions are functions, not methods.

## Deprecated

## Removed

## Fixed


# 0.4.2

## Added

 * Added a feature to enable or disable bzip2 zip file support
 * Lots of small documentation fixes and improvements thanks to lovely contributors
 * Added termhn's `ggez_snake` to the examples, 'cause it's awesome
 * Added `timer::get_remaining_update_time()` to let you easily do sub-frame timing for interpolation and such.
 * Many small improvements and cleanups

## Changed

 * Version bumped lots of dependencies: zip, rand, rodio, rusttype
 * Switched to the `app_dirs2` crate to avoid a bug in upcoming rustc change

## Deprecated

## Removed

## Fixed

 * Made `Image::from_rgba8` properly check that the array you pass it is the right size
 * Fixed more documentation bugs (https://github.com/ggez/ggez/issues/303).

# 0.4.1

## Added

 * Added `Text::into_inner()` and related methods to get ahold of a `Text` object's underlying `Image`
 * Added `SoundData::new()` and `Source::set_repeat()`/`Source::get_repeat()` (thanks jupart!)
 * Added `Context::process_event()` to smooth out a bump or two in the
   API for writing custom event loops.  This does change the API a little, but the old style should still work.
 * Added functions for taking screenshots and saving `Image`'s (thanks DenialAdams!)

## Changed

 * Version-bumped `lyon` crate

## Deprecated

 * Deprecated `BoundSpriteBatch`, since you can just clone an `Image`
   relatively cheaply.

## Removed

 * Nothing

## Fixed

 * Fixed bug in `mouse::get_position()`, see https://github.com/ggez/ggez/issues/283
 * Lots of small documentation fixes from a variety of awesome sharp-eyed contributors
 * Fixed bug that was making canvas's render upside-down https://github.com/ggez/ggez/issues/252

# 0.4.0

## Added

 * Added `mouse` module with some utility functions
 * Added some utility functions to query window size
 * Sprite batching implemented by termhn!
 * Added mesh builders allowing you to build complex meshes simply.
 * Integrated nalgebra to provide point and vector types.
 * Added MSAA, blend modes, other graphics toys (thanks termhn!)
 * Added graphics_settings example to show hot to play with graphics modes
 * Made the render pipeline just use matrices instead of separate transform elements
 * SHADERS!  Woo, thanks nlordell!
 * Added `Filesystem::mount()` function and made examples use it; they no longer need the `cargo-resource-root` feature
 * Added filesystem and graphics setting examples
 * Added more useful/informative constructors for `Color`
 * Added ability to select OpenGL version
 * Added some useful methods to `Rect`
 * Added a FAQ and some other documentation
 * Added a `ContextBuilder` type that allows finer control over creating a `Context`
 * Added an optional `color` value to `DrawParam`, which overrides the default foreground color.  Life would be simpler removing the foreground color entirely...

## Changed

 * First off, there will be some switches in process: We're going to make the master branch STABLE, tracking the latest release,
   and create a devel branch that new work will be pushed to.  That way people don't check out master and get some WIP stuff.
 * The coordinate system moved from origin-at-center, x-increasing-up to origin-at-top-left, x-increasing-down
 * Updated all dependencies to newer versions
 * Refactored EventHandler interface, again
 * Altered timestep functions to be nicer and made examples use them consistently
 * Updated to Lyon 0.8, which brings some bugfixes
 * Refactored Conf interface a little to separate "things that can be changed at runtime" from "things which must be specified at init time".

## Deprecated

## Removed

 * Removed `get_line_width()` and `set_line_width()` and made line widths parameters where necessary
 * Did the same for `get/set_point_size()`
 * Removed inaccurate `timer::sleep_until_next_frame()`, added `timer::yield_now()`.

## Fixed

 * Fixed some bugs with type visibility and directory paths.
 * Fixed a few smallish filesystem bugs
 * Got the 3D cube example working and shuffled around the gfx-rs interface methods a little, so we could make more of the graphics innards hidden while still exposing the useful bits.

# 0.3.4

 * Backported correction to SRGB color conversions
 * Added std::error::Error implementation for GameError

# 0.3.3

 * Documentation and unit test updates
 * Derive some common traits on types

# 0.3.2

 * Fixed bug in conf.toml reading and writing (thanks chinatsu)
 * Made filesystem.print_all() a little more informative
 * Added graphics::set_mode() function to allow setting window size, etc.
 * Added some functions to allow querying fullscreen modes and such
 * Made gamepad example test all input
 * Added bindings to the `mint` crate (a whole one type conversion)
 * Implemented stop() for audio

# 0.3.1

 * Fixed bug in when CARGO_MANIFEST_DIR is checked (thanks 17cupsofcoffee)
 * Added experimental support for SDL's gamepads (thanks kampffrosch94)
 * Re-improved resource-not-found error messages (thanks 17cupsofcoffee)
 * Fixed minor bug with text rendering alpha, added more useful methods to `Text`
 * Fixed bug with text wrapping (I hope)
 * VERY EXPERIMENTAL functions for exposing the gfx-rs rendering context to a bold user

# 0.3.0

 * Almost everything is now pure rust; the only C dependency is libsdl2.
 * Entirely new rendering engine using `gfx-rs` backed by OpenGL 3.2
 * New (if limited) 2D drawing primitives using `lyon`
 * Font rendering still uses `rusttype` but it's still cool
 * New option to enable/disable vsync
 * New sound system using `rodio`, supporting pure Rust loading of WAV, Vorbis and FLAC files
 * Configuration system now uses `serde` rather than `rustc_serialize`
 * Refactored event loop handling somewhat to make it less magical and more composable.
 * New filesystem indirection code using `app_dirs`, and `cargo-resource-root` feature flag.

# 0.2.2

Added `set_color_mod` and `set_alpha_mod` functions which I'd forgotten

# 0.2.1

IIRC, switched from SDL_ttf to rusttype because of horrible evil API's not playing nice with
lifetimes.

# 0.2.0

Made a fairly fully fleshed out SDL implementation

# 0.1.0

Initial proof of concept
