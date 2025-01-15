//! Error types and conversion functions.
use std::error::Error;
use std::fmt;
use std::string::FromUtf8Error;
use std::sync::Arc;

/// An enum containing all kinds of game framework errors.
#[derive(Debug)]
pub enum GameError {
    /// An error when intializing the graphics system.
    GraphicsInitializationError,
    /// An error in the filesystem layout
    FilesystemError(String),
    /// An error in the config file
    ConfigError(String),
    /// Happens when an `winit::event_loop::EventLoopProxy` attempts to
    /// wake up an `winit::event_loop::EventLoop` that no longer exists.
    EventLoopError(winit::error::EventLoopError),
    /// An error trying to load a resource, such as getting an invalid image file.
    ResourceLoadError(String),
    /// Unable to find a resource; the `Vec` is the paths it searched for and associated errors
    ResourceNotFound(String, Vec<(std::path::PathBuf, GameError)>),
    /// Something went wrong with the mesh
    MeshError(String),
    /// Something went wrong in the renderer
    RenderError(String),
    /// Something went wrong when requesting a logical device from the graphics API.
    RequestDeviceError(wgpu::RequestDeviceError),
    /// Something went wrong in the audio playback
    AudioError(String),
    /// Something went wrong trying to set or get window properties.
    WindowError(String),
    /// Something went wrong trying to create a window
    WindowCreationError(winit::error::OsError),
    /// Something went wrong trying to read from a file
    #[allow(clippy::upper_case_acronyms)]
    IOError(Arc<std::io::Error>),
    /// Something went wrong trying to load a font
    FontError(glyph_brush::ab_glyph::InvalidFont),
    /// Shader encoding error (not valid utf-8)
    ShaderEncodingError(FromUtf8Error),
    /// Something went wrong applying video settings.
    VideoError(String),
    /// Something went wrong with the `gilrs` gamepad-input library.
    GamepadError(String),
    /// Something went wrong with the `lyon` shape-tesselation library.
    LyonError(String),
    /// Something went wrong when drawing text.
    GlyphBrushError(glyph_brush::BrushError),
    /// Attempted to draw text with a non-existent font name.
    FontSelectError(String),
    /// Something went wrong when asynchronously mapping a GPU buffer.
    BufferAsyncError(wgpu::BufferAsyncError),
    /// Deadlock when trying to lock a mutex.
    LockError,
    /// A custom error type for use by users of ggez.
    /// This lets you handle custom errors that may happen during your game (such as, trying to load a malformed file for a level)
    /// using the same mechanism you handle ggez's other errors.
    ///
    /// Please include an informative message with the error.
    CustomError(String),
}

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            GameError::ConfigError(ref s) => write!(f, "Config error: {s}"),
            GameError::ResourceLoadError(ref s) => write!(f, "Error loading resource: {s}"),
            GameError::ResourceNotFound(ref s, ref paths) => {
                write!(f, "Resource not found: {s}, searched in paths {paths:?}")
            }
            GameError::WindowError(ref e) => write!(f, "Window creation error: {e}"),
            GameError::CustomError(ref s) => write!(f, "Custom error: {s}"),
            GameError::RequestDeviceError(ref e) => {
                write!(f, "Failed to request logical device: {e}")
            }
            GameError::GlyphBrushError(ref e) => write!(f, "Text rendering error: {e}"),
            GameError::FontSelectError(ref e) => write!(f, "No such font '{e}'"),
            GameError::BufferAsyncError(ref e) => write!(f, "Async buffer map error: {e}"),
            _ => write!(f, "GameError {self:?}"),
        }
    }
}

impl Error for GameError {
    fn cause(&self) -> Option<&dyn Error> {
        match *self {
            GameError::RequestDeviceError(ref e) => Some(e),
            GameError::WindowCreationError(ref e) => Some(e),
            GameError::IOError(ref e) => Some(&**e),
            GameError::FontError(ref e) => Some(e),
            GameError::GlyphBrushError(ref e) => Some(e),
            GameError::BufferAsyncError(ref e) => Some(e),
            GameError::EventLoopError(ref e) => Some(e),
            _ => None,
        }
    }
}

