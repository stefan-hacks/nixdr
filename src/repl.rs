//! repl.rs — Interactive flake exploration, powered by `nix eval --json`
//!
//! Usage:
//!   nixdr repl .                       # Explore current flake
//!   nixdr repl github:stefan-hacks/nixit  # Explore remote flake
//!   nixdr repl github:stefan-hacks/nixit#nixosConfigurations.ghost.config  # Deep path
//!
//! Commands:
//!   ls            — list current level attributes
//!   cd <path>     — navigate into an attribute
//!   cat <attr>    — show attribute value (JSON pretty-printed)
//!   tree          — show tree of current level
//!   find <pattern> — search for attributes matching pattern
//!   info          — show current path and flake info
//!   exit / quit   — leave the repl

use std::io::{self, BufRead, Write};
use owo_colors::OwoColorize;
use serde_json::Value;

use crate::printer::Printer;

/// State for the interactive repl.
struct ReplState {
    /// The flake reference (e.g. "github:stefan-hacks/nixit")
    flake_ref: String,
    /// Current navigation path (e.g. ["nixosConfigurations", "ghost", "config"])
    path: Vec<String>,
    /// Cached JSON for current level
    current: Value,
}

pub fn run(flake_ref: &str, printer: &Printer) {
    let mut state = match init_state(flake_ref, printer) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{} {}", "❌".truecolor(243, 139, 168), e.truecolor(243, 139, 168));
            std::process::exit(1);
        }
    };

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    print_banner(&mut stdout, printer);
    print_prompt(&mut stdout, &state, printer);

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            print_prompt(&mut stdout, &state, printer);
            continue;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        match parts[0] {
            "exit" | "quit" | "q" => break,
            "help" | "h" | "?" => print_help(&mut stdout, printer),
            "info" => print_info(&mut stdout, &state, printer),
            "ls" => cmd_ls(&mut stdout, &state, printer),
            "cd" => {
                if parts.len() < 2 {
                    let _ = writeln!(&mut stdout,
                        "{} {}",
                        "⚠️".truecolor(250, 179, 135),
                        "Usage: cd <attribute>".truecolor(250, 179, 135)
                    );
                } else {
                    cmd_cd(&mut state, parts[1], printer);
                }
            }
            "cat" => {
                if parts.len() < 2 {
                    let _ = writeln!(&mut stdout,
                        "{} {}",
                        "⚠️".truecolor(250, 179, 135),
                        "Usage: cat <attribute>".truecolor(250, 179, 135)
                    );
                } else {
                    cmd_cat(&mut stdout, &state, parts[1], printer);
                }
            }
            "tree" => cmd_tree(&mut stdout, &state, printer),
            "find" => {
                let pattern = parts.get(1).unwrap_or(&"");
                cmd_find(&mut stdout, &state, pattern, printer);
            }
            other => {
                // Try implicit cd on unknown input
                let _ = writeln!(
                    &mut stdout,
                    "{} Unknown command '{}'. Type 'help' for commands.",
                    "❓".truecolor(250, 179, 135),
                    other
                );
            }
        }

        print_prompt(&mut stdout, &state, printer);
    }

    let _ = writeln!(&mut stdout,
        "\n{}",
        "Goodbye ❄️".truecolor(137, 220, 235)
    );
}

fn init_state(flake_ref: &str, printer: &Printer) -> Result<ReplState, String> {
    // Strip any #fragment for the base flake ref
    let (base_ref, fragment) = if let Some(pos) = flake_ref.find('#') {
        let (a, b) = flake_ref.split_at(pos);
        (a, Some(&b[1..]))
    } else {
        (flake_ref, None)
    };

    // If there's a fragment with dots (a path), we'll handle it after loading base
    let path: Vec<String> = fragment
        .filter(|f| f.contains('.'))
        .map(|f| f.split('.').map(String::from).collect())
        .unwrap_or_default();

    let json = eval_flake_attr(base_ref, &[], printer)?;
    Ok(ReplState {
        flake_ref: base_ref.to_string(),
        path,
        current: json,
    })
}

fn eval_flake_attr(flake_ref: &str, path: &[String], _printer: &Printer) -> Result<Value, String> {
    let mut args = vec!["eval", "--json"];

    let attr_path = if path.is_empty() {
        flake_ref.to_string()
    } else {
        format!("{}#{}", flake_ref, path.join("."))
    };

    args.push(&attr_path);

    let output = std::process::Command::new("nix")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to run nix eval: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Evaluation failed: {}",
            stderr.lines().next().unwrap_or("")
        ));
    }

    serde_json::from_slice(&output.stdout).map_err(|e| format!("JSON parse error: {}", e))
}

