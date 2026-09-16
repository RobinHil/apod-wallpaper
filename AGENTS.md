# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A background utility (Tauri 2: Rust backend, React + TypeScript settings panel) that sets NASA's Astronomy Picture of the Day as the desktop wallpaper once a day. No window most of the time, no Dock or dash presence, one background task. It runs on macOS and on GNOME; Windows is the stated next target.

The README is unusually detailed and carries the reasoning behind most of the design decisions. Read its "Design notes" section before changing scheduling, storage, video handling, the panel's state model or anything per-platform; a change that contradicts one of those notes needs the note updated too.

Documentation in this repository is written one paragraph per line. Do not reflow prose to a column limit, the editor wraps it.

## Commands

```bash
pnpm install                                 # first time
pnpm tauri dev                               # dev, with a live-reloading panel
pnpm bundle                                  # release packages for this machine
pnpm bundle --target universal-apple-darwin  # what CI ships for macOS
./packaging/linux/bundle.sh                  # the same, on Linux, with what the AppImage needs
pnpm exec tsc --noEmit                       # frontend typecheck (there is no ESLint)
```

On Linux prefer the script over `pnpm bundle`: the AppImage step needs four environment variables and two host packages it does not check for, and the script is where they are kept. The `.deb` and the `.rpm` come out either way.

The checks CI gates on, all from `src-tauri/` except the last:

```bash
cargo fmt --all --check
cargo clippy --locked -- -D warnings                # as shipped
cargo clippy --locked --all-targets -- -D warnings  # tests included
cargo test --locked
cargo test --locked scheduler::tests::backoff_doubles_up_to_the_cap   # one test
pnpm exec tsc --noEmit
```

Clippy twice is deliberate: `--all-targets` pulls in dev-dependencies whose features cargo unifies with the library's, so a feature the code needs but the manifest never declares passes there and fails the dependency-free release build. The first command is the one that matches what users install.

## The checks only see the platform they run on

This is the single most important thing to know before touching this repository. The compiler compiles one `cfg` branch and clippy lints one `cfg` branch, so running the block above on Linux says nothing whatsoever about the macOS backends, and running it on a Mac says nothing about the GNOME ones. CI therefore runs it on both, and that matrix is not duplication to be tidied away.

In practice: a change to `video_frame_macos.rs` verified on a Linux checkout has not been verified at all. Say so plainly rather than reporting it as tested, and let CI be the judge.

## Per-platform toolchain

macOS needs the Xcode Command Line Tools. Linux needs the WebKitGTK, GTK, appindicator and GStreamer development packages; the README lists them per distribution under "Building from source".

Node is required everywhere, and the package manager is pnpm: `pnpm-lock.yaml` is the committed lockfile, `packageManager` in `package.json` pins the pnpm version, and CI runs `pnpm install --frozen-lockfile`. Never run `npm` here; it would write a `package-lock.json` the project no longer has.

A debug binary run directly (`./target/debug/apod-wallpaper`) loads the panel from `devUrl`, so it shows a connection error unless Vite is running. Use `pnpm tauri dev`, which starts both.

## Version numbers

The version is declared four times: `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json` and `packaging/arch/PKGBUILD`. CI fails if they disagree, and if a `v*` tag does not match them. Change all four together.

Three more values move as a set: `bundle.macOS.minimumSystemVersion` in `tauri.conf.json`, `build.target` in `vite.config.ts`, and the macOS version the README advertises. Tailwind's output (cascade layers, `@property`) is what sets the floor at 13.3; below it the panel comes up unstyled. The Linux floor is set by the CI runner's glibc instead, and the README states it the same way.

## Architecture

### The four seams

Everything that is not portable lives in four places, and nothing else in the crate knows which platform it is on:

| Seam | macOS | GNOME |
|---|---|---|
| `wallpaper::set_wallpaper` | Apple event to System Events | GSettings `org.gnome.desktop.background` |
| `os_events::watch` | `NSWorkspaceDidWake`, `NSApplicationDidChangeScreenParameters` | `PrepareForSleep` (logind), `MonitorsChanged` (Mutter) |
| `screen::size` | `app.primary_monitor()` | `Mutter.DisplayConfig.GetCurrentState` |
| `video_frame::backend::Decoder` | `video_frame_macos.rs`, AVFoundation | `video_frame_gnome.rs`, GStreamer |

