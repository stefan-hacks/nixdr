# nixdr — Nix Error Doctor

A blazingly fast Rust tool that reads Nix error messages and produces
human-readable, colorized, actionable diagnostics.

## Why nixdr?

Nix error traces are printed **bottom-up**: the first frame is deep inside
nixpkgs; the last frame is your code. `nixdr` restructures them into a
natural top-down flow, classifies the error into one of five known patterns,
and attaches context-aware fix suggestions.

## Installation

### Via Nix flake

```bash
nix run github:stefan-hacks/nixdr -- build .
```

### From source

```bash
git clone https://github.com/stefan-hacks/nixdr.git
cd nixdr
cargo build --release
sudo cp target/release/nixdr /usr/local/bin/
```

## Usage

### Wrapper mode (recommended)

Replace `nix` commands with `nixdr` equivalents — it runs the underlying
`nix` command, captures stderr, and pretty-prints any errors:

```bash
nixdr build .                    # nix build with error diagnosis
nixdr eval --expr '...'          # nix eval with error diagnosis
nixdr check                      # nix flake check with error diagnosis
nixdr rebuild switch --flake .#  # nixos-rebuild with error diagnosis
nixdr develop .                  # nix develop with error diagnosis
nixdr run nixpkgs#hello          # nix run with error diagnosis
```

### Pipe mode

```bash
nix build . --show-trace 2>&1 | nixdr --stdin
```

### JSON output (for CI / editors)

```bash
nix build . --show-trace 2>&1 | nixdr --stdin --json
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

## NixOS / Home Manager Integration

Add to your `home.packages` or `environment.systemPackages`:

```nix
nixdr.packages.${pkgs.system}.default
```

Or use the flake output directly in a shell:

```nix
{ inputs, pkgs, ... }:
{
  home.packages = [ inputs.nixdr.packages.${pkgs.system}.default ];
}
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