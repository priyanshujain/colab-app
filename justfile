set shell := ["bash", "-euo", "pipefail", "-c"]

app := "Colab Notebook"
bundle := "target/release/bundle/macos"
identity := env("APPLE_SIGNING_IDENTITY", "Developer ID Application: Priyanshu Jain (TQV87WLXK3)")

default:
    @just --list

dev:
    npm --prefix ui install
    cd src-tauri && ../ui/node_modules/.bin/tauri dev

test:
    cargo test --workspace
    npm --prefix ui run check
    npm --prefix ui test

build:
    npm --prefix ui install
    cd src-tauri && APPLE_SIGNING_IDENTITY="{{identity}}" ../ui/node_modules/.bin/tauri build --bundles app

install: build
    #!/usr/bin/env bash
    set -euo pipefail
    src="{{bundle}}/{{app}}.app"
    if [ -w /Applications ]; then dir=/Applications; else dir="$HOME/Applications"; mkdir -p "$dir"; fi
    dest="$dir/{{app}}.app"
    running() { pgrep -f "{{app}}.app/Contents/MacOS/colab-app$" > /dev/null; }
    if running; then
      osascript -e 'quit app "{{app}}"' 2> /dev/null || true
      for _ in $(seq 40); do running || break; sleep 0.25; done
      if running; then echo "just: {{app}} is still running; quit it and try again." >&2; exit 1; fi
    fi
    rm -rf "$dest"
    mv "$src" "$dest"
    echo "Installed to $dest"
    open "$dest"
