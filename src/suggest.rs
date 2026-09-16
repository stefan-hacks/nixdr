//! Suggestion engine — attaches actionable fixes to error reports.
//!
//! Each error class gets a set of context-aware suggestions based on
//! common patterns from the Nix ecosystem and nixit deployments.

use crate::diagnosis::{ErrorClass, ErrorReport, Suggestion};
use crate::trace;

/// Enrich an ErrorReport with actionable suggestions.
pub fn enrich(mut report: ErrorReport) -> ErrorReport {
    let suggestions = match &report.class {
        ErrorClass::InfiniteRecursion => suggest_infinite_recursion(&report),
        ErrorClass::NotAFunction => suggest_not_a_function(&report),
        ErrorClass::AttributeMissing { attribute } => suggest_attribute_missing(&report, attribute),
        ErrorClass::UndefinedVariable { variable } => suggest_undefined_variable(&report, variable),
        ErrorClass::BuilderFailed { drv, exit_code } => suggest_builder_failed(&report, drv, *exit_code),
        ErrorClass::HashMismatch { specified, got } => suggest_hash_mismatch(&report, specified.as_deref(), got.as_deref()),
        ErrorClass::Unknown => suggest_unknown(&report),
    };
    report.suggestions = suggestions;
    report
}

fn suggest_infinite_recursion(report: &ErrorReport) -> Vec<Suggestion> {
    let mut suggs = vec![
        Suggestion {
            title: "Add a default value or guard".to_string(),
            description: "An attribute depends on itself. Break the cycle with `lib.mkDefault` or a conditional.".to_string(),
            command: None,
            code: Some(
                "# Instead of:\n#   myOption = config.myOption + \"extra\";\n\n# Use:\n  myOption = lib.mkDefault \"defaultValue\";".to_string()),
        },
        Suggestion {
            title: "Check for mutual recursion".to_string(),
            description: "Two or more modules may reference each other. Use `lib.mkForce` to override one side.".to_string(),
            command: None,
            code: Some("config.foo = lib.mkForce \"override\";".to_string()),
        },
        Suggestion {
            title: "Use builtins.trace to debug".to_string(),
            description: "Insert trace calls to see which attribute triggers the loop.".to_string(),
            command: report.user_location.as_ref().map(|loc|
                format!("nix eval --file {} --show-trace 2>&1 | nixdr --stdin", loc.file)),
            code: Some("builtins.trace \"Reached here\" value".to_string()),
        },
    ];

    if trace::contains_builtin(report, "map") {
        suggs.push(Suggestion {
            title: "Check for list recursion in map".to_string(),
            description: "Recursive `builtins.map` or `mapAttrs` calls with self-referencing lists cause this.".to_string(),
            command: None,
            code: Some("# Ensure the mapped list is finite and doesn't reference itself".to_string()),
        });
    }

    suggs
}

fn suggest_not_a_function(report: &ErrorReport) -> Vec<Suggestion> {
    let mut suggs = vec![
        Suggestion {
            title: "Check parentheses / argument application".to_string(),
            description: "A value is being called like a function. Verify the expression isn't missing parentheses or an argument.".to_string(),
            command: None,
            code: Some(
                "# Wrong:  myPkg.override { foo = \"bar\"; } extraArg\n# Right:  myPkg.override { foo = \"bar\"; } extraArg\n# Or:     (myPkg.override { foo = \"bar\"; }) extraArg".to_string()),
        },
        Suggestion {
            title: "Verify the value is a function before calling".to_string(),
            description: "Use `builtins.isFunction` or `builtins.typeOf` to inspect the value.".to_string(),
            command: report.user_location.as_ref().map(|loc|
                format!("nix eval --file {} --show-trace", loc.file)),
            code: Some("builtins.typeOf myValue  # → \"function\", \"set\", \"list\", etc.".to_string()),
        },
    ];

    // Detect common pattern: missing parentheses around attrset
    if report.raw.contains("while evaluating a condition") {
        suggs.push(Suggestion {
            title: "Condition expected a boolean, got a function/set".to_string(),
            description: "If conditions in Nix must be booleans. Wrap the expression in `lib.mkIf`.".to_string(),
            command: None,
            code: Some("config = lib.mkIf condition { option = value; };".to_string()),
        });
    }

    suggs
}

