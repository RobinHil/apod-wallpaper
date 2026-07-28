# APOD Wallpaper

A small cross-platform desktop app (Windows, macOS, Linux) that sets NASA's
Astronomy Picture of the Day (APOD) as your wallpaper. It lives in the system
tray and runs in the background.

Built with [Tauri 2](https://tauri.app): Rust backend, vanilla
TypeScript/HTML/CSS settings panel. No JS framework, no superfluous dependency.

## Features

- **Picture of the day**: fetches the current APOD through the NASA API and
  applies it as the wallpaper.
- **Random mode**: draws a date at random from the whole APOD archive (since
  16 June 1995); "Refresh now" draws a new image immediately.
- **Specific date mode**: shows the APOD for a date you pick, bounded between
  16 June 1995 (the first APOD) and today.
- **Automatic checks**: at startup, then continuously while the app runs (a new
  daily image is picked up automatically).
- **Read-only tray**: the tray menu shows the image title, its date and its
  copyright, and lets you open the panel or quit. Every setting and the manual
  refresh live in the panel.
- **Predictable UI**: each panel action blocks the interface (with a visible
  indicator) until it has fully applied, and every error is shown in a banner --
  no operation fails silently.
- **Screen-ratio aware** (default "blurred fill" mode): the original image is
  centred whole and undistorted over a scaled-up, blurred and darkened copy of
  itself that fills the screen. A "crop to fill" mode (no blur) is available in
  the settings. No text is burned into the image: credits (date, copyright)
  stay visible in the tray and the panel.
- **Local store**: history of the most recently downloaded images (60 max) with
  their metadata in `metadata.json`.
- **Offline mode**: on a network outage or an exceeded API quota, the last
  loaded image stays in place, the app retries silently in the background
  (every 15 minutes) and the offline state is shown in the tray and the panel.
- **Configurable API key**: `DEMO_KEY` by default, personal key saved from the
  panel (stored locally).

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable, via rustup)
- [Node.js](https://nodejs.org) 18 or newer, with npm
- Tauri's system prerequisites for your OS:
  <https://tauri.app/start/prerequisites/>

Per platform:

| OS      | Extra dependencies |
|---------|--------------------|
| Windows | WebView2 (preinstalled on Windows 10/11), Microsoft C++ Build Tools |
| macOS   | Xcode Command Line Tools (`xcode-select --install`) |
| Linux   | `webkit2gtk-4.1`, `libappindicator3` (or `libayatana-appindicator`), `librsvg2`, `patchelf` -- see the Tauri prerequisites page for the exact list per distribution |

Linux note: the tray icon requires an environment that supports
`StatusNotifierItem`/AppIndicator (the "AppIndicator" extension is needed under
GNOME).

## Install and run in development

```bash
cd apod-wallpaper
npm install
npm run tauri dev
```

On first launch the app:

1. queries the APOD API (with `DEMO_KEY` when no key is configured);
2. downloads the image (HD when available) and stores it locally;
3. composes the final image at the primary screen's resolution (blurred fill +
   centred image, no burned-in text);
4. sets it as the wallpaper;
5. installs itself in the tray. The settings window is hidden by default: open
   it from the tray menu ("Open APOD Wallpaper").

## Production build

```bash
npm run tauri build
```

Artifacts land in `src-tauri/target/release/bundle/`:

- **Windows**: `.msi` (WiX) and `.exe` (NSIS) installers -- build from Windows
- **macOS**: `.app` bundle and `.dmg` image -- build from macOS
- **Linux**: `.deb`, `.rpm` and `.AppImage` -- build from Linux

Tauri does not support cross-compilation: every platform is built from its
target OS (in CI, a GitHub Actions matrix over
`windows-latest`/`macos-latest`/`ubuntu-latest` is the usual approach).

## Configuring the NASA API key

By default the app uses `DEMO_KEY`, limited to **30 requests/hour and 50
requests/day** (per IP address). That is enough for normal use, but a free
personal key is recommended:

1. Request a key at <https://api.nasa.gov/> (simple form, key sent by email).
2. Open the app panel (tray menu, "Open APOD Wallpaper").
3. Paste the key in the "NASA API key" field and click "Save".

The key is stored locally in `settings.json` (see "Local data" below) and is
only ever sent to the NASA API.

## Project layout

