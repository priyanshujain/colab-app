# Building and Releasing

The repository is a Rust workspace with a Svelte UI. `just` wraps the common tasks. The source is at [github.com/priyanshujain/colab-app](https://github.com/priyanshujain/colab-app).

## Requirements

- Rust, stable. The workspace uses edition 2024.
- Go on your `PATH`. The server build compiles the vendored libtailscale in `crates/server/libtailscale`.
- Node.js with npm, for the UI in `ui/` and the Tauri CLI.
- [just](https://github.com/casey/just), to run the recipes below.

## Where things are

| Path | Contents |
|---|---|
| `src-tauri/` | The Tauri app: Rust shell and `tauri.conf.json`. |
| `crates/` | The server and library crates. See [Crates](process-model.md#crates). |
| `ui/` | The Svelte 5 UI, built with Vite. |
| `ui/src/bindings/` | TypeScript types that ts-rs generates from Rust. `.cargo/config.toml` sets the folder. |
| `skills/` | The agent skills: `colab-notebook` and `install-colab-notebook`. |
| `docs/` | These docs, an mdBook with its sources in `docs/src/`. |

## Recipes

| Command | What it does |
|---|---|
| `just dev` | Installs UI packages and runs `tauri dev` with the Vite dev server. |
| `just test` | Runs `cargo test --workspace`, the Svelte type check and the Vitest tests. |
| `just build` | Builds a signed `Colab Notebook.app` into `target/release/bundle/macos`. |
| `just install` | Builds, quits a running copy, moves the app to `/Applications` (or `~/Applications`) and opens it. |

`just build` signs with the identity in `APPLE_SIGNING_IDENTITY`. Set it to your own Developer ID certificate before you build. A build with another signature makes macOS ask for your password when the app reads the Keychain.

In the browser, `npm --prefix ui run dev` runs the UI alone against a mock API in `ui/src/lib/mock.ts`. That is handy for layout work.

## The server alone

The server builds on macOS and Linux without the UI:

```sh
cargo build --release -p colab-server
cargo test -p colab-server
```

The `server` GitHub workflow builds and tests it on Ubuntu for every push and pull request. See [Run the server on Linux](remote-hosts.md#run-the-server-on-linux) to install it.

## Tests

Rust integration tests live in `crates/*/tests`. UI tests sit next to their code as `ui/src/lib/*.test.ts`. Some tests start real kernels in a uv project, so they need uv and Python on the machine. The CI job installs uv for that reason.

## Releases

A release bumps only the patch number, for example 0.1.9 to 0.1.10. The version lives once, in `[workspace.package]` of the root `Cargo.toml`. Every crate and the app read it from there.

1. Make sure the tree is clean and `just test` passes.
1. Bump the version in the root `Cargo.toml`.
1. Build the app with `--bundles app`, sign it with a Developer ID and notarize it.
1. Make the DMG with `hdiutil`, then sign, notarize and staple it. Check the app and the DMG with `spctl`.
1. Commit `Cargo.toml` and `Cargo.lock` as `release X.Y.Z`, tag `vX.Y.Z` and push both.
1. Create the GitHub release with `Colab-Notebook-X.Y.Z-aarch64.dmg` attached.

> Tauri's own DMG bundler fails on current macOS, because `hdiutil` prints a deprecation warning. So the release builds only the app bundle and makes the DMG with `hdiutil` directly.

## The docs

These docs are an [mdBook](https://rust-lang.github.io/mdBook/). The sources are Markdown files in `docs/src/`. `docs/src/SUMMARY.md` sets the sidebar order and the group titles. `docs/book.toml` holds the settings, and `docs/calm.css` adds a few style rules. Images go in `docs/src/images/`.

To preview the docs while you edit, install mdBook 0.5.4 and run:

```sh
just docs
```

That runs `mdbook serve docs`. It serves the docs at `http://localhost:3000` and rebuilds them on each change. `mdbook build docs` writes the static output to `docs/book/`, which git ignores.

To add a page, write a Markdown file in `docs/src/` and add one line for it to `SUMMARY.md`. Link to other pages by their `.md` file names. The Skills page includes the files in `skills/` with `\{{#include}}`, so a change to a skill shows up in the docs at the next build.

The `pages` workflow builds the docs with a pinned mdBook binary on each push to `main` that touches `docs/`. It then publishes `docs/book/` to GitHub Pages. The Markdown notes at the top of `docs/` are not part of the book and stay out of git.