fn suggest_attribute_missing(_report: &ErrorReport, attribute: &str) -> Vec<Suggestion> {
    let mut suggs = vec![
        Suggestion {
            title: "Check the attribute name for typos".to_string(),
            description: format!("'{}' doesn't exist in the target set. Verify spelling.", attribute),
            command: None,
            code: None,
        },
        Suggestion {
            title: "Use `?` to check existence before accessing".to_string(),
            description: "Guard attribute access with `or` or `lib.optionalAttrs`.".to_string(),
            command: None,
            code: Some(format!(
                "# Safe access:\nset.{} or default\n\n# Conditional merge:\nlib.optionalAttrs (set ? {}) {{ ... = set.{}; }}"
                , attribute, attribute, attribute)),
        },
        Suggestion {
            title: "Search nixpkgs for the correct attribute".to_string(),
            description: "The attribute may have been renamed or removed.".to_string(),
            command: Some(format!("nix search nixpkgs {}", attribute)),
            code: None,
        },
    ];

    // Detect nixos-option pattern
    if attribute.contains("services.") || attribute.contains("programs.") {
        suggs.push(Suggestion {
            title: "Verify the NixOS module is imported".to_string(),
            description: "NixOS options are only available when their module is loaded.".to_string(),
            command: Some("nixos-option {}".to_string()),
            code: None,
        });
    }

    suggs
}

fn suggest_undefined_variable(report: &ErrorReport, variable: &str) -> Vec<Suggestion> {
    let mut suggs = vec![
        Suggestion {
            title: "Check for typos in the variable name".to_string(),
            description: format!("'{}' is not defined in this scope. Verify spelling.", variable),
            command: None,
            code: None,
        },
        Suggestion {
            title: "Ensure the variable is in scope".to_string(),
            description: "Variables must be defined before use or passed as function arguments.".to_string(),
            command: None,
            code: Some(format!(
                "# Wrong: using an undefined name directly\n  myValue\n\n# Right: define it first\n  let myValue = 42; in myValue\n\n# Or pass as argument:\n  {{ myValue, ... }}: {{ /* ... */ }}"))
        },
        Suggestion {
            title: "If a nixpkgs package, add pkgs. prefix".to_string(),
            description: "Packages in nixpkgs must be referenced from the package set.".to_string(),
            command: Some(format!("nix search nixpkgs {}", variable)),
            code: Some(format!(
                "# Wrong:\n  environment.systemPackages = [ {} ];\n\n# Right:\n  environment.systemPackages = [ pkgs.{} ];", variable, variable)),
        },
        Suggestion {
            title: "If a flake input, ensure it is declared".to_string(),
            description: "Flake inputs must be declared in flake.nix before use in modules.".to_string(),
            command: None,
            code: Some(
                "# In flake.nix inputs:\n  my-input = { url = \"...\"; };\n\n# Then in modules:\n  inherit (inputs) my-input;".to_string()),
        },
    ];

    // Context: systemPackages with missing pkgs. prefix
    if report.raw.contains("environment.systemPackages") || report.raw.contains("environment.packages") {
        suggs.push(Suggestion {
            title: "System packages require pkgs. prefix".to_string(),
            description: "Packages in `environment.systemPackages` must come from `pkgs`.".to_string(),
            command: Some(format!("nix search nixpkgs {}", variable)),
            code: Some(format!(
                "# Change:\n  {}\n\n# To:\n  pkgs.{}", variable, variable)),
        });
    }

    // Context: in a flake.nix inputs block
    if report.raw.contains("flake.nix") || report.raw.contains("inputs.") {
        suggs.push(Suggestion {
            title: "Flake input may not be passed to this module".to_string(),
            description: "Make sure the input is passed via specialArgs or extraSpecialArgs.".to_string(),
            command: None,
            code: Some(
                "# In your host definition:\n  specialArgs = {\n    inherit (inputs) my-input;\n  };".to_string()),
        });
    }

    suggs
}

