//! show.rs — `nixdr show <flake-ref>` implementation
//!
//! Displays a structured overview of any flake: inputs, outputs, packages,
//! nixosConfigurations, homeConfigurations, and a summary.
//!
//! Example:
//!   nixdr show .
//!   nixdr show github:stefan-hacks/nixit
//!   nixdr show nixpkgs#hello
//!   nixdr show github:LnL7/nix-darwin

use std::io::{self, Write};
use owo_colors::OwoColorize;
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::Value;

use crate::printer::Printer;

/// Run the `show` command: display structured flake metadata.
pub fn run(flake_ref: &str, printer: &Printer) -> io::Result<()> {
    let pb = ProgressBar::new_spinner();
    let style = ProgressStyle::default_spinner()
        .template("{spinner} {msg}")
        .unwrap()
        .tick_strings(&[
            "❄️ ", "🌨️ ", "❄️ ", "🌨️ ", "❄️ ", "🌨️ ", "❄️ ", "✨",
        ]);
    pb.set_style(style);

    let running = "Inspecting".truecolor(137, 220, 235).to_string();
    let target = flake_ref.truecolor(203, 166, 247).to_string();
    pb.set_message(format!("{} {}", running, target));

    // Fetch flake metadata via `nix flake metadata --json`
    let meta = fetch_flake_metadata(flake_ref);
    pb.finish_and_clear();

    let mut stdout = io::stdout();

    // ── HEADER ──────────────────────────────────────────────
    let width = terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(60)
        .min(80);

    let sep = "─".repeat(width);
    let _ = writeln!(&mut stdout,
        "\n{} {} {}",
        sep.truecolor(203, 166, 247),
        "FLAKE OVERVIEW".truecolor(203, 166, 247).bold(),
        sep.truecolor(203, 166, 247)
    );

    match meta {
        Ok(json) => {
            // Description / URL
            if let Some(url) = json.get("url").and_then(|v| v.as_str()) {
                let _ = writeln!(
                    &mut stdout,
                    "{} {}",
                    "📦".truecolor(137, 220, 235),
                    url.truecolor(203, 166, 247).bold()
                );
            }
            if let Some(desc) = json.get("description").and_then(|v| v.as_str()) {
                let _ = writeln!(
                    &mut stdout,
                    "   {}",
                    printer.dim(desc)
                );
            }

            // ── INPUTS ──────────────────────────────────────────
            if let Some(inputs) = json.get("locks").and_then(|l| l.get("nodes")) {
                if let Some(root) = inputs.get("root").and_then(|r| r.get("inputs")) {
                    let _ = writeln!(
                        &mut stdout,
                        "\n{}",
                        "── Inputs ─────────────────────────────────────────".truecolor(108, 112, 134)
                    );
                    if let Some(map) = root.as_object() {
                        for (name, val) in map.iter().take(10) {
                            let key = name.truecolor(137, 180, 250).to_string();
                            let info = if let Some(url) = val.as_str()
                                .and_then(|s| inputs.get(s))
                                .and_then(|n| n.get("original"))
                                .and_then(|o| o.get("url"))
                                .and_then(|u| u.as_str())
                            {
                                format!(" → {}", url)
                            } else {
                                String::new()
                            };
                            let info_colored = if printer.use_color {
                                info.truecolor(108, 112, 134).to_string()
                            } else {
                                info
                            };
                            let _ = writeln!(&mut stdout,
                                "  {} {}{}",
                                "├─".truecolor(108, 112, 134),
                                key,
                                info_colored
                            );
                        }
                        if map.len() > 10 {
                            let _ = writeln!(
                                &mut stdout,
                                "  {} {}",
                                "└─".truecolor(108, 112, 134),
                                printer.dim(&format!("… and {} more", map.len() - 10))
                            );
                        }
                    }
                }
            }

            // ── OUTPUTS ─────────────────────────────────────────
            if let Some(outputs) = json.get("locks").and_then(|l| l.get("root")).and_then(|r| r.get("inputs")) {
                if let Some(obj) = outputs.as_object() {
                    let _ = writeln!(
                        &mut stdout,
                        "\n{}",
                        "── Outputs ────────────────────────────────────────".truecolor(108, 112, 134)
                    );
                    for (name, _) in obj.iter().take(10) {
                        let _ = writeln!(
                            &mut stdout,
                            "  {} {}",
                            "├─".truecolor(108, 112, 134),
                            name.truecolor(250, 179, 135)
                        );
                    }
                }
            }

            // ── SYSTEMS ─────────────────────────────────────────
            if let Some(path_locked) = json.get("path").and_then(|v| v.as_str()) {
                // Try to get available systems from nix flake show
                let _ = writeln!(
                    &mut stdout,
                    "\n{} {}",
                    "🔒".truecolor(137, 220, 235),
                    format!("Locked: {}", path_locked).truecolor(108, 112, 134)
                );
            }
        }
        Err(e) => {
            let _ = writeln!(
                &mut stdout,
                "\n{} {}",
                "❌".truecolor(243, 139, 168),
                e.truecolor(243, 139, 168)
            );
        }
    }

    let _ = writeln!(
        &mut stdout,
        "{}\n",
        "─".repeat(width).truecolor(203, 166, 247)
    );
    Ok(())
}

fn fetch_flake_metadata(flake_ref: &str) -> Result<Value, String> {
    let output = std::process::Command::new("nix")
        .args([
            "flake",
            "metadata",
            "--json",
            flake_ref,
        ])
        .output()
        .map_err(|e| format!("Failed to run `nix flake metadata`: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "`nix flake metadata` failed ({}): {}",
            output.status.code().unwrap_or(-1),
            stderr.trim().lines().last().unwrap_or("")
        ));
    }

    serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse metadata JSON: {}", e))
}
