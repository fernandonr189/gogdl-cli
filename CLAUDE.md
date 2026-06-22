# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project overview

`gogdl-cli` is a Rust command-line tool for downloading, configuring, and running GOG games on Linux, with automatic Proton/Wine prefix management and cloud save sync. It's a single-binary clap-based CLI (no workspace, no library crate).

## Project harness (Notion) — read and maintain this

This project has a **Notion harness** that is the source of truth for requirements, architecture, and dependency-API knowledge. It is reached through the configured Notion MCP server. **Read the relevant sections before contributing, and update them after.** If the Notion MCP server is unavailable, proceed using this file and note in your summary that the harness could not be updated.

Harness root: **gogdl-cli** — `https://app.notion.com/p/gogdl-cli-38676f0f9861819d8a6af552c7df7ded`

| Section | Read it to learn… | Update it when… |
| --- | --- | --- |
| **Structure** (`https://app.notion.com/p/Structure-38676f0f986181f9b437ead72288d3d1`) | The blueprint: modules, command dispatch, auth flow, persistence model, transfer/progress pattern, runner flow. | You add or rework a module, command, data flow, or persisted field. |
| **Specifications** (`https://app.notion.com/p/Specifications-38676f0f9861816a9996d5938751078d`) | The requirements: per-command behavior, flags, validations, guarantees. | You introduce a feature, validation, or behavioral change. |
| **Skills** (`https://app.notion.com/p/Skills-38676f0f986181df8a20fb5b20e6a9ed`) | API cheat-sheets for each dependency — especially the **gogdl-lib** and **prefix-manager** child pages (signatures, return types, gotchas not visible in this repo). | You learn a new detail about any dependency's API (new method, surprising return type, error variant, quirk). |
| **Devlog** (`https://app.notion.com/p/Devlog-38676f0f986181fca24be34849bf1706`) | Chronological history of notable changes/decisions/discoveries. | You complete meaningful work — append a dated entry (newest on top). |

Workflow: **before** a change, read Specifications → Structure → the Skills entry for any dependency you'll call. **After** a change, update Specifications/Structure/Skills as applicable, then append a Devlog entry. To edit a page, use the Notion MCP `API-update-page-markdown` tool (`type: update_content` for targeted edits, or `replace_content` to rewrite). Mermaid diagrams are supported via ```` ```mermaid ```` fenced blocks. Keep harness edits scoped to what actually changed.

## Commands

```bash
cargo build              # debug build
cargo build --release    # optimized build (LTO + strip enabled, see Cargo.toml)
cargo run -- <args>      # e.g. cargo run -- games -l
cargo check               # fast type-check
cargo clippy              # lint
cargo fmt                 # format
cargo install --path .   # install the `gogdl` binary locally
```

There is currently no test suite in this repo.

The binary depends on two sibling crates pulled directly from git (pinned by tag/rev in `Cargo.toml`):
- `gogdl-lib` (`fernandonr189/gogdl-lib`) — GOG API client, auth, downloads, cloud saves.
- `prefix-manager` (`fernandonr189/prefix-manager`) — Proton-GE release listing/download.

Functionality that looks missing here (e.g. GOG API endpoints, build/chunk download logic, Proton release fetching) almost always lives in one of those crates, not in this repo. Bumping their version requires editing the `tag`/`rev` in `Cargo.toml`.

## Architecture

### Command dispatch (`src/main.rs`, `src/cli.rs`)

`cli.rs` defines the clap `Commands` enum (`Login`, `Download`, `Games`, `Proton`, `Manage`, `Run`) and the `ManageAction` subcommand enum. `main.rs` matches on the parsed args and dispatches into `src/commands/*`.

Every top-level command follows the same dual-mode pattern: **if invoked with no flags/args, it runs an interactive `dialoguer`-based menu; if invoked with flags, it runs the direct/scriptable path.** E.g. `gogdl proton` opens a menu, `gogdl proton -l` lists versions directly. When adding a new capability, add both an interactive entry point and a direct CLI flag/subcommand for it — see `handle_proton` (interactive) vs `handle_proton_cli` (direct) in `src/commands/proton.rs` as the reference shape, similarly `handle_manage`/`Manage{game_id, action}` and `handle_games`/`list_games_cli`.

