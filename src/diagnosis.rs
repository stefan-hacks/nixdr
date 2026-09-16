//! Error classification and data structures for Nix error reports.

use serde::{Deserialize, Serialize};

/// Classification of Nix errors into known patterns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "class", rename_all = "snake_case")]
pub enum ErrorClass {
    InfiniteRecursion,
    NotAFunction,
    AttributeMissing { attribute: String },
    UndefinedVariable { variable: String },
    BuilderFailed { drv: String, exit_code: i32 },
    HashMismatch { specified: Option<String>, got: Option<String> },
    Unknown,
}

/// A structured report for a single Nix error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorReport {
    #[serde(flatten)]
    pub class: ErrorClass,
    pub summary: String,
    pub detail: Option<String>,
    /// Where Nix detected the crash (usually deep in nixpkgs).
    pub location: Option<Location>,
    /// The last trace frame — closest to your code.
    pub user_location: Option<Location>,
    pub trace: Vec<TraceFrame>,
    pub suggestions: Vec<Suggestion>,
    /// Raw stderr for reference.
    pub raw: String,
}

/// Source code location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

/// A single trace frame from "… while evaluating".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceFrame {
    pub description: String,
    pub frame_type: String,
    pub location: Option<Location>,
    pub attribute: Option<String>,
    pub builtin: Option<String>,
}

/// An actionable suggestion for fixing the error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub title: String,
    pub description: String,
    pub command: Option<String>,
    pub code: Option<String>,
}
