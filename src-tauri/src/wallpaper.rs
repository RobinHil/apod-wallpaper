use std::path::Path;

/// Applique l'image en fond d'ecran. La crate `wallpaper` est la seule brique
/// utilisee :
/// - Windows : `SystemParametersInfo` (API systeme) ;
/// - macOS   : AppleScript applique a tous les bureaux ;
/// - Linux   : GNOME et derives, KDE Plasma, XFCE, LXDE, MATE, Cinnamon,
///   Deepin, puis repli `swaybg` (Wayland) ou `feh` (X11).
///
/// Un environnement que la crate ne sait pas piloter remonte une erreur
/// explicite, affichee dans le panneau.
///
/// # Themes clair / sombre
///
/// Windows, macOS et tous les bureaux Linux pris en charge n'ont qu'un seul
/// fond d'ecran, commun aux deux themes : l'appel ci-dessous suffit, l'image
/// s'affiche en mode sombre comme en mode clair. GNOME 42 et suivants font
/// exception avec une cle distincte pour le theme sombre, completee par
/// [`gnome::set_dark_uri`].
pub fn set_wallpaper(path: &Path) -> Result<(), String> {
    let path = path
        .to_str()
        .ok_or_else(|| "Chemin du fond d'écran invalide (UTF-8 attendu).".to_string())?;

    ::wallpaper::set_from_path(path).map_err(|e| failure_message(&e.to_string()))?;

    // L'image est deja composee a la taille exacte de l'ecran ; on force
    // malgre tout le mode « remplir » la ou la crate sait le faire (macOS ne
    // l'expose pas). Echec non bloquant.
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    let _ = ::wallpaper::set_mode(::wallpaper::Mode::Crop);

    #[cfg(target_os = "linux")]
    gnome::set_dark_uri(path);

    Ok(())
}

/// Sous Linux l'echec vient presque toujours d'un bureau non reconnu : on
/// nomme l'environnement detecte et la liste de ceux qui fonctionnent.
#[cfg(target_os = "linux")]
fn failure_message(detail: &str) -> String {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .or_else(|_| std::env::var("DESKTOP_SESSION"))
        .unwrap_or_else(|_| "inconnu".to_string());
    format!(
        "Impossible de définir le fond d'écran (environnement de bureau : {desktop}). \
         Environnements pris en charge : GNOME et dérivés (Unity, Budgie, Pantheon), \
         KDE Plasma, XFCE, LXDE, MATE, Cinnamon, Deepin, ainsi que les compositeurs \
         disposant de swaybg (Wayland) ou feh (X11). Détail : {detail}"
    )
}

#[cfg(not(target_os = "linux"))]
fn failure_message(detail: &str) -> String {
    format!("Impossible de définir le fond d'écran : {detail}")
}

/// Complement GNOME. La crate `wallpaper` ne renseigne que `picture-uri`, la
/// cle lue par le theme clair. Depuis GNOME 42 le theme sombre lit une cle
/// separee, `picture-uri-dark` : sans elle, un utilisateur en mode sombre ne
/// verrait jamais l'image changer.
///
/// Le module n'utilise que la bibliotheque standard et compile donc sur toutes
/// les plateformes — `cargo check` et les tests le verifient depuis n'importe
/// quel OS — mais il n'est appele que sous Linux.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod gnome {
    use std::process::Command;

    /// Aligne `picture-uri-dark` sur l'image appliquee. Silencieux et non
    /// bloquant : avant GNOME 42 la cle n'existe pas et `gsettings` echoue
    /// sans consequence, l'image restant appliquee via `picture-uri`.
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
                &gvariant_string(&format!("file://{path}")),
            ])
            .status();
    }

    /// Reprend la condition de `wallpaper::linux::gnome::is_compliant` : on ne
    /// complete que les environnements que la crate a effectivement pilotes
    /// via le schema `org.gnome.desktop.background`. `XDG_CURRENT_DESKTOP`
    /// vaut par exemple `ubuntu:GNOME` ou `Budgie:GNOME`, d'ou le `contains`.
    fn uses_gnome_schema(desktop: &str) -> bool {
        desktop.contains("GNOME") || desktop == "Unity" || desktop == "Pantheon"
    }

    /// `gsettings` attend une valeur GVariant : une chaine se donne entre
    /// guillemets, avec antislash et guillemet echappes. Indispensable pour un
    /// chemin contenant une espace (nom d'utilisateur avec espace).
    fn gvariant_string(value: &str) -> String {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    }

    #[cfg(test)]
    mod tests {
        use super::{gvariant_string, uses_gnome_schema};

        #[test]
        fn detect_gnome_family_desktops() {
            for desktop in ["GNOME", "ubuntu:GNOME", "Budgie:GNOME", "Unity", "Pantheon"] {
                assert!(uses_gnome_schema(desktop), "{desktop}");
            }
            for desktop in ["KDE", "XFCE", "X-Cinnamon", "MATE", "sway", ""] {
                assert!(!uses_gnome_schema(desktop), "{desktop}");
            }
        }

        #[test]
        fn quote_paths_for_gsettings() {
            assert_eq!(
                gvariant_string("file:///home/rh/wall-2026-07-28-blur.jpg"),
                "\"file:///home/rh/wall-2026-07-28-blur.jpg\""
            );
            assert_eq!(
                gvariant_string("file:///home/jean dupont/a\"b.jpg"),
                "\"file:///home/jean dupont/a\\\"b.jpg\""
            );
        }
    }
}
