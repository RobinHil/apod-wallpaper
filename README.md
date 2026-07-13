# APOD Wallpaper

Application desktop legere et multiplateforme (Windows, macOS, Linux) qui definit
automatiquement l'image astronomique du jour de la NASA (APOD, *Astronomy Picture
of the Day*) comme fond d'ecran. Elle vit dans la barre d'etat systeme (system tray)
et fonctionne en arriere-plan.

Construite avec [Tauri 2](https://tauri.app) : backend Rust, popup de reglages en
TypeScript/HTML/CSS vanilla. Pas de framework JS, pas de dependance superflue.

## Fonctionnalites

- **Image du jour** : recupere l'APOD courante via l'API NASA et l'applique en fond d'ecran.
- **Mode aleatoire** : tire une date au sort dans tout l'historique APOD (depuis le
  16 juin 1995) a chaque demarrage de l'application.
- **Verification automatique** : au demarrage, puis en continu tant que l'application
  tourne (nouvelle image quotidienne detectee automatiquement).
- **Rafraichissement manuel** depuis le menu du tray ou le panneau de reglages.
- **Adaptation intelligente au ratio de l'ecran** (mode par defaut "fond flou") :
  l'image originale est centree entiere et sans deformation par-dessus une version
  d'elle-meme agrandie, floutee et assombrie qui remplit l'ecran. Un mode
  "recadrer pour remplir" (sans flou) est disponible dans les reglages.
- **Incrustation des credits** : la date et le copyright (quand il existe) sont
  incrustes en bas a droite de l'image, en blanc sur un cartouche sombre
  semi-transparent, lisible sur toute image.
- **Cache local** : historique des dernieres images telechargees (60 max) avec leurs
  metadonnees dans `metadata.json`.
- **Mode hors-ligne** : en cas de coupure reseau ou de quota API depasse, la derniere
  image chargee reste en place, l'application reessaie silencieusement en arriere-plan
  (toutes les 15 minutes) et l'etat hors-ligne est indique dans le tray et le panneau.
- **Cle API configurable** : `DEMO_KEY` par defaut, cle personnelle enregistrable
  depuis le panneau (persistee localement).

## Prerequis

