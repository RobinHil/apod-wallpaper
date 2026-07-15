use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::fmt;

const ENDPOINT: &str = "https://api.nasa.gov/planetary/apod";

/// Reponse de l'API APOD. `copyright` est absent pour les images du domaine
/// public produites par la NASA ; quand il est present, il doit etre conserve
/// et affiche partout (cache, incrustation, interface).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Apod {
    pub date: String,
    pub title: String,
    pub explanation: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub hdurl: Option<String>,
    pub media_type: String,
    #[serde(default)]
    pub copyright: Option<String>,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
}

impl Apod {
    pub fn is_video(&self) -> bool {
        self.media_type == "video"
    }

    /// Vrai si une image exploitable en fond d'ecran existe : l'image
    /// elle-meme, ou la vignette pour une video. L'API ne fournit pas de
    /// fichier video (seulement un lien d'integration YouTube/Vimeo), donc
    /// la vignette est la seule representation possible d'une video.
    pub fn has_image(&self) -> bool {
        match self.media_type.as_str() {
            "image" => self.hdurl.is_some() || self.url.is_some(),
            "video" => self.thumbnail_url.is_some(),
            _ => false,
        }
    }

    /// URL a telecharger en priorite : haute definition pour une image,
    /// vignette en resolution maximale pour une video YouTube.
    pub fn preferred_download_url(&self) -> Option<String> {
        match self.media_type.as_str() {
            "image" => self
                .hdurl
                .as_deref()
                .or(self.url.as_deref())
                .map(str::to_string),
            "video" => {
                let thumb = self.thumbnail_url.as_deref()?;
                Some(youtube_maxres(thumb).unwrap_or_else(|| thumb.to_string()))
            }
            _ => None,
        }
    }

    /// URL de repli si le premier telechargement echoue : URL standard pour
    /// une image, vignette d'origine pour une video (la version maxres
    /// n'existe pas pour les videos anciennes ou basse definition).
    pub fn fallback_download_url(&self) -> Option<String> {
        match self.media_type.as_str() {
            "image" => match (self.hdurl.as_deref(), self.url.as_deref()) {
                (Some(_), Some(u)) => Some(u.to_string()),
                _ => None,
            },
            "video" => {
                let thumb = self.thumbnail_url.as_deref()?;
                // Un repli n'a de sens que si on a tente la version maxres.
                youtube_maxres(thumb).map(|_| thumb.to_string())
            }
            _ => None,
        }
    }

    /// Le champ copyright renvoye par l'API contient souvent des retours a la
    /// ligne parasites : on normalise en espaces simples.
    fn normalize(mut self) -> Self {
        if let Some(c) = self.copyright.take() {
            let cleaned = c.split_whitespace().collect::<Vec<_>>().join(" ");
            if !cleaned.is_empty() {
                self.copyright = Some(cleaned);
            }
        }
        self
    }
}

/// Pour une vignette YouTube standard (0.jpg, hqdefault.jpg...), construit
/// l'URL de la vignette en resolution maximale (maxresdefault.jpg, souvent
/// 1280x720 ; verifie disponible pour les videos NASA recentes). Renvoie
/// None si l'URL n'est pas une vignette YouTube connue : l'appelant garde
/// alors l'URL d'origine.
fn youtube_maxres(url: &str) -> Option<String> {
    if !url.contains("img.youtube.com") && !url.contains("ytimg.com") {
        return None;
    }
    let (head, tail) = url.rsplit_once('/')?;
    let name = tail.strip_suffix(".jpg")?;
    const VARIANTS: [&str; 8] = [
        "0", "1", "2", "3", "default", "mqdefault", "hqdefault", "sddefault",
    ];
    if VARIANTS.contains(&name) {
        Some(format!("{head}/maxresdefault.jpg"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::youtube_maxres;

    #[test]
    fn upgrade_standard_youtube_thumbnails() {
        assert_eq!(
            youtube_maxres("https://img.youtube.com/vi/abc123/0.jpg").as_deref(),
            Some("https://img.youtube.com/vi/abc123/maxresdefault.jpg")
        );
        assert_eq!(
            youtube_maxres("https://i.ytimg.com/vi/abc123/hqdefault.jpg").as_deref(),
            Some("https://i.ytimg.com/vi/abc123/maxresdefault.jpg")
        );
    }

    #[test]
    fn leave_other_urls_untouched() {
        assert_eq!(youtube_maxres("https://vimeo.com/thumb/42.jpg"), None);
        assert_eq!(
            youtube_maxres("https://img.youtube.com/vi/abc123/maxresdefault.jpg"),
            None
        );
        assert_eq!(youtube_maxres("https://img.youtube.com/vi/abc123/0.png"), None);
    }
}

#[derive(Debug)]
pub enum ApiError {
    /// Erreur reseau : pas de connexion, DNS, timeout... => mode hors-ligne.
    Network(String),
    /// Quota de la cle API depasse (HTTP 429).
    RateLimited,
    /// Pas d'APOD pour la date demandee (jours manquants dans l'historique).
    NotFound,
    /// Autre erreur HTTP.
    Http(u16),
    /// Reponse illisible.
    Parse(String),
}

impl ApiError {
    /// Vrai si l'erreur correspond a une indisponibilite temporaire justifiant
    /// le mode hors-ligne et des tentatives silencieuses en arriere-plan.
    pub fn is_offline(&self) -> bool {
        matches!(
            self,
            ApiError::Network(_) | ApiError::RateLimited | ApiError::Http(500..=599)
        )
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::Network(e) => write!(f, "Erreur réseau : {e}"),
            ApiError::RateLimited => write!(
                f,
                "Quota de la clé API dépassé (DEMO_KEY : 30 requêtes/heure). Réessai automatique plus tard."
            ),
            ApiError::NotFound => write!(
                f,
                "Aucune APOD n'a été publiée à cette date (l'historique comporte quelques jours sans publication)."
            ),
            ApiError::Http(code) => write!(f, "Erreur HTTP {code} de l'API NASA."),
            ApiError::Parse(e) => write!(f, "Réponse de l'API illisible : {e}"),
        }
    }
}

/// Interroge l'API APOD. `date: None` renvoie la derniere image publiee (ce
/// qui evite les erreurs de fuseau horaire : c'est l'API qui decide de la
/// date du jour, pas l'horloge locale).
pub async fn fetch_apod(
    client: &reqwest::Client,
    api_key: &str,
    date: Option<NaiveDate>,
) -> Result<Apod, ApiError> {
    // thumbs=true : l'API joint la vignette des videos, seule representation
    // exploitable en fond d'ecran (aucun fichier video n'est fourni).
    let mut request = client
        .get(ENDPOINT)
        .query(&[("api_key", api_key), ("thumbs", "true")]);
    if let Some(d) = date {
        request = request.query(&[("date", d.format("%Y-%m-%d").to_string())]);
    }

    let response = request
        .send()
        .await
        .map_err(|e| ApiError::Network(e.to_string()))?;

    match response.status().as_u16() {
        200 => response
            .json::<Apod>()
            .await
            .map(Apod::normalize)
            .map_err(|e| ApiError::Parse(e.to_string())),
        429 => Err(ApiError::RateLimited),
        // L'API repond 404 ou 400 pour les dates sans publication.
        400 | 404 => Err(ApiError::NotFound),
        code => Err(ApiError::Http(code)),
    }
}

/// Telecharge les octets bruts d'une image.
pub async fn download_image(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, ApiError> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| ApiError::Network(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        return Err(ApiError::Http(status.as_u16()));
    }

    response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| ApiError::Network(e.to_string()))
}