/// A convenient result type consisting of a return type and a `GameError`
pub type GameResult<T = ()> = Result<T, GameError>;

impl From<std::io::Error> for GameError {
    fn from(e: std::io::Error) -> GameError {
        GameError::IOError(Arc::new(e))
    }
}

impl From<toml_edit::de::Error> for GameError {
    fn from(e: toml_edit::de::Error) -> GameError {
        let errstr = format!("TOML decode error: {e}");

        GameError::ConfigError(errstr)
    }
}

impl From<toml_edit::ser::Error> for GameError {
    fn from(e: toml_edit::ser::Error) -> GameError {
        let errstr = format!("TOML error (possibly encoding?): {e}");
        GameError::ConfigError(errstr)
    }
}

impl From<zip::result::ZipError> for GameError {
    fn from(e: zip::result::ZipError) -> GameError {
        let errstr = format!("Zip error: {e}");
        GameError::ResourceLoadError(errstr)
    }
}

#[cfg(feature = "audio")]
impl From<rodio::decoder::DecoderError> for GameError {
    fn from(e: rodio::decoder::DecoderError) -> GameError {
        let errstr = format!("Audio decoder error: {e:?}");
        GameError::AudioError(errstr)
    }
}

#[cfg(feature = "audio")]
impl From<rodio::PlayError> for GameError {
    fn from(e: rodio::PlayError) -> GameError {
        let errstr = format!("Audio playing error: {e:?}");
        GameError::AudioError(errstr)
    }
}

impl From<image::ImageError> for GameError {
    fn from(e: image::ImageError) -> GameError {
        let errstr = format!("Image load error: {e}");
        GameError::ResourceLoadError(errstr)
    }
}
impl From<winit::error::OsError> for GameError {
    fn from(e: winit::error::OsError) -> GameError {
        GameError::WindowCreationError(e)
    }
}

#[cfg(feature = "gamepad")]
impl From<gilrs::Error> for GameError {
    fn from(s: gilrs::Error) -> GameError {
        let errstr = format!("Gamepad error: {s}");
        GameError::GamepadError(errstr)
    }
}

impl From<lyon::lyon_tessellation::TessellationError> for GameError {
    fn from(s: lyon::lyon_tessellation::TessellationError) -> GameError {
        let errstr =
            format!("Error while tesselating shape (did you give it an infinity or NaN?): {s:?}");
        GameError::LyonError(errstr)
    }
}

impl From<lyon::lyon_tessellation::geometry_builder::GeometryBuilderError> for GameError {
    fn from(s: lyon::lyon_tessellation::geometry_builder::GeometryBuilderError) -> GameError {
        let errstr =
            format!("Error while building geometry (did you give it too many vertices?): {s:?}");
        GameError::LyonError(errstr)
    }
}

impl From<wgpu::RequestDeviceError> for GameError {
    fn from(s: wgpu::RequestDeviceError) -> GameError {
        GameError::RequestDeviceError(s)
    }
}

impl From<winit::error::EventLoopError> for GameError {
    fn from(e: winit::error::EventLoopError) -> GameError {
        GameError::EventLoopError(e)
    }
}

impl From<Arc<std::io::Error>> for GameError {
    fn from(s: Arc<std::io::Error>) -> GameError {
        GameError::IOError(s)
    }
}

impl From<glyph_brush::ab_glyph::InvalidFont> for GameError {
    fn from(s: glyph_brush::ab_glyph::InvalidFont) -> GameError {
        GameError::FontError(s)
    }
}

impl From<glyph_brush::BrushError> for GameError {
    fn from(s: glyph_brush::BrushError) -> GameError {
        GameError::GlyphBrushError(s)
    }
}

impl From<wgpu::BufferAsyncError> for GameError {
    fn from(s: wgpu::BufferAsyncError) -> GameError {
        GameError::BufferAsyncError(s)
    }
}
