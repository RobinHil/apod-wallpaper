# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A macOS menu bar utility (Tauri 2: Rust backend, React + TypeScript settings
panel) that sets NASA's Astronomy Picture of the Day as the desktop wallpaper
once a day. No Dock icon, no window most of the time, one background task.

The README is unusually detailed and carries the reasoning behind most of the
design decisions. Read its "Design notes" section before changing scheduling,
storage, video handling or the panel's state model; a change that contradicts
one of those notes needs the note updated too.

## Commands

```bash
pnpm install                                 # first time
pnpm tauri dev                               # dev, with a live-reloading panel
pnpm bundle                                  # release .app + .dmg for this machine
pnpm bundle --target universal-apple-darwin  # what CI ships
pnpm exec tsc --noEmit                       # frontend typecheck (there is no ESLint)
```

The checks CI gates on, in order, all from `src-tauri/` except the last:

```bash
cargo fmt --all --check
cargo clippy --locked -- -D warnings                # as shipped
cargo clippy --locked --all-targets -- -D warnings  # tests included
cargo test --locked
cargo test --locked scheduler::tests::backoff_doubles_up_to_the_cap   # one test
```

Clippy twice is deliberate: `--all-targets` pulls in dev-dependencies whose
features cargo unifies with the library's, so a feature the code needs but the
manifest never declares passes there and fails the dependency-free release
build. The first command is the one that matches what users install.

## Building requires macOS

The Rust crate has no `cfg(target_os)` gates: `objc2-app-kit`,
`objc2-av-foundation`, `objc2-core-graphics` and the `wallpaper` crate are
unconditional dependencies, and `os_events.rs`, `video_frame.rs` and
`wallpaper.rs` call Apple frameworks directly. `cargo build`, `cargo clippy`
and `cargo test` therefore only work on macOS. On a Linux or Windows checkout,
only the frontend side (`pnpm exec tsc --noEmit`, `pnpm build`) can be
verified; say so rather than reporting a Rust change as tested.

Node is required too, and the package manager is pnpm: `pnpm-lock.yaml` is the
committed lockfile, `packageManager` in `package.json` pins the pnpm version,
and CI runs `pnpm install --frozen-lockfile`. Never run `npm` here; it would
write a `package-lock.json` the project no longer has.

## Version numbers

The version is declared three times, in `package.json`, `src-tauri/Cargo.toml`
and `src-tauri/tauri.conf.json`. CI fails if they disagree, and if a `v*` tag
does not match them. Change all three together.

Three more values move as a set: `bundle.macOS.minimumSystemVersion` in
`tauri.conf.json`, `build.target` in `vite.config.ts`, and the macOS version
the README advertises. Tailwind's output (cascade layers, `@property`) is what
sets the floor at 13.3; below it the panel comes up unstyled.

## Architecture

### The backend owns everything

`src-tauri/src/lib.rs` is the hub: it holds `AppData` behind a
`tokio::sync::Mutex`, defines `UiState` (the whole contract with the panel),
registers the seven `#[tauri::command]`s, builds the menu bar item and spawns
the single background task.

Every command returns a complete `UiState`, and `refresh_ui` pushes one on the
`state-updated` event whenever the backend changes something by itself. The
panel renders what it is given and derives no application state of its own, so
the two can never disagree. Adding a field means touching `UiState` in
`lib.rs` and the mirrored interface in `src/types.ts` (snake_case, as serde
emits it).

`UpdateLock` serialises updates so the scheduler and a panel action never run
one at the same time; the loser emits `update-waiting` so the panel's overlay
can say what it is waiting on.

### The update pipeline

`updater::update(app, force)` is the only entry point, called by the scheduler
(`force = false`) and by every panel action that changes what should be on the
desktop (`force = true`). Its shape:

1. Not forced and `state.json` already answers the current settings, with both
   files on disk: return immediately, no API call, no wallpaper-set call.
2. Right image but wrong fit mode or screen size: `reapply` recomposes from the
   stored original, no network.
3. Otherwise `nasa_api::fetch_apod`, then walk `Apod::sources()` (best first,
   each entry a fallback for the ones before it), download and decode in memory,
   then `install`: store the original, compose, atomic-rename into place, hand
   it to the desktop, and only then `store.commit`.

A failure never touches the desktop. The previous image stays on disk and on
screen until the new one has actually been applied; `state.json` and
`settings.json` go through `store::write_atomic` (temp file, fsync, rename,
fsync the directory).