- [Rust](https://www.rust-lang.org/tools/install) (stable, via rustup)
- [Node.js](https://nodejs.org) 18 ou plus recent, avec npm
- Les prerequis systeme de Tauri selon votre OS :
  <https://tauri.app/start/prerequisites/>

Par plateforme :

| OS      | Dependances supplementaires |
|---------|-----------------------------|
| Windows | WebView2 (preinstalle sur Windows 10/11), Microsoft C++ Build Tools |
| macOS   | Xcode Command Line Tools (`xcode-select --install`) |
| Linux   | `webkit2gtk-4.1`, `libappindicator3` (ou `libayatana-appindicator`), `librsvg2`, `patchelf` -- voir la page de prerequis Tauri pour la liste exacte selon la distribution |

Note Linux : l'icone tray requiert un environnement qui prend en charge les
`StatusNotifierItem`/AppIndicator (extension "AppIndicator" necessaire sous GNOME).

### Hyprland / Waybar

- **Tray** : le module `"tray"` de Waybar implemente StatusNotifierItem et
  DBusMenu ; l'icone et son menu fonctionnent tels quels (ajoutez `"tray"` aux
  modules de votre configuration Waybar si ce n'est pas deja fait).
- **Fond d'ecran** : Hyprland est detecte automatiquement
  (`HYPRLAND_INSTANCE_SIGNATURE`) et l'application pilote le daemon en place,
  dans cet ordre : `swww` (si `swww query` repond), puis `hyprpaper` (via
  `hyprctl hyprpaper`, ipc actif requis). L'un des deux doit tourner, par
  exemple avec `exec-once = swww-daemon` dans `hyprland.conf`.
- **Fenetre de reglages** : sous un compositeur tiling, le panneau s'ouvre
  comme une fenetre normale ; pour le faire flotter :
  `windowrulev2 = float, title:^(APOD Wallpaper)$`

## Installation et lancement en developpement

```bash
cd apod-wallpaper
npm install
npm run tauri dev
```

Au premier lancement, l'application :

1. interroge l'API APOD (avec `DEMO_KEY` si aucune cle n'est configuree) ;
2. telecharge l'image (HD si disponible) et l'enregistre dans le cache ;
3. compose l'image finale a la resolution de l'ecran principal (fond flou +
   image centree + incrustation date/copyright) ;
4. la definit comme fond d'ecran ;
5. s'installe dans la barre d'etat. La fenetre de reglages est cachee par defaut :
   elle s'ouvre via le menu du tray ("Details et reglages...").

## Build de production

```bash
npm run tauri build
```

Les artefacts sont generes dans `src-tauri/target/release/bundle/` :

- **Windows** : installeur `.msi` (WiX) et `.exe` (NSIS) -- a construire depuis Windows
- **macOS** : bundle `.app` et image `.dmg` -- a construire depuis macOS
- **Linux** : `.deb`, `.rpm` et `.AppImage` -- a construire depuis Linux

La compilation croisee n'est pas prise en charge par Tauri : chaque plateforme se
construit depuis l'OS cible (en CI, une matrice GitHub Actions
`windows-latest`/`macos-latest`/`ubuntu-latest` est l'approche habituelle).

## Configuration de la cle API NASA

Par defaut l'application utilise `DEMO_KEY`, limitee a **30 requetes/heure et
50 requetes/jour** (par adresse IP). C'est suffisant pour un usage normal, mais une
cle personnelle gratuite est recommandee :

1. Demandez une cle sur <https://api.nasa.gov/> (formulaire simple, cle recue par email).
2. Ouvrez le panneau de l'application (menu tray, entree "Details et reglages...").
3. Collez la cle dans le champ "Cle API NASA" et cliquez sur "Enregistrer".

La cle est stockee localement dans `settings.json` (voir "Donnees locales" ci-dessous)
et n'est envoyee qu'a l'API NASA.

## Structure du projet

```
apod-wallpaper/
|- src-tauri/                  # Backend Rust
|  |- src/
|  |  |- main.rs               # Point d'entree binaire
|  |  |- lib.rs                # Setup Tauri : tray, menu, scheduler, commandes
|  |  |- nasa_api.rs           # Appel API APOD, parsing, typologie d'erreurs
|  |  |- cache.rs              # Historique local (metadata.json + fichiers images)
|  |  |- image_compose.rs      # Fond flou/recadrage + incrustation date/copyright
|  |  |- wallpaper.rs          # Definition du fond d'ecran par plateforme
|  |  `- settings.rs           # Cle API, mode, ajustement ; persistance JSON
|  |- assets/
|  |  |- DejaVuSans.ttf        # Police embarquee pour l'incrustation
|  |  `- DejaVuSans-LICENSE.txt
|  |- capabilities/default.json
|  `- tauri.conf.json
|- src/                        # Frontend du popup (vanilla TypeScript)
|  |- main.ts                  # Rendu de l'etat, commandes vers le backend
|  `- styles.css               # Theme clair/sombre (prefers-color-scheme)
|- index.html                  # Structure du panneau (icones SVG inline)
`- README.md
```

## Donnees locales

L'application ecrit dans le dossier de donnees standard de l'OS
(`com.rh.apod-wallpaper`) :

- **macOS** : `~/Library/Application Support/com.rh.apod-wallpaper/`
- **Windows** : `%APPDATA%\com.rh.apod-wallpaper\`
- **Linux** : `~/.local/share/com.rh.apod-wallpaper/`

Contenu :

```
settings.json                  # cle API, mode (jour/aleatoire), ajustement
cache/
|- metadata.json               # historique des images et de leurs metadonnees
|- images/<date>.<ext>         # images originales telechargees
`- wallpapers/apod-<date>-<fit>.jpg   # compositions finales appliquees
```

## Choix techniques notables

- **APOD de type video** : certaines publications APOD sont des videos (YouTube ou
  Vimeo). Les vignettes fournies par l'API sont de trop basse resolution pour un
  fond d'ecran, donc l'application ne les utilise pas. En mode "image du jour",
  l'image precedente est conservee et le menu du tray le signale ; en mode
  aleatoire, une nouvelle date est tiree au sort automatiquement.
- **Mode jour sans parametre de date** : l'application demande a l'API "la derniere
  image publiee" plutot que la date locale, ce qui elimine les decalages de fuseau
  horaire (l'APOD est publiee sur le fuseau de la cote Est americaine).
- **Flou gaussien economique** : le fond est floute sur une version reduite (1/8) de
  l'image puis re-agrandi ; le rendu est equivalent a un flou prononce sur l'image
  pleine taille pour une fraction du cout CPU.
- **Nom de fichier variable** : la composition finale inclut la date et le mode
  d'ajustement dans son nom de fichier, car certains bureaux (macOS notamment)
  mettent le fond d'ecran en cache par chemin et ignorent un fichier reecrit en place.
- **Droits d'auteur** : le champ `copyright` de l'API est conserve dans le cache,
  incruste sur l'image et affiche dans l'interface. Quand il est present, l'image
  n'est **pas** dans le domaine public : elle appartient a son auteur et l'usage
  est limite au fond d'ecran personnel. Les images sans copyright sont produites
  par la NASA et relevent du domaine public.

## Limitations connues

- **Linux** : la definition du fond d'ecran depend de l'environnement de bureau.
  Sont pris en charge : Hyprland (via swww ou hyprpaper, voir plus haut) et, via
  la crate `wallpaper` : GNOME, KDE Plasma, XFCE, MATE, Cinnamon, Budgie, Deepin,
  sway, i3 et autres gestionnaires compatibles `feh`. Sur un environnement non
  reconnu, un message explicite est affiche dans le tray.
- **Multi-ecrans** : l'image est composee a la resolution de l'ecran principal ;
  les ecrans secondaires recoivent la meme image (le support d'une composition par
  ecran est une evolution possible).
- **Verification quotidienne** : la detection de la nouvelle image du jour se fait
  par sondage toutes les 15 minutes apres minuit (heure locale), jusqu'a ce que
  l'API publie la nouvelle APOD.
- **macOS** : le changement de fond d'ecran passe par un evenement AppleScript ;
  au premier lancement, macOS peut demander l'autorisation de controler
  "System Events" (a accepter).

## Licences

- Code du projet : a definir par l'auteur du depot.
- Police DejaVu Sans embarquee : licence libre Bitstream Vera / DejaVu
  (voir `src-tauri/assets/DejaVuSans-LICENSE.txt`).
- Les images APOD avec mention de copyright restent la propriete de leurs auteurs.