fn print_banner(stdout: &mut io::StdoutLock, printer: &Printer) {
    let width = terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(60)
        .min(80);
    let sep = "━".repeat(width);
    let _ = writeln!(stdout, "\n{}", sep.truecolor(203, 166, 247));
    let _ = writeln!(
        stdout,
        "  {}  {}",
        "❄️".truecolor(137, 220, 235),
        "nixdr repl — Interactive Flake Explorer".truecolor(203, 166, 247).bold()
    );
    let _ = writeln!(
        stdout,
        "  {} {}",
        "  ",
        printer.dim("Navigate any flake with ls, cd, cat, tree, find")
    );
    let _ = writeln!(stdout, "{}\n", sep.truecolor(203, 166, 247));
}

fn print_prompt(stdout: &mut io::StdoutLock, state: &ReplState, _printer: &Printer) {
    let path_str = if state.path.is_empty() {
        "~".truecolor(137, 220, 235).to_string()
    } else {
        state.path.join(".").truecolor(137, 220, 235).to_string()
    };
    let _ = write!(
        stdout,
        "{} {} {} ",
        "❄️".truecolor(203, 166, 247),
        path_str,
        "▸".truecolor(108, 112, 134)
    );
    let _ = stdout.flush();
}

fn print_help(stdout: &mut io::StdoutLock, printer: &Printer) {
    let _ = writeln!(
        stdout,
        "\n{}",
        "Commands:".truecolor(203, 166, 247).bold()
    );
    let cmds = [
        ("ls", "List attributes at current level"),
        ("cd <attr>", "Navigate into an attribute"),
        ("cat <attr>", "Show attribute value (pretty-printed)"),
        ("tree", "Show tree view of current level"),
        ("find <pattern>", "Search for attributes by name"),
        ("info", "Show current flake and path"),
        ("help", "Show this help"),
        ("exit / quit", "Leave the repl"),
    ];
    for (cmd, desc) in cmds {
        let _ = writeln!(
            stdout,
            "  {} {:16} {}",
            "│".truecolor(108, 112, 134),
            cmd.truecolor(137, 220, 235),
            printer.dim(desc)
        );
    }
}

fn print_info(stdout: &mut io::StdoutLock, state: &ReplState, _printer: &Printer) {
    let _ = writeln!(
        stdout,
        "{} {}",
        "📦 Flake:".truecolor(203, 166, 247),
        state.flake_ref.truecolor(137, 220, 235)
    );
    let path_str = state.path.join(".");
    let _ = writeln!(
        stdout,
        "{} {}",
        "🧭 Path:  ".truecolor(203, 166, 247),
        if state.path.is_empty() {
            "~".truecolor(108, 112, 134).to_string()
        } else {
            path_str.truecolor(137, 220, 235).to_string()
        }
    );
    let type_name = match state.current {
        Value::Object(_) => "attribute set",
        Value::Array(_) => "list",
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Bool(_) => "boolean",
        Value::Null => "null",
    };
    let _ = writeln!(
        stdout,
        "{} {}",
        "📄 Type:  ".truecolor(203, 166, 247),
        type_name.truecolor(250, 179, 135)
    );
}

fn cmd_ls(stdout: &mut io::StdoutLock, state: &ReplState, printer: &Printer) {
    match &state.current {
        Value::Object(map) => {
            let _ = writeln!(stdout, "\n{}", "Attributes:".truecolor(203, 166, 247).bold());
            for (i, (k, v)) in map.iter().enumerate() {
                let marker = if i == map.len() - 1 { "└─" } else { "├─" };
                let type_icon = json_type_icon(v);
                let _ = writeln!(
                    stdout,
                    "  {} {} {:24} {}",
                    marker.truecolor(108, 112, 134),
                    type_icon,
                    k.truecolor(137, 220, 235),
                    json_type_name(v).truecolor(108, 112, 134)
                );
            }
            let _ = writeln!(stdout);
        }
        Value::Array(arr) => {
            let _ = writeln!(stdout, "\n{}", format!("List ({} items):", arr.len()).truecolor(203, 166, 247).bold());
            for (i, v) in arr.iter().enumerate().take(20) {
                let marker = if i == arr.len() - 1 || i == 19 { "└─" } else { "├─" };
                let _ = writeln!(
                    stdout,
                    "  {} [{:3}] {}",
                    marker.truecolor(108, 112, 134),
                    i.to_string().truecolor(137, 220, 235),
                    preview_value(v, 40).truecolor(108, 112, 134)
                );
            }
            if arr.len() > 20 {
                let _ = writeln!(stdout, "  {} {}", "└─".truecolor(108, 112, 134), printer.dim(&format!("… and {} more", arr.len() - 20)));
            }
            let _ = writeln!(stdout);
        }
        other => {
            let _ = writeln!(
                stdout,
                "{} {}",
                "📄 Value:".truecolor(203, 166, 247),
                preview_value(other, 200).truecolor(137, 220, 235)
            );
        }
    }
}

