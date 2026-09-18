//! nixdr — Nix error diagnosis and human-readable reporting tool
//!
//! Reads Nix stderr, parses error messages, classifies them into known
//! patterns, and prints colorized, actionable diagnostics.
//!
//! Golden rule for reading Nix traces: they are printed bottom-up.
//! Your code is at the BOTTOM of the trace; the crash is at the TOP.
//!
//! Usage:
//!   nixdr build .                     # wrapper: run nix build with diagnosis
//!   nixdr eval --expr '...'           # wrapper: run nix eval with diagnosis
//!   nix build 2>&1 | nixdr --stdin    # pipe: diagnose after the fact
//!   nix build 2>&1 | nixdr --stdin --json   # JSON output for CI

use std::io::{self, Read, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

use clap::{ColorChoice, Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;

use parser::NixErrorParser;
use printer::{ColorMode, Printer};

mod diagnosis;
mod parser;
mod printer;
mod suggest;
mod trace;
mod show;
mod repl;

// ═══════════════════════════════════════════════════════════════════════════
// CLI
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Parser)]
#[command(
    name = "nixdr",
    about = "Nix Error Doctor — human-readable, colorized diagnostics",
    version,
    color = ColorChoice::Auto
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Read error from stdin instead of running a command
    #[arg(long, global = true)]
    stdin: bool,

    /// JSON output (machine-readable)
    #[arg(long, global = true)]
    json: bool,

    /// Color mode: auto | always | never
    #[arg(long, global = true, value_name = "MODE", default_value = "auto")]
    color: String,

    /// Show full trace including nixpkgs internals
    #[arg(long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// `nix build` with error diagnosis
    Build { args: Vec<String> },
    /// `nix eval` with error diagnosis
    Eval { args: Vec<String> },
    /// `nix flake check` with error diagnosis
    Check { args: Vec<String> },
    /// `nixos-rebuild` with error diagnosis
    Rebuild { args: Vec<String> },
    /// `nix develop` with error diagnosis
    Develop { args: Vec<String> },
    /// `nix run` with error diagnosis
    Run { args: Vec<String> },
    /// Show structured overview of a flake (local or remote)
    Show {
        #[arg(help = "Flake reference: ., github:owner/repo, nixpkgs#pkg, etc.")]
        flake_ref: String,
        /// Skip local clone; inspect directly via nix flake metadata
        #[arg(long, default_value_t = false)]
        remote: bool,
    },
    /// Interactive repl for exploring flake attributes
    Repl {
        #[arg(help = "Flake reference: ., github:owner/repo, github:owner/repo#attr.path")]
        flake_ref: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let color_mode = parse_color_mode(&cli.color);
    let printer = Printer::new(color_mode);

    if cli.stdin {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .expect("Failed to read stdin");
        let has_error = process_stdin(&buf, &cli, &printer);
        std::process::exit(if has_error { 1 } else { 0 });
    }

    let exit_code = match cli.command {
        Some(Commands::Build { ref args }) => {
            execute_with_spinner(("nix", vec!["build"], args.clone()), &printer)
        }
        Some(Commands::Eval { ref args }) => {
            execute_with_spinner(("nix", vec!["eval"], args.clone()), &printer)
        }
        Some(Commands::Check { ref args }) => {
            execute_with_spinner(("nix", vec!["flake", "check"], args.clone()), &printer)
        }
        Some(Commands::Rebuild { ref args }) => {
            execute_with_spinner(("nixos-rebuild", vec![], args.clone()), &printer)
        }
        Some(Commands::Develop { ref args }) => {
            execute_with_spinner(("nix", vec!["develop"], args.clone()), &printer)
        }
        Some(Commands::Run { ref args }) => {
            execute_with_spinner(("nix", vec!["run"], args.clone()), &printer)
        }
        Some(Commands::Show { ref flake_ref, .. }) => {
            show::run(flake_ref, &printer).ok();
            0
        }
        Some(Commands::Repl { ref flake_ref }) => {
            repl::run(flake_ref, &printer);
            0
        }
        None => {
            eprintln!("Usage: nixdr <command> [args...]");
            eprintln!("       nixdr --stdin < raw_nix_stderr");
            std::process::exit(1);
        }
    };

    std::process::exit(exit_code);
}

// ═══════════════════════════════════════════════════════════════════════════
// Color mode parsing
// ═══════════════════════════════════════════════════════════════════════════

