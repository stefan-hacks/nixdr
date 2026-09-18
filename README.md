<div align="center">

# ❄️ nixdr

**Nix Error Doctor — human-readable, colorized, actionable diagnostics**

[![License: MIT](https://img.shields.io/badge/License-MIT-mauve?style=flat-square&color=cba6f7)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021-peach?style=flat-square&color=fab387)](https://www.rust-lang.org/)
[![Nix](https://img.shields.io/badge/Nix-Flakes-sky?style=flat-square&color=89dceb)](https://nixos.org/)
[![Catppuccin](https://img.shields.io/badge/Theme-Mocha-pink?style=flat-square&color=f38ba8)](https://github.com/catppuccin/catppuccin)

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/catppuccin/catppuccin/main/assets/misc/transparent.png">
  <img src="https://raw.githubusercontent.com/catppuccin/catppuccin/main/assets/misc/transparent.png" width="100%" height="0">
</picture>

</div>

> **Nix error traces are printed bottom-up.** The first frame is deep inside nixpkgs; the last frame is your code. `nixdr` restructures them into a natural top-down flow, classifies the error into one of six known patterns, and attaches context-aware fix suggestions.

---

## 🚀 Quick Start

No installation required. Run `nixdr` directly from GitHub:

```bash
# Diagnose a Nix expression
nix run github:stefan-hacks/nixdr -- eval --expr 'let x = x; in x'

# Diagnose a build
nix run github:stefan-hacks/nixdr -- build .

# Diagnose flake checks
nix run github:stefan-hacks/nixdr -- check --show-trace

# Pipe mode: diagnose any Nix command
nix build . --show-trace 2>&1 | nix run github:stefan-hacks/nixdr -- --stdin
```

---

## 📦 Installed Usage

Once `nixdr` is in your `PATH`, use it as a drop-in replacement for `nix` commands.

### Your Own Repository

```bash
cd ~/my-flake

# Check your flake for errors
nixdr check --show-trace

# Evaluate a specific expression
nixdr eval --expr '{ a = 1; }.b'

# Build with diagnosis
nixdr build . --show-trace

# NixOS rebuild with diagnosis
nixdr rebuild switch --flake .#ghost
```

### Other People's Repositories

```bash
# Clone any Nix/NixOS repo and diagnose it
git clone https://github.com/some-user/nixos-config.git /tmp/their-config
cd /tmp/their-config

# Check their flake (dry-run, no changes to your system)
nix flake check 2>&1 | nixdr --stdin

# Evaluate their NixOS configuration without building
nix eval .#nixosConfigurations.hostname.config.system.build.toplevel --show-trace 2>&1 | nixdr --stdin

# Build one of their packages to see if it compiles
nix build .#some-package 2>&1 | nixdr --stdin
```

### Pipe Mode — Retroactive Diagnosis

Already ran a Nix command and got an error? Pipe the stderr through `nixdr`:

```bash
# Basic pipe
nix flake check --show-trace 2>&1 | nixdr --stdin

# JSON output for scripts/CI
nix build . --show-trace 2>&1 | nixdr --stdin --json

# Filter only the error class
nix build . --show-trace 2>&1 | nixdr --stdin --json | jq -r '.class'

# Check if build succeeded programmatically
nix build . 2>&1 | nixdr --stdin && echo "BUILD OK" || echo "BUILD FAILED"
```

### Advanced Usage

```bash
# Show full trace (not filtered to user code)
nixdr check --show-trace --verbose

# Limit trace depth
nixdr eval --expr '...' --show-trace --max-trace-depth 5

# Force color even when piping to less
nix build . 2>&1 | nixdr --stdin --color=always | less -R

# No color (for logs)
nixdr check --color=never
```

---

## 🎬 Visual Experience

When you run `nixdr`, you get live feedback:

### Success — No errors detected

```
❄️  Running nix flake check...
```

Then a green bordered banner:

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✅   No errors detected!   nix flake check
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

### Error detected — Analysis in progress

```
🔍  Analyzing error trace...
```

Then the structured diagnosis is printed with error class, location, trace, and actionable suggestions.

### Warnings separated from errors

Nix prints warnings to stderr even on success. `nixdr` separates them:

```
Notices:
  warning: ignoring untrusted substituter 'https://look.cachix.org'

[UNDEFINED VARIABLE] Undefined variable: 'handy' not found in scope
  at /home/stefan-hacks/.config/nixit/modules/nixos/packages.nix:266:5

Suggestions:
  1. Check for typos in the variable name
  2. If a nixpkgs package, add pkgs. prefix
     Change to: environment.systemPackages = [ pkgs.handy ];
```

---

## 🎨 Before & After

### ❌ Raw Nix Output

```
error: infinite recursion encountered
       at «string»:1:9:
            1| let x = x; in x
             |         ^
```

### ✅ nixdr Output

```
[INFINITE RECURSION] Infinite recursion: an attribute depends on itself
  at «string»:1:9

Details:
  at «string»:1:9:
  1| let x = x; in x
  |         ^

Suggestions:

  1. Add a default value or guard
     An attribute depends on itself. Break the cycle with `lib.mkDefault`.
     Change to:
       myOption = lib.mkDefault "defaultValue";

  2. Check for mutual recursion
     Two or more modules may reference each other. Use `lib.mkForce`.
     Change to:
       config.foo = lib.mkForce "override";
```

---

## 🏷️ Error Classes

| Badge | Class | Trigger | Typical Fix |
|:---:|:---|:---|:---|
| 🔴 | **Infinite Recursion** | Attribute depends on itself | `lib.mkDefault`, `lib.mkForce` |
| 🟠 | **Not a Function** | Value called as function | Check parentheses, argument count |
| 🔵 | **Missing Attribute** | Key not found in set | Typo check, `?` guard, `or` default |
| 🟢 | **Undefined Variable** | Name not in scope | Add `pkgs.`, `let`, or import |
| 🟣 | **Builder Failed** | Compilation/test failure | `nix log`, fix source, check deps |
| 🟡 | **Hash Mismatch** | FOD content changed | Update `sha256`, `cargoHash`, etc. |

---

## 🖌️ Theme

`nixdr` uses the **Catppuccin Mocha** palette:

| Color | Hex | Role |
|:---:|:---:|:---|
| 💜 Mauve | `#cba6f7` | Borders, headings, badges |
| 💚 Green | `#a6e3a1` | OK, success, correct paths |
| ❤️ Red | `#f38ba8` | Errors, infinite recursion |
| 🧡 Peach | `#fab387` | Warnings, suggestions |
| 🩵 Sky | `#89dceb` | Info, locations, attributes |

Disable colors with `--color=never` or `NO_COLOR=1`.

---

## 🔧 Installation

### Nix Run (One-shot, no persistence)

```bash
nix run github:stefan-hacks/nixdr -- --help
```

### Nix Profile (Persistent user install)

```bash
nix profile install github:stefan-hacks/nixdr
nixdr --help
```

### NixOS / Home Manager (Declarative)

```nix
# flake.nix inputs
inputs.nixdr = {
  url = "github:stefan-hacks/nixdr";
  inputs.nixpkgs.follows = "nixpkgs";
};

# configuration.nix
{ inputs, pkgs, ... }:
{
  environment.systemPackages = [
    inputs.nixdr.packages.${pkgs.system}.default
  ];
}

# Or home.nix
{ inputs, pkgs, ... }:
{
  home.packages = [
    inputs.nixdr.packages.${pkgs.system}.default
  ];
}
```

### From Source

```bash
git clone https://github.com/stefan-hacks/nixdr.git
cd nixdr
cargo build --release
# Binary at ./target/release/nixdr
```

---

## 🛠️ Contributing

```bash
# Run tests
cargo test

# Check formatting
cargo fmt --check

# Lint
cargo clippy
```

---

## 📜 License

MIT — see [LICENSE](LICENSE).

---

<div align="center">

Made with ❄️ for the Nix ecosystem

</div>