fn cmd_cd(state: &mut ReplState, attr: &str, _printer: &Printer) {
    match &state.current {
        Value::Object(map) => {
            if let Some(val) = map.get(attr) {
                state.path.push(attr.to_string());
                state.current = val.clone();
            } else {
                eprintln!("{} Attribute '{}' not found.", "❌".truecolor(243, 139, 168), attr);
            }
        }
        Value::Array(arr) => {
            if let Ok(idx) = attr.parse::<usize>() {
                if let Some(val) = arr.get(idx) {
                    state.path.push(attr.to_string());
                    state.current = val.clone();
                } else {
                    eprintln!("{} Index {} out of bounds.", "❌".truecolor(243, 139, 168), idx);
                }
            } else {
                eprintln!("{} Current value is a list. Use numeric index (e.g. cd 0).", "❌".truecolor(243, 139, 168));
            }
        }
        _ => {
            eprintln!("{} Cannot navigate into a scalar value.", "❌".truecolor(243, 139, 168));
        }
    }
}

fn cmd_cat(stdout: &mut io::StdoutLock, state: &ReplState, attr: &str, printer: &Printer) {
    let val = match &state.current {
        Value::Object(map) => map.get(attr).cloned(),
        Value::Array(arr) => attr.parse::<usize>().ok().and_then(|i| arr.get(i).cloned()),
        _ => None,
    };

    match val {
        Some(v) => {
            let pretty = serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string());
            print_colored_json(stdout, &pretty, printer);
        }
        None => {
            let _ = writeln!(
                stdout,
                "{} Not found: '{}'",
                "❌".truecolor(243, 139, 168),
                attr
            );
        }
    }
}

fn cmd_tree(stdout: &mut io::StdoutLock, state: &ReplState, printer: &Printer) {
    let _ = writeln!(stdout, "\n{}", "Tree:".truecolor(203, 166, 247).bold());
    print_tree(stdout, &state.current, "", true, 0, 4, printer);
    let _ = writeln!(stdout);
}

fn print_tree(
    stdout: &mut io::StdoutLock,
    value: &Value,
    prefix: &str,
    is_last: bool,
    depth: usize,
    max_depth: usize,
    printer: &Printer,
) {
    if depth >= max_depth {
        let _ = writeln!(stdout, "{}{} {}", prefix, "└─".truecolor(108, 112, 134), printer.dim("…"));
        return;
    }

    match value {
        Value::Object(map) => {
            for (i, (k, v)) in map.iter().enumerate() {
                let last = i == map.len() - 1;
                let marker = if last { "└─" } else { "├─" };
                let new_prefix = format!("{}{}   ", prefix, if last { " " } else { "│" });
                let _ = writeln!(
                    stdout,
                    "{}{} {:20} {}",
                    prefix,
                    marker.truecolor(108, 112, 134),
                    k.truecolor(137, 220, 235),
                    json_type_name(v).truecolor(108, 112, 134)
                );
                if v.is_object() || v.is_array() {
                    print_tree(stdout, v, &new_prefix, last, depth + 1, max_depth, printer);
                }
            }
        }
        Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                let last = i == arr.len() - 1;
                let marker = if last { "└─" } else { "├─" };
                let new_prefix = format!("{}{}   ", prefix, if last { " " } else { "│" });
                let _ = writeln!(
                    stdout,
                    "{}{} [{:3}] {}",
                    prefix,
                    marker.truecolor(108, 112, 134),
                    i.to_string().truecolor(137, 220, 235),
                    json_type_name(v).truecolor(108, 112, 134)
                );
                if v.is_object() || v.is_array() {
                    print_tree(stdout, v, &new_prefix, last, depth + 1, max_depth, printer);
                }
            }
        }
        other => {
            let marker = if is_last { "└─" } else { "├─" };
            let _ = writeln!(
                stdout,
                "{}{} {}",
                prefix,
                marker.truecolor(108, 112, 134),
                preview_value(other, 40).truecolor(137, 220, 235)
            );
        }
    }
}