fn parse_color_mode(s: &str) -> ColorMode {
    match s.to_lowercase().as_str() {
        "always" => ColorMode::Always,
        "never" => ColorMode::Never,
        _ => ColorMode::Auto,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Execute wrapper command with live spinner
// ═══════════════════════════════════════════════════════════════════════════

fn execute_with_spinner(
    (bin, fixed_args, user_args): (&str,
    Vec<&str>,
    Vec<String>),
    printer: &Printer,
) -> i32 {
    let desc = format!("{} {}", bin, fixed_args.join(" "));
    let pb = start_spinner(&desc, printer.use_color);

    let mut cmd = Command::new(bin);
    for a in &fixed_args {
        cmd.arg(a);
    }
    for a in &user_args {
        cmd.arg(a);
    }

    let output = cmd
        .stderr(Stdio::piped())
        .stdout(Stdio::inherit())
        .output()
        .expect("Failed to spawn nix command");

    let code = output.status.code().unwrap_or(1);
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    pb.finish_and_clear();

    // If exit code is 0, nix succeeded. Print any stderr raw (warnings/notices).
    if code == 0 {
        let (notices, _) = separate_notices(&stderr);
        if !notices.is_empty() {
            print_notices(&notices, printer);
        }
        print_success_banner(&desc, printer.use_color);
        return 0;
    }

    // Error detected — analyze
    let analysis_pb = start_analysis_spinner(printer.use_color);
    let (notices, error_text) = separate_notices(&stderr);
    analysis_pb.finish_and_clear();

    // If the only stderr content was notices (no actual error block), print them
    if error_text.trim().is_empty() {
        if !notices.is_empty() {
            print_notices(&notices, printer);
        }
        // Nix exited non-zero but only printed warnings. Show raw stderr.
        printer.print_raw(&stderr);
        return code;
    }

    let mut report = NixErrorParser::new(&error_text).parse();
    report = suggest::enrich(report);

    if !notices.is_empty() {
        print_notices(&notices, printer);
    }

    printer.print(&report);

    code
}

// ═══════════════════════════════════════════════════════════════════════════
// Pipe mode (stdin)
// ═══════════════════════════════════════════════════════════════════════════

fn process_stdin(stderr: &str, cli: &Cli, printer: &Printer) -> bool {
    let (notices, error_text) = separate_notices(stderr);

    // If we found no error block AND there were notices, there is no real error
    let text_to_parse = if error_text.is_empty() && !notices.is_empty() {
        ""
    } else if error_text.is_empty() {
        stderr
    } else {
        &error_text
    };

    if text_to_parse.trim().is_empty() {
        if !notices.is_empty() {
            print_notices(&notices, printer);
        }
        print_no_errors(printer.use_color);
        return false;
    }

    let analysis_pb = start_analysis_spinner(printer.use_color);
    let mut report = NixErrorParser::new(text_to_parse).parse();
    report = suggest::enrich(report);
    analysis_pb.finish_and_clear();

    if !notices.is_empty() {
        print_notices(&notices, printer);
    }

    if cli.json {
        match serde_json::to_string_pretty(&report) {
            Ok(j) => println!("{}", j),
            Err(e) => eprintln!("JSON serialization failed: {}", e),
        }
    } else {
        printer.print(&report);
    }

    true
}

// ═══════════════════════════════════════════════════════════════════════════
// Separate Nix notices/warnings from actual errors
// ═══════════════════════════════════════════════════════════════════════════

/// Nix prints warnings and notices to stderr even on success.
/// These lines start with "warning:" or are known notice patterns.
/// Returns (notices, error_text) where error_text contains the actual error.
fn separate_notices(stderr: &str) -> (Vec<String>, String) {
    let known_notice_patterns = [
        "warning:",
        "Using saved setting for",
        "ignoring untrusted",
        "Pass '--accept-flake-config'",
        "you are not a trusted user",
        "Run `man nix.conf`",
    ];

    let mut notices = Vec::new();
    let mut error_lines = Vec::new();
    let mut in_error_block = false;

    for line in stderr.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("error:") || trimmed.starts_with("error ") {
            in_error_block = true;
        }

        let is_notice = known_notice_patterns
            .iter()
            .any(|pat| trimmed.contains(pat));
        if is_notice && !in_error_block {
            notices.push(line.to_string());
        } else {
            error_lines.push(line.to_string());
        }
    }

    // If we found no error block AND only notices exist, there is no real error
    let error_text = if error_lines.is_empty() && !notices.is_empty() {
        String::new()
    } else if error_lines.is_empty() {
        stderr.to_string()
    } else {
        error_lines.join("\n")
    };

    (notices, error_text)
}

fn print_notices(notices: &[String], printer: &Printer) {
    let mut stdout = io::stdout();
    let _ = writeln!(&mut stdout);
    let _ = writeln!(&mut stdout, "{}", printer.dim("Notices:"));
    for notice in notices {
        let _ = writeln!(&mut stdout, "  {}", printer.dim(notice));
    }
    let _ = writeln!(&mut stdout);
}

// ═══════════════════════════════════════════════════════════════════════════
// Spinner helpers
// ═══════════════════════════════════════════════════════════════════════════

fn start_spinner(desc: &str, use_color: bool) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    let tick_strings: Vec<String> = if use_color {
        vec![
            "❄️ ".to_string(),
            "🌨️ ".to_string(),
            "⛄ ".to_string(),
            "🌨️ ".to_string(),
            "❄️ ".to_string(),
        ]
    } else {
        vec![
            "⠋".to_string(),
            "⠙".to_string(),
            "⠹".to_string(),
            "⠸".to_string(),
            "⠼".to_string(),
            "⠴".to_string(),
            "⠦".to_string(),
            "⠧".to_string(),
            "⠇".to_string(),
            "⠏".to_string(),
        ]
    };
    let tick_refs: Vec<&str> = tick_strings.iter().map(|s| s.as_str()).collect();
    let style = ProgressStyle::default_spinner()
        .tick_strings(&tick_refs)
        .template("{spinner} {msg}")
        .unwrap();
    pb.set_style(style);
    pb.enable_steady_tick(Duration::from_millis(120));

    if use_color {
        let running = "Running".truecolor(137, 220, 235).to_string();
        let cmd_colored = desc.truecolor(203, 166, 247).to_string();
        pb.set_message(format!("{} {}", running, cmd_colored));
    } else {
        pb.set_message(format!("Running {}", desc));
    }

    pb
}

