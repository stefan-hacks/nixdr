//! Trace analysis utilities — helpers for reading trace frames bottom-up.
//!
//! The golden rule: Nix traces are printed bottom-up. The FIRST frame is
//! deep inside nixpkgs; the LAST frame is closest to your code.

use crate::diagnosis::{ErrorReport, TraceFrame};

/// Returns the trace frames ordered from nixpkgs (top) to user code (bottom).
/// This is the natural Nix order — bottom-up.
#[allow(dead_code)]
pub fn trace_bottom_up(report: &ErrorReport) -> &[TraceFrame] {
    &report.trace
}

/// Returns the single trace frame closest to user code (the last one).
#[allow(dead_code)]
pub fn user_frame(report: &ErrorReport) -> Option<&TraceFrame> {
    report.trace.last()
}

/// Returns frames that reference files outside /nix/store (i.e. user code).
pub fn user_frames(report: &ErrorReport) -> Vec<&TraceFrame> {
    report.trace
        .iter()
        .filter(|f| {
            f.location.as_ref().map(|loc| !loc.file.starts_with("/nix/store")).unwrap_or(false)
        })
        .collect()
}

/// Returns frames that reference files inside /nix/store (nixpkgs internals).
#[allow(dead_code)]
pub fn nixpkgs_frames(report: &ErrorReport) -> Vec<&TraceFrame> {
    report.trace
        .iter()
        .filter(|f| {
            f.location.as_ref().map(|loc| loc.file.starts_with("/nix/store")).unwrap_or(false)
        })
        .collect()
}

/// Checks if the trace contains any builtins (e.g. `builtins.map`).
pub fn contains_builtin(report: &ErrorReport, name: &str) -> bool {
    report.trace.iter().any(|f| {
        f.builtin.as_ref().map(|b| b == name).unwrap_or(false)
    })
}

/// Checks if the trace evaluates a specific attribute.
#[allow(dead_code)]
pub fn evaluates_attribute(report: &ErrorReport, name: &str) -> bool {
    report.trace.iter().any(|f| {
        f.attribute.as_ref().map(|a| a == name).unwrap_or(false)
    })
}