fn suggest_builder_failed(_report: &ErrorReport, drv: &str, exit_code: i32) -> Vec<Suggestion> {
    let mut suggs = vec![
        Suggestion {
            title: "Inspect the build log".to_string(),
            description: "The derivation failed during build. Read the full log for the actual compiler/script error.".to_string(),
            command: Some(format!("nix log {}", drv)),
            code: None,
        },
        Suggestion {
            title: "Enter the build shell and debug interactively".to_string(),
            description: "Run the build phases manually to isolate the failure.".to_string(),
            command: Some(format!("nix develop {} -L\n# or:\nnix-build {} --check", drv, drv)),
            code: None,
        },
        Suggestion {
            title: "Check for missing buildInputs".to_string(),
            description: "The build may fail because a library or tool is not in the build environment.".to_string(),
            command: None,
            code: Some(
                "nativeBuildInputs = with pkgs; [ cmake pkg-config ];\nbuildInputs = with pkgs; [ openssl zlib ];".to_string()),
        },
    ];

    if exit_code == 1 {
        suggs.push(Suggestion {
            title: "Exit code 1 is generic — search the log for the real error".to_string(),
            description: "Most builders exit 1 on any failure. Look for 'error:', 'fatal:', or 'undefined reference' in the log.".to_string(),
            command: Some(format!("nix log {} | grep -E 'error:|fatal:|undefined' | head -20", drv)),
            code: None,
        });
    }

    suggs
}

fn suggest_hash_mismatch(
    _report: &ErrorReport,
    specified: Option<&str>,
    got: Option<&str>,
) -> Vec<Suggestion> {
    let mut suggs = vec![
        Suggestion {
            title: "Update the hash to the correct value".to_string(),
            description: "Replace the declared hash with the one Nix computed.".to_string(),
            command: None,
            code: got.map(|g| format!("# Replace:\n#   hash = \"sha256-OLD\";\n# With:\n  hash = \"{}\";", g)),
        },
        Suggestion {
            title: "Use a fake hash to discover the correct one".to_string(),
            description: "Set a known-wrong hash, let Nix tell you the correct one.".to_string(),
            command: None,
            code: Some(
                "# Set:\n  hash = \"sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\";\n# Then run `nix build` and copy the 'got:' hash.".to_string()),
        },
        Suggestion {
            title: "Verify the source URL is still valid".to_string(),
            description: "Hash mismatches often mean upstream changed the tarball or tag.".to_string(),
            command: None,
            code: None,
        },
    ];

    if specified.is_some() && got.is_some() {
        suggs.push(Suggestion {
            title: "Compare the two hashes directly".to_string(),
            description: "The hashes differ. If only a few characters changed, upstream may have republished.".to_string(),
            command: None,
            code: None,
        });
    }

    suggs
}

fn suggest_unknown(report: &ErrorReport) -> Vec<Suggestion> {
    let mut suggs = vec![
        Suggestion {
            title: "Run with --show-trace for full context".to_string(),
            description: "The error is unclassified. Full trace may reveal the pattern.".to_string(),
            command: None,
            code: None,
        },
        Suggestion {
            title: "Check Nix version and nixpkgs channel".to_string(),
            description: "Some errors are version-specific. Ensure your flake.lock is up to date.".to_string(),
            command: Some("nix --version && nix flake metadata".to_string()),
            code: Some("nix flake update".to_string()),
        },
        Suggestion {
            title: "Search the raw error text online".to_string(),
            description: "This exact error may have been solved by the community.".to_string(),
            command: report.summary.split('\n').next().map(|first_line| {
                format!("echo '{}' | xargs -I{{}} firefox 'https://search.nixos.org/packages?query={{}}'", first_line)
            }),
            code: None,
        },
    ];

    if trace::user_frames(report).is_empty() && !report.trace.is_empty() {
        suggs.push(Suggestion {
            title: "No user code in trace — error is in nixpkgs internals".to_string(),
            description: "All trace frames point to /nix/store. This may be a nixpkgs bug or channel mismatch.".to_string(),
            command: Some("nix flake update && nix flake check".to_string()),
            code: None,
        });
    }

    suggs
}