The first three hold both implementations in one file behind `#[cfg(target_os = ...)]`. `video_frame.rs` keeps the shared part, which instants to probe and how to score a frame, and selects a backend file with `#[cfg_attr(..., path = "...")]`, because each backend is long enough to deserve its own file.

Adding Windows means writing those four and nothing else. Resist the urge to reach for `cfg` anywhere but here; if a fifth thing turns out to need it, that is a signal the seam is in the wrong place.

Dependencies follow the same split: `[target.'cfg(target_os = "macos")'.dependencies]` and `[target.'cfg(target_os = "linux")'.dependencies]` in `Cargo.toml`. Both sides deliberately use only what the desktop already links, AppKit and AVFoundation on one, GLib and GStreamer on the other, so neither platform bundles a decoder or a D-Bus library of its own. `gstreamer` is pinned to 0.21 because that is the release wanting the same `glib` 0.18 Tauri's `gtk` already pulls in; bumping it drags a second glib into the tree.

### The backend owns everything

`src-tauri/src/lib.rs` is the wiring: it holds `AppData` behind a `tokio::sync::Mutex`, defines `UiState` (the whole contract with the panel), and starts everything. The three things it used to do inline live next door: `commands.rs` has the seven `#[tauri::command]`s, `panel.rs` the settings window, `tray.rs` the tray item and its two live labels.

Every command returns a complete `UiState`, and `refresh_ui` pushes one on the `state-updated` event whenever the backend changes something by itself. The panel renders what it is given and derives no application state of its own, so the two can never disagree.

`UiState` is written twice, here and by hand in `src/types.ts` (snake_case, as serde emits it). Two tests in `lib.rs` keep them honest: `the_panel_sees_exactly_the_fields_this_sends` compares the field names serde actually emits against the ones `types.ts` declares, and `the_panel_spells_the_enum_values_the_way_serde_does` checks the `Mode` and `FitMode` strings. Without them, adding a field on one side and forgetting the other breaks nothing until a user opens the panel.

`UpdateLock` serialises updates so the scheduler and a panel action never run one at the same time; the loser emits `update-waiting` so the panel's overlay can say what it is waiting on.

The frontend is entirely platform-agnostic and contains no macOS or GNOME wording. Keep it that way.

### The update pipeline

`updater::update(app, force)` is the only entry point, called by the scheduler (`force = false`) and by every panel action that changes what should be on the desktop (`force = true`).

How much of an update is actually needed is decided by `what_is_due`, a pure function returning `Due::{Nothing, Recompose, Fetch}`. It is pure on purpose: it crosses the mode, the publication date, whether the files survived, the fit mode, the screen size and `force`, it is the most intricate reasoning in the application, and inside `run` it needed an `AppHandle` and so could not be tested at all. Change the rules there and add a case to its tests; do not scatter the conditions back into `run`.

1. `Due::Nothing`: return immediately, no API call, no wallpaper-set call.
2. `Due::Recompose`: `reapply` recomposes from the stored original, no network.
3. `Due::Fetch`: `nasa_api::fetch_apod`, then walk `Apod::sources()` (best first, each entry a fallback for the ones before it), download and decode in memory, then `install`: store the original, compose, atomic-rename into place, hand it to the desktop, and only then `store.commit`.

A failure never touches the desktop. The previous image stays on disk and on screen until the new one has actually been applied; `state.json` and `settings.json` go through `store::write_atomic` (temp file, fsync, rename, fsync the directory).

`Outcome::AwaitingPublication` is not a failure: daily mode sends no date, so just after local midnight the API still serves yesterday's picture, which is applied while the scheduler looks again every 30 minutes.

### Scheduling

`scheduler::run` is the app's only background task, and the only timer. It attempts an update, then sleeps to just past the next local midnight, or on a jittered exponential backoff (10 s to 15 min) after a failure. Nothing polls.

Screen changes and wake-ups arrive from `os_events::watch`, which releases a permit on a shared `tokio::sync::Notify` the scheduler races its sleep against. Waking is followed by a 3 s settle, since a monitor change emits a burst of notifications on both platforms.