```
apod-wallpaper/
|- src-tauri/                  # Rust backend
|  |- src/
|  |  |- main.rs               # Binary entry point
|  |  |- lib.rs                # Tauri setup: tray, menu, scheduler, commands
|  |  |- nasa_api.rs           # APOD API calls, parsing, error taxonomy
|  |  |- cache.rs              # Local history (metadata.json + image files)
|  |  |- image_compose.rs      # Ratio handling: blurred fill or crop
|  |  |- wallpaper.rs          # Per-platform wallpaper setting
|  |  `- settings.rs           # API key, mode, fit; JSON persistence
|  |- icons/
|  |  `- app-icon.svg          # Vector source of the icon (regenerate with `tauri icon`)
|  |- capabilities/default.json
|  `- tauri.conf.json
|- src/                        # Panel frontend (vanilla TypeScript)
|  |- main.ts                  # State rendering, commands to the backend
|  `- styles.css               # Light/dark theme (prefers-color-scheme)
|- index.html                  # Panel structure (inline SVG icons)
`- README.md
```

## Local data

The app writes to the OS standard data directory (`com.rh.apod-wallpaper`):

- **macOS**: `~/Library/Application Support/com.rh.apod-wallpaper/`
- **Windows**: `%APPDATA%\com.rh.apod-wallpaper\`
- **Linux**: `~/.local/share/com.rh.apod-wallpaper/`

Contents:

```
settings.json                  # API key, mode (daily/random/specific), chosen date, fit
cache/
|- metadata.json               # image history and metadata
|- images/<date>.<ext>         # downloaded original images
`- wallpapers/wall-<date>-<fit>.jpg   # final applied compositions
```

## Notable design decisions

- **Video APODs**: some APOD entries are videos. The API serves no video file
  (only a YouTube/Vimeo embed link), so an animated wallpaper is not feasible
  without heavy dependencies (stream download, a permanent player behind the
  desktop, continuous CPU/battery drain). Instead the video **thumbnail** is
  used: for YouTube the maximum-resolution version (`maxresdefault`, usually
  1280x720) is tried first, falling back to the standard thumbnail. The panel
  flags it as a video and offers a direct link to watch it; the tray appends
  "(video)" to the title. When no thumbnail is available, the previous image is
  kept (daily mode) or a new date is drawn (random mode).
- **Daily mode sends no date parameter**: the app asks the API for "the most
  recently published image" rather than the local date, which removes time-zone
  skew (APOD is published on US Eastern time) and handles days with no
  publication: the previous day (or the latest published date) is shown, with an
  informational message in the panel until today's APOD is available.
- **Days with no publication**: the APOD archive has a few dates with no entry
  (mostly in 1995). Per mode: "picture of the day" shows the latest publication
  with an indicator; "random" silently draws another date; "specific date"
  rejects the date with an error, keeps the current wallpaper and restores the
  previous mode.
- **Light and dark themes**: on Windows, macOS and nearly every Linux desktop
  the wallpaper is a single image applied whatever the active theme. GNOME 42
  and later are the one exception: the dark theme reads a separate key
  (`picture-uri-dark`), which the app sets alongside `picture-uri` so the image
  changes in dark mode too.
- **Cheap gaussian blur**: the backdrop is blurred on a 1/8 scale copy then
  scaled back up; the result matches a heavy blur on the full-size image for a
  fraction of the CPU cost.
- **Varying file name**: the final composition embeds the date and the fit mode
  in its file name, because some desktops (macOS in particular) cache the
  wallpaper by path and ignore a file rewritten in place.
- **Errors are never silent**: panel commands (mode change, refresh, fit, API
  key) wait for the Rust side to finish and return any error to the frontend,
  which blocks the UI while waiting and shows the error in a banner. The
  background loop records its failures in the status visible from the panel.
- **Copyright**: the API's `copyright` field is kept in the store and shown in
  the tray and the panel. When present, the image is **not** public domain: it
  belongs to its author and use is limited to a personal wallpaper. Images
  without a copyright are produced by NASA and are public domain.

## Known limitations

- **Linux**: setting the wallpaper depends on the desktop environment.
  Supported through the `wallpaper` crate: GNOME and its derivatives (Unity,
  Budgie, Pantheon), KDE Plasma, XFCE, LXDE, MATE, Cinnamon, Deepin, and as a
  last resort any compositor with `swaybg` (Wayland) or `feh` (X11) installed.
  On an unrecognised environment an explicit message is shown in the panel.
  Compositors with no declared desktop (Hyprland, sway, i3...) go through that
  fallback: each application relaunches `swaybg`, which can conflict with a
  wallpaper daemon already in place (`swww`, `hyprpaper`); those environments
  are not officially supported.
- **Multiple monitors**: the image is composed at the primary screen's
  resolution; secondary screens get the same image (per-screen composition is a
  possible evolution).
- **Daily check**: the new daily image is detected by polling every 15 minutes
  after midnight (local time), until the API publishes the new APOD.
- **macOS**: setting the wallpaper goes through an AppleScript event; on first
  launch macOS may ask for permission to control "System Events" (accept it).

## Licence

- Project code: to be defined by the repository owner.
- APOD images with a copyright notice remain the property of their authors.
