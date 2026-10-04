//! Font documents and the blend operation that builds a new face from two compatible ones.

mod blend;
mod error;
mod font;
mod import;
mod sfnt;
mod ttf;
mod typeface;
mod ufo;
mod webfont;

pub use blend::{CompatIssue, blend_fonts, compatibility};
pub use error::FoundryError;
pub use font::{
    Contour, FONT_FORMAT, FONT_VERSION, Font, Glyph, MAX_UPM, MIN_UPM, Metrics, Point, PointKind,
};
