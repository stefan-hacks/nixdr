//! Nix error parser — converts raw Nix stderr into structured ErrorReport.
//!
//! Handles both evaluation errors (with traces) and build errors (with drv paths).

use regex::Regex;
use crate::diagnosis::{ErrorClass, ErrorReport, TraceFrame, Location};

pub struct NixErrorParser<'a> {
    raw: &'a str,
}

impl<'a> NixErrorParser<'a> {
    pub fn new(raw: &'a str) -> Self {
        let _lines: Vec<_> = raw.lines().collect();
        Self { raw }
    }

    pub fn parse(&mut self) -> ErrorReport {
        let text = self.raw;

        // Try each classifier in order (most specific first)
        if let Some(report) = Self::try_infinite_recursion(text) {
            return report;
        }
        if let Some(report) = Self::try_value_not_function(text) {
            return report;
        }
        if let Some(report) = Self::try_attribute_missing(text) {
            return report;
        }
        if let Some(report) = Self::try_builder_failed(text) {
            return report;
        }
        if let Some(report) = Self::try_hash_mismatch(text) {
            return report;
        }
        if let Some(report) = Self::try_undefined_variable(text) {
            return report;
        }

        // Fallback: generic error with trace
        Self::parse_generic(text)
    }

    fn try_infinite_recursion(text: &str) -> Option<ErrorReport> {
        if text.contains("infinite recursion encountered") {
            let summary = "Infinite recursion: an attribute depends on itself".to_string();
            let trace = Self::extract_trace(text);
            let location = Self::extract_crash_location(text);
            let user_location = trace.last().and_then(|f| f.location.clone());

            Some(ErrorReport {
                class: ErrorClass::InfiniteRecursion,
                summary,
                detail: Self::extract_detail(text),
                location,
                user_location,
                trace,
                suggestions: vec![],
                raw: text.to_string(),
            })
        } else {
            None
        }
    }

    fn try_value_not_function(text: &str) -> Option<ErrorReport> {
        let re = Regex::new(r"value is not a (function|function while calling a function)").unwrap();
        if re.is_match(text) {
            let summary = "Type error: a value was used where a function was expected".to_string();
            let trace = Self::extract_trace(text);
            let location = Self::extract_crash_location(text);
            let user_location = trace.last().and_then(|f| f.location.clone());

            Some(ErrorReport {
                class: ErrorClass::NotAFunction,
                summary,
                detail: Self::extract_detail(text),
                location,
                user_location,
                trace,
                suggestions: vec![],
                raw: text.to_string(),
            })
        } else {
            None
        }
    }

    fn try_attribute_missing(text: &str) -> Option<ErrorReport> {
        let re = Regex::new(r"attribute '([^']+)' missing").unwrap();
        if let Some(caps) = re.captures(text) {
            let attr = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
            let summary = format!("Missing attribute: '{}' not found", attr);
            let trace = Self::extract_trace(text);
            let location = Self::extract_crash_location(text);
            let user_location = trace.last().and_then(|f| f.location.clone());

            Some(ErrorReport {
                class: ErrorClass::AttributeMissing { attribute: attr },
                summary,
                detail: Self::extract_detail(text),
                location,
                user_location,
                trace,
                suggestions: vec![],
                raw: text.to_string(),
            })
        } else {
            None
        }
    }

    fn try_undefined_variable(text: &str) -> Option<ErrorReport> {
        let re = Regex::new(r"undefined variable '([^']+)'").unwrap();
        if let Some(caps) = re.captures(text) {
            let var = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
            let summary = format!("Undefined variable: '{}' not found in scope", var);
            let trace = Self::extract_trace(text);
            let location = Self::extract_crash_location(text);
            let user_location = trace.last().and_then(|f| f.location.clone());

            Some(ErrorReport {
                class: ErrorClass::UndefinedVariable { variable: var },
                summary,
                detail: Self::extract_detail(text),
                location,
                user_location,
                trace,
                suggestions: vec![],
                raw: text.to_string(),
            })
        } else {
            None
        }
    }

    fn try_builder_failed(text: &str) -> Option<ErrorReport> {
        let re = Regex::new(r"builder for '([^']+)' failed with exit code (\d+)").unwrap();
        if let Some(caps) = re.captures(text) {
            let drv = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
            let code = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
            let summary = format!("Build failed: '{}' exited with code {}", drv, code);
            let trace = Self::extract_trace(text);
            let location = Self::extract_crash_location(text);

            Some(ErrorReport {
                class: ErrorClass::BuilderFailed { drv, exit_code: code },
                summary,
                detail: Self::extract_detail(text),
                location,
                user_location: None,
                trace,
                suggestions: vec![],
                raw: text.to_string(),
            })
        } else {
            None
        }
    }