fn cmd_find(stdout: &mut io::StdoutLock, state: &ReplState, pattern: &str, _printer: &Printer) {
    let mut results: Vec<(String, &Value)> = Vec::new();
    find_recursive(&state.current, &state.path, pattern, &mut results, 0, 3
    );

    if results.is_empty() {
        let _ = writeln!(
            stdout,
            "{} No attributes matching '{}' found.",
            "🔍".truecolor(108, 112, 134),
            pattern.truecolor(250, 179, 135)
        );
    } else {
        let _ = writeln!(
            stdout,
            "\n{} {}",
            "🔍 Found".truecolor(203, 166, 247),
            format!("{} matches:", results.len()).truecolor(137, 220, 235)
        );
        for (path, val) in &results {
            let _ = writeln!(
                stdout,
                "  {} {} {}",
                "├─".truecolor(108, 112, 134),
                path.truecolor(137, 220, 235),
                json_type_name(val).truecolor(108, 112, 134)
            );
        }
        let _ = writeln!(stdout);
    }
}

fn find_recursive<'a>(
    value: &'a Value,
    path: &[String],
    pattern: &str,
    results: &mut Vec<(String, &'a Value)>,
    depth: usize,
    max_depth: usize,
) {
    if depth >= max_depth {
        return;
    }
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let full_path = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", path.join("."), k)
                };
                if k.contains(pattern) || full_path.contains(pattern) {
                    results.push((full_path.clone(), v));
                }
                find_recursive(v, &[full_path], pattern, results, depth + 1, max_depth);
            }
        }
        Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                let full_path = format!("{}[{}]", path.join("."), i);
                find_recursive(v, &[full_path], pattern, results, depth + 1, max_depth);
            }
        }
        _ => {}
    }
}

// ── JSON pretty-printing with Catppuccin colors ──────────────────────────

fn print_colored_json(stdout: &mut io::StdoutLock, json_str: &str, printer: &Printer) {
    if !printer.use_color {
        let _ = writeln!(stdout, "{}", json_str);
        return;
    }

    let mut indent = 0;
    let mut in_string = false;
    let mut escape = false;
    let mut string_buf = String::new();
    let mut line = String::new();

    for ch in json_str.chars() {
        if in_string {
            if escape {
                string_buf.push(ch);
                escape = false;
                continue;
            }
            if ch == '\\' {
                string_buf.push(ch);
                escape = true;
                continue;
            }
            if ch == '"' {
                string_buf.push(ch);
                line.push_str(&string_buf.truecolor(166, 227, 161).to_string());
                string_buf.clear();
                in_string = false;
                continue;
            }
            string_buf.push(ch);
            continue;
        }

        match ch {
            '"' => {
                in_string = true;
                string_buf.push(ch);
            }
            '{' | '[' => {
                line.push_str(&ch.to_string().truecolor(137, 220, 235).to_string());
                indent += 1;
            }
            '}' | ']' => {
                indent -= 1;
                line.push_str(&ch.to_string().truecolor(137, 220, 235).to_string());
            }
            ':' => {
                line.push_str(&ch.to_string().truecolor(203, 166, 247).to_string());
            }
            ',' => {
                line.push_str(&ch.to_string().truecolor(108, 112, 134).to_string());
            }
            '\n' => {
                let _ = writeln!(stdout, "{}", line);
                line.clear();
                if indent > 0 {
                    line.push_str(&"  ".repeat(indent).truecolor(108, 112, 134).to_string());
                }
            }
            ' ' | '\t' => {
                // skip whitespace outside strings
            }
            c => {
                // numbers, booleans, null
                line.push_str(&c.to_string().truecolor(245, 224, 220).to_string());
            }
        }
    }

    if !line.is_empty() {
        let _ = writeln!(stdout, "{}", line);
    }
    let _ = writeln!(stdout);
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn json_type_icon(v: &Value) -> String {
    let icon = match v {
        Value::Object(_) => "📁",
        Value::Array(_) => "📋",
        Value::String(_) => "📝",
        Value::Number(_) => "🔢",
        Value::Bool(_) => "🔘",
        Value::Null => "⭕",
    };
    icon.to_string()
}

fn json_type_name(v: &Value) -> String {
    match v {
        Value::Object(map) => format!("{{ {} attrs }}", map.len()),
        Value::Array(arr) => format!("[ {} items ]", arr.len()),
        Value::String(s) => {
            if s.len() > 30 {
                format!("\"{}…\"", &s[..27])
            } else {
                format!("\"{}\"", s)
            }
        }
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
    }
}

fn preview_value(v: &Value, max_len: usize) -> String {
    let s = match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    if s.len() > max_len {
        format!("{}…", &s[..max_len])
    } else {
        s
    }
}
