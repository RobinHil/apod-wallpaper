#!/usr/bin/env bash
#
# Builds the Linux packages: a .deb, an .rpm and an AppImage.
#
# This wrapper exists for the AppImage alone. The other two build anywhere, and
# `pnpm bundle` on its own is enough for them. But `linuxdeploy` and its two
# plugins assume a Debian-shaped host, and they fail one at a time, minutes
# apart, each with a message that names anything except the cause. Every
# variable set below answers one of those assumptions.
#
# The same variables are set by the Linux CI job, so there is one incantation
# to keep correct rather than one per distribution and one more in the README.
#
# Anything already set in the environment is left alone, and every argument is
# passed through to `pnpm bundle`:
#
#   ./packaging/linux/bundle.sh
#   ./packaging/linux/bundle.sh --bundles deb

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/../.."

missing=()

# Not distribution-specific: this is what pulls gst-plugins-bad into the
# AppImage, and with it the OpenH264 decoder a video APOD published as an H.264
# file needs. Without it the AppImage carries the media framework and not one
# codec, which nothing reveals until someone hits such an APOD.
#
# OpenH264 rather than libav, so the licence section of the README stays true.
export GSTREAMER_INCLUDE_BAD_PLUGINS="${GSTREAMER_INCLUDE_BAD_PLUGINS:-1}"

# `linuxdeploy` is itself an AppImage and mounts itself with libfuse2, which
# current distributions no longer ship. Told to unpack instead, it stops
# caring.
export APPIMAGE_EXTRACT_AND_RUN="${APPIMAGE_EXTRACT_AND_RUN:-1}"

# The `strip` linuxdeploy carries is too old to read the `.relr.dyn` sections a
# current toolchain emits, and it treats every failure as fatal. The release
# profile strips the binary already.
export NO_STRIP="${NO_STRIP:-1}"

# `patchelf` is shelled out to by the GStreamer plugin, on every distribution.
command -v patchelf > /dev/null || missing+=("patchelf")

# Arch and its derivatives, where linuxdeploy's Debian assumptions surface as
# two more missing pieces. `ID_LIKE` is what catches EndeavourOS and Manjaro.
if [ -r /etc/os-release ] && grep -qE '^(ID|ID_LIKE)=.*arch' /etc/os-release; then
  # The GStreamer plugin guesses a Debian multiarch path for
  # `gst-plugin-scanner` and has no fallback; Arch keeps it beside the plugins.
  export GSTREAMER_HELPERS_DIR="${GSTREAMER_HELPERS_DIR:-/usr/lib/gstreamer-1.0}"

  # Since gdk-pixbuf 2.44 the loaders are built into the library and this
  # directory is gone, but linuxdeploy's GTK plugin copies it without checking
  # whether it exists. Any package that installs a loader there recreates it;
  # webp-pixbuf-loader is the smallest, and pacman then owns the directory
  # rather than an orphan being left behind.
  [ -d /usr/lib/gdk-pixbuf-2.0/2.10.0 ] || missing+=("webp-pixbuf-loader")
fi

if [ ${#missing[@]} -gt 0 ]; then
  echo "Missing, and the AppImage step fails without them:" >&2
  printf '  %s\n' "${missing[@]}" >&2
  echo >&2
  echo "  sudo pacman -S --needed ${missing[*]}      # Arch" >&2
  echo "  sudo apt install ${missing[*]}             # Debian, Ubuntu" >&2
  echo >&2
  echo "The .deb and the .rpm are produced before the AppImage step, so" >&2
  echo "running \`pnpm bundle\` directly still leaves you with those two." >&2
  exit 1
fi

exec pnpm bundle "$@"
