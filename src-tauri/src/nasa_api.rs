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
    pub fn is_image(&self) -> bool {
        self.media_type == "image" && self.best_image_url().is_some()
    }

    /// URL a telecharger : haute definition si disponible, sinon standard.
    pub fn best_image_url(&self) -> Option<&str> {
        self.hdurl.as_deref().or(self.url.as_deref())
    }

    /// URL de repli si le telechargement HD echoue.
    pub fn fallback_image_url(&self) -> Option<&str> {
        match (self.hdurl.as_deref(), self.url.as_deref()) {
            (Some(_), Some(u)) => Some(u),
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
            ApiError::NotFound => write!(f, "Aucune image APOD pour cette date."),
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
    let mut request = client.get(ENDPOINT).query(&[("api_key", api_key)]);
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