`AppSettings::load()` runs once in `main` before dispatch; most command handlers take `&mut AppSettings` and call `settings.save().await` after mutating it.

### Auth (`src/auth.rs`, `src/secret.rs`)

Tokens are stored via `org.freedesktop.secrets` (the `secret-service` crate), not in `AppSettings`/disk JSON. `manage_auth(gogdl)` is the standard pre-flight call for any command that hits the GOG API: it recovers the stored token, validates it, and transparently refreshes + re-stores it on `ClientError::TokenExpired`, or exits(1) instructing the user to `login` on `ClientError::NotLoggedIn`. Call `manage_auth` before any `gogdl_lib::GogDl` call that requires auth (downloads, library listing, cloud saves); it is *not* needed for Proton management, which doesn't touch GOG auth.

### Settings persistence (`src/settings.rs`)

`AppSettings` is the single persisted state blob (JSON, via `directories::ProjectDirs::from("com", "fernandonr189", "gogdl")`):
- Config dir → `settings.json` (the serialized `AppSettings` itself).
- Data dir → default root for downloaded games (`<data>/games`) and Proton versions (`<data>/proton`); per-game Wine prefixes live under `<data>/prefixes/<game_id>`.

`DownloadedGame` carries everything needed to run a game later: build id, install path, optional Proton version/prefix path/executable, plus user-set `args`/`environment_variables`. A game is only "ready to run" once both `proton_version` and `executable` are set — `runner.rs` and `management.rs` both check for this and prompt interactively to fill gaps when missing.

`save()` writes to a sibling `settings.json.tmp` then atomically renames it onto `settings.json` (crash-safe — never a partial write). `load()` backs up a corrupt `settings.json` to `settings.json.bak` (overwriting any previous backup) before reinitializing, instead of silently discarding it.

### CLI hints (`src/hint.rs`)

Interactive flows print the scriptable CLI-equivalent command after the user makes a choice (`hint::print_command_hint(&hint::xxx_command(...))`), so users can learn the direct syntax. When adding a new interactive action, add a matching `*_command(...)` builder in `hint.rs` and call `print_command_hint` at the point the action is taken, mirroring existing call sites in `commands/games.rs`, `commands/management.rs`, `commands/proton.rs`, `commands/runner.rs`.

### Progress reporting pattern

Downloads/uploads (`commands/download.rs`, `commands/management.rs::{upload,download}_save_files_for_game`, `commands/proton.rs::download_version*`) all follow the same shape: spawn the actual transfer via `tokio::spawn`, stream progress over an `mpsc::unbounded_channel`, and drive a `\r`-redrawn progress bar in the awaiting loop. Follow this shape for any new transfer-style operation rather than introducing a different progress abstraction.

### Game execution (`src/commands/runner.rs`)

`run_game` lazily fills in missing config (Proton version, executable) via interactive prompts even when invoked from CLI mode with just a `game_id` — there's no separate "configure" gate before running. It creates the Wine prefix on first run by invoking `<proton_path>/proton run wineboot` with `STEAM_COMPAT_CLIENT_INSTALL_PATH`/`STEAM_COMPAT_DATA_PATH` env vars, then launches the game the same way with user-configured env vars/args appended. `STEAM_COMPAT_CLIENT_INSTALL_PATH` is a single shared, persistent `<data_path>/steam` directory (created on demand) — Proton just needs *a* writable directory there, not a real Steam install; `STEAM_COMPAT_DATA_PATH` is the per-game Wine prefix.

`find_executables` (recursive `.exe` scan with skip-lists for installers/redistributables) lives in `src/commands/common.rs` and is shared by `commands/management.rs` and `commands/runner.rs` — add any new cross-command helpers there too rather than duplicating.
