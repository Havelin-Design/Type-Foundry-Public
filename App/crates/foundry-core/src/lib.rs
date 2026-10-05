//! Font documents and the blend operation that builds a new face from two compatible ones.

mod blend;
mod edit;
mod error;
mod family;
mod font;
mod import;
mod sfnt;
mod svgfont;
mod ttf;
mod typeface;
mod ufo;
mod webfont;

pub use blend::{CompatIssue, blend_fonts, compatibility};
pub use edit::{Anchor, Matrix, MetricsUpdate};
pub use error::FoundryError;
pub use family::{
    ExportFormat, FAMILY_FORMAT, FAMILY_VERSION, FamilyFile, FamilyIssue, StyleUpdate,
    check_family, export_family, load_family, save_family,
};
pub use font::{
    Contour, FONT_FORMAT, FONT_VERSION, Font, Glyph, MAX_UPM, MIN_UPM, Metrics, Point, PointKind,
    Style,
};
