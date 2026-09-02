# APOD Wallpaper

A macOS application that downloads NASA's Astronomy Picture of the Day (APOD)
and sets it as the desktop picture, once a day. It runs in the background, has
no window of its own most of the time, and is opened from its menu bar icon or
by launching it again.

Requires **macOS 13.3 or later**, on Apple silicon or Intel.

macOS is the only platform supported today. Windows and Linux are the goal:
Tauri already builds for both, and the parts that are genuinely tied to macOS
are few, essentially setting the desktop picture, watching for screen changes
and wake-ups, and decoding a video still. Everything else, the scheduling, the
API client, the image composition and the panel, is portable as it stands.

Built with [Tauri 2](https://tauri.app): a Rust backend and a settings panel
written in React and TypeScript, styled with Tailwind CSS.

> [!NOTE]
> This project was written with heavy AI assistance, and is stated up front so
> you can read the code knowing where it came from. The design decisions, the
> review of every change and the testing on real hardware are mine; a large part
> of the code itself was generated. Judge it on what it does, and on the reasons
> given in [Design notes](#design-notes), rather than on who typed it.

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

- **Picture of the day**: the image the APOD API reports as the most recently
  published one. No date is sent with the request, so the API decides what
  "today" is and the local time zone cannot skew the result.
- **Random**: a date drawn at random from the whole archive, which starts on
  16 June 1995. Clicking "Random" again draws another one immediately.
- **Specific date**: the APOD published on a date you pick, between 16 June
  1995 and today. A few days in the archive have no publication; those are
  reported and the current wallpaper is kept.

Some APOD entries are videos, and a still is used for those. YouTube and Vimeo
entries use the thumbnail the API publishes, the maximum-resolution one when
YouTube has it. Entries published as a plain video file have no thumbnail, so a
frame is decoded out of the file itself. The panel says which of the two is on
the desktop and offers a link to watch the video.

### When the wallpaper changes

The wallpaper is updated when the app starts and at the local day change, and
the process does nothing at all in between. There is one background task; it
attempts an update, then sleeps until just after the next midnight.

Three things end that sleep early:

- **A screen change** : a new resolution, a scale change, a display plugged
  in or unplugged. The wallpaper is recomposed for the new size from the
  original already on disk, without touching the network.
- **Waking from sleep** : the pending wait was measured against a clock that
  stops while the Mac is asleep, so the task is told about the wake-up and
  re-reads the wall clock.
- **A failure** : no network, an exhausted API quota, an API outage. The app
  retries with an exponential backoff (10 s, 20 s, 40 s, ...) with ±20 %
  jitter, capped at 15 minutes, until it succeeds. The wallpaper in place is
  never disturbed by a failure.

When today's picture has not been published yet, the most recent one is applied
and the app looks again every 30 minutes rather than hammering the API.

"Refresh now", in the panel, applies an image immediately, including
re-applying the current one, which is what restores it if you have set another
desktop picture by hand since.

### How the image is fitted to the screen

The image is composed at the exact pixel size of the main display, in one of
two modes:

- **Blurred fill** (default): the whole image, undistorted, centred over a
  blurred and darkened copy of itself that fills the screen. Nothing is cropped
  and nothing is stretched.
- **Crop**: the image is cropped to the screen's aspect ratio and fills it.

No text is ever burned into the image. The date and the copyright are shown in
the panel and in the menu bar menu.

### The panel and the menu bar

Everything is in the **settings panel**: the current image and its credits, the
three modes, the fit mode, the manual refresh, the NASA API key, and the quit
button. Closing it leaves the app running in the background.

The **menu bar item** shows the current image's title, date and copyright, and
opens the panel or quits. Everything it offers exists in the panel as well.

The app has no Dock icon, it is a background utility, declared as such
through `LSUIElement`. Starting it puts nothing on screen: it goes straight to
the menu bar and sets to work. The panel is only ever opened deliberately,
either from the menu bar item or by launching the application again from the
Finder, Spotlight or Launchpad. That second launch does not start a copy; it
brings up the panel of the instance already running.

The one exception is the very first launch, which opens the panel once so the
app is not invisible on a machine that has never run it. Every later start,
including at login, is silent.

---

## Command-line options

```
apod-wallpaper [OPTIONS]

  -h, --help     Show the usage message
  -V, --version  Show the version
```

Neither option starts the application; both print and exit. There is nothing
here that changes how it runs, because there is only one way it runs: quietly.

To run the binary inside the bundle directly:

```bash
"/Applications/APOD Wallpaper.app/Contents/MacOS/apod-wallpaper" --help
```

---

## Where files are stored

Everything lives in `~/Library/Application Support/com.rh.apod-wallpaper/`:

```
settings.json                            API key, mode, chosen date, fit mode
state.json                               the wallpaper currently applied
current/<date>.<ext>                     the downloaded original
current/wall-<date>-<fit>-<w>x<h>.jpg    the composition set as the wallpaper
```

Only one image is kept, the one on your desktop. Everything else is deleted
as soon as a new wallpaper has been applied, so the directory stays under a few
megabytes.

---

## Building from source

Three things are needed: the Xcode Command Line Tools,
[Rust](https://www.rust-lang.org/tools/install) (stable, through rustup) and
[Node.js](https://nodejs.org) 22 or newer with npm (CI builds on 24, the
active LTS).

```bash
xcode-select --install
npm install
npm run tauri dev      # development, with a live-reloading panel
npm run bundle         # release bundle
```

`npm run bundle` produces
`src-tauri/target/release/bundle/macos/APOD Wallpaper.app` and a `.dmg` next
to it, built for the machine it runs on.

Releases are one universal bundle instead, which is what CI builds. Both
targets have to be installed for it:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run bundle, --target universal-apple-darwin
```

Naming a target moves the output under
`src-tauri/target/<triple>/release/bundle/`, so
`universal-apple-darwin/release/bundle/` here.

Before opening a pull request, the same checks CI runs:

```bash
cd src-tauri
cargo fmt --all --check
cargo clippy --locked, -D warnings                 # lints, as the app is shipped
cargo clippy --locked --all-targets, -D warnings   # lints, tests included
cargo test --locked
cd ..
npx tsc --noEmit
```

Clippy twice is not a typo: `--all-targets` builds the tests, whose
dev-dependencies can supply a feature the library needs but never declares.
That builds, and the release build, which has no dev-dependencies, then
fails. The first command is the one that matches what users install.

---

## Installing

Release builds are downloadable from the
[releases page](https://github.com/RobinHil/apod-wallpaper/releases). There is
one `.dmg` and it is universal: the same file runs on Apple silicon and on
Intel.

Open the `.dmg` and drag the application into `Applications`. The builds are
**not signed**, an Apple code-signing certificate is a paid subscription --
so the first launch is refused with a message about the app being damaged. That
is Gatekeeper's quarantine flag, not a corrupted download. Clear it once:

```bash
xattr -d com.apple.quarantine "/Applications/APOD Wallpaper.app"
```

Alternatively, right-click the app, choose *Open* and confirm, or allow it from
*System Settings > Privacy & Security* right after the failed launch.

The first time the app sets the wallpaper, macOS asks for permission to control
**System Events**: that Apple event is how the desktop picture is set. If you
refuse, re-enable it under *System Settings > Privacy & Security >
Automation*.

Launching it opens the panel once, the only time it does, applies the
first wallpaper and leaves the app running in the background.

---

## Starting at login

The application does not register itself: a login item is a change to your
session, and you make it yourself, once.

Open *System Settings > General > Login Items & Extensions*, and under **Open
at Login** add `/Applications/APOD Wallpaper.app`. That is all there is to it.

Nothing appears when you log in. Earlier versions needed a launch agent here,
purely so the app could be passed a flag telling it not to open its panel; the
app no longer opens it on its own, so the flag and the agent are both gone. If
you still have `~/Library/LaunchAgents/com.rh.apod-wallpaper.plist` from one of
those versions, remove it:

```bash
launchctl unload ~/Library/LaunchAgents/com.rh.apod-wallpaper.plist
rm -f ~/Library/LaunchAgents/com.rh.apod-wallpaper.plist
```

---

## Uninstalling

Quit the application first, from the panel or the menu bar.

Remove it from *System Settings > General > Login Items & Extensions* if you
added it there, then:

```bash
rm -rf "/Applications/APOD Wallpaper.app"
```

Finally the local data, which nothing else touches:

```bash
rm -rf ~/Library/Application\ Support/com.rh.apod-wallpaper
```

Your desktop is still pointing at the last image the app applied, which lived
in that directory. **Choose another desktop picture before deleting it**, or
the background goes blank at the next login.

---

## NASA API key

By default the app uses `DEMO_KEY`, which NASA limits to **30 requests per hour
and 50 per day, per IP address**. The app makes at most a handful of requests a
day, so that is usually plenty; a personal key is worth having if you share an
IP address with other users of the API.

1. Request one at <https://api.nasa.gov/>, a short form, the key arrives by
   email.
2. Open the panel, paste it into the "NASA API key" field, click "Save".

Saving a key immediately retries the update, which is normally why you are
typing one in. The key is stored in `settings.json` and is only ever sent to
the NASA API; error messages have the request URL stripped out of them so it
cannot leak into the panel.

---

## Project layout

```
apod-wallpaper/
|- src-tauri/                    # Rust backend
|  |- src/
|  |  |- main.rs                 # Binary entry point
|  |  |- lib.rs                  # Tauri setup: menu bar, panel window, commands
|  |  |- scheduler.rs            # The only background task: when to update
|  |  |- os_events.rs            # Screen-change and wake-from-sleep notifications
|  |  |- updater.rs              # What an update does, end to end
|  |  |- nasa_api.rs             # APOD API calls, parsing, error taxonomy
|  |  |- store.rs                # state.json + the two image files, atomic writes
|  |  |- image_compose.rs        # Ratio handling: blurred fill or crop
|  |  |- video_frame.rs          # A still frame out of a video, via AVFoundation
|  |  |- wallpaper.rs            # Setting the desktop picture
|  |  `- settings.rs             # API key, mode, fit; JSON persistence
|  |- Info.plist                 # Menu bar app (LSUIElement) + Apple events usage
|  |- capabilities/default.json
|  `- tauri.conf.json
|- src/                          # Panel frontend (React + TypeScript)
|  |- main.tsx                   # Mounts the panel into index.html
|  |- App.tsx                    # Composes the cards
|  |- useAppState.ts             # Backend state, commands, pushed events
|  |- useSyncedField.ts          # Text fields the backend also owns
|  |- types.ts                   # UiState: the contract with the Rust side
|  |- links.ts                   # APOD page and video URLs
|  |- classes.ts                 # Utility strings more than one card needs
|  |- components/                # One file per card, plus the SVG icons
|  `- styles.css                 # Tailwind entry point: the palette, light and dark
|- index.html                    # Mount point for the panel
`- .github/workflows/ci.yml      # Lint/test gate, universal bundle, release
```

---

## Design notes

### Scheduling: once a day, and otherwise asleep

The application is meant to be invisible in Activity Monitor. There is exactly
one background task, and it does this:

1. Attempt an update. There is no separate "am I online?" probe, the fetch is
   the probe, which is one round trip instead of two.
2. On success, sleep until the next local day change.
3. On failure, or when today's APOD is not published yet, retry on a backoff
   until it succeeds, then go back to step 2.

Nothing else is armed: no polling of the API, no periodic reapplication, no
timer that exists only to ask whether anything has happened. When the wallpaper
is up to date the process is sleeping on a single timer, for the nine or so
hours to the next midnight, not in instalments.

A sleep that long has to survive the Mac being suspended, and the timer counts
against a clock that stops while it is. macOS posts `NSWorkspaceDidWake` on
resume, so the task is woken, re-reads the wall clock and decides again --
which costs one observer and no polling.

Screen changes arrive the same way, as a notification rather than as something
looked for: `NSApplicationDidChangeScreenParameters` covers a resolution
change, a scale change, and a display plugged in or unplugged. Both
notifications come from AppKit, which Tauri's macOS backend already links, so
neither adds a crate to the build.

The same reasoning applies to retries. Subscribing to `NWPathMonitor` would be
another framework and another resident observer, to learn something a
connection attempt reports locally in about a millisecond when there is no
network. A capped backoff is cheaper than the machinery to avoid it.

### Nothing is redone that does not need to be

`state.json` records the applied image and the inputs its composition depended
on, fit mode and screen size. At startup, if that record already answers the
current settings and both files are on disk, the app does nothing at all: no
API call, no download, no wallpaper-set call. Restarting five times in a day
costs five `state.json` reads.

Changing the fit mode, or moving to a display with a different resolution,
recomposes from the stored original without touching the network.

When macOS reports no main display at all, lid closed, no external screen --
the size the wallpaper was last composed for is reused. That is not a
resolution change, and recomposing for a guessed size would replace a correct
wallpaper with a wrong one.

### Failures never break the desktop

The download is validated by decoding it in memory, composed into a wallpaper,
and only then moved into place with atomic renames. The previous image stays on
disk and on the desktop until the new one has actually been applied, so a
partial download, a full disk or a crash mid-update cannot leave a black or
broken background. `state.json` and `settings.json` are written the same way.

### Setting the desktop picture

macOS exposes no public API for the desktop picture that works across every
Space at once; the supported route is an Apple event to **System Events**,
which is the AppleScript the `wallpaper` crate wraps. Two consequences the app
has to live with:

- `Info.plist` carries `NSAppleEventsUsageDescription`. Without it the system
  refuses the event outright rather than prompting, and the first wallpaper
  would fail with nothing the user could act on.
- The call is thoroughly blocking, seconds of it when the desktop is busy, so
  it runs on tokio's blocking pool, never on a runtime worker, where it would
  hold up every panel command queued behind it.

### The menu bar item is optional

The panel carries every setting, the refresh and the quit button; launching the
app again opens that panel; and a menu bar item that fails to build is logged
and stepped over rather than being a startup error. Nothing the application
does depends on it existing.

### Starting is not the same as being asked to appear

Starting the app puts nothing on screen. It is started at login, and a login
that throws a window at you is precisely what a background utility must not do.
Opening the panel is therefore always a deliberate act, and there are two of
them, which reach the app by two different routes.

Clicking the menu bar item is the direct one. Launching the application again
is the indirect one, and it splits in two: for the installed `.app`, macOS does
not start a second process at all, LaunchServices reactivates the one already
running and sends it `applicationShouldHandleReopen:`, surfacing in Tauri as
`RunEvent::Reopen`. Running the binary inside the bundle directly *does* start
a second process, and there the single-instance plugin hands the launch to the
running instance and exits. Both end in the same place.

The one automatic opening left is the very first launch, decided by the absence
of `settings.json`. On a machine that has never run the app there is no menu bar
icon the user has learnt to look for and no wallpaper to notice, so a completely
silent first start would be indistinguishable from one that failed.

### Video APODs

A video is not a wallpaper, so a still is taken from it. Which still depends on
how the video was published, and APOD does it two ways.

Most video entries are **YouTube or Vimeo embeds**. The API has thumbnails for
those, and asked for them (`thumbs=true`) it returns one. YouTube stores that
one picture at several sizes and does not generate the big ones for every
video, so they are tried biggest first, `maxresdefault` (1280x720), then
`sddefault` (640x480), with the thumbnail as published as the last resort.
It is the same picture at each step; starting from more pixels only means less
upscaling on the way to a screen-sized wallpaper.

A thumbnail is what the uploader chose, which is not always a frame of the
video: it is sometimes a cover with a title burned into it, and that is what
lands on the desktop. Nothing here can tell the two apart, and it is left that
way, video APODs are a handful of days a year, and telling a cover from a
frame would take OCR.

The rest are served as a **plain file** on apod.nasa.gov, an `.mp4`. The API
has no thumbnail for those and returns an empty string in its place, so the file
is downloaded and a frame is decoded out of it. The decoding is done by
AVFoundation, which is part of macOS: the app links against it the same way it
already does against AppKit to set the desktop picture, so this adds nothing for
anyone to install, and the formats that work are the ones the system can play.

The frame is not the first one, videos open on black, on a fade-in, or on a
title card. Four instants spread through the video are tried in turn, and the
first one with enough contrast to be a picture rather than a flat colour is
kept; if all four are flat, the least flat of them is. What gets archived is
that frame as a JPEG, not the video: the stored original is what a later
fit-mode or resolution change recomposes from, and keeping tens of megabytes to
decode again each time would buy nothing.

Either way the panel flags the entry and links to the video, to YouTube or
Vimeo for an embed, to the APOD page for a file, which is where a raw `.mp4` is
meant to be watched, and the menu bar appends "(video)" to the title. When
nothing at all can be made of an entry, the current wallpaper is kept in daily
mode, and another date is drawn in random mode.

### Daily mode sends no date

The app asks the API for "the most recently published image" rather than for
the local date, which removes the time-zone skew (APOD is published on US
Eastern time). Just after local midnight the API still serves yesterday's
picture; that counts as *not yet satisfied*, so the app applies it if it is new
and keeps looking until today's appears. Pinning the request to the local date
would skip today's picture entirely on some days.

### The panel holds no state of its own

The backend is the single source of truth. Every panel command returns a whole
`UiState`, and the backend pushes one on `state-updated` whenever it changes
something by itself, the daily update, a screen change, a wake from sleep.
The panel renders what it is given and never computes a setting locally, so
the two can never disagree about what is applied.

Two exceptions, both deliberate and both local to a component: the date picker
stays visible from the moment "Specific date" is clicked, before any mode has
actually changed, and the two text fields hold what is being typed. Those
fields are re-seeded from every push, except while they have the focus --
otherwise a background update landing mid-sentence would wipe out a
half-entered API key.

While a command is in flight the whole UI is covered by an overlay and pushed
updates are ignored, so nothing moves under the pointer between the click and
the result.

### One palette, two appearances

The colours are Tailwind theme tokens: `--color-card` in the `@theme` block of
`styles.css` is what makes `bg-card`, `border-card` and `text-card` exist. Dark
mode redefines those same variables under `prefers-color-scheme`, which is why
no component carries a `dark:` variant, the utilities already point at the
variable, and the variable changes underneath them.

Two of Tailwind's preflight rules are handed back to WebKit in the same file.
It tints the placeholder from the input's own colour, and it strips the padding
out of the date field, which leaves that control shorter than the text input
beside it. Both are reverted rather than worked around, so the two form rows
line up.

Utility strings that more than one card needs live in `classes.ts` as
constants. They are never merged: two utilities setting the same property are
resolved by their order in the generated stylesheet and not by their order in
the attribute, so a variant such as the selected segment spells out its own
colours instead of layering them over a base.

### One bundle for both architectures

A release is a single universal `.dmg` rather than one per architecture. It is
twice the size, 10 MB instead of 5, which for something downloaded once is
a better trade than asking every user which of two files they need.

Keeping the Intel slice is not sentiment. Rosetta translates x86_64 to ARM
and never the reverse, so an Apple-silicon-only build runs on no Intel Mac at
all, and Intel hardware runs every version of macOS this app supports. CI
checks that both slices are present and that both are compiled for the
advertised minimum.

macOS 26 is the last release supporting Intel hardware, and those machines go
on receiving security updates for about three years after it. That, rather
than the arrival of newer Macs, is when the slice stops earning its place.

### Other

- **The stylesheet sets the supported macOS, not the backend**: Tailwind
  emits cascade layers and `@property`, which arrived in the WebKit that
  shipped with macOS 13.3. Below that the panel does not degrade, it comes up
  unstyled, so 13.3 is what `minimumSystemVersion` records. `build.target` in
  `vite.config.ts` names the same Safari, so the bundled JavaScript never
  outruns the browser the stylesheet already requires. The three move together
  or not at all.
- **Cheap gaussian blur**: the backdrop is blurred on a 1/8 scale copy and
  scaled back up. The result is indistinguishable from a heavy blur on the
  full-size image, for a fraction of the CPU.
- **Varying file name**: the composition carries the date, fit mode and screen
  size in its name, because macOS caches the desktop picture by path and
  ignores a file rewritten in place.
- **Errors are never silent**: panel commands wait for the Rust side to finish
  and return any error to the frontend, which blocks the UI meanwhile and shows
  the message in a banner. The background task records its failures in the
  status line the panel displays.
- **Copyright**: the API's `copyright` field is preserved and shown in the
  panel and the menu bar. When it is present the image is **not** public
  domain: it belongs to its author, and using it is limited to a personal
  wallpaper. Images without one are NASA's and are public domain.

---

## Troubleshooting

**The application starts but there is no icon in the Dock.** Expected: it is a
background utility with no Dock icon. Use the menu bar item, or launch the
application again from Spotlight or the Finder to bring the panel back up.

**The wallpaper does not change.** Open the panel: every failure is shown
there, in a banner or in the status line at the bottom. If nothing is reported
and the desktop still does not change, the Apple event is most likely being
denied, check *System Settings > Privacy & Security > Automation* and make
sure **APOD Wallpaper** is allowed to control **System Events**. To see what
the desktop is currently pointing at:

```bash
osascript -e 'tell application "System Events" to get picture of current desktop'
```

It should be a file in `~/Library/Application Support/com.rh.apod-wallpaper/current/`.

**"APOD Wallpaper is damaged and can't be opened."** The quarantine flag on an
unsigned download. See [Installing](#installing).

**Started at login and nothing appeared.** That is the intended behaviour --
there is no window at login, by design. Check it is actually running, and look
for its icon in the menu bar:

```bash
pgrep -a apod-wallpaper
```

If it is not running, confirm the entry under *System Settings > General >
Login Items & Extensions* points at `/Applications/APOD Wallpaper.app` and is
switched on.

---

## Known limitations

- **Multiple displays**: the image is composed at the main display's
  resolution, and macOS applies it to every desktop and every Space. On a
  second screen of a different size it is scaled to fit. Plugging, unplugging
  or resizing a display is noticed and recomposed for.
- **The Apple event permission** is asked for once and has to be granted. A
  denied automation permission is silent from the app's side: the event fails,
  the error reaches the panel, but nothing can re-prompt for it, it has to be
  re-enabled in System Settings.
- **Unsigned builds**: the Gatekeeper step in the install section is needed on
  every download. Signing properly requires a paid Apple Developer certificate.
- **macOS only, for now**: three pieces are written against Apple frameworks,
  namely setting the desktop picture, watching for screen and wake events, and
  pulling a still out of a video. Porting to Windows and Linux means swapping
  those three, not rewriting the app.

---

## Licence

The project code is released under the MIT licence, see [LICENSE](LICENSE).

Everything it depends on is permissively licensed and compatible with that
choice: Tauri, serde, reqwest, tokio, image, rand, chrono and the `objc2`
crates are MIT or Apache-2.0, and the `wallpaper` crate is Unlicense. No
GPL-licensed component is linked in. Video stills are decoded through Apple's
AVFoundation rather than a bundled decoder, so no codec library is
redistributed here either.

The images are a separate matter, and they are not mine to license:

- APOD entries **carrying a copyright notice** remain the property of their
  authors. The app displays that notice with the image, in the panel and in the
  menu bar, and never strips it.
- Entries **without** such a notice are usually NASA material, which is in the
  public domain in the United States. NASA's media guidelines still ask that its
  imagery not be used in a way that implies endorsement.

The application downloads these images for your own desktop. Redistributing
them is your responsibility, not the application's.
