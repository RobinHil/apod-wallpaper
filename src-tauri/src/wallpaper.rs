use std::path::Path;

/// Definit l'image comme fond d'ecran du systeme :
/// - Windows : SystemParametersInfo (crate `wallpaper`)
/// - macOS   : script AppleScript applique a tous les bureaux (crate `wallpaper`)
/// - Linux   : Hyprland est gere en direct (swww puis hyprpaper) ; les autres
///   environnements (GNOME, KDE Plasma, XFCE, MATE, Cinnamon, sway...) passent
///   par la crate `wallpaper`, avec une erreur explicite sinon.
pub fn set_wallpaper(path: &Path) -> Result<(), String> {
    let path_str = path
        .to_str()
        .ok_or_else(|| "Chemin du fond d'écran invalide (UTF-8 attendu).".to_string())?;

    #[cfg(target_os = "linux")]
    if hyprland::is_active() {
        return hyprland::set(path_str);
    }

    ::wallpaper::set_from_path(path_str).map_err(|e| describe_error(&e.to_string()))?;

    // L'image est deja composee a la taille exacte de l'ecran ; on force
    // malgre tout le mode "remplir" la ou c'est possible, par securite.
    // Son echec n'est pas bloquant.
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        let _ = ::wallpaper::set_mode(::wallpaper::Mode::Crop);
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn describe_error(detail: &str) -> String {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .or_else(|_| std::env::var("DESKTOP_SESSION"))
        .unwrap_or_else(|_| "inconnu".to_string());
    format!(
        "Impossible de définir le fond d'écran (environnement de bureau : {desktop}). \
         Environnements pris en charge : Hyprland (via swww ou hyprpaper), GNOME, \
         KDE Plasma, XFCE, MATE, Cinnamon, sway et gestionnaires compatibles feh. \
         Détail : {detail}"
    )
}

#[cfg(not(target_os = "linux"))]
fn describe_error(detail: &str) -> String {
    format!("Impossible de définir le fond d'écran : {detail}")
}

/// Prise en charge de Hyprland. La crate `wallpaper` ne le connait pas et se
/// rabattrait sur un `swaybg` relance a chaque image (fuite de processus,
/// conflit avec le daemon de l'utilisateur). On pilote donc directement le
/// daemon en place : swww d'abord, hyprpaper ensuite.
///
/// Le module compile sur toutes les plateformes (il n'utilise que std) pour
/// etre verifie par `cargo check` partout ; seul l'appel est reserve a Linux.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod hyprland {
    use std::process::Command;

    pub fn is_active() -> bool {
        std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()
            || std::env::var("XDG_CURRENT_DESKTOP")
                .map(|d| d.to_lowercase().contains("hyprland"))
                .unwrap_or(false)
    }

    pub fn set(path: &str) -> Result<(), String> {
        // swww : si le daemon repond a `swww query`, c'est lui qui gere le fond.
        if run("swww", &["query"]).is_ok() {
            return run("swww", &["img", path])
                .map(|_| ())
                .map_err(|e| format!("swww n'a pas pu appliquer l'image : {e}"));
        }

        // hyprpaper via hyprctl : preload puis application a tous les ecrans
        // (moniteur vide = tous), puis liberation des images inutilisees.
        let preload = run("hyprctl", &["hyprpaper", "preload", path]);
        if preload.is_ok() {
            let target = format!(",{path}");
            return match run("hyprctl", &["hyprpaper", "wallpaper", &target]) {
                Ok(_) => {
                    let _ = run("hyprctl", &["hyprpaper", "unload", "unused"]);
                    Ok(())
                }
                Err(e) => Err(format!("hyprpaper n'a pas pu appliquer l'image : {e}")),
            };
        }

        Err(
            "Hyprland détecté mais aucun daemon de fond d'écran ne répond. \
             Lancez swww-daemon (recommandé) ou hyprpaper (avec ipc activé), \
             par exemple via exec-once dans hyprland.conf."
                .to_string(),
        )
    }

    /// Execute une commande et ne la considere reussie que si son code de
    /// retour est 0 et que sa sortie ne signale pas d'erreur (hyprctl repond
    /// parfois avec un code 0 mais un message d'erreur en clair).
    fn run(cmd: &str, args: &[&str]) -> Result<String, String> {
        let output = Command::new(cmd)
            .args(args)
            .output()
            .map_err(|e| format!("{cmd} introuvable ou non exécutable : {e}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let lowered = format!("{} {}", stdout.to_lowercase(), stderr.to_lowercase());
        if !output.status.success()
            || lowered.contains("error")
            || lowered.contains("couldn't")
            || lowered.contains("no such")
        {
            return Err(if stderr.is_empty() { stdout } else { stderr });
        }
        Ok(stdout)
    }
}