Do not add a timer, a connectivity monitor or a periodic reapplication here. The README argues each of those out explicitly.

### Memory

An update is the only time anything large is allocated: a full-resolution APOD decoded in memory, then two screen-sized buffers. `updater::update` ends with `memory::release_to_os`, which on glibc asks the allocator to hand that back rather than holding it through the twelve hours of sleep that follow. Measured: 13.1 MB of private memory on a start with nothing to do, 25.7 MB after one update without the call. It is a documented no-op on musl and macOS.

Opening the panel is the expensive act, not keeping it open: 28.6 MB for a process that never opened it, 154 MB once it has been opened and closed, because WebKitGTK keeps its share for the process's life. Nothing in the application may be made to require the panel.

### Blocking work

Image decoding, composition, video frame extraction and the wallpaper call itself all run on tokio's blocking pool via `spawn_blocking`, never on a runtime worker. The runtime is built by hand in `lib::run` with two worker threads. `panic = "abort"` is deliberately absent from the release profile so a malformed image surfaces as an error rather than killing the process.

### Storage

One image is kept, the one on the desktop. `store::Store` owns `state.json` plus `current/`, and `commit` prunes everything the current record does not reference. The wallpaper file name carries date, fit mode and screen size (`wall-<date>-<fit>-<w>x<h>.jpg`) because both desktops cache the background by name, macOS by path and GNOME by URI, and ignore a file rewritten in place.

`screen::size` returns `None` when the desktop reports no main display (lid closed, no external screen). Callers must not treat that as a resolution change: `updater::run` falls back to the size already recorded. Its clamp floor is `image_compose::MIN_SCREEN`, declared once because the measurement and the composition must agree or every later comparison sees a change that never happened.

On GNOME the size comes from Mutter rather than from Tauri on purpose: under Wayland with fractional scaling GTK 3 reports the logical size, so a 1920x1200 panel at 125% would be composed at 1536x960 and then enlarged by the compositor. Do not "simplify" this back to `primary_monitor()`.

### Images and video

All decoding goes through `image_compose::decode` / `decode_bytes`, which sniff the format from the bytes rather than trusting the server's file name. Decoding happens before anything is written to disk, which is what lets the caller fall back to the next source URL.

Video APODs become a still: YouTube and Vimeo embeds use the published thumbnail (tried `maxresdefault`, then `sddefault`, then as published), while entries served as a plain `.mp4` have no thumbnail and get a frame decoded from the file, probing four instants and keeping the first with enough contrast. What is archived is that frame as a JPEG, not the video.

Both backends use the system's codecs, so both inherit its gaps: a distribution shipping no H.264 decoder produces no frame. `updater::run` already handles that by keeping the current wallpaper and saying why, so it needs no special case.

`nasa_api::network` strips the URL out of every reqwest error, because the request URL carries the user's API key and every error reaches the panel.

### The panel

`useAppState` holds all of it: `run` invokes a command, blocks the UI behind an overlay, adopts the returned state, and shows any failure in the banner. Pushed `state-updated` events are ignored while a command is in flight.

Two deliberate exceptions to the backend-owns-everything rule, both local: the date picker's visibility, and the text fields, which use `useSyncedField` and re-seed from pushes only while unfocused, so a background update landing mid-sentence cannot wipe a half-typed API key.

Styling is Tailwind v4 with the palette as `@theme` tokens in `styles.css`; dark mode redefines the same variables under `prefers-color-scheme`, which is why no component carries a `dark:` variant. Shared utility strings live in `classes.ts` and are never merged: two utilities setting the same property are resolved by stylesheet order, not attribute order, so a variant spells out its own colours instead of layering them over a base.

### Opening the panel

Starting the app puts nothing on screen. The routes back to the panel, all in `lib.rs`: the tray item, `RunEvent::Reopen` (macOS only, the bundle launched again, which macOS answers without starting a second process), and the single-instance plugin (a second process, which is what a relaunch does on GNOME). The one automatic opening is the very first launch, detected by the absence of `settings.json`.

