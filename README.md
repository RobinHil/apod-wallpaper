# APOD Wallpaper

An application that downloads NASA's Astronomy Picture of the Day (APOD) and sets it as the desktop picture, once a day. It runs in the background, has no window of its own most of the time, and is opened from its tray icon or by launching it again.

| Platform | Requires | Ships as |
|---|---|---|
| macOS | 13.3 or later, Apple silicon or Intel | one universal `.dmg` |
| Linux, GNOME | GNOME 42 or later, X11 or Wayland | `.deb`, `.rpm`, `.AppImage`, and a PKGBUILD for Arch |

Windows is the remaining goal. Only four things are ever written twice -- setting the desktop picture, being told about screen changes and wake-ups, measuring the screen, and decoding a still out of a video -- and everything else, the scheduling, the API client, the image composition and the panel, is shared as it stands. Adding a platform means writing those four and nothing more.

GNOME is named rather than "Linux" on purpose. The wallpaper is set through GNOME's own settings and the two system events come from Mutter and logind, so KDE, Xfce and the rest are not supported today and the application says so rather than silently doing nothing.

Built with [Tauri 2](https://tauri.app): a Rust backend and a settings panel written in React and TypeScript, styled with Tailwind CSS.

The project has a page of its own at [robinhil.github.io/apod-wallpaper](https://robinhil.github.io/apod-wallpaper/), in English and [in French](https://robinhil.github.io/apod-wallpaper/fr/): what the application does, the day's picture and the download links. It is built from `landing/` and published to GitHub Pages by its own workflow.

> [!NOTE]
> This project was written with heavy AI assistance, and is stated up front so you can read the code knowing where it came from. The design decisions, the review of every change and the testing on real hardware are mine; a large part of the code itself was generated. Judge it on what it does, and on the reasons given in [Design notes](#design-notes), rather than on who typed it.

---

## Table of contents

- [How it works](#how-it-works)
- [Command-line options](#command-line-options)
- [Where files are stored](#where-files-are-stored)
- [Building from source](#building-from-source)
- [Installing](#installing)
- [Starting at login](#starting-at-login)
- [Uninstalling](#uninstalling)
- [NASA API key](#nasa-api-key)
- [Project layout](#project-layout)
- [Design notes](#design-notes)
- [Troubleshooting](#troubleshooting)
- [Known limitations](#known-limitations)
- [Licence](#licence)

---

## How it works

### Choosing the image

Three modes, selected in the settings panel:

- **Picture of the day**: the image the APOD API reports as the most recently published one. No date is sent with the request, so the API decides what "today" is and the local time zone cannot skew the result.
- **Random**: a date drawn at random from the whole archive, which starts on 16 June 1995. Clicking "Random" again draws another one immediately.
- **Specific date**: the APOD published on a date you pick, between 16 June 1995 and today. A few days in the archive have no publication; those are reported and the current wallpaper is kept.

Some APOD entries are videos, and a still is used for those. YouTube and Vimeo entries use the thumbnail the API publishes, the maximum-resolution one when YouTube has it. Entries published as a plain video file have no thumbnail, so a frame is decoded out of the file itself. The panel says which of the two is on the desktop and offers a link to watch the video.

### When the wallpaper changes

The wallpaper is updated when the app starts and at the local day change, and the process does nothing at all in between. There is one background task; it attempts an update, then sleeps until just after the next midnight.

Three things end that sleep early:

- **A screen change** : a new resolution, a scale change, a display plugged in or unplugged. The wallpaper is recomposed for the new size from the original already on disk, without touching the network.
- **Waking from sleep** : the pending wait was measured against a clock that stops while the machine is suspended, so the task is told about the wake-up and re-reads the wall clock.
- **A failure** : no network, an exhausted API quota, an API outage. The app retries with an exponential backoff (10 s, 20 s, 40 s, ...) with ±20 % jitter, capped at 15 minutes, until it succeeds. The wallpaper in place is never disturbed by a failure.

When today's picture has not been published yet, the most recent one is applied and the app looks again every 30 minutes rather than hammering the API.

"Refresh now", in the panel, applies an image immediately, including re-applying the current one, which is what restores it if you have set another desktop picture by hand since.

### How the image is fitted to the screen

The image is composed at the exact pixel size of the main display, in one of two modes:

- **Blurred fill** (default): the whole image, undistorted, centred over a blurred and darkened copy of itself that fills the screen. Nothing is cropped and nothing is stretched.
- **Crop**: the image is cropped to the screen's aspect ratio and fills it.

No text is ever burned into the image. The date and the copyright are shown in the panel and in the tray menu.

### The panel and the tray

Everything is in the **settings panel**: the current image and its credits, the three modes, the fit mode, the manual refresh, the NASA API key, and the quit button. Closing it leaves the app running in the background.

The **tray item** -- the menu bar on macOS, the top bar on GNOME -- shows the current image's title, date and copyright, and opens the panel or quits. Everything it offers exists in the panel as well, which matters more on GNOME than on macOS: see below.

The app takes no space in the Dock or the dash. On macOS that is declared through `LSUIElement`; on GNOME an application with no window open occupies nothing already. Starting it puts nothing on screen either way: it goes straight to the tray and sets to work. The panel is only ever opened deliberately, from the tray item or by launching the application again -- from the Finder, Spotlight or Launchpad on macOS, from the Activities overview on GNOME. That second launch does not leave a second copy running; it brings up the panel of the instance already there.

The one exception is the very first launch, which opens the panel once so the app is not invisible on a machine that has never run it. Every later start, including at login, is silent.

> [!IMPORTANT]
> **GNOME has no tray.** Since GNOME 3.26 the shell shows no status icons without an extension, [AppIndicator and KStatusNotifierItem Support](https://extensions.gnome.org/extension/615/appindicator-support/) being the usual one. Without it the application runs perfectly well and simply has no icon, and launching it again from the overview is how you reach the panel. Nothing it does depends on the tray existing.

---

## Command-line options

```
apod-wallpaper [OPTIONS]

  -h, --help     Show the usage message
  -V, --version  Show the version
```

Neither option starts the application; both print and exit. There is nothing here that changes how it runs, because there is only one way it runs: quietly.

To run the binary inside the macOS bundle directly:

```bash
"/Applications/APOD Wallpaper.app/Contents/MacOS/apod-wallpaper" --help
```

On Linux it is on the `PATH` already, so `apod-wallpaper --help` is enough.

---

## Where files are stored

Everything lives in one directory, wherever the platform puts application data:

| Platform | Directory |
|---|---|
| macOS | `~/Library/Application Support/com.rh.apod-wallpaper/` |
| Linux | `~/.local/share/com.rh.apod-wallpaper/` |

```
settings.json                            API key, mode, chosen date, fit mode
state.json                               the wallpaper currently applied
current/<date>.<ext>                     the downloaded original
current/wall-<date>-<fit>-<w>x<h>.jpg    the composition set as the wallpaper
```

Only one image is kept, the one on your desktop. Everything else is deleted as soon as a new wallpaper has been applied, so the directory stays under a few megabytes.

---

## Building from source

Everywhere: [Rust](https://www.rust-lang.org/tools/install) (stable) and [Node.js](https://nodejs.org) 22 or newer (CI builds on 24, the active LTS).

The package manager is [pnpm](https://pnpm.io). The version CI builds with is recorded as `packageManager` in `package.json`; pnpm reads that field itself and switches to the version named there, so any recent pnpm will do to start:

```bash
npm install -g pnpm     # or `brew install pnpm`, or your distribution's package
```

### The platform toolchain

On **macOS**, the Xcode Command Line Tools:

```bash
xcode-select --install
```

On **Linux**, the development headers Tauri and `video_frame` build against. The panel is rendered by WebKitGTK, the tray by libayatana-appindicator, and a video APOD is decoded by GStreamer:

```bash
# Debian, Ubuntu
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev \
  libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev

# Fedora, RHEL
sudo dnf install webkit2gtk4.1-devel gtk3-devel \
  libayatana-appindicator-gtk3-devel librsvg2-devel \
  gstreamer1-devel gstreamer1-plugins-base-devel

# Arch
sudo pacman -S --needed webkit2gtk-4.1 gtk3 libayatana-appindicator \
  librsvg gst-plugins-base gst-plugins-good
```

### Building

```bash
pnpm install
pnpm tauri dev         # development, with a live-reloading panel
pnpm bundle            # release packages for the machine it runs on
```

On macOS `pnpm bundle` produces `src-tauri/target/release/bundle/macos/APOD Wallpaper.app` and a `.dmg` next to it. On Linux it produces a `.deb`, an `.rpm` and an `.AppImage`, each in its own directory under `src-tauri/target/release/bundle/`.

The `.deb` and `.rpm` build anywhere, and `pnpm bundle` is all they need. The AppImage is fussier, because `linuxdeploy` and its two plugins assume a Debian-shaped host, and they fail one at a time rather than all at once. There is a wrapper for exactly that:

```bash
./packaging/linux/bundle.sh          # arguments are passed through to `pnpm bundle`
```

It sets the environment those plugins want, adds the two things Arch needs when it is running on Arch, and checks for the programs they shell out to before starting rather than twenty minutes in. The Linux CI job runs the same script, so there is one incantation to keep correct instead of one here and one in the workflow.

It installs nothing, so this part is still yours to run:

```bash
sudo apt install patchelf                              # Debian, Ubuntu
sudo pacman -S --needed patchelf webp-pixbuf-loader    # Arch
```

What the script sets, each answering a genuine missing piece rather than papering over one:

- `patchelf` is shelled out to by the GStreamer plugin, on every distribution.
- `webp-pixbuf-loader` exists to recreate `/usr/lib/gdk-pixbuf-2.0/2.10.0`. Since gdk-pixbuf 2.44 the loaders are built into the library and that directory is gone, but linuxdeploy's GTK plugin copies it without checking. Any package that installs a loader there will do; this is the smallest, and pacman owns the directory rather than leaving an orphan behind.
- `APPIMAGE_EXTRACT_AND_RUN` is for hosts that have dropped libfuse2, since `linuxdeploy` is an AppImage that otherwise mounts itself.
- `NO_STRIP` is for the `strip` linuxdeploy carries, which is too old to read the `.relr.dyn` sections a current toolchain emits and treats every failure as fatal.
- `GSTREAMER_HELPERS_DIR` is because the GStreamer plugin guesses a Debian multiarch path for `gst-plugin-scanner` with no fallback, while Arch keeps it beside the plugins.

`GSTREAMER_INCLUDE_BAD_PLUGINS` is not distribution-specific: it is what pulls in the OpenH264 decoder, without which the AppImage bundles a media framework that cannot decode a video APOD. OpenH264 rather than libav, so the licence section below stays true.

The two other packages are produced before the AppImage step, so a failure there still leaves you with them.

macOS releases are one universal bundle instead, which is what CI builds. Both targets have to be installed for it:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
pnpm bundle --target universal-apple-darwin
```

Naming a target moves the output under `src-tauri/target/<triple>/release/bundle/`, so `universal-apple-darwin/release/bundle/` here.

Arch is the one target Tauri has no bundler for, so `packaging/aur/PKGBUILD` builds it natively instead:

```bash
cd packaging/aur
makepkg -si
```

Note what that builds: the tarball GitHub generates for the `v$pkgver` tag, not the working tree beside it. It therefore only succeeds against a tag that already contains everything the build needs, `pnpm-lock.yaml` included, so pointing it at a release older than that fails in `pnpm install --frozen-lockfile` rather than anywhere interesting. Bump `pkgver` and run `updpkgsums` when cutting a release; a correction to the recipe alone bumps `pkgrel` instead.

On macOS, CI mounts the `.dmg` it built, copies the app into `/Applications` and starts it from there, which is the same route a person takes. Being unsigned and never downloaded by a browser, it carries no quarantine attribute, so that run says nothing about the Gatekeeper step below.

CI installs every Linux package as well, each on the distribution it targets: the `.deb` on Debian, the `.rpm` on Fedora, the Arch package on Arch, and the AppImage on Fedora too. Each container starts stock and lets its own package manager resolve what the package declares, which is the only thing that checks those dependency names, written by hand and resolved nowhere else. A release is published only once all of that has passed.

CI builds the Arch package too, in an `archlinux:base-devel` container, from the checkout rather than from the published tag. It then unpacks the result and checks that the binary inside embeds the panel, because the way this recipe fails is not a build error: built through `cargo build` instead of the Tauri CLI, the binary compiles, passes its tests, installs, sets a wallpaper, and only then shows a connection error where the panel should be.

Before opening a pull request, the same checks CI runs:

```bash
cd src-tauri
cargo fmt --all --check
cargo clippy --locked -- -D warnings                 # lints, as the app is shipped
cargo clippy --locked --all-targets -- -D warnings   # lints, tests included
cargo test --locked
cd ..
pnpm exec tsc --noEmit
```

Clippy twice is not a typo: `--all-targets` builds the tests, whose dev-dependencies can supply a feature the library needs but never declares. That builds, and the release build, which has no dev-dependencies, then fails. The first command is the one that matches what users install.

These checks only ever see the platform they run on. The compiler compiles one `cfg` branch and clippy lints one `cfg` branch, so a change to the GNOME backends is not checked by running this on a Mac, nor the Apple ones by running it on Linux. CI runs the whole block on both for that reason, and a change to either backend deserves the same treatment before it is pushed.

---

## Installing

Release builds are downloadable from the [releases page](https://github.com/RobinHil/apod-wallpaper/releases).

### macOS

There is one `.dmg` and it is universal: the same file runs on Apple silicon and on Intel.

Open the `.dmg` and drag the application into `Applications`. The builds are **not signed**, an Apple code-signing certificate is a paid subscription -- so the first launch is refused with a message about the app being damaged. That is Gatekeeper's quarantine flag, not a corrupted download. Clear it once:

```bash
xattr -d com.apple.quarantine "/Applications/APOD Wallpaper.app"
```

Alternatively, right-click the app, choose *Open* and confirm, or allow it from *System Settings > Privacy & Security* right after the failed launch.

The first time the app sets the wallpaper, macOS asks for permission to control **System Events**: that Apple event is how the desktop picture is set. If you refuse, re-enable it under *System Settings > Privacy & Security > Automation*.

Launching it opens the panel once, the only time it does, applies the first wallpaper and leaves the app running in the background.

### Linux

Pick the package your distribution understands. The `.deb` and `.rpm` pull in what they need; the `.AppImage` needs nothing installed and runs anywhere, which is also how Arch users can try it before building the PKGBUILD.

```bash
# Debian, Ubuntu
sudo apt install ./APOD*.deb

# Fedora, RHEL
sudo dnf install ./APOD*.rpm

# Anywhere, including Arch
chmod +x APOD*.AppImage
./APOD*.AppImage
```

The packages are built on Ubuntu 24.04, so they need **glibc 2.39 or later**: Debian 13, Ubuntu 24.04, Fedora 40 and anything more recent, Arch included. Debian 12 and Ubuntu 22.04 are too old for them, and that is a deliberate trade rather than an oversight: distributions are not cross-compiled for one another, what decides where a package runs is the glibc it was linked against, and a newer build reaches fewer systems rather than more. Anyone on one of those two can still build from source, where nothing requires a recent glibc.

For Arch, `packaging/aur/PKGBUILD` builds natively from source instead; see [Building from source](#building-from-source).

Nothing appears when it starts, bar the first launch, which opens the panel once and applies the first wallpaper. If you want the tray icon, install the [AppIndicator extension](https://extensions.gnome.org/extension/615/appindicator-support/); without it the application is simply icon-less and everything still works.

---

## Starting at login

The application does not register itself. Starting at login is a change to your session, and you make it yourself, once.

### macOS

Open *System Settings > General > Login Items & Extensions*, and under **Open at Login** add `/Applications/APOD Wallpaper.app`. That is all there is to it.

Nothing appears when you log in. Earlier versions needed a launch agent here, purely so the app could be passed a flag telling it not to open its panel; the app no longer opens it on its own, so the flag and the agent are both gone. If you still have `~/Library/LaunchAgents/com.rh.apod-wallpaper.plist` from one of those versions, remove it:

```bash
launchctl unload ~/Library/LaunchAgents/com.rh.apod-wallpaper.plist
rm -f ~/Library/LaunchAgents/com.rh.apod-wallpaper.plist
```

### GNOME

Either tick **APOD Wallpaper** under *Startup Applications* in GNOME Tweaks, or drop the desktop entry the package already installed into the autostart directory:

```bash
mkdir -p ~/.config/autostart
cp "/usr/share/applications/APOD Wallpaper.desktop" ~/.config/autostart/
```

There is no systemd user unit, and deliberately so: the application already sleeps between day changes and needs nothing restarted or supervised, so a unit would add a second thing to configure for no behaviour that is not already there.

---

## Uninstalling

Quit the application first, from the panel or the tray.

**Choose another desktop picture before you delete the data directory.** The image on your desktop lives in it, and on both platforms the background goes blank at the next login if it disappears from under them.

### macOS

Remove it from *System Settings > General > Login Items & Extensions* if you added it there, then:

```bash
rm -rf "/Applications/APOD Wallpaper.app"
rm -rf ~/Library/Application\ Support/com.rh.apod-wallpaper
```

### Linux

```bash
sudo apt remove apod-wallpaper      # or: sudo dnf remove apod-wallpaper
                                    # or: sudo pacman -R apod-wallpaper
rm -f ~/.config/autostart/"APOD Wallpaper.desktop"
rm -rf ~/.local/share/com.rh.apod-wallpaper
```

An AppImage is a single file, so deleting it is the whole uninstall; the data directory above is still worth removing.

---

## NASA API key

By default the app uses `DEMO_KEY`, which NASA limits to **30 requests per hour and 50 per day, per IP address**. The app makes at most a handful of requests a day, so that is usually plenty; a personal key is worth having if you share an IP address with other users of the API.

1. Request one at <https://api.nasa.gov/>, a short form, the key arrives by email.
2. Open the panel, paste it into the "NASA API key" field, click "Save".

Saving a key immediately retries the update, which is normally why you are typing one in. The key is stored in `settings.json` and is only ever sent to the NASA API; error messages have the request URL stripped out of them so it cannot leak into the panel.

---

## Project layout

Four files carry everything that is not portable, marked below. Each holds both implementations, because reading them side by side is how you tell that the two desktops are being asked to do the same thing.

```
apod-wallpaper/
|- src-tauri/                    # Rust backend
|  |- src/
|  |  |- main.rs                 # Binary entry point
|  |  |- lib.rs                  # Wiring: shared state, the UiState contract, startup
|  |  |- commands.rs             # The seven commands the panel invokes
|  |  |- panel.rs                # The settings window, built on demand
|  |  |- tray.rs                 # The tray item and its two live labels
|  |  |- scheduler.rs            # The only background task: when to update
|  |  |- updater.rs              # What an update does, end to end
|  |  |- nasa_api.rs             # APOD API calls, parsing, error taxonomy
|  |  |- store.rs                # state.json + the two image files, atomic writes
|  |  |- image_compose.rs        # Ratio handling: blurred fill or crop
|  |  |- settings.rs             # API key, mode, fit; JSON persistence
|  |  |- memory.rs               # Returning an update's working set to the OS
|  |  |
|  |  |- wallpaper.rs            # [per-platform] Setting the desktop picture
|  |  |- os_events.rs            # [per-platform] Screen-change and wake-up notifications
|  |  |- screen.rs               # [per-platform] Measuring the main display
|  |  |- video_frame.rs          # Choosing which frame of a video to use
|  |  |- video_frame_macos.rs    #   ... decoded by AVFoundation
|  |  `- video_frame_gnome.rs    #   ... decoded by GStreamer
|  |- Info.plist                 # macOS: background app (LSUIElement) + Apple events usage
|  |- capabilities/default.json
|  `- tauri.conf.json            # Bundle targets and the Linux package dependencies
|- src/                          # Panel frontend (React + TypeScript), shared
|  |- main.tsx                   # Mounts the panel into index.html
|  |- App.tsx                    # Composes the cards
|  |- useAppState.ts             # Backend state, commands, pushed events
|  |- useSyncedField.ts          # Text fields the backend also owns
|  |- types.ts                   # UiState: the contract with the Rust side
|  |- links.ts                   # APOD page and video URLs
|  |- classes.ts                 # Utility strings more than one card needs
|  |- components/                # One file per card, plus the SVG icons
|  `- styles.css                 # Tailwind entry point: the palette, light and dark
|- packaging/aur/                # What Tauri has no bundler for
|  |- PKGBUILD
|  `- apod-wallpaper.desktop
|- landing/                      # The page on GitHub Pages, unrelated to the app
|- index.html                    # Mount point for the panel
`- .github/workflows/
   |- ci.yml                     # Lint/test gate on both platforms, bundles, release
   `- landing.yml                # Builds landing/ and publishes it to Pages
```

---

## Design notes

### Scheduling: once a day, and otherwise asleep

The application is meant to be invisible in Activity Monitor. There is exactly one background task, and it does this:

1. Attempt an update. There is no separate "am I online?" probe, the fetch is the probe, which is one round trip instead of two.
2. On success, sleep until the next local day change.
3. On failure, or when today's APOD is not published yet, retry on a backoff until it succeeds, then go back to step 2.

Nothing else is armed: no polling of the API, no periodic reapplication, no timer that exists only to ask whether anything has happened. When the wallpaper is up to date the process is sleeping on a single timer, for the nine or so hours to the next midnight, not in instalments.

A sleep that long has to survive the machine being suspended, and the timer counts against a clock that stops while it is. So the desktop is asked to say when it happened, and the task is woken, re-reads the wall clock and decides again -- which costs one observer and no polling. Screen changes arrive the same way, as a notification rather than as something looked for.

| | Waking from sleep | Screen changed |
|---|---|---|
| macOS | `NSWorkspaceDidWake` | `NSApplicationDidChangeScreenParameters` |
| GNOME | `PrepareForSleep` on logind | `MonitorsChanged` on Mutter |

Both AppKit notifications come from a framework Tauri's macOS backend already links, and both GNOME signals travel over D-Bus, which GLib -- already linked by its Linux backend -- speaks. Neither pair adds anything to the build.

`PrepareForSleep` fires twice per suspend, `true` on the way down and `false` on the way back up; only the second is acted on. Waking is what invalidates the pending wait, and running an update into a machine that is about to stop executing it would achieve nothing.

The same reasoning applies to retries. Subscribing to `NWPathMonitor`, or to NetworkManager's state on the other side, would be another resident observer to learn something a connection attempt reports locally in about a millisecond when there is no network. A capped backoff is cheaper than the machinery to avoid it.

### Nothing is redone that does not need to be

`state.json` records the applied image and the inputs its composition depended on, fit mode and screen size. At startup, if that record already answers the current settings and both files are on disk, the app does nothing at all: no API call, no download, no wallpaper-set call. Restarting five times in a day costs five `state.json` reads.

Changing the fit mode, or moving to a display with a different resolution, recomposes from the stored original without touching the network.

When the desktop reports no main display at all, lid closed, no external screen -- the size the wallpaper was last composed for is reused. That is not a resolution change, and recomposing for a guessed size would replace a correct wallpaper with a wrong one.

### The screen is measured by the compositor, not the toolkit

Tauri reports monitor sizes, and on macOS that is the answer: it hands back the backing size, Retina scaling included.

On GNOME it is the wrong answer. Under Wayland with fractional scaling, GTK 3 reports the *logical* size, so a 1920x1200 panel at 125% comes back as 1536x960. Composing from that would hand GNOME an image it then has to enlarge to fit the screen -- which is precisely the blurry wallpaper this application exists to avoid, arrived at by a different route.

So Mutter is asked instead, over D-Bus, for the mode the monitor is actually in. Reading its reply is a nest of tuples and it is worth the unpleasantness: it is the only source that knows the real pixels.

### Failures never break the desktop

The download is validated by decoding it in memory, composed into a wallpaper, and only then moved into place with atomic renames. The previous image stays on disk and on the desktop until the new one has actually been applied, so a partial download, a full disk or a crash mid-update cannot leave a black or broken background. `state.json` and `settings.json` are written the same way.

### Setting the desktop picture

macOS exposes no public API for the desktop picture that works across every Space at once; the supported route is an Apple event to **System Events**, which is the AppleScript the `wallpaper` crate wraps. Two consequences the app has to live with:

- `Info.plist` carries `NSAppleEventsUsageDescription`. Without it the system refuses the event outright rather than prompting, and the first wallpaper would fail with nothing the user could act on.
- The call is thoroughly blocking, seconds of it when the desktop is busy, so it runs on tokio's blocking pool, never on a runtime worker, where it would hold up every panel command queued behind it.

GNOME keeps the background in GSettings rather than behind an API, so setting it is writing `picture-uri`, `picture-uri-dark` and `picture-options` under `org.gnome.desktop.background`, and flushing. Three things worth saying about that:

- Both URI keys are written, so the picture does not change with the light and dark theme. It is the same image either way, and leaving the dark key behind would swap the wallpaper every time the theme did.
- `picture-options` is set to `zoom` even though the composition already matches the screen exactly. It only decides what GNOME does on the occasions it disagrees about that size, a monitor change it noticed first being the usual one, and filling without distorting is the failure worth having.
- Opening a GSettings schema that does not exist aborts the process, which is exactly what a non-GNOME session would do. The schema is looked up first, so that turns into an error the panel can show instead of a crash.

Both platforms cache the background by what it is called -- macOS by path, GNOME by URI -- and the file name already carries the date, the fit mode and the screen size, so neither needs a cache-busting step of its own.

### The tray item is optional

The panel carries every setting, the refresh and the quit button; launching the app again opens that panel; and a tray item that fails to build is logged and stepped over rather than being a startup error. Nothing the application does depends on it existing.

That was a design principle on macOS. On GNOME it is a requirement: the shell has shown no status icons since 3.26 without an extension, so for a good number of users the icon will simply never appear. The application is built so that this costs them a convenience and nothing else.

### Starting is not the same as being asked to appear

Starting the app puts nothing on screen. It is started at login, and a login that throws a window at you is precisely what a background utility must not do. Opening the panel is therefore always a deliberate act, and there are two of them, which reach the app by two different routes.

Clicking the tray item is the direct one. Launching the application again is the indirect one, and it splits in two: for the installed macOS `.app`, macOS does not start a second process at all, LaunchServices reactivates the one already running and sends it `applicationShouldHandleReopen:`, surfacing in Tauri as `RunEvent::Reopen`. Everywhere else -- the binary inside the macOS bundle run directly, and every launch on GNOME -- a second process really does start, and there the single-instance plugin hands the launch to the running instance and exits. Both end in the same place.

The one automatic opening left is the very first launch, decided by the absence of `settings.json`. On a machine that has never run the app there is no tray icon the user has learnt to look for and no wallpaper to notice, so a completely silent first start would be indistinguishable from one that failed. On GNOME, where the tray icon may never exist at all, that first panel is also where the user learns the application is running.

### Video APODs

A video is not a wallpaper, so a still is taken from it. Which still depends on how the video was published, and APOD does it two ways.

Most video entries are **YouTube or Vimeo embeds**. The API has thumbnails for those, and asked for them (`thumbs=true`) it returns one. YouTube stores that one picture at several sizes and does not generate the big ones for every video, so they are tried biggest first, `maxresdefault` (1280x720), then `sddefault` (640x480), with the thumbnail as published as the last resort. It is the same picture at each step; starting from more pixels only means less upscaling on the way to a screen-sized wallpaper.

A thumbnail is what the uploader chose, which is not always a frame of the video: it is sometimes a cover with a title burned into it, and that is what lands on the desktop. Nothing here can tell the two apart, and it is left that way, video APODs are a handful of days a year, and telling a cover from a frame would take OCR.

The rest are served as a **plain file** on apod.nasa.gov, an `.mp4`. The API has no thumbnail for those and returns an empty string in its place, so the file is downloaded and a frame is decoded out of it. The decoder is whichever one the desktop already has: AVFoundation on macOS, GStreamer on GNOME. Both are linked the same way the wallpaper machinery already links AppKit and GLib, so neither adds anything for anyone to install, and on both the formats that work are the ones the system can play.

That last clause is a real limitation and not a disclaimer. Fedora ships no H.264 decoder, for patent reasons, so a video APOD published as H.264 yields no frame there until `gstreamer1-libav` is installed from RPM Fusion. The application treats that exactly as it treats a video macOS cannot open: it says so and keeps the wallpaper already in place.

The frame is not the first one, videos open on black, on a fade-in, or on a title card. Four instants spread through the video are tried in turn, and the first one with enough contrast to be a picture rather than a flat colour is kept; if all four are flat, the least flat of them is. That choice is shared code, and only opening the file and decoding one instant differ between the two platforms. What gets archived is that frame as a JPEG, not the video: the stored original is what a later fit-mode or resolution change recomposes from, and keeping tens of megabytes to decode again each time would buy nothing.

Videos shot in portrait carry their rotation as metadata rather than in the pixels, so it has to be applied or the frame lands on its side. macOS has `setAppliesPreferredTrackTransform`; GNOME gets the same result from `videoflip method=automatic`, which reads the orientation tag.

Either way the panel flags the entry and links to the video, to YouTube or Vimeo for an embed, to the APOD page for a file, which is where a raw `.mp4` is meant to be watched, and the tray appends "(video)" to the title. When nothing at all can be made of an entry, the current wallpaper is kept in daily mode, and another date is drawn in random mode.

### Daily mode sends no date

The app asks the API for "the most recently published image" rather than for the local date, which removes the time-zone skew (APOD is published on US Eastern time). Just after local midnight the API still serves yesterday's picture; that counts as *not yet satisfied*, so the app applies it if it is new and keeps looking until today's appears. Pinning the request to the local date would skip today's picture entirely on some days.

### The panel holds no state of its own

The backend is the single source of truth. Every panel command returns a whole `UiState`, and the backend pushes one on `state-updated` whenever it changes something by itself, the daily update, a screen change, a wake from sleep. The panel renders what it is given and never computes a setting locally, so the two can never disagree about what is applied.

Two exceptions, both deliberate and both local to a component: the date picker stays visible from the moment "Specific date" is clicked, before any mode has actually changed, and the two text fields hold what is being typed. Those fields are re-seeded from every push, except while they have the focus -- otherwise a background update landing mid-sentence would wipe out a half-entered API key.

While a command is in flight the whole UI is covered by an overlay and pushed updates are ignored, so nothing moves under the pointer between the click and the result.

### One palette, two appearances

The colours are Tailwind theme tokens: `--color-card` in the `@theme` block of `styles.css` is what makes `bg-card`, `border-card` and `text-card` exist. Dark mode redefines those same variables under `prefers-color-scheme`, which is why no component carries a `dark:` variant, the utilities already point at the variable, and the variable changes underneath them.

Two of Tailwind's preflight rules are handed back to WebKit in the same file. It tints the placeholder from the input's own colour, and it strips the padding out of the date field, which leaves that control shorter than the text input beside it. Both are reverted rather than worked around, so the two form rows line up.

Utility strings that more than one card needs live in `classes.ts` as constants. They are never merged: two utilities setting the same property are resolved by their order in the generated stylesheet and not by their order in the attribute, so a variant such as the selected segment spells out its own colours instead of layering them over a base.

### One bundle for both architectures

A macOS release is a single universal `.dmg` rather than one per architecture. It is twice the size, 10 MB instead of 5, which for something downloaded once is a better trade than asking every user which of two files they need.

Keeping the Intel slice is not sentiment. Rosetta translates x86_64 to ARM and never the reverse, so an Apple-silicon-only build runs on no Intel Mac at all, and Intel hardware runs every version of macOS this app supports. CI checks that both slices are present and that both are compiled for the advertised minimum.

macOS 26 is the last release supporting Intel hardware, and those machines go on receiving security updates for about three years after it. That, rather than the arrival of newer Macs, is when the slice stops earning its place.

### Three Linux packages, and why not one

There is no universal binary trick on Linux, because the thing that varies is not the architecture but the packaging: distributions disagree about how software is installed, not about what instructions the processor runs. So there are three formats rather than one file, and each is the right answer for somebody.

Nor is any of this cross-compilation, whatever the word suggests. A single x86_64 build is produced and packaged three ways; what decides where it runs is the glibc it was linked against, and that is forward-compatible and not backward. So the runner image is a real choice and not a detail: an older one reaches more systems, a newer one fewer. CI builds on Ubuntu 24.04, the most recent image that is generally available rather than preview, and the floor that falls out of it is stated above the way macOS 13.3 is.

Arch is the exception, and it is a deliberate one. Tauri has no bundler for it, and rather than invent a fourth artifact the repository carries a `PKGBUILD` that builds from source the way Arch expects. The AppImage is there for anyone who would rather not.

### Other

- **The stylesheet sets the supported macOS, not the backend**: Tailwind emits cascade layers and `@property`, which arrived in the WebKit that shipped with macOS 13.3. Below that the panel does not degrade, it comes up unstyled, so 13.3 is what `minimumSystemVersion` records. `build.target` in `vite.config.ts` names the same Safari, so the bundled JavaScript never outruns the browser the stylesheet already requires. The three move together or not at all. WebKitGTK on Linux is far newer than that floor, so it never binds.
- **Cheap gaussian blur**: the backdrop is blurred on a 1/8 scale copy and scaled back up. The result is indistinguishable from a heavy blur on the full-size image, for a fraction of the CPU.
- **Varying file name**: the composition carries the date, fit mode and screen size in its name, because both desktops cache the background by what it is called -- macOS by path, GNOME by URI -- and ignore a file rewritten in place.
- **Memory is given back after an update**: an update is the only moment anything large is allocated, a full-resolution APOD decoded in memory plus two screen-sized buffers, and the process then sleeps for twelve hours. Freed is not returned, so `memory::release_to_os` asks the allocator once, at the point where everything large has certainly been dropped. Measured on GNOME with a 2.2-megapixel original: 13.1 MB of private memory on a start with nothing to do, 25.7 MB after one update without it. glibc only; musl and macOS have no equivalent worth binding by hand.
- **Opening the panel is what costs memory, not keeping it open**: 28.6 MB for a process that has never opened it, 154 MB once it has been opened and closed, because WebKitGTK keeps its share for the life of the process. Nothing in the application requires the panel, so a user who never opens it pays none of that.
- **Errors are never silent**: panel commands wait for the Rust side to finish and return any error to the frontend, which blocks the UI meanwhile and shows the message in a banner. The background task records its failures in the status line the panel displays.
- **Copyright**: the API's `copyright` field is preserved and shown in the panel and the tray. When it is present the image is **not** public domain: it belongs to its author, and using it is limited to a personal wallpaper. Images without one are NASA's and are public domain.

---

## Troubleshooting

**There is no icon anywhere.** Expected on macOS: it is a background utility with no Dock icon, so use the menu bar item or launch the application again. Expected on GNOME too, and for a second reason: the shell shows no tray icons without the [AppIndicator extension](https://extensions.gnome.org/extension/615/appindicator-support/). Either way, launching the application again brings the panel up rather than starting a second copy.

**The wallpaper does not change.** Open the panel first: every failure is shown there, in a banner or in the status line at the bottom. If nothing is reported and the desktop still has not changed, ask the desktop what it thinks it is showing.

```bash
# macOS
osascript -e 'tell application "System Events" to get picture of current desktop'

# GNOME
gsettings get org.gnome.desktop.background picture-uri
```

It should name a file in the `current/` directory listed under [Where files are stored](#where-files-are-stored). If it does and the screen disagrees, the desktop is at fault rather than the application.

On macOS, a wallpaper that never changes and reports nothing is almost always the Apple event being denied: check *System Settings > Privacy & Security > Automation* and make sure **APOD Wallpaper** is allowed to control **System Events**.

**The wallpaper looks soft, or is scaled oddly, on GNOME.** Check that the composition matches your screen. The file name carries the size it was composed for, so compare it with what the compositor reports:

```bash
gsettings get org.gnome.desktop.background picture-uri
busctl --user call org.gnome.Mutter.DisplayConfig /org/gnome/Mutter/DisplayConfig \
  org.gnome.Mutter.DisplayConfig GetCurrentState
```

**A video APOD produced no frame, on Linux.** The decoder is the system's, so the gap is the system's too. Fedora and RHEL ship no H.264 decoder for patent reasons; install `gstreamer1-libav` from RPM Fusion. On Debian and Ubuntu the package is `gstreamer1.0-libav`, on Arch `gst-libav`. To check what GStreamer can decode:

```bash
gst-inspect-1.0 | grep -iE "h264|avdec"
```

**"APOD Wallpaper is damaged and can't be opened."** The quarantine flag on an unsigned macOS download. See [Installing](#installing).

**Started at login and nothing appeared.** That is the intended behaviour, there is no window at login, by design. Check it is actually running:

```bash
pgrep -a apod-wallpaper
```

If it is not, confirm the login entry: *System Settings > General > Login Items & Extensions* pointing at `/Applications/APOD Wallpaper.app` on macOS, or `~/.config/autostart/APOD Wallpaper.desktop` on GNOME.

---

## Known limitations

- **Multiple displays**: the image is composed at the main display's resolution, and the desktop applies it to every screen, every Space and every workspace. On a second screen of a different size it is scaled to fit. Plugging, unplugging or resizing a display is noticed and recomposed for.
- **GNOME only, on Linux**: the wallpaper goes through GNOME's settings and the two system events come from Mutter and logind. On KDE, Xfce or anything else the application starts and then reports that it cannot set the wallpaper, rather than failing silently. Supporting another desktop means writing the same four pieces again for it.
- **No system tray on stock GNOME**: since 3.26 the shell shows no status icons without an extension. The application is built not to need one, so the cost is the convenience of the icon and nothing else.
- **Video APODs need a decoder the system has**: true on both platforms, and it bites on Fedora and RHEL, which ship no H.264 decoder for patent reasons. See [Troubleshooting](#troubleshooting). The AppImage is the exception: it carries its own GStreamer, OpenH264 included, which is most of why it is three times the size of the other packages.
- **The Apple event permission** is asked for once and has to be granted. A denied automation permission is silent from the app's side: the event fails, the error reaches the panel, but nothing can re-prompt for it, it has to be re-enabled in System Settings.
- **Unsigned builds**: the Gatekeeper step in the install section is needed on every macOS download. Signing properly requires a paid Apple Developer certificate. The Linux packages are unsigned too, which matters less because no equivalent gate exists there.
- **x86_64 only, on Linux**: ARM machines have to build from source. Nothing in the code is architecture-specific; it is a matter of adding a second CI runner when there is demand for it.
- **Windows, not yet**: the four per-platform pieces are the whole of the work, and the rest of the application is already shared.

---

## Licence

The project code is released under the MIT licence, see [LICENSE](LICENSE).

Everything it depends on is permissively licensed and compatible with that choice: Tauri, serde, reqwest, tokio, image, rand, chrono and the `objc2` crates are MIT or Apache-2.0, and the `wallpaper` crate is Unlicense.

The Linux side needs a word of its own. The `gtk-rs` and `gstreamer-rs` bindings are MIT, but the libraries they bind are LGPL: GTK, GLib and GStreamer's own core. Linking against them dynamically, which is what happens here, is exactly what the LGPL is designed to allow, and the packages depend on the distribution's copies rather than shipping their own. The AppImage does carry copies, as an AppImage must, and the LGPL is satisfied there by the same mechanism: the libraries are separate files inside it, replaceable, and unmodified.

No GPL-licensed component is linked in on either platform. Video stills are decoded through the system's own framework, AVFoundation or GStreamer, rather than a bundled decoder, so no codec library is redistributed here either. That is also why a distribution's own patent policy, and not this project's licence, decides whether an H.264 video APOD can be decoded on it.

The images are a separate matter, and they are not mine to license:

- APOD entries **carrying a copyright notice** remain the property of their authors. The app displays that notice with the image, in the panel and in the menu bar, and never strips it.
- Entries **without** such a notice are usually NASA material, which is in the public domain in the United States. NASA's media guidelines still ask that its imagery not be used in a way that implies endorsement.

The application downloads these images for your own desktop. Redistributing them is your responsibility, not the application's.
