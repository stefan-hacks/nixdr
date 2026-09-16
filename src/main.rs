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
//!
//! Design:
//! - Spinner: Catppuccin-themed progress during Nix execution
//! - Parser: tokenises Nix stderr into structured ErrorReport
//! - Classifier: maps to one of 5 error classes
//! - Suggester: attaches actionable fixes
//! - Printer: bordered panel output with Catppuccin Mocha colors

use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use std::io::{self, BufRead, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

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
    let use_color = printer.use_color;

    // Read Nix stderr
    let stderr_text = if cli.stdin {
        read_stdin()
    } else if let Some(cmd) = cli.command {
        let nix_cmd_info = describe_nix_command(&cmd);
        let spinner = start_spinner(&nix_cmd_info, use_color);
        
        let result = run_nix_command(cmd, cli.trace);
        spinner.finish_and_clear();
        
        match result {
            Ok((stdout, stderr, code)) => {
                // Pass through stdout
                if let Some(ref s) = stdout {
                    let _ = io::stdout().write_all(s.as_bytes());
                }
                // If exit code is 0, nix succeeded.
                if code == 0 {
                    print_success_banner(&nix_cmd_info, use_color);
                    std::process::exit(0);
                }
                stderr.unwrap_or_default()
            }
            Err(e) => {
                eprintln!("{} {}", "❌".to_string(), e.bright_red());
                std::process::exit(1);
            }
        }
    } else {
        eprintln!("nixdr: expected --stdin or a subcommand (build, eval, check, rebuild, develop, run)");
        std::process::exit(1);
    };

    if stderr_text.trim().is_empty() {
        print_no_errors(use_color);
        std::process::exit(0);
    }

    // Show analysis spinner
    let analysis_spinner = start_analysis_spinner(use_color);
    
    // Parse
    let mut parser = NixErrorParser::new(&stderr_text);
    let report = parser.parse();
    
    // Suggest fixes if enabled
    let report = if cli.no_suggest {
        report
    } else {
        suggest::enrich(report)
    };
    
    analysis_spinner.finish_and_clear();

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

    // Exit with error since there was stderr content
    std::process::exit(1);
}

fn describe_nix_command(cmd: &NixCommand) -> String {
    match cmd {
        NixCommand::Build { .. } => "nix build".to_string(),
        NixCommand::Eval { .. } => "nix eval".to_string(),
        NixCommand::Check { .. } => "nix flake check".to_string(),
        NixCommand::Rebuild { .. } => "nixos-rebuild".to_string(),
        NixCommand::Develop { .. } => "nix develop".to_string(),
        NixCommand::Run { .. } => "nix run".to_string(),
    }
}

fn start_spinner(desc: &str, use_color: bool) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.enable_steady_tick(Duration::from_millis(100));
    
    if use_color {
        let style = ProgressStyle::default_spinner()
            .tick_strings(&[
                "❄️  ",
                "❄️  ",
                "🌨️  ",
                "❄️  ",
                "🌨️  ",
                "❄️  ",
                "🌨️  ",
                "✅  ",
            ])
            .template("{spinner} {msg}")
            .unwrap();
        pb.set_style(style);
        let running = "Running".truecolor(137, 220, 235).to_string();
        let cmd_colored = desc.truecolor(203, 166, 247).to_string();
        pb.set_message(format!("{} {}", running, cmd_colored));
    } else {
        let style = ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap();
        pb.set_style(style);
        pb.set_message(format!("Running {}", desc));
    }
    
    pb
}

fn start_analysis_spinner(use_color: bool) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.enable_steady_tick(Duration::from_millis(80));
    
    if use_color {
        let style = ProgressStyle::default_spinner()
            .tick_strings(&[
                "❄️  ",
                "🔍  ",
                "❄️  ",
                "🔍  ",
                "❄️  ",
                "✅  ",
            ])
            .template("{spinner} {msg}")
            .unwrap();
        pb.set_style(style);
        let msg = "Analyzing error trace...".to_string();
        let colored = msg.truecolor(250, 179, 135).to_string();
        pb.set_message(colored);
    } else {
        let style = ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap();
        pb.set_style(style);
        pb.set_message("Analyzing error trace...");
    }
    
    pb
}

fn print_success_banner(cmd: &str, use_color: bool) {
    let width = terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(60)
        .min(80);

    if use_color {
        let border_raw = "━".repeat(width);
        let border = border_raw.bright_green();
        let inner_raw = format!("  {}  ", cmd);
        let inner = inner_raw.bright_green();
        let check_raw = "✅".to_string();
        let check = check_raw.bright_green();
        println!("\n{}", border);
        println!("{}   No errors detected!   {}", check, inner);
        println!("{}", border);
        println!();
    } else {
        let border = "=".repeat(width);
        println!("\n{}", border);
        println!("[OK] {} — No errors detected!", cmd);
        println!("{}", border);
        println!();
    }
}

fn print_no_errors(use_color: bool) {
    if use_color {
        println!(
            "\n{} {}\n",
            "✅".to_string().bright_green(),
            "No Nix errors detected in provided stderr.".bright_green()
        );
    } else {
        println!("\n[OK] No Nix errors detected in provided stderr.\n");
    }
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
