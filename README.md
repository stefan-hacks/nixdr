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

> **Nix error traces are printed bottom-up.** The first frame is deep inside nixpkgs; the last frame is your code. `nixdr` restructures them into a natural top-down flow, classifies the error into one of five known patterns, and attaches context-aware fix suggestions.

---

## 🚀 Quick Start (No Installation)

Run `nixdr` directly from GitHub without installing anything:

```bash
# Diagnose a Nix expression
nix run github:stefan-hacks/nixdr -- eval --expr 'let x = x; in x'

# Diagnose a build
nix run github:stefan-hacks/nixdr -- build .

# Diagnose flake checks
nix run github:stefan-hacks/nixdr -- check --show-trace

# Diagnose a NixOS rebuild
nix run github:stefan-hacks/nixdr -- rebuild switch --flake .#

# Pipe mode: diagnose any Nix command
nix build . --show-trace 2>&1 | nix run github:stefan-hacks/nixdr -- --stdin

# JSON output for CI / editors
nix build . --show-trace 2>&1 | nix run github:stefan-hacks/nixdr -- --stdin --json
```

---

## 📦 Installed Usage

Once `nixdr` is in your `PATH` (via `nix run`, `nix profile install`, or NixOS/Home Manager):

### Wrapper Commands

Replace `nix` with `nixdr` to get automatic error diagnosis:

```bash
nixdr build .                          # nix build + diagnosis
nixdr eval --expr '...'                # nix eval + diagnosis
nixdr check --show-trace               # nix flake check + diagnosis
nixdr rebuild switch --flake .#ghost   # nixos-rebuild + diagnosis
nixdr develop .                        # nix develop + diagnosis
nixdr run nixpkgs#hello                # nix run + diagnosis
```

### Pipe Mode

Already ran the command? Pipe stderr retroactively:

```bash
nix build . --show-trace 2>&1 | nixdr --stdin
nix flake check --show-trace 2>&1 | nixdr --stdin
```

### JSON Mode

For CI pipelines, editors, or programmatic consumption:

```bash
nix build . 2>&1 | nixdr --stdin --json | jq '.class'
```

Example output:

```json
{
  "class": "infinite_recursion",
  "summary": "Infinite recursion: an attribute depends on itself",
  "location": { "file": "«string»", "line": 1, "column": 9 },
  "suggestions": [
    {
      "title": "Add a default value or guard",
      "description": "An attribute depends on itself. Break the cycle with `lib.mkDefault` or a conditional.",
      "code": "myOption = lib.mkDefault \"defaultValue\";"
    }
  ]
}
```

---

## 🎬 Visual Experience

When you run `nixdr`, you get live feedback:

### Success — No errors detected

During the Nix command, a Catppuccin-themed spinner runs:

```
❄️  Running nix flake check...
```

On success, a green bordered banner appears:

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✅   No errors detected!   nix flake check
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

### Error detected — Analysis in progress

If an error occurs, the spinner changes:

```
🔍  Analyzing error trace...
```

Then the structured diagnosis is printed with the full error class, location, trace, and actionable suggestions.

### Pipe mode — Retroactive analysis

```bash
nix build . --show-trace 2>&1 | nixdr --stdin
```

The analysis spinner appears briefly, then the full diagnosis is printed.

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

  3. Use builtins.trace to debug
     Insert trace calls to see which attribute triggers the loop.
     Change to:
       builtins.trace "Reached here" value
```

---

## 🏷️ Error Classes

| Badge | Class | Trigger | Typical Fix |
|:---:|:---|:---|:---|
| 🔴 | **Infinite Recursion** | Attribute depends on itself | `lib.mkDefault`, `lib.mkForce` |
| 🟠 | **Not a Function** | Value called as function | Check parentheses, argument count |
| 🔵 | **Missing Attribute** | Key not found in set | Typo check, `?` guard, `or` default |
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

### Nix Run (One-shot)

```bash
nix run github:stefan-hacks/nixdr -- --help
```

### Nix Profile (Persistent)

```bash
nix profile install github:stefan-hacks/nixdr
nixdr --help
```

### NixOS / Home Manager (Declarative)

```nix
# flake.nix
inputs.nixdr = {
  url = "github:stefan-hacks/nixdr";
  inputs.nixpkgs.follows = "nixpkgs";
};

# configuration.nix or home.nix
{ inputs, pkgs, ... }:
{
  environment.systemPackages = [
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