`Outcome::AwaitingPublication` is not a failure: daily mode sends no date, so
just after local midnight the API still serves yesterday's picture, which is
applied while the scheduler looks again every 30 minutes.

### Scheduling

`scheduler::run` is the app's only background task, and the only timer. It
attempts an update, then sleeps to just past the next local midnight, or on a
jittered exponential backoff (10 s to 15 min) after a failure. Nothing polls.

Screen changes and wake-ups arrive as AppKit notifications
(`NSApplicationDidChangeScreenParameters`, `NSWorkspaceDidWake`) registered in
`os_events::watch`, which release a permit on a shared `tokio::sync::Notify`
the scheduler races its sleep against. Waking is followed by a 3 s settle,
since a monitor change emits a burst of notifications.

Do not add a timer, a connectivity monitor or a periodic reapplication here.
The README argues each of those out explicitly.

### Blocking work

Image decoding, composition, video frame extraction and the wallpaper call
itself (an Apple event to System Events, seconds of it) all run on tokio's
blocking pool via `spawn_blocking`, never on a runtime worker. The runtime is
built by hand in `lib::run` with two worker threads. `panic = "abort"` is
deliberately absent from the release profile so a malformed image surfaces as
an error rather than killing the process.

### Storage

One image is kept, the one on the desktop. `store::Store` owns
`state.json` plus `current/`, and `commit` prunes everything the current
record does not reference. The wallpaper file name carries date, fit mode and
screen size (`wall-<date>-<fit>-<w>x<h>.jpg`) because macOS caches the desktop
picture by path and ignores a file rewritten in place.

`crate::screen_size` returns `None` when macOS reports no main display (lid
closed, no external screen). Callers must not treat that as a resolution
change: `updater::run` falls back to the size already recorded. Its clamp
floor is `image_compose::MIN_SCREEN`, declared once because the measurement
and the composition must agree or every later comparison sees a change that
never happened.

### Images and video

All decoding goes through `image_compose::decode` / `decode_bytes`, which
sniff the format from the bytes rather than trusting the server's file name.
Decoding happens before anything is written to disk, which is what lets the
caller fall back to the next source URL.

Video APODs become a still: YouTube and Vimeo embeds use the published
thumbnail (tried `maxresdefault`, then `sddefault`, then as published), while
entries served as a plain `.mp4` have no thumbnail and get a frame decoded by
AVFoundation in `video_frame.rs`, probing four instants and keeping the first
with enough contrast. What is archived is that frame as a JPEG, not the video.

`nasa_api::network` strips the URL out of every reqwest error, because the
request URL carries the user's API key and every error reaches the panel.

### The panel

`useAppState` holds all of it: `run` invokes a command, blocks the UI behind
an overlay, adopts the returned state, and shows any failure in the banner.
Pushed `state-updated` events are ignored while a command is in flight.

Two deliberate exceptions to the backend-owns-everything rule, both local:
the date picker's visibility, and the text fields, which use `useSyncedField`
and re-seed from pushes only while unfocused, so a background update landing
mid-sentence cannot wipe a half-typed API key.

Styling is Tailwind v4 with the palette as `@theme` tokens in `styles.css`;
dark mode redefines the same variables under `prefers-color-scheme`, which is
why no component carries a `dark:` variant. Shared utility strings live in
`classes.ts` and are never merged: two utilities setting the same property are
resolved by stylesheet order, not attribute order, so a variant spells out its
own colours instead of layering them over a base.

### Opening the panel

Starting the app puts nothing on screen. Three routes open the panel, all in
`lib.rs`: the menu bar item, `RunEvent::Reopen` (the bundle launched again,
which macOS answers without starting a second process), and the
single-instance plugin (the binary inside the bundle run directly). The one
automatic opening is the very first launch, detected by the absence of
`settings.json`.

A menu bar item that fails to build is logged and stepped over; nothing the
app does depends on it existing.

## Tests

Unit tests live in `#[cfg(test)] mod tests` at the bottom of each Rust module
(`nasa_api`, `updater`, `image_compose`, `video_frame`, `settings`,
`scheduler`, `lib`). There is no frontend test runner; the gate is
`pnpm exec tsc --noEmit` with `strict`, `noUnusedLocals` and
`noUnusedParameters`.

Several tests assert invariants rather than behaviour, such as
`the_sleep_cap_never_splits_a_wait` or the usage text matching the flags
actually parsed. Keep them honest when the constants they guard change.
