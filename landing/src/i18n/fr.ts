import type { Copy } from "./en";

/**
 * The French copy.
 *
 * The settings panel reproduced in the hero stays in English on purpose: the
 * application itself is English, and a translated screenshot would promise a
 * window that does not exist. Only the page around it is translated.
 */
export const fr: Copy = {
  htmlLang: "fr",
  documentTitle: "APOD Wallpaper - l'univers, une fois par jour, sur votre bureau",
  documentDescription:
    "Un utilitaire discret qui pose l'image astronomique du jour de la NASA en fond d'ecran, une fois par jour. Natif, silencieux, open source. macOS et GNOME.",

  nav: {
    sections: [
      { href: "#craft", label: "Ce qui change" },
      { href: "#how", label: "Comment ça marche" },
      { href: "#modes", label: "Modes" },
      { href: "#download", label: "Télécharger" },
    ],
    source: "Le code sur GitHub",
    download: "Télécharger",
    language: "Langue",
  },

  hero: {
    eyebrow: "Libre et open source, pour macOS et GNOME",
    titleLead: "Chaque matin,",
    titleMiddle: "l'univers change",
    titleAccent: "de décor.",
    lede: "APOD Wallpaper pose l'image astronomique du jour de la NASA sur votre bureau, composée aux pixels exacts de votre écran. Puis elle s'efface : aucune fenêtre, aucune icône dans le Dock, une tâche de fond endormie jusqu'à minuit.",
    download: "Télécharger, c'est gratuit",
    source: "En lire chaque ligne",
    proof: [
      "13,1 Mo de mémoire au repos",
      "Une seule tâche de fond",
      "Aucun compte, aucune télémétrie",
      "30 ans d'archives",
    ],
    liveAbove: (title: string, date: string) => `En direct ci-dessus : ${title}, ${date}`,
    waitingForNasa: "L'image du jour se charge ici, directement depuis la NASA",
    altBlurFill: "L'image astronomique du jour, ajustée à un bureau",
    altCrop: "L'image astronomique du jour, recadrée à un bureau",
  },

  features: {
    eyebrow: "Ce qui change",
    heading: "Une application de fond d'écran que vous oublierez avoir installée.",
    lede: "La plupart réclament un compte, gardent un dossier de cent images et laissent un assistant tourner toute la journée. Celle-ci garde une seule image, parle à une seule API, et passe les vingt-trois heures et cinquante-neuf minutes restantes à dormir.",
    items: [
      {
        title: "Discrète par construction",
        body: "Aucune fenêtre la plupart du temps, rien dans le Dock ni dans le dash, une tâche de fond. Elle se réveille au changement de jour, fait son travail et se rendort. Entre deux minuits, le processus ne fait rien du tout.",
      },
      {
        title: "Composée à vos pixels",
        body: "L'image est composée à la résolution exacte de votre écran principal. Branchez un moniteur, changez l'échelle, fermez le capot : elle est recomposée depuis l'original déjà sur le disque, sans toucher au réseau.",
      },
      {
        title: "Votre bureau n'est jamais cassé",
        body: "Pas de réseau, un quota épuisé, une panne : le fond d'écran en place reste en place. Rien n'est écrit tant que la nouvelle image n'a pas été décodée, et les tentatives s'espacent au lieu de marteler.",
      },
      {
        title: "Une seule image sur le disque",
        body: "Exactement une image conservée, celle que vous regardez, plus l'original dont elle est tirée. Tout le reste est effacé dès qu'un nouveau fond d'écran est posé. Le dossier reste à quelques mégaoctets.",
      },
      {
        title: "Les vidéos deviennent des images",
        body: "Certaines publications sont des films. Une image en est décodée, ou la vignette publiée est utilisée, et le panneau vous dit laquelle et propose un lien pour regarder la vidéo comme il faut.",
      },
      {
        title: "Native, pas un navigateur déguisé",
        body: "Du Rust, avec un panneau Tauri que vous ouvrez une fois par mois. Elle emprunte les bibliothèques que votre système embarque déjà : aucun décodeur, aucun runtime et aucun service à elle.",
      },
    ],
  },

  how: {
    eyebrow: "Comment ça marche",
    heading: "Quatre choses par jour, puis le silence.",
    lede: "Toute l'application tient en une tâche de fond et une poignée de décisions. Les voici, dans l'ordre.",
    steps: [
      {
        title: "Elle demande à la NASA quel jour on est",
        body: "En mode quotidien, aucune date n'accompagne la requête : c'est l'API qui décide de ce que veut dire aujourd'hui, et aucun fuseau horaire ne peut fausser la réponse. Si l'image du jour n'est pas encore publiée, celle de la veille reste affichée et l'application regarde de nouveau une demi-heure plus tard.",
      },
      {
        title: "Elle compose, elle n'étire jamais",
        body: "L'image entière, sans déformation, centrée sur une copie floutée et assombrie d'elle-même qui remplit l'écran. Ou recadrée à votre format, si vous préférez. Rien n'est écrasé et aucun texte n'est jamais incrusté dans l'image.",
      },
      {
        title: "Elle confie le fichier au bureau",
        body: "Par le mécanisme du bureau lui-même : un Apple event sur macOS, GSettings sur GNOME. Le fichier est écrit sous un nom temporaire puis renommé en place, si bien qu'un fond d'écran à moitié écrit sur le disque n'est pas un état qui existe.",
      },
      {
        title: "Elle dort jusqu'à minuit",
        body: "Une tâche, une attente, aucun minuteur et aucun sondage. Rien ne tourne, rien n'écoute le réseau, rien n'interroge un serveur toutes les cinq minutes.",
      },
    ],
    wakesLabel: "Quatre choses écourtent ce sommeil :",
    wakes: [
      "Un changement d'écran",
      "Une sortie de veille",
      "Un échec à réessayer",
      "Votre clic sur Refresh now",
    ],
  },

  modes: {
    eyebrow: "Ce qui arrive sur votre écran",
    heading: "Le ciel du jour, un ciel au hasard, ou celui de votre anniversaire.",
    lede: "Trente ans d'images, une par jour, chacune accompagnée des mots de l'astronome. Trois façons de choisir celle devant laquelle vous vous réveillez.",
    items: [
      {
        title: "Image du jour",
        body: "Ce que la NASA a publié ce matin. Le réglage par défaut, et la raison d'être de l'application.",
      },
      {
        title: "Au hasard",
        body: "Une date tirée dans toute l'archive, qui commence le 16 juin 1995. Rappuyez pour en tirer une autre.",
      },
      {
        title: "Une date à vous",
        body: "L'image publiée le jour de votre naissance, le jour de votre rencontre, le jour du déménagement. N'importe quel jour depuis 1995.",
      },
    ],
    fitHeading: "Deux façons d'ajuster un rectangle de ciel à un rectangle d'écran.",
    fitLabel: "Mode d'ajustement",
    fits: {
      blur_fill: {
        label: "Fond flouté",
        note: "L'image entière, sans déformation, sur une copie floutée d'elle-même. Rien n'est rogné et rien n'est étiré.",
      },
      crop: {
        label: "Recadrage",
        note: "Recadrée à votre format et remplissant l'écran bord à bord, quand vous préférez perdre une bordure plutôt qu'en voir une.",
      },
    },
  },

  download: {
    eyebrow: "Télécharger",
    heading: "Installez-la une fois. Remarquez-la chaque matin.",
    ledeBefore:
      "Aucun compte, aucun abonnement, aucune télémétrie, rien à configurer. Elle utilise l'API publique de la NASA avec une clé de démonstration partagée, et vous pouvez coller une ",
    ledeLink: "clé gratuite à vous",
    ledeAfter: " dans le panneau si vous préférez votre propre quota.",
    platforms: {
      macos: {
        name: "macOS",
        requires: "13.3 ou plus récent, Apple silicon ou Intel",
        formats: ["Un .dmg universel"],
        note: "Le paquet n'est pas signé : effacez une fois l'attribut de quarantaine après l'installation.",
      },
      linux: {
        name: "Linux, GNOME",
        requires: "GNOME 42 ou plus récent, X11 ou Wayland, x86_64",
        formats: [".deb", ".rpm", ".AppImage", "PKGBUILD pour Arch"],
        note: "GNOME n'affiche aucune icône de statut par défaut. L'application est faite pour s'en passer, et la relancer depuis la vue d'ensemble ouvre le panneau.",
      },
    },
    copyCommand: "Copier la commande",
    copied: "Copié",
    copySelected: "Copie refusée : la commande est sélectionnée, copiez-la à la main",
    copyFailed: "Copie refusée : sélectionnez la commande",
    ctaTitle: "Windows est la prochaine étape. Quatre morceaux de code l'en séparent.",
    ctaBody:
      "Poser le fond d'écran, être prévenu d'un changement d'écran, mesurer l'écran, décoder une image de vidéo. Tout le reste est déjà écrit et partagé. Le dépôt le détaille, et les pull requests sont bienvenues.",
    ctaPrimary: "Prendre la dernière version",
    ctaSecondary: "Mettre une étoile sur GitHub",
    ctaNotes: "Ou lisez d'abord les notes de conception, elles sont inhabituellement longues",
  },

  faq: {
    eyebrow: "Questions",
    heading: "Les réponses honnêtes.",
    items: [
      {
        q: "Faut-il un compte ou une clé d'API ?",
        a: "Ni l'un ni l'autre. L'application parle à l'API publique de la NASA avec la clé de démonstration partagée, qui autorise trente requêtes par heure et par adresse, là où l'application en fait une poignée par jour. Si vous partagez votre adresse avec d'autres utilisateurs de cette API, une clé personnelle gratuite se demande en une minute et se colle directement dans le panneau.",
      },
      {
        q: "Qu'est-ce qu'elle coûte à ma batterie et à ma mémoire ?",
        a: "Entre deux mises à jour, rien de mesurable : une tâche qui attend une horloge. Un démarrage sans rien à faire tient dans 13,1 Mo de mémoire privée, et après une mise à jour l'allocateur est prié de rendre l'image au système plutôt que de la garder pendant les douze heures de sommeil qui suivent.",
      },
      {
        q: "Pourquoi GNOME et pas tous les bureaux Linux ?",
        a: "Parce que le fond d'écran passe par les réglages de GNOME, et que les deux événements système viennent de Mutter et de logind. Sur KDE ou Xfce, l'application démarre puis vous dit qu'elle ne peut pas poser le fond d'écran, plutôt que d'échouer en silence. Prendre en charge un autre bureau revient à réécrire ces mêmes morceaux, et le code est rangé pour que ce soit un petit travail.",
      },
      {
        q: "Que se passe-t-il avec un second écran ?",
        a: "L'image est composée à la résolution de l'écran principal et le bureau l'applique partout. Branchez un écran, débranchez-en un, changez l'échelle : l'application recompose depuis l'original qu'elle a déjà, sans requête réseau.",
      },
      {
        q: "À qui appartiennent les images ?",
        a: "Pas à cette application. Les publications portant une mention de copyright appartiennent à leurs auteurs, et cette mention est affichée avec l'image et jamais retirée. Le reste est en général du matériel de la NASA, dans le domaine public aux États-Unis. L'application les télécharge pour votre bureau ; ce que vous en faites ensuite vous regarde.",
      },
      {
        q: "Dans quelle langue est l'application elle-même ?",
        a: "En anglais, panneau et menu compris, sur les deux plateformes. Cette page parle la langue de votre navigateur ; l'application, pas encore.",
      },
    ],
  },

  footer: {
    tagline:
      "Une image de l'univers, choisie par un astronome, qui vous attend quand vous vous asseyez.",
    source: "Le code",
    licence: "Licence MIT",
    apod: "NASA APOD",
    legal:
      "Sans affiliation avec la NASA. Astronomy Picture of the Day est un service de la NASA et de la Michigan Technological University. Les images portant une mention de copyright restent la propriété de leurs auteurs ; l'application affiche cette mention et ne la retire jamais. Le code est sous licence MIT et a été écrit avec une forte assistance de l'IA, ce que le dépôt annonce d'emblée.",
  },
};
