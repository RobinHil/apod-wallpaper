/**
 * The English copy, and the shape every other language has to match: `Copy`
 * is derived from this object, so a section added here fails to compile in
 * `fr.ts` until it is translated there too.
 */
export const en = {
  htmlLang: "en",
  documentTitle: "APOD Wallpaper - the universe, once a day, on your desktop",
  documentDescription:
    "A background utility that sets NASA's Astronomy Picture of the Day as your desktop wallpaper, once a day. Native, quiet, open source. macOS and GNOME.",

  nav: {
    sections: [
      { href: "#craft", label: "Why it is different" },
      { href: "#how", label: "How it works" },
      { href: "#modes", label: "Modes" },
      { href: "#download", label: "Download" },
    ],
    source: "Source on GitHub",
    download: "Download",
    language: "Language",
  },

  hero: {
    eyebrow: "Free and open source, for macOS and GNOME",
    titleLead: "Every morning,",
    titleMiddle: "the universe",
    titleAccent: "redecorates.",
    lede: "APOD Wallpaper puts NASA's Astronomy Picture of the Day on your desktop, composed to the exact pixels of your screen. Then it gets out of the way: no window, no Dock icon, one background task asleep until midnight.",
    download: "Download it free",
    source: "Read every line of it",
    proof: [
      "13.1 MB of memory, idle",
      "One background task",
      "No account, no telemetry",
      "30 years of archive",
    ],
    liveAbove: (title: string, date: string) => `Live above: ${title}, ${date}`,
    waitingForNasa: "Today's picture loads here, straight from NASA",
    altBlurFill: "The Astronomy Picture of the Day, fitted to a desktop",
    altCrop: "The Astronomy Picture of the Day, cropped to a desktop",
  },

  features: {
    eyebrow: "Why it is different",
    heading: "A wallpaper app you will forget is installed.",
    lede: "Most of them ask for a login, keep a folder of a hundred pictures and leave a helper running all day. This one holds a single image, speaks to one API, and spends the other twenty-three hours and fifty-nine minutes asleep.",
    items: [
      {
        title: "Quiet by design",
        body: "No window most of the time, nothing in the Dock or the dash, one background task. It wakes at the day change, does its work, and goes back to sleep. Between two midnights the process does nothing at all.",
      },
      {
        title: "Composed to your pixels",
        body: "The picture is composed at the exact resolution of your main display. Plug a monitor in, change the scale, close the lid: it recomposes from the original already on disk, without touching the network.",
      },
      {
        title: "Your desktop is never broken",
        body: "No network, an exhausted quota, an outage: the wallpaper in place stays where it is. Nothing is written until the new image has been decoded, and retries back off instead of hammering.",
      },
      {
        title: "One image on disk",
        body: "Exactly one picture is kept, the one you are looking at, plus the original it was composed from. Everything else is pruned the moment a new wallpaper lands. The folder stays a few megabytes.",
      },
      {
        title: "Videos become stills",
        body: "Some entries are films. A frame is decoded out of the file, or the published thumbnail is used, and the panel tells you which and offers a link to watch the thing properly.",
      },
      {
        title: "Native, not a browser in a coat",
        body: "Rust, with a Tauri panel you open once a month. It borrows the frameworks your system already ships, so it carries no decoder, no runtime and no service of its own.",
      },
    ],
  },

  how: {
    eyebrow: "How it works",
    heading: "Four things a day, then silence.",
    lede: "The whole application is one background task and a handful of decisions. Here is every one of them, in order.",
    steps: [
      {
        title: "It asks NASA what today is",
        body: "In daily mode no date is sent with the request, so the API decides what today means and no time zone can skew the answer. If tomorrow's picture is not published yet, yesterday's stays up and the app looks again in half an hour.",
      },
      {
        title: "It composes, it never stretches",
        body: "The whole picture, undistorted, centred over a blurred and darkened copy of itself that fills the screen. Or cropped to your aspect ratio, if that is what you prefer. Nothing is squashed and no text is ever burned into the image.",
      },
      {
        title: "It hands the file to the desktop",
        body: "Through the desktop's own mechanism: an Apple event on macOS, GSettings on GNOME. The file is written under a temporary name and renamed into place, so a wallpaper half written to disk is not a state that exists.",
      },
      {
        title: "It sleeps until midnight",
        body: "One task, one wait, no timer and no polling. Nothing runs, nothing listens for the network, nothing checks a server every five minutes.",
      },
    ],
    wakesLabel: "Four things end the sleep early:",
    wakes: [
      "A screen change",
      "Waking from sleep",
      "A failure to retry",
      "Your click on Refresh now",
    ],
  },

  modes: {
    eyebrow: "What lands on your screen",
    heading: "Today's sky, a random one, or the one over your birthday.",
    lede: "Thirty years of pictures, one per day, each with the astronomer's own words underneath. Three ways to choose which one you wake up to.",
    items: [
      {
        title: "Picture of the day",
        body: "Whatever NASA published this morning. The default, and the reason the app exists.",
      },
      {
        title: "Random",
        body: "A date drawn from the entire archive, which starts on 16 June 1995. Press it again for another.",
      },
      {
        title: "A date of your own",
        body: "The picture published the day you were born, the day you met them, the day you moved. Any day since 1995.",
      },
    ],
    fitHeading: "Two ways to fit a rectangle of sky to a rectangle of screen.",
    fitLabel: "Fit mode",
    fits: {
      blur_fill: {
        label: "Blurred fill",
        note: "The whole picture, undistorted, over a blurred copy of itself. Nothing is cropped and nothing is stretched.",
      },
      crop: {
        label: "Crop",
        note: "Cropped to your aspect ratio and filling the screen edge to edge, when you would rather lose a border than see one.",
      },
    },
  },

  download: {
    eyebrow: "Download",
    heading: "Install it once. Notice it every morning.",
    ledeBefore:
      "No account, no subscription, no telemetry, nothing to configure. It uses NASA's public API with a shared demo key out of the box, and you can paste a ",
    ledeLink: "free key of your own",
    ledeAfter: " into the panel if you would rather have your own quota.",
    platforms: {
      macos: {
        name: "macOS",
        requires: "13.3 or later, Apple silicon or Intel",
        formats: ["One universal .dmg"],
        note: "The build is unsigned, so clear the quarantine flag once after installing:",
      },
      linux: {
        name: "Linux, GNOME",
        requires: "GNOME 42 or later, X11 or Wayland, x86_64",
        formats: [".deb", ".rpm", ".AppImage", "PKGBUILD for Arch"],
        note: "Stock GNOME shows no tray icons. The app is built not to need one, and launching it again from the overview opens the panel:",
      },
    },
    copyCommand: "Copy the command",
    copied: "Copied",
    copySelected: "Could not copy: the command is selected, copy it by hand",
    copyFailed: "Could not copy: select the command instead",
    ctaTitle: "Windows is next. Four pieces of code stand between it and your desktop.",
    ctaBody:
      "Setting the wallpaper, hearing about screen changes, measuring the screen, decoding a frame. Everything else is already written and shared. The repository says so in detail, and pull requests are welcome.",
    ctaPrimary: "Get the latest release",
    ctaSecondary: "Star it on GitHub",
    ctaNotes: "Or read the design notes first, they are unusually long",
  },

  faq: {
    eyebrow: "Questions",
    heading: "The honest answers.",
    items: [
      {
        q: "Does it need an account or an API key?",
        a: "Neither. It talks to NASA's public API with the shared demo key, which allows thirty requests an hour per address, and the app makes a handful a day. If you share an address with other users of that API, a free personal key takes a minute to request and is pasted straight into the panel.",
      },
      {
        q: "What does it do to my battery and my memory?",
        a: "Between two updates, nothing measurable: one task waiting on a clock. A start with nothing to do sits at 13.1 MB of private memory, and after an update the allocator is asked to hand the picture back to the system rather than hold it through the next twelve hours of sleep.",
      },
      {
        q: "Why GNOME and not every Linux desktop?",
        a: "Because the wallpaper is set through GNOME's own settings, and the two system events come from Mutter and logind. On KDE or Xfce the app starts and then tells you it cannot set the wallpaper, rather than failing silently. Supporting another desktop means writing those same pieces for it, and the code is arranged so that it is a small job.",
      },
      {
        q: "What happens on a second monitor?",
        a: "The image is composed at the main display's resolution and the desktop applies it everywhere. Plug a screen in, unplug one, change the scale, and the app recomposes from the original it already has, without a network request.",
      },
      {
        q: "Who owns the pictures?",
        a: "Not this app. Entries carrying a copyright notice belong to their authors, and that notice is shown with the image and never stripped. The rest is usually NASA material, public domain in the United States. The app downloads them for your own desktop; what you do with them afterwards is yours to answer for.",
      },
      {
        q: "What language is the app itself in?",
        a: "English, panel and tray alike, on both platforms. This page speaks your browser's language; the application does not, yet.",
      },
    ],
  },

  footer: {
    tagline:
      "A picture of the universe, chosen by an astronomer, waiting for you when you sit down.",
    source: "Source",
    licence: "MIT licence",
    apod: "NASA APOD",
    legal:
      "Not affiliated with NASA. Astronomy Picture of the Day is a service of NASA and Michigan Technological University. Images carrying a copyright notice remain the property of their authors; the application shows that notice and never removes it. The code is MIT licensed and was written with heavy AI assistance, which the repository states up front.",
  },
};

/** The shape a language has to provide, taken from the English one. */
export type Copy = typeof en;
