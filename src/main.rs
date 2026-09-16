//! nixdr — Nix error diagnosis and human-readable reporting tool
//!
//! Reads Nix stderr, parses error messages, classifies them into known
//! patterns, and prints colorized, actionable diagnostics.
//!
//! Usage:
//!   nixdr build .#myPackage          # wraps `nix build`, intercepts stderr
//!   nixdr eval --expr '...'           # wraps `nix eval`, intercepts stderr
//!   nixdr check                       # wraps `nix flake check`
//!   nixdr --stdin                     # reads nix stderr from stdin
//!   nixdr --json                      # output JSON instead of pretty-print
//!   nixdr --theme mocha               # Catppuccin Mocha color scheme
//!
//! Design:
//! - Parser: tokenises Nix stderr into structured ErrorReport
//! - Classifier: maps to one of 5 error classes
//! - Suggester: attaches actionable fixes
//! - Printer: Catppuccin Mocha colorized output (or JSON)

use clap::{Parser, Subcommand};
use std::io::{self, BufRead, Write};
use std::process::{Command, Stdio};

mod diagnosis;
mod parser;
mod printer;
mod suggest;
mod trace;

use parser::NixErrorParser;
use printer::{ColorMode, Printer};

/// Nix error diagnosis and human-readable reporting tool.
#[derive(Parser)]
#[command(name = "nixdr")]
#[command(about = "Nix error diagnosis and human-readable reporting tool")]
#[command(version)]
struct Cli {
    /// Enable JSON output instead of pretty-printed diagnostics.
    #[arg(long, global = true)]
    json: bool,

    /// Color output mode.
    #[arg(long, global = true, value_enum, default_value = "auto")]
    color: ColorModeArg,

    /// Disable suggestions (print error summary only).
    #[arg(long, global = true)]
    no_suggest: bool,

    /// Show full trace (equivalent to --show-trace).
    #[arg(long, global = true)]
    trace: bool,

    /// Read Nix stderr from stdin instead of running a command.
    #[arg(long, global = true)]
    stdin: bool,

    /// Subcommand to run (wrapping the corresponding `nix` command).
    #[command(subcommand)]
    command: Option<NixCommand>,
}

#[derive(Clone, Copy, Debug, PartialEq, clap::ValueEnum)]
enum ColorModeArg {
    Auto,
    Always,
    Never,
}

#[derive(Subcommand)]
enum NixCommand {
    /// Build a derivation (wraps `nix build`).
    Build {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Evaluate a Nix expression (wraps `nix eval`).
    Eval {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Run flake checks (wraps `nix flake check`).
    Check {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Rebuild NixOS config (wraps `nixos-rebuild`).
    Rebuild {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Develop in a Nix shell (wraps `nix develop`).
    Develop {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Run a derivation (wraps `nix run`).
    Run {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    let color_mode = match cli.color {
        ColorModeArg::Auto => ColorMode::Auto,
        ColorModeArg::Always => ColorMode::Always,
        ColorModeArg::Never => ColorMode::Never,
    };

    let printer = Printer::new(color_mode);

    // Read Nix stderr
    let stderr_text = if cli.stdin {
        read_stdin()
    } else if let Some(cmd) = cli.command {
        match run_nix_command(cmd, cli.trace) {
            Ok((stdout, stderr, code)) => {
                // Pass through stdout
                if let Some(ref s) = stdout {
                    let _ = io::stdout().write_all(s.as_bytes());
                }
                // If exit code is 0, nix succeeded. Ignore any stderr
                // (Nix prints warnings/notices to stderr even on success).
                if code == 0 {
                    std::process::exit(0);
                }
                stderr.unwrap_or_default()
            }
            Err(e) => {
                eprintln!("nixdr: failed to run nix command: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        eprintln!("nixdr: expected --stdin or a subcommand (build, eval, check, rebuild, develop, run)");
        std::process::exit(1);
    };

    if stderr_text.trim().is_empty() {
        eprintln!("nixdr: no stderr to diagnose");
        std::process::exit(0);
    }

    // Parse
    let mut parser = NixErrorParser::new(&stderr_text);
    let report = parser.parse();

    // Suggest fixes if enabled
    let report = if cli.no_suggest {
        report
    } else {
        suggest::enrich(report)
    };

    // Print
    if cli.json {
        match serde_json::to_string_pretty(&report) {
            Ok(json) => println!("{}", json),
            Err(e) => {
                eprintln!("nixdr: JSON serialization failed: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        printer.print(&report);
    }

    // Exit with the same exit code as the wrapped command
    std::process::exit(1);
}

fn read_stdin() -> String {
    let stdin = io::stdin();
    let mut buf = String::new();
    for line in stdin.lock().lines() {
        if let Ok(l) = line {
            buf.push_str(&l);
            buf.push('\n');
        }
    }
    buf
}

fn run_nix_command(cmd: NixCommand, show_trace: bool) -> Result<(Option<String>, Option<String>, i32), String> {
    let (nix_cmd, args): (&str, Vec<String>) = match cmd {
        NixCommand::Build { args } => ("nix", {
            let mut v = vec!["build".to_string()];
            if show_trace { v.push("--show-trace".to_string()); }
            v.extend(args);
            v
        }),
        NixCommand::Eval { args } => ("nix", {
            let mut v = vec!["eval".to_string()];
            if show_trace { v.push("--show-trace".to_string()); }
            v.extend(args);
            v
        }),
        NixCommand::Check { args } => ("nix", {
            let mut v = vec!["flake".to_string(), "check".to_string()];
            if show_trace { v.push("--show-trace".to_string()); }
            v.extend(args);
            v
        }),
        NixCommand::Rebuild { args } => ("nixos-rebuild", {
            let mut v = vec![];
            v.extend(args);
            if show_trace && !v.iter().any(|a| a == "--show-trace") {
                v.push("--show-trace".to_string());
            }
            v
        }),
        NixCommand::Develop { args } => ("nix", {
            let mut v = vec!["develop".to_string()];
            if show_trace { v.push("--show-trace".to_string()); }
            v.extend(args);
            v
        }),
        NixCommand::Run { args } => ("nix", {
            let mut v = vec!["run".to_string()];
            if show_trace { v.push("--show-trace".to_string()); }
            v.extend(args);
            v
        }),
    };

    let mut command = Command::new(nix_cmd);
    command.args(&args);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|e| format!("failed to spawn {}: {}", nix_cmd, e))?;

    let stdout = child
        .stdout
        .take()
        .map(|mut r| {
            let mut s = String::new();
            let _ = std::io::Read::read_to_string(&mut r, &mut s);
            s
        })
        .filter(|s| !s.is_empty());

    let stderr = child
        .stderr
        .take()
        .map(|mut r| {
            let mut s = String::new();
            let _ = std::io::Read::read_to_string(&mut r, &mut s);
            s
        })
        .filter(|s| !s.is_empty());

    let status = child
        .wait()
        .map_err(|e| format!("failed to wait for {}: {}", nix_cmd, e))?;

    let code = status.code().unwrap_or(1);

    Ok((stdout, stderr, code))
}