fn start_analysis_spinner(use_color: bool) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    let tick_strings: Vec<String> = if use_color {
        vec![
            "🔍 ".to_string(),
            "✨ ".to_string(),
            "🔍 ".to_string(),
            "✨ ".to_string(),
            "🔍 ".to_string(),
            "✨ ".to_string(),
        ]
    } else {
        vec![
            "⠋".to_string(),
            "⠙".to_string(),
            "⠹".to_string(),
            "⠸".to_string(),
            "⠼".to_string(),
            "⠴".to_string(),
            "⠦".to_string(),
            "⠧".to_string(),
            "⠇".to_string(),
            "⠏".to_string(),
        ]
    };
    let tick_refs: Vec<&str> = tick_strings.iter().map(|s| s.as_str()).collect();
    let style = ProgressStyle::default_spinner()
        .tick_strings(&tick_refs)
        .template("{spinner} {msg}")
        .unwrap();
    pb.set_style(style);
    pb.enable_steady_tick(Duration::from_millis(120));

    if use_color {
        let msg = "Analyzing error trace...".to_string();
        let colored = msg.truecolor(250, 179, 135).to_string();
        pb.set_message(colored);
    } else {
        pb.set_message("Analyzing error trace...".to_string());
    }

    pb
}

// ═══════════════════════════════════════════════════════════════════════════
// Success / no-error banners
// ═══════════════════════════════════════════════════════════════════════════

fn print_success_banner(cmd: &str, use_color: bool) {
    let width = terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(60)
        .min(80);

    if use_color {
        let line = "━".repeat(width).truecolor(166, 227, 161).to_string();
        let ok = "✅".to_string();
        let label = format!("{}   No errors detected!   {}", ok, cmd)
            .truecolor(166, 227, 161)
            .bold()
            .to_string();
        println!("\n{}", line);
        println!("{}", label);
        println!("{}\n", line);
    } else {
        let line = "=".repeat(width);
        println!("\n{}", line);
        println!("[OK]   No errors detected!   {}", cmd);
        println!("{}\n", line);
    }
}

fn print_no_errors(use_color: bool) {
    if use_color {
        println!(
            "\n{}  {}",
            "✅".to_string(),
            "No errors found in provided stderr."
                .truecolor(166, 227, 161)
                .to_string()
        );
    } else {
        println!("\n[OK] No errors found in provided stderr.");
    }
}