    fn try_hash_mismatch(text: &str) -> Option<ErrorReport> {
        let re = Regex::new(r"hash mismatch in fixed-output derivation").unwrap();
        if re.is_match(text) {
            let (specified, got) = Self::extract_hashes(text);
            let summary = "Hash mismatch: downloaded content doesn't match declared hash".to_string();
            let trace = Self::extract_trace(text);
            let location = Self::extract_crash_location(text);

            Some(ErrorReport {
                class: ErrorClass::HashMismatch { specified, got },
                summary,
                detail: Self::extract_detail(text),
                location,
                user_location: None,
                trace,
                suggestions: vec![],
                raw: text.to_string(),
            })
        } else {
            None
        }
    }

    fn parse_generic(text: &str) -> ErrorReport {
        // Find the first actual error line (skip leading warnings/notices)
        let error_line = text
            .lines()
            .find(|l| l.starts_with("error:"))
            .map(|l| l.to_string())
            .or_else(|| text.lines().next().map(|l| l.to_string()))
            .unwrap_or_else(|| "unknown error".to_string());

        let summary = if error_line.starts_with("error:") {
            error_line.trim_start_matches("error:").trim().to_string()
        } else {
            error_line
        };
        let trace = Self::extract_trace(text);
        let location = Self::extract_crash_location(text);
        let user_location = trace.last().and_then(|f| f.location.clone());

        ErrorReport {
            class: ErrorClass::Unknown,
            summary,
            detail: Self::extract_detail(text),
            location,
            user_location,
            trace,
            suggestions: vec![],
            raw: text.to_string(),
        }
    }

    /// Extract the "at /nix/store/...:line:col" crash location from the top of the error.
    fn extract_crash_location(text: &str) -> Option<Location> {
        let re = Regex::new(r"at ([^:]+):(\d+):(\d+)").unwrap();
        text.lines().find_map(|line| {
            re.captures(line).and_then(|caps| {
                let file = caps.get(1)?.as_str().to_string();
                let line = caps.get(2)?.as_str().parse().ok()?;
                let column = caps.get(3)?.as_str().parse().ok()?;
                Some(Location { file, line, column })
            })
        })
    }

    /// Extract all trace frames from "… while evaluating" lines.
    fn extract_trace(text: &str) -> Vec<TraceFrame> {
        let mut frames = Vec::new();
        let trace_re = Regex::new(r"… (while .+)").unwrap();
        let expr_re = Regex::new(r"in the expression at ([^:]+):(\d+):(\d+)").unwrap();
        let attr_re = Regex::new(r"while evaluating the attribute '([^']+)'").unwrap();
        let builtin_re = Regex::new(r"while calling the '([^']+)' builtin").unwrap();

        for line in text.lines() {
            if let Some(caps) = trace_re.captures(line) {
                let description = caps.get(1).unwrap().as_str().to_string();

                let location = expr_re.captures(line).map(|c| Location {
                    file: c.get(1).unwrap().as_str().to_string(),
                    line: c.get(2).unwrap().as_str().parse().unwrap_or(0),
                    column: c.get(3).unwrap().as_str().parse().unwrap_or(0),
                });

                let frame_type = if line.contains("in the expression") {
                    "expression"
                } else if line.contains("while evaluating the attribute") {
                    "attribute"
                } else if line.contains("while calling the") {
                    "builtin"
                } else if line.contains("while calling") {
                    "function"
                } else if line.contains("while evaluating") {
                    "evaluation"
                } else {
                    "unknown"
                };

                let attribute = attr_re.captures(line)
                    .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));

                let builtin = builtin_re.captures(line)
                    .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));

                frames.push(TraceFrame {
                    description,
                    frame_type: frame_type.to_string(),
                    location,
                    attribute,
                    builtin,
                });
            }
        }

        // Preserve order as found in stderr (which is bottom-up for traces)
        frames
    }

    /// Extract any additional detail lines after the error type.
    fn extract_detail(text: &str) -> Option<String> {
        let lines: Vec<_> = text.lines().collect();
        let mut detail_lines = Vec::new();
        let mut in_detail = false;

        for line in lines {
            if line.starts_with("error:") || line.contains("infinite recursion") {
                in_detail = true;
                continue;
            }
            if line.starts_with("at ") {
                // Crash location line — skip
                continue;
            }
            if line.starts_with("… ") {
                // Trace lines — we've moved past detail
                break;
            }
            if in_detail && !line.trim().is_empty() {
                detail_lines.push(line.trim());
            }
        }

        if detail_lines.is_empty() {
            None
        } else {
            Some(detail_lines.join("\n"))
        }
    }

    /// Extract specified and got hashes from hash mismatch errors.
    fn extract_hashes(text: &str) -> (Option<String>, Option<String>) {
        let specified_re = Regex::new(r"specified:\s*(sha256-[A-Za-z0-9+/=]+)").unwrap();
        let got_re = Regex::new(r"got:\s*(sha256-[A-Za-z0-9+/=]+)").unwrap();

        let specified = specified_re.captures(text)
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));
        let got = got_re.captures(text)
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));

        (specified, got)
    }
}
