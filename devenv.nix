{ pkgs, lib, ... }:

{
  # Enable Rust toolchain (uses nixpkgs default)
  languages.rust.enable = true;

  # Additional packages
  packages = with pkgs; [
    graphviz # For DOT graph visualization
    d2 # For D2 diagram rendering
    python3 # For hooks
    sccache # Rust compilation cache (faster rebuilds)
    lld # Fast linker (2-5x faster linking)
    cargo-sweep # Remove old build artifacts (cargo sweep --time 30)
    opencode # AI coding agent for the terminal
    # Fonts (for copying to static folder)
    inter
    jetbrains-mono
  ];

  # Browser automation for testing
  claude.code.mcpServers.playwright = {
    type = "stdio";
    command = "npx";
    args = [ "@playwright/mcp@latest" ];
  };

  # Gemini UX review via consult-llm-mcp (requires GEMINI_API_KEY env var)
  # System prompt configured in ~/.consult-llm-mcp/SYSTEM_PROMPT.md
  claude.code.mcpServers.consult-llm = {
    type = "stdio";
    command = "npx";
    args = [
      "-y"
      "consult-llm-mcp"
    ];
    env = {
      CONSULT_LLM_DEFAULT_MODEL = "gemini-3-pro-preview";
      CONSULT_LLM_ALLOWED_MODELS = "gemini-3-pro-preview";
    };
  };

  # Environment variables
  env = {
    RUST_BACKTRACE = "1";
    RUSTC_WRAPPER = "sccache"; # Use sccache for faster rebuilds
  };

  # Better diffs
  difftastic.enable = true;

  # Git hooks
  git-hooks.hooks = {
    nixpkgs-fmt.enable = true; # Format nix files
    rustfmt.enable = true;
    clippy = {
      enable = true;
      settings.denyWarnings = true; # Match CI strictness
    };

    dg-fmt = {
      enable = true;
      name = "dg fmt";
      entry = "${pkgs.writeShellScript "dg-fmt" ''
        if [ -x ./target/release/dg ]; then
          ./target/release/dg fmt --check
        elif command -v dg &> /dev/null; then
          dg fmt --check
        fi
      ''}";
      files = "\\decisions/.*\\.md$";
      pass_filenames = false;
    };

    dg-lint = {
      enable = true;
      name = "dg lint";
      entry = "${pkgs.writeShellScript "dg-lint" ''
        if [ -x ./target/release/dg ]; then
          ./target/release/dg lint
        elif command -v dg &> /dev/null; then
          dg lint
        fi
      ''}";
      files = "\\decisions/.*\\.md$";
      pass_filenames = false;
    };

    check-devenv = {
      enable = true;
      name = "validate devenv.nix";
      entry = "${pkgs.writeShellScript "check-devenv" ''
        ${pkgs.nix}/bin/nix eval --file devenv.nix --apply 'x: true' 2>&1 || {
          echo "ERROR: devenv.nix failed to evaluate"
          exit 1
        }
      ''}";
      files = "devenv\\.nix$";
      pass_filenames = false;
    };

    check-version-tag = {
      enable = true;
      name = "check version matches tag";
      stages = [ "pre-push" ];
      entry = "${pkgs.writeShellScript "check-version-tag" ''
        # Check if we're pushing a version tag
        while read local_ref local_sha remote_ref remote_sha; do
          if [[ "$local_ref" =~ ^refs/tags/v([0-9]+\.[0-9]+\.[0-9]+)$ ]]; then
            TAG_VERSION="''${BASH_REMATCH[1]}"
            CARGO_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')

            if [ "$TAG_VERSION" != "$CARGO_VERSION" ]; then
              echo "ERROR: Tag version ($TAG_VERSION) doesn't match Cargo.toml version ($CARGO_VERSION)"
              echo "Update Cargo.toml to version $TAG_VERSION before tagging"
              exit 1
            fi
            echo "Version check passed: v$TAG_VERSION"
          fi
        done
      ''}";
      pass_filenames = false;
    };
  };

  # Scripts
  scripts = {
    build.exec = "cargo build";
    release.exec = "cargo build --release";
    test.exec = "cargo test";
    install.exec = "cargo install --path crates/dg-cli && cargo install --path crates/dg-mcp";
    clean.exec = "cargo clean && (cd $DEVENV_ROOT/cc-eval && cargo clean 2>/dev/null || true)";
    sweep.exec = "cargo sweep --time 30 && (cd $DEVENV_ROOT/cc-eval && cargo sweep --time 30 2>/dev/null || true)";

    # CSS build commands (uses Tailwind v4 standalone for DaisyUI support)
    css-build.exec = "$DEVENV_ROOT/src/serve/static/tailwindcss -i src/serve/static/input.css -o src/serve/static/tailwind.css --minify";
    css-watch.exec = "$DEVENV_ROOT/src/serve/static/tailwindcss -i src/serve/static/input.css -o src/serve/static/tailwind.css --watch";

    # Full build (CSS + release binary)
    build-all.exec = ''
      echo "Building CSS..."
      "$DEVENV_ROOT/src/serve/static/tailwindcss" -i src/serve/static/input.css -o src/serve/static/tailwind.css --minify
      echo "Building release binary..."
      cargo build --release
    '';

    # dg alias - uses release build if available, falls back to debug
    dg.exec = ''
      if [ -x "$DEVENV_ROOT/target/release/dg" ]; then
        "$DEVENV_ROOT/target/release/dg" "$@"
      elif [ -x "$DEVENV_ROOT/target/debug/dg" ]; then
        "$DEVENV_ROOT/target/debug/dg" "$@"
      else
        echo "dg not built. Run: cargo build"
        exit 1
      fi
    '';

    # cc-eval - Claude Code evaluation runner
    cc-eval.exec = ''
      if [ -x "$DEVENV_ROOT/cc-eval/target/release/cc-eval" ]; then
        "$DEVENV_ROOT/cc-eval/target/release/cc-eval" "$@"
      elif [ -x "$DEVENV_ROOT/cc-eval/target/debug/cc-eval" ]; then
        "$DEVENV_ROOT/cc-eval/target/debug/cc-eval" "$@"
      else
        echo "cc-eval not built. Building..."
        (cd "$DEVENV_ROOT/cc-eval" && cargo build --release)
        "$DEVENV_ROOT/cc-eval/target/release/cc-eval" "$@"
      fi
    '';

    # Build cc-eval
    build-eval.exec = "(cd $DEVENV_ROOT/cc-eval && cargo build --release)";

    # Full eval pipeline: build → unit tests → integration tests → eval run
    test-eval.exec = ''
      set -euo pipefail

      echo "=== Step 1/5: Build workspace ==="
      cargo build --release

      echo ""
      echo "=== Step 2/5: Build cc-eval ==="
      (cd "$DEVENV_ROOT/cc-eval" && cargo build --release)

      echo ""
      echo "=== Step 3/5: Workspace unit tests ==="
      cargo test

      echo ""
      echo "=== Step 4/5: cc-eval unit tests ==="
      (cd "$DEVENV_ROOT/cc-eval" && cargo test)

      echo ""
      echo "=== Step 5/5: Eval run ==="
      "$DEVENV_ROOT/cc-eval/target/release/cc-eval" run --no-container "$@"
    '';
    test-eval-unit.exec = "(cd $DEVENV_ROOT/cc-eval && cargo test)";
  };

  # ============================================================================
  # Claude Code Integration
  # ============================================================================

  claude.code.enable = true;

  # Permissions
  claude.code.permissions = {
    defaultMode = "default";

    rules = {
      Bash = {
        allow = [
          "dg:*" # All dg commands
          "cc-eval:*" # Claude Code evaluation runner
          "d2:*" # D2 diagram rendering
          "cargo:*" # Rust build
          "git:*" # Git operations
          "ls:*"
          "cat:*"
        ];
        deny = [
          "rm -rf:*"
          "sudo:*"
        ];
      };
    };
  };

  # Hooks
  claude.code.hooks = {
    stop = {
      enable = true;
      name = "Verify build, types, tests, and clean git";
      hookType = "Stop";
      command = toString (pkgs.writeShellScript "claude-stop-hook" ''
        set -eo pipefail
        ROOT="''${DEVENV_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
        cd "$ROOT"
        D=$(mktemp -d)
        trap 'rm -rf "$D"' EXIT

        # Build release binary first (synchronous) so target/release/dg is always up to date
        if ! cargo build --release >"$D/build.log" 2>&1; then
          echo "FAIL" > "$D/build"
        fi

        # Tests and UI checks run in parallel (use different profiles, safe to overlap)
        (cargo test >"$D/test.log" 2>&1 || echo "FAIL" > "$D/test") &
        if [ -d "$ROOT/ui" ]; then
          (cd "$ROOT/ui" && bun run check >"$D/svelte.log" 2>&1 || echo "FAIL" > "$D/svelte") &
        fi
        wait

        # Collect errors — only show last 5 lines of each failure
        E=""
        [ -f "$D/build" ] && E="$E\n- cargo build --release failed:\n$(grep -E '^error' "$D/build.log" | head -5)"
        [ -f "$D/svelte" ] && E="$E\n- svelte-check failed:\n$(grep 'ERROR' "$D/svelte.log" | head -5)"
        [ -f "$D/test" ] && E="$E\n- cargo test failed:\n$(grep -E '^(error|test .* FAILED|FAILED)' "$D/test.log" | head -5)"
        [ -n "$(git status --porcelain)" ] && E="$E\n- Uncommitted files exist. Please commit your changes."

        if [ -n "$E" ]; then
          echo -e "Stop hook found issues:\n$E" >&2
          echo "" >&2
          echo "Please fix these issues and try again." >&2
          exit 2
        fi
        echo "All checks passed." >&2
      '');
    };
  };

  # Slash commands
  claude.code.commands = {
    dg-stats = "dg stats";
    dg-graph = "dg graph";
    dg-list = "dg list";
    dg-principles = "dg principles";
    dg-suggest = "dg suggest";
    dg-serve = "dg serve --open";
  };

  # Shell hook
  enterShell = ''
    # Remind to build if dg doesn't exist
    if [ ! -x "$DEVENV_ROOT/target/release/dg" ] && [ ! -x "$DEVENV_ROOT/target/debug/dg" ]; then
      echo ""
      echo "NOTE: Local dg not built yet. Run: cargo build --release"
    fi

    # Warn if target/ is too large (> 5GB)
    if [ -d "$DEVENV_ROOT/target" ]; then
      TARGET_SIZE_KB=$(du -sk "$DEVENV_ROOT/target" 2>/dev/null | cut -f1)
      TARGET_SIZE_GB=$((TARGET_SIZE_KB / 1024 / 1024))
      if [ "$TARGET_SIZE_GB" -ge 5 ]; then
        echo ""
        echo "⚠️  WARNING: target/ is ''${TARGET_SIZE_GB}GB - stale build artifacts accumulating"
        echo "   Run: sweep (removes artifacts older than 30 days)"
        echo "   Or:  clean (full clean, slower rebuild)"
      fi
    fi

    # Copy fonts from Nix store to static folder for rust-embed
    mkdir -p "$DEVENV_ROOT/src/serve/static/fonts"
    cp ${pkgs.inter}/share/fonts/truetype/InterVariable.ttf "$DEVENV_ROOT/src/serve/static/fonts/" 2>/dev/null || true
    cp ${pkgs.jetbrains-mono}/share/fonts/WOFF2/JetBrainsMono-Regular.woff2 "$DEVENV_ROOT/src/serve/static/fonts/" 2>/dev/null || true

    # Download Tailwind CSS v4 standalone (required for DaisyUI v5)
    if [ ! -f "$DEVENV_ROOT/src/serve/static/tailwindcss" ]; then
      echo "Downloading Tailwind CSS v4 standalone..."
      ARCH=$(uname -m)
      case "$ARCH" in
        arm64|aarch64) BINARY="tailwindcss-macos-arm64" ;;
        x86_64) BINARY="tailwindcss-macos-x64" ;;
        *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
      esac
      curl -sL "https://github.com/tailwindlabs/tailwindcss/releases/latest/download/$BINARY" -o "$DEVENV_ROOT/src/serve/static/tailwindcss"
      chmod +x "$DEVENV_ROOT/src/serve/static/tailwindcss"
    fi

    # Download DaisyUI standalone bundles if not present
    if [ ! -f "$DEVENV_ROOT/src/serve/static/daisyui.mjs" ]; then
      echo "Downloading DaisyUI bundle..."
      curl -sL "https://github.com/saadeghi/daisyui/releases/latest/download/daisyui.mjs" -o "$DEVENV_ROOT/src/serve/static/daisyui.mjs"
      curl -sL "https://github.com/saadeghi/daisyui/releases/latest/download/daisyui-theme.mjs" -o "$DEVENV_ROOT/src/serve/static/daisyui-theme.mjs"
    fi
  '';
}
