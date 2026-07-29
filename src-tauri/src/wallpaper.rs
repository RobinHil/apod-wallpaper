use std::path::Path;

/// Sets the image as the desktop wallpaper. The `wallpaper` crate is the only
/// backend used:
/// - Windows: `SystemParametersInfo` (system API);
/// - macOS: AppleScript applied to every desktop;
/// - Linux: GNOME and derivatives, KDE Plasma, XFCE, LXDE, MATE, Cinnamon,
///   Deepin, then a `swaybg` (Wayland) or `feh` (X11) fallback.
///
/// A desktop the crate cannot drive produces an explicit error, shown in the
/// settings panel.
///
/// # Light and dark themes
///
/// Windows, macOS and every supported Linux desktop have a single wallpaper
/// shared by both themes, so the call below is enough: the image shows up in
/// dark mode just as it does in light mode. GNOME 42 and later are the sole
/// exception, with a separate key for the dark theme, handled by
/// [`gnome::set_dark_uri`].
pub fn set_wallpaper(path: &Path) -> Result<(), String> {
    let path = path
        .to_str()
        .ok_or_else(|| "Invalid wallpaper path (UTF-8 expected).".to_string())?;

    ::wallpaper::set_from_path(path).map_err(|e| failure_message(&e.to_string()))?;

    // The image is already composed at the exact screen size, but we still ask
    // for "fill" wherever the crate supports it, as a safety net (macOS does
    // not expose it). Failure is not fatal.
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    let _ = ::wallpaper::set_mode(::wallpaper::Mode::Crop);

    #[cfg(target_os = "linux")]
    gnome::set_dark_uri(path);

    Ok(())
}

/// On Linux a failure almost always means an unrecognised desktop, so name the
/// one we detected along with the ones that do work.
#[cfg(target_os = "linux")]
fn failure_message(detail: &str) -> String {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .or_else(|_| std::env::var("DESKTOP_SESSION"))
        .unwrap_or_else(|_| "unknown".to_string());
    format!(
        "Could not set the wallpaper (desktop environment: {desktop}). \
         Supported environments: GNOME and derivatives (Unity, Budgie, Pantheon), \
         KDE Plasma, XFCE, LXDE, MATE, Cinnamon, Deepin, plus any compositor with \
         swaybg (Wayland) or feh (X11) installed. Details: {detail}"
    )
}

#[cfg(not(target_os = "linux"))]
fn failure_message(detail: &str) -> String {
    format!("Could not set the wallpaper: {detail}")
}

/// GNOME add-on. The `wallpaper` crate only writes `picture-uri`, the key the
/// light theme reads. Since GNOME 42 the dark theme reads a separate key,
/// `picture-uri-dark`: without it, a user in dark mode would never see the
/// image change.
///
/// The module only uses the standard library, so it compiles on every platform
/// -- `cargo check` and the tests cover it from any OS -- but it is only called
/// on Linux.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod gnome {
    use std::process::Command;

    /// Points `picture-uri-dark` at the image we just applied. Silent and
    /// non-fatal: before GNOME 42 the key does not exist and `gsettings` fails
    /// harmlessly, the image still being applied through `picture-uri`.
    pub fn set_dark_uri(path: &str) {
        let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") else {
            return;
        };
        if !uses_gnome_schema(&desktop) {
            return;
        }
        let _ = Command::new("gsettings")
            .args([
                "set",
                "org.gnome.desktop.background",
                "picture-uri-dark",
                &gvariant_string(&file_uri(path)),
            ])
            .status();
    }

    /// Builds a `file://` URI from a filesystem path.
    ///
    /// GLib parses the value as a URI, so anything outside the unreserved set
    /// has to be percent-encoded: a space (a user name with a space in it), a
    /// `#` (everything after it would be read as a fragment), a `?`, or any
    /// non-ASCII byte. Encoding is done byte by byte, which is what a URI
    /// wants and what keeps a non-UTF-8-looking path intact.
    fn file_uri(path: &str) -> String {
        // `/` stays literal: it is the path separator in the URI too.
        const KEPT: &[u8] = b"-._~/";
        let mut uri = String::from("file://");
        for byte in path.as_bytes() {
            if byte.is_ascii_alphanumeric() || KEPT.contains(byte) {
                uri.push(*byte as char);
            } else {
                uri.push_str(&format!("%{byte:02X}"));
            }
        }
        uri
    }

    /// Mirrors the condition in `wallpaper::linux::gnome::is_compliant`: only
    /// complete the desktops the crate actually drove through the
    /// `org.gnome.desktop.background` schema. `XDG_CURRENT_DESKTOP` holds
    /// values such as `ubuntu:GNOME` or `Budgie:GNOME`, hence the `contains`.
    fn uses_gnome_schema(desktop: &str) -> bool {
        desktop.contains("GNOME") || desktop == "Unity" || desktop == "Pantheon"
    }

    /// `gsettings` expects a GVariant value: a string is quoted, with
    /// backslashes and quotes escaped. Required for paths containing a space,
    /// such as a user name with a space in it.
    fn gvariant_string(value: &str) -> String {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    }

    #[cfg(test)]
    mod tests {
        use super::{file_uri, gvariant_string, uses_gnome_schema};

        #[test]
        fn detects_gnome_family_desktops() {
            for desktop in ["GNOME", "ubuntu:GNOME", "Budgie:GNOME", "Unity", "Pantheon"] {
                assert!(uses_gnome_schema(desktop), "{desktop}");
            }
            for desktop in ["KDE", "XFCE", "X-Cinnamon", "MATE", "sway", ""] {
                assert!(!uses_gnome_schema(desktop), "{desktop}");
            }
        }

        #[test]
        fn quotes_paths_for_gsettings() {
            assert_eq!(
                gvariant_string("file:///home/rh/wall-2026-07-28-blur.jpg"),
                "\"file:///home/rh/wall-2026-07-28-blur.jpg\""
            );
            assert_eq!(
                gvariant_string("file:///home/jean%20dupont/a\"b.jpg"),
                "\"file:///home/jean%20dupont/a\\\"b.jpg\""
            );
        }

        #[test]
        fn percent_encodes_everything_a_uri_cannot_carry() {
            assert_eq!(
                file_uri("/home/rh/.local/share/wall-2026-07-28-blur-3456x2234.jpg"),
                "file:///home/rh/.local/share/wall-2026-07-28-blur-3456x2234.jpg"
            );
            // A space, and a `#` that would otherwise start a fragment.
            assert_eq!(
                file_uri("/home/jean dupont/n#1.jpg"),
                "file:///home/jean%20dupont/n%231.jpg"
            );
            // Non-ASCII is encoded per UTF-8 byte, not per character.
            assert_eq!(file_uri("/home/josé/a.jpg"), "file:///home/jos%C3%A9/a.jpg");
        }
    }
}