A tray item that fails to build is logged and stepped over. On macOS that is defensive; on GNOME it is the common case, since the shell shows no status icons without the AppIndicator extension. Nothing the app does may depend on the tray existing.

## Packaging

`tauri.conf.json` lists every bundle target and Tauri skips the ones foreign to the host, so one list covers both platforms. `bundle.linux.deb.depends` and `.rpm.depends` name the GStreamer and appindicator runtime packages.

Arch has no Tauri bundler, so `packaging/arch/PKGBUILD` assembles the package itself, from `pnpm tauri build --no-bundle` plus the desktop entry beside it. Through the CLI rather than `cargo build --release`, and that is not a stylistic preference: Tauri decides between a development and a production build from the `custom-protocol` feature, which the CLI sets and a bare cargo invocation does not, so `cargo build --release` yields a binary that loads its panel from `devUrl` and, once installed, reports that it cannot reach localhost. Nothing warns about it, which is why `build-arch` exists in CI: it builds the PKGBUILD in an `archlinux:base-devel` container, from the checkout rather than from the published tag, then unpacks the package and fails if the binary embeds no `index-*.js`. That assertion is the whole point of the job; a development build is valid in every other respect. That desktop entry is deliberately not `NoDisplay`: without a tray, launching the app from the overview is the only route to the panel that always works. The PKGBUILD builds the published tag, not the working tree, so it only succeeds against a tag that already carries `pnpm-lock.yaml`.

`bundle.linux.deb.depends` and `bundle.linux.rpm.depends` are package names written by hand, and the RPM ones are Fedora's although the package is built on Ubuntu. `smoke-install` in CI is what resolves them: each case starts from a stock image of the distribution the package targets and lets its own package manager install it, then runs `--version`, which loads every shared library the binary names without needing a display. Four cases: the `.deb` under `debian:stable-slim`, the `.rpm` under `fedora:latest`, the AppImage on Fedora, and the Arch package under `archlinux:base`. That last one means more than the same step inside build-arch would: there `makepkg -s` had installed every dependency before the package existed, so nothing proved they resolve from a clean system, and `pacman -U` on a stock image does. The AppImage case asks only whether it starts away from a Debian-shaped host; `--version` exits before GStreamer is reached, so a codec missing from the bundle is not something this catches. Nothing is compiled in that job, and it is the only place a glibc other than the runner's ever sees this binary.

`release` waits on `build-macos`, `build-linux`, `build-arch` and `smoke-install`. A tag is publishable only once every package has been built and installed somewhere.

`bundle.linux.appimage.bundleMediaFramework` is `true`, and it matters more than it looks: Tauri defaults it to `false`, which produces an AppImage carrying GStreamer's libraries and not one codec. The failure is invisible until someone hits a video APOD, because everything else works. The Linux CI job installs the runtime plugin packages for the same reason, since linuxdeploy can only bundle what is present on the builder, and sets `GSTREAMER_INCLUDE_BAD_PLUGINS` for the OpenH264 decoder. OpenH264 rather than libav, so the licence section of the README stays true.

Building the AppImage on a developer machine needs `patchelf` everywhere and, on Arch, two more environment variables and a package. All of it lives in `packaging/linux/bundle.sh`, which the Linux CI job runs as well: the variables used to be spelled out in the workflow and again in the README, where they drifted. Add one there, not in either. The script sets nothing it cannot explain and refuses to start when a program it needs is missing, rather than failing in `linuxdeploy` twenty minutes later.

## Tests

Unit tests live in `#[cfg(test)] mod tests` at the bottom of each Rust module (`nasa_api`, `updater`, `image_compose`, `video_frame`, `settings`, `scheduler`, `lib`). There is no frontend test runner; the gate is `pnpm exec tsc --noEmit` with `strict`, `noUnusedLocals` and `noUnusedParameters`.

Several tests assert invariants rather than behaviour, such as `the_sleep_cap_never_splits_a_wait` or the usage text matching the flags actually parsed. Keep them honest when the constants they guard change.

Nothing unit-tests the four seams, and nothing should try: they are thin wrappers over a desktop that has to be running. Verify them by running the app; the README's Troubleshooting section lists the commands that show what the desktop actually did.
