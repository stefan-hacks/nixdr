//! Pretty printer — Catppuccin Mocha theme for Nix error reports.
//!
//! Colors:
//! - Mauve   (#cba6f7)   → headings, borders, emphasis
//! - Green   (#a6e3a1)   → OK, success, correct paths
//! - Red     (#f38ba8)   → errors, crash location
//! - Peach   (#fab387)   → warnings, suggestions
//! - Sky     (#89dceb)   → file paths, locations
//! - Blue    (#89b4fa)   → trace frames, secondary info
//! - Overlay0 (#6c7086)  → dimmed text, separators
//!
//! Use `owo-colors` for portable terminal colorization.

use owo_colors::OwoColorize;
use std::io::{self, Write};

use crate::diagnosis::{ErrorClass, ErrorReport, Location, Suggestion, TraceFrame};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

pub struct Printer {
    _color_mode: ColorMode,
    pub use_color: bool,
}

impl Printer {
    pub fn new(color_mode: ColorMode) -> Self {
        let use_color = match color_mode {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => {
                atty::is(atty::Stream::Stdout) &&
                    std::env::var("NO_COLOR").is_err() &&
                    std::env::var("TERM").map(|t| t != "dumb").unwrap_or(true)
            }
        };
        Self {
            _color_mode: color_mode,
            use_color,
        }
    }

    pub fn print(&self, report: &ErrorReport) {
        let mut stdout = io::stdout();

        // Header: error class badge + summary
        let badge = self.badge(report);
        let _ = writeln!(&mut stdout, "\n{} {}", badge, self.style_summary(&report.summary));

        // Crash location (top of trace)
        if let Some(ref loc) = report.location {
            let _ = writeln!(&mut stdout,
                "  {} {}",
                self.dim("at"),
                self.style_location(loc)
            );
        }

        // Detail block
        if let Some(ref detail) = report.detail {
            let _ = writeln!(&mut stdout, "\n{}\n{}",
                self.dim("Details:"),
                self.indent(detail, 2)
            );
        }

        // Trace frames (bottom-up)
        if !report.trace.is_empty() {
            let _ = writeln!(&mut stdout, "\n{}", self.dim("Trace (bottom → top):"));
            for (idx, frame) in report.trace.iter().enumerate() {
                self.print_trace_frame(&mut stdout, idx, frame, idx == report.trace.len() - 1);
            }
        }

        // User location (last frame = your code)
        if let Some(ref loc) = report.user_location {
            let _ = writeln!(
                &mut stdout,
                "\n{} {}",
                self.style_user("Your code:"),
                self.style_location(loc)
            );
        }

        // Suggestions
        if !report.suggestions.is_empty() {
            let _ = writeln!(&mut stdout, "\n{}", self.style_suggest("Suggestions:"));
            for (i, sugg) in report.suggestions.iter().enumerate() {
                self.print_suggestion(&mut stdout, i + 1, sugg);
            }
        }

        // Separator
        let width = terminal_size::terminal_size()
            .map(|(w, _)| w.0 as usize)
            .unwrap_or(60);
        let _ = writeln!(&mut stdout, "\n{}",
            self.dim(&"─".repeat(width))
        );
    }

    fn badge(&self, report: &ErrorReport) -> String {
        let label = match report.class {
            ErrorClass::InfiniteRecursion => "INFINITE RECURSION",
            ErrorClass::NotAFunction => "TYPE ERROR",
            ErrorClass::AttributeMissing { .. } => "MISSING ATTRIBUTE",
            ErrorClass::UndefinedVariable { .. } => "UNDEFINED VARIABLE",
            ErrorClass::BuilderFailed { .. } => "BUILD FAILED",
            ErrorClass::HashMismatch { .. } => "HASH MISMATCH",
            ErrorClass::Unknown => "UNKNOWN ERROR",
        };

        if self.use_color {
            let colored = match report.class {
                ErrorClass::InfiniteRecursion | ErrorClass::NotAFunction |
                ErrorClass::AttributeMissing { .. } | ErrorClass::UndefinedVariable { .. } | ErrorClass::Unknown => {
                    label.bright_red().bold().to_string()
                }
                ErrorClass::BuilderFailed { .. } => {
                    label.bright_magenta().bold().to_string()
                }
                ErrorClass::HashMismatch { .. } => {
                    label.bright_yellow().bold().to_string()
                }
            };
            format!("[{colored}]")
        } else {
            format!("[{label}]")
        }
    }

    fn style_summary(&self, text: &str) -> String {
        if self.use_color {
            text.bright_white().bold().to_string()
        } else {
            text.to_string()
        }
    }

    fn style_location(&self, loc: &Location) -> String {
        let s = format!("{}:{}:{}", loc.file, loc.line, loc.column);
        if self.use_color {
            s.bright_cyan().underline().to_string()
        } else {
            s
        }
    }

    fn style_user(&self, text: &str) -> String {
        if self.use_color {
            text.bright_green().bold().to_string()
        } else {
            text.to_string()
        }
    }

    fn style_suggest(&self, text: &str) -> String {
        if self.use_color {
            text.bright_yellow().bold().to_string()
        } else {
            text.to_string()
        }
    }

    fn dim(&self, text: &str) -> String {
        if self.use_color {
            text.bright_black().to_string()
        } else {
            text.to_string()
        }
    }

    fn indent(&self, text: &str, spaces: usize) -> String {
        let prefix = " ".repeat(spaces);
        text.lines()
            .map(|line| format!("{}{}", prefix, line))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn print_trace_frame(&self,
        stdout: &mut io::Stdout,
        _idx: usize,
        frame: &TraceFrame,
        is_last: bool,
    ) {
        let arrow = if is_last { "└─►" } else { "├─►" };
        let arrow_colored = if self.use_color {
            if is_last {
                arrow.bright_green().to_string()
            } else {
                arrow.bright_blue().to_string()
            }
        } else {
            arrow.to_string()
        };

        let desc = if self.use_color {
            frame.description.bright_blue().to_string()
        } else {
            frame.description.clone()
        };

        let loc_str = frame.location.as_ref()
            .map(|loc| {
                let s = format!("  at {}:{}:{}", loc.file, loc.line, loc.column);
                if self.use_color {
                    s.bright_black().to_string()
                } else {
                    s
                }
            })
            .unwrap_or_default();

        let _ = writeln!(stdout, "  {} {} {}", arrow_colored, desc, loc_str);
    }

    fn print_suggestion(&self,
        stdout: &mut io::Stdout,
        num: usize,
        sugg: &Suggestion,
    ) {
        let num_str = if self.use_color {
            format!("  {}.", num).bright_yellow().to_string()
        } else {
            format!("  {}.", num)
        };

        let title = if self.use_color {
            sugg.title.bright_white().bold().to_string()
        } else {
            sugg.title.clone()
        };

        let _ = writeln!(stdout, "\n{} {}", num_str, title);
        let _ = writeln!(stdout, "     {}", sugg.description);

        if let Some(ref cmd) = sugg.command {
            let cmd_str = if self.use_color {
                cmd.bright_cyan().to_string()
            } else {
                cmd.clone()
            };
            let _ = writeln!(stdout, "     {} {}", self.dim("Run:"), cmd_str);
        }

        if let Some(ref code) = sugg.code {
            let _code_str = if self.use_color {
                code.bright_green().to_string()
            } else {
                code.clone()
            };
            let _ = writeln!(stdout, "     {}\n{}",
                self.dim("Change to:"),
                self.indent(code, 6)
            );
        }
    }
}
