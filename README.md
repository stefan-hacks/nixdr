# nixdr — Nix Error Doctor

A blazingly fast Rust tool that reads Nix error messages and produces
human-readable, colorized, actionable diagnostics.

**No installation required** — run directly with `nix run`.

## Quick Start (No Install)

Replace any `nix` command with `nix run`:

```bash
# Instead of: nix build . --show-trace
nix run github:stefan-hacks/nixdr -- build .

# Instead of: nix eval --expr '...' --show-trace
nix run github:stefan-hacks/nixdr -- eval --expr 'let x = x; in x'

# Instead of: nix flake check --show-trace
nix run github:stefan-hacks/nixdr -- check

# Instead of: nixos-rebuild switch --flake .#
nix run github:stefan-hacks/nixdr -- rebuild switch --flake .#

# nix develop / nix run work too
nix run github:stefan-hacks/nixdr -- develop .
nix run github:stefan-hacks/nixdr -- run nixpkgs#hello
```

`nixdr` intercepts stderr from the wrapped command, parses any Nix error,
classifies it, and prints a beautiful diagnostic. If the build succeeds,
you see normal output — `nixdr` is silent on success.

### Pipe mode (if you can't replace the command)

```bash
nix build . --show-trace 2>&1 | nix run github:stefan-hacks/nixdr -- --stdin
```

### JSON output (for CI / editors)

```bash
nix build . --show-trace 2>&1 | nix run github:stefan-hacks/nixdr -- --stdin --json
```

## Why nixdr?

Nix error traces are printed **bottom-up**: the first frame is deep inside
nixpkgs; the last frame is your code. `nixdr` restructures them into a
natural top-down flow, classifies the error into one of five known patterns,
and attaches context-aware fix suggestions with code snippets.

**Before:**
```
error: infinite recursion encountered
       at /nix/store/...-nixpkgs/lib/modules.nix:...
       ... 30 more frames ...
       at /home/you/flake.nix:42:5
```

**After (nixdr):**
```
[INFINITE RECURSION] Infinite recursion: an attribute depends on itself
  at «string»:1:9

Suggestions:
  1. Add a default value or guard
     An attribute depends on itself. Break the cycle with `lib.mkDefault`.
     Change to:
       myOption = lib.mkDefault "defaultValue";
```

## Error Classes

| Class | Trigger | Typical Fix |
|---|---|---|
| **Infinite Recursion** | Attribute depends on itself | `lib.mkDefault`, `lib.mkForce` |
| **Not a Function** | Value called as function | Check parentheses, argument count |
| **Missing Attribute** | Key not found in set | Typo check, `?` guard, `or` default |
| **Builder Failed** | Compilation/test failure | `nix log`, fix source, check deps |
| **Hash Mismatch** | FOD content changed | Update `sha256`, `cargoHash`, etc. |

## Colors

`nixdr` uses the **Catppuccin Mocha** palette by default:
- **Mauve** (`#cba6f7`) — borders, headings
- **Green** (`#a6e3a1`) — OK, correct paths
- **Red** (`#f38ba8`) — errors, infinite recursion
- **Peach** (`#fab387`) — warnings
- **Sky** (`#89dceb`) — info, locations

Disable with `--color=never` or `NO_COLOR=1`.

## Installing (Optional)

If you want `nixdr` in your PATH permanently:

### Via Nix flake (declarative)

Add the input to your flake and include the package:

```nix
{
  inputs.nixdr.url = "github:stefan-hacks/nixdr";

  # In your NixOS or Home Manager config:
  environment.systemPackages = [
    inputs.nixdr.packages.${pkgs.system}.default
  ];
}
```

### From source

```bash
git clone https://github.com/stefan-hacks/nixdr.git
cd nixdr
cargo build --release
# Binary at target/release/nixdr
```

## Contributing

```bash
# Run tests
cargo test

# Check formatting
cargo fmt --check

# Lint
cargo clippy
```

## License

MIT