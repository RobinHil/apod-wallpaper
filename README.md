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
  16 June 1995).
- **Specific date mode**: shows the APOD for a date you pick, bounded between
  16 June 1995 (the first APOD) and today.
- **Once a day, then nothing**: the wallpaper is updated at startup and at the
  local day change, and the app does no work at all in between. "Refresh now"
  applies a new image on demand.
- **Read-only tray**: the tray menu shows the image title, its date and its
  copyright, and lets you open the panel or quit. Every setting and the manual
  refresh live in the panel.
- **Start at login**: optional, toggled from the panel.
- **Predictable UI**: each panel action blocks the interface (with a visible
  indicator) until it has fully applied, and every error is shown in a banner --
  no operation fails silently.
- **Screen-ratio aware** (default "blurred fill" mode): the original image is
  centred whole and undistorted over a scaled-up, blurred and darkened copy of
  itself that fills the screen. A "crop to fill" mode (no blur) is available in
  the settings. No text is burned into the image: credits (date, copyright)
  stay visible in the tray and the panel.
- **Offline tolerant**: on a network outage or an exceeded API quota, the
  wallpaper in place is left untouched and the app retries with a backoff until
  it succeeds. The offline state is shown in the tray and the panel.
- **Configurable API key**: `DEMO_KEY` by default, personal key saved from the
  panel (stored locally).

## Installing a release build

Download the artifact for your platform from the
[releases page](https://github.com/RobinHil/apod-wallpaper/releases).

Release builds are **not signed**, because Apple and Microsoft code-signing
certificates are paid subscriptions. Each platform therefore needs a one-off
step to tell the OS you trust the app; they are described below.

### Linux

**AppImage** -- self-contained, runs anywhere, no root needed:

```bash
chmod +x APOD_Wallpaper_*.AppImage
./APOD_Wallpaper_*.AppImage
```

For a menu entry and an icon, install
[Gear Lever](https://flathub.org/apps/it.mijorus.gearlever) or
[AppImageLauncher](https://github.com/TheAssassin/AppImageLauncher), which
integrate the AppImage on first run. Without one of those, the tray icon works
but the app will not appear in your application menu.

**Debian, Ubuntu, Mint** (`.deb`):

```bash
sudo apt install ./apod-wallpaper_*_amd64.deb
```

**Fedora, RHEL, openSUSE** (`.rpm`):

```bash
sudo dnf install ./apod-wallpaper-*.x86_64.rpm
```

Use the `aarch64`/`arm64` artifacts on ARM machines (Raspberry Pi, Asahi, ARM
servers).

### macOS

Open the `.dmg` and drag the app into `Applications`. On first launch macOS
refuses to open it, because the build is not notarised:

> "APOD Wallpaper" is damaged and can't be opened.

That message is Gatekeeper's quarantine flag, not a corrupted download. Clear
it once:

```bash
xattr -d com.apple.quarantine "/Applications/APOD Wallpaper.app"
```

Then open the app normally. Alternatively, right-click the app and choose
*Open*, then confirm; or allow it from *System Settings > Privacy & Security*
right after the failed launch.

The first time the app sets your wallpaper, macOS asks for permission to
control **System Events**. Accept it: that Apple event is how the desktop
picture is set. If you refuse, you can re-enable it under
*System Settings > Privacy & Security > Automation*.

### Windows

Run the `.exe` (NSIS) or `.msi` installer. SmartScreen will show:

> Windows protected your PC

Click **More info**, then **Run anyway**. This appears because the installer
carries no Authenticode signature; it is expected for unsigned open-source
builds.

WebView2 is required and is preinstalled on Windows 10 and 11. The `.exe`
installer downloads it automatically if it is missing; the `.msi` does not, so
prefer the `.exe` on older systems.

## Start at login

Tick **Start automatically at login** in the panel. That is all that is needed
on every platform; the app registers itself using the OS mechanism:

| OS      | What gets created |
|---------|-------------------|
| Linux   | `~/.config/autostart/APOD Wallpaper.desktop` |
| macOS   | `~/Library/LaunchAgents/com.rh.apod-wallpaper.plist` |
| Windows | Value `APOD Wallpaper` under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` |

Unticking the box removes it again. You can also manage it from the OS:
*Settings > Apps > Startup* on Windows, *System Settings > General > Login
Items* on macOS, or your desktop's "Startup Applications" tool on Linux.

## Uninstalling

Quit the app from the tray menu first, and untick "Start automatically at
login" so no launcher entry is left behind.

| OS      | Command |
|---------|---------|
| Linux (deb) | `sudo apt remove apod-wallpaper` |
| Linux (rpm) | `sudo dnf remove apod-wallpaper` |
| Linux (AppImage) | delete the `.AppImage` file |
| macOS   | drag `/Applications/APOD Wallpaper.app` to the Trash |
| Windows | *Settings > Apps > Installed apps > APOD Wallpaper > Uninstall* |

Then remove the local data, which no uninstaller touches:

```bash
# Linux
rm -rf ~/.local/share/com.rh.apod-wallpaper "~/.config/autostart/APOD Wallpaper.desktop"

# macOS
rm -rf ~/Library/Application\ Support/com.rh.apod-wallpaper \
       ~/Library/LaunchAgents/com.rh.apod-wallpaper.plist
```

```powershell
# Windows (PowerShell)
Remove-Item -Recurse -Force "$env:APPDATA\com.rh.apod-wallpaper"
```

Note that your desktop still points at the last image the app applied, which
lived inside that directory. **Pick a new wallpaper before deleting it**, or
your desktop background will go blank at the next login.

## Where files are stored

| OS      | Location |
|---------|----------|
| Linux   | `~/.local/share/com.rh.apod-wallpaper/` |
| macOS   | `~/Library/Application Support/com.rh.apod-wallpaper/` |
| Windows | `%APPDATA%\com.rh.apod-wallpaper\` |

```
settings.json                            API key, mode, chosen date, fit mode
state.json                               the wallpaper currently applied
current/<date>.<ext>                     the downloaded original
current/wall-<date>-<fit>-<w>x<h>.jpg    the composition set as the wallpaper
```

Only one image is kept: the one on your desktop. The directory stays under a
few megabytes.

## Runtime dependencies

| OS | Required |
|----|----------|
| Windows | WebView2 runtime (preinstalled on Windows 10/11) |
| macOS | macOS 10.15 or newer; nothing to install |
| Linux | `webkit2gtk-4.1`, `libappindicator3` (or `libayatana-appindicator3`), `librsvg2` |

TLS is handled by rustls, compiled into the binary, so there is **no OpenSSL
runtime dependency** on Linux.

Setting the wallpaper on Linux shells out to whatever your desktop provides,
all of which ship with their desktop: `gsettings` (GNOME and derivatives),
`qdbus` (KDE Plasma), `xfconf-query` (XFCE), `pcmanfm` (LXDE), `dconf`
(Cinnamon, MATE, Deepin). On a compositor with no declared desktop, `swaybg`
(Wayland) or `feh` (X11) must be installed -- see "Known limitations".

The tray icon requires an environment that supports
`StatusNotifierItem`/AppIndicator (the "AppIndicator" extension is needed under
GNOME).

## Building from source

Prerequisites:

- [Rust](https://www.rust-lang.org/tools/install) (stable, via rustup)
- [Node.js](https://nodejs.org) 18 or newer, with npm
- Tauri's system prerequisites for your OS:
  <https://tauri.app/start/prerequisites/>

| OS      | Build dependencies |
|---------|--------------------|
| Windows | Microsoft C++ Build Tools |
| macOS   | Xcode Command Line Tools (`xcode-select --install`) |
| Linux   | `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`, `patchelf` |

```bash
npm install
npm run tauri dev     # development
npm run tauri build   # production bundles
```

Artifacts land in `src-tauri/target/release/bundle/`: `.msi` and `.exe` on
Windows, `.app` and `.dmg` on macOS, `.deb`, `.rpm` and `.AppImage` on Linux.

Tauri does not support cross-compilation between operating systems: each
platform is built from its own OS. See `.github/workflows/ci.yml` for the
matrix that does this in CI.

## Configuring the NASA API key

By default the app uses `DEMO_KEY`, limited to **30 requests/hour and 50
requests/day** (per IP address). Since the app makes at most a handful of
requests a day, that is plenty; a free personal key is still recommended if you
share an IP with other API users:

1. Request a key at <https://api.nasa.gov/> (simple form, key sent by email).
2. Open the app panel (tray menu, "Open APOD Wallpaper").
3. Paste the key in the "NASA API key" field and click "Save".

The key is stored locally in `settings.json` and is only ever sent to the NASA
API.

## Project layout

```
apod-wallpaper/
|- src-tauri/                  # Rust backend
|  |- src/
|  |  |- main.rs               # Binary entry point
|  |  |- lib.rs                # Tauri setup: tray, panel window, commands
|  |  |- scheduler.rs          # The only background task: when to update
|  |  |- os_events.rs          # Screen-change and wake-from-sleep notifications
|  |  |- updater.rs            # What an update does, end to end
|  |  |- nasa_api.rs           # APOD API calls, parsing, error taxonomy
|  |  |- store.rs              # state.json + the two image files, atomic writes
|  |  |- image_compose.rs      # Ratio handling: blurred fill or crop
|  |  |- wallpaper.rs          # Per-platform wallpaper setting
|  |  `- settings.rs           # API key, mode, fit; JSON persistence
|  |- Info.plist               # macOS: menu-bar app + Apple events usage
|  |- capabilities/default.json
|  `- tauri.conf.json
|- src/                        # Panel frontend (vanilla TypeScript)
|  |- main.ts                  # State rendering, commands to the backend
|  `- styles.css               # Light/dark theme (prefers-color-scheme)
|- index.html                  # Panel structure (inline SVG icons)
`- .github/workflows/ci.yml    # Lint/test gate + cross-platform build matrix
```

## Notable design decisions

### Scheduling: once a day, and otherwise asleep

The app is meant to be invisible in `top`. There is exactly one background
task, and it does this:

1. Attempt an update. There is no separate "am I online?" probe -- the fetch
   is the probe, which is one round trip instead of two.
2. On success, sleep until the next local day change.
3. On failure, or when today's APOD is not published yet, retry with an
   exponential backoff (10s, 20s, 40s ...) with +/-20% jitter, capped at 15
   minutes, until it succeeds. Then go back to step 2.

Nothing else is armed. When the wallpaper is up to date, the process is
sleeping on a single timer -- for the nine or so hours until the next midnight,
not in periodic instalments.

Two things can invalidate that sleep, and both arrive as OS notifications
rather than being looked for:

- **The screen changes.** A new resolution, a monitor plugged in or unplugged,
  displays rearranged. The wallpaper is recomposed from the original already on
  disk, within seconds and without touching the network.
- **The machine wakes from sleep.** The timer is measured against a clock that
  does not advance while suspended, so without this a sleep started before a
  closed lid would fire hours after midnight.

Each platform is served by the toolkit its Tauri backend already links, so none
of this adds a crate to the build: `NSApplicationDidChangeScreenParameters` and
`NSWorkspaceDidWake` on macOS, `GdkScreen` on Linux (which covers X11 and
Wayland alike), `WM_DISPLAYCHANGE` and `WM_POWERBROADCAST` on Windows.

Resume from suspend is the one gap: on Linux it would mean a D-Bus client for
logind, a dependency and a connection held open for the life of the process to
catch one signal. There the sleep is split into six-hour stretches instead.
This does not make the daily update any later -- the remaining time is
recomputed at every wake, so the last stretch still ends exactly at the day
change -- it only bounds how long a suspended machine takes to catch up.

The same reasoning applies to retries. Subscribing to NetworkManager,
NWPathMonitor and the Windows connectivity APIs would be roughly 200 lines of
platform-specific code, and on Linux it would keep a D-Bus connection resident
forever -- against the point of the exercise. A connection attempt with no
network fails locally in about a millisecond, so a capped backoff is cheaper
than the machinery to avoid it.

### Nothing is redone that does not need to be

`state.json` records the applied image and the composition inputs (fit mode and
screen size). At startup, if the record already answers the current settings
and both files are on disk, the app does nothing at all: no API call, no
download, no wallpaper-set call. Restarting five times in a day costs five
`state.json` reads.

Changing the fit mode, or moving to a monitor with a different resolution,
recomposes from the stored original without touching the network.

When the platform reports no monitor at all -- lid closed, session locked --
the size the wallpaper was last composed for is reused. That is not a
resolution change, and recomposing for a guessed size would replace a correct
wallpaper with a wrong one.

### Failures never break the desktop

The download goes to a temporary file, is validated by decoding it, is composed
into a wallpaper, and only then are both files moved into place with atomic
renames. The previous image stays on disk and on your desktop until the new one
has actually been applied, so a partial download, a full disk or a crash
mid-update cannot leave a black or broken background. `state.json` and
`settings.json` are written the same way.

### Video APODs

Some APOD entries are videos. The API serves no video file (only a
YouTube/Vimeo embed link), so an animated wallpaper is not feasible without
heavy dependencies. Instead the video **thumbnail** is used: for YouTube the
maximum-resolution version (`maxresdefault`, usually 1280x720) is tried first,
falling back to the standard thumbnail. The panel flags it as a video and
offers a direct link to watch it; the tray appends "(video)" to the title. When
no thumbnail is available, the current wallpaper is kept (daily mode) or a new
date is drawn (random mode).

### Daily mode sends no date parameter

The app asks the API for "the most recently published image" rather than the
local date, which removes time-zone skew (APOD is published on US Eastern
time). Just after local midnight the API still serves yesterday's picture; that
counts as *not yet satisfied*, so the app applies it if it is new and keeps
retrying until today's is published. Pinning to yesterday's would skip today's
entirely.

### Light and dark themes

On Windows, macOS and nearly every Linux desktop the wallpaper is a single
image applied whatever the active theme. GNOME 42 and later are the one
exception: the dark theme reads a separate key (`picture-uri-dark`), which the
app sets alongside `picture-uri` so the image changes in dark mode too.

### Other

- **Cheap gaussian blur**: the backdrop is blurred on a 1/8 scale copy then
  scaled back up; the result matches a heavy blur on the full-size image for a
  fraction of the CPU cost.
- **Varying file name**: the composition embeds the date, fit mode and screen
  size in its file name, because some desktops (macOS in particular) cache the
  wallpaper by path and ignore a file rewritten in place.
- **Errors are never silent**: panel commands wait for the Rust side to finish
  and return any error to the frontend, which blocks the UI while waiting and
  shows the error in a banner. The background task records its failures in the
  status visible from the panel.
- **Copyright**: the API's `copyright` field is kept and shown in the tray and
  the panel. When present, the image is **not** public domain: it belongs to
  its author and use is limited to a personal wallpaper. Images without a
  copyright are produced by NASA and are public domain.

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
- **KDE Plasma**: the wallpaper is set by evaluating a script through
  `qdbus`. Some Qt6-only distributions ship that binary as `qdbus6` with no
  `qdbus` alias, in which case setting the wallpaper fails with the explicit
  error shown in the panel; installing the Qt5 `qdbus` tool fixes it. This has
  not been verified on Plasma 6.
- **Multiple monitors**: the image is composed at the primary screen's
  resolution; secondary screens get the same image. Plugging, unplugging or
  resizing a screen is noticed and recomposed for; which screen counts as
  primary is whatever the OS reports.
- **Linux and suspend**: waking from suspend is not reported (it would take a
  D-Bus client for logind), so an update due while the machine was asleep
  happens within six hours of the machine coming back rather than immediately.
- **macOS**: setting the wallpaper goes through an AppleScript event, which
  requires the Automation permission described in the install section.
- **Unsigned builds**: see the install section for the Gatekeeper and
  SmartScreen steps. Signing them properly needs paid certificates from Apple
  and a Windows CA.

## Licence

- Project code: to be defined by the repository owner.
- APOD images with a copyright notice remain the property of their authors.
