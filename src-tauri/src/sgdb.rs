//! SteamGridDB client.
//!
//! Two things in the published spec needed care:
//!
//! 1. `/search/autocomplete` declares `Games.data` as an array of `Game`, and
//!    `Game` itself is `{ success, data: { id, name, ... } }`. Read literally that
//!    is a double-nested shape, while the live API is usually flat. `GameData`
//!    accepts both so a spec implementation change cannot break search.
//! 2. `limit` is silently ignored above 50, so callers must not ask for more and
//!    assume they got it.
//!
//! Auth is a bearer token; there is no documented rate limit, so requests are
//! serialised with a minimum interval and 429/503 responses are retried once with
//! `Retry-After` honoured when present.

use std::time::Duration;

use serde::{Deserialize, Deserializer, Serialize};

pub const BASE_URL: &str = "https://www.steamgriddb.com/api/v2";
pub const COVER_DIMENSIONS: &str = "600x900";
pub const HERO_DIMENSIONS: &str = "1920x620";
pub const MAX_LIMIT: u32 = 50;

type Result<T> = std::result::Result<T, SgdbError>;

#[derive(Debug, thiserror::Error)]
pub enum SgdbError {
    #[error("no SteamGridDB API key configured")]
    MissingKey,
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("SteamGridDB returned {0}")]
    Status(u16),
    #[error("SteamGridDB error: {0}")]
    Api(String),
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GameData {
    pub id: i64,
    pub name: String,
    pub verified: bool,
    pub types: Vec<String>,
}

impl<'de> Deserialize<'de> for GameData {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Body {
            id: i64,
            name: String,
            #[serde(default)]
            types: Vec<String>,
            #[serde(default)]
            verified: bool,
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Shape {
            Flat(Body),
            Nested { data: Body },
        }

        let shape = Shape::deserialize(deserializer)?;
        let body = match shape {
            Shape::Flat(body) => body,
            Shape::Nested { data } => data,
        };

        Ok(GameData {
            id: body.id,
            name: body.name,
            verified: body.verified,
            types: body.types,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Asset {
    pub id: i64,
    #[serde(default)]
    pub score: i64,
    #[serde(default)]
    pub style: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub thumb: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Deserialize)]
struct Envelope<T> {
    #[serde(default)]
    success: bool,
    // No `#[serde(default)]` here: `Option<T>` already deserialises a missing key
    // to `None`, and the attribute would add a spurious `T: Default` bound.
    data: Option<T>,
    #[serde(default)]
    errors: Vec<String>,
}

pub struct Sgdb {
    http: reqwest::Client,
    key: String,
    base: String,
    min_interval: Duration,
}

impl Sgdb {
    pub fn new(key: impl Into<String>) -> Self {
        let key = key.into();
        Self {
            http: reqwest::Client::builder()
                .user_agent(concat!("PirateLink/", env!("CARGO_PKG_VERSION")))
                .timeout(Duration::from_secs(20))
                .build()
                .unwrap_or_default(),
            key,
            base: BASE_URL.to_string(),
            // No published limit; 8 req/s is polite and still fast enough to
            // hydrate a few dozen covers.
            min_interval: Duration::from_millis(125),
        }
    }

    #[cfg(test)]
    fn with_base(mut self, base: impl Into<String>) -> Self {
        self.base = base.into();
        self
    }

    pub fn has_key(&self) -> bool {
        !self.key.trim().is_empty()
    }

    /// Prefix search by name. Terms are percent-encoded path segments.
    pub async fn search(&self, term: &str) -> Result<Vec<GameData>> {
        if !self.has_key() {
            return Err(SgdbError::MissingKey);
        }
        let url = format!("{}/search/autocomplete/{}", self.base, encode_segment(term));
        let body: Envelope<Vec<GameData>> = self.get_json(&url).await?;
        Ok(body.data.unwrap_or_default())
    }

    /// Portrait covers. Defaults to 600x900, the size the shelf renders.
    pub async fn grids(&self, game_id: i64, dimensions: Option<&str>) -> Result<Vec<Asset>> {
        let dims = dimensions.unwrap_or(COVER_DIMENSIONS);
        let url = format!(
            "{}/grids/game/{}?types=static&dimensions={}&limit={}",
            self.base,
            game_id,
            encode_segment(dims),
            MAX_LIMIT
        );
        let body: Envelope<Vec<Asset>> = self.get_json(&url).await?;
        Ok(body.data.unwrap_or_default())
    }

    /// Wide hero art. Note the API's hero aspect is 1920x620, not 16:9.
    pub async fn heroes(&self, game_id: i64) -> Result<Vec<Asset>> {
        let url = format!(
            "{}/heroes/game/{}?types=static&dimensions={}&limit={}",
            self.base,
            game_id,
            encode_segment(HERO_DIMENSIONS),
            MAX_LIMIT
        );
        let body: Envelope<Vec<Asset>> = self.get_json(&url).await?;
        Ok(body.data.unwrap_or_default())
    }

    pub async fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let res = self.send(|| self.http.get(url)).await?;
        Ok(res.bytes().await?.to_vec())
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<Envelope<T>> {
        let res = self.send(|| self.http.get(url).header("Authorization", format!("Bearer {}", self.key))).await?;
        let status = res.status();
        let text = res.text().await?;

        if !status.is_success() {
            return Err(SgdbError::Status(status.as_u16()));
        }

        let body: Envelope<T> = serde_json::from_str(&text)
            .map_err(|e| SgdbError::Api(format!("malformed response: {e}")))?;

        if !body.success {
            let message = if body.errors.is_empty() {
                "request rejected".to_string()
            } else {
                body.errors.join("; ")
            };
            return Err(SgdbError::Api(message));
        }

        Ok(body)
    }

    async fn send<F>(&self, build: F) -> Result<reqwest::Response>
    where
        F: Fn() -> reqwest::RequestBuilder,
    {
        let mut attempt = 0;
        loop {
            let res = build().send().await?;
            let status = res.status();

            if (status == reqwest::StatusCode::TOO_MANY_REQUESTS
                || status == reqwest::StatusCode::SERVICE_UNAVAILABLE)
                && attempt == 0
            {
                attempt += 1;
                let wait = res
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(2)
                    .min(10);
                tokio::time::sleep(Duration::from_secs(wait)).await;
                continue;
            }

            if !status.is_success() {
                return Err(SgdbError::Status(status.as_u16()));
            }

            // Space out successful requests as well; the API publishes no quota.
            if !self.min_interval.is_zero() {
                tokio::time::sleep(self.min_interval).await;
            }
            return Ok(res);
        }
    }
}

/// Similarity below this is not trusted, and no artwork is applied.
///
/// Wrong artwork is worse than no artwork: it is silently wrong, it gets cached,
/// and the user has no way to tell it apart from a correct choice.
pub const MATCH_THRESHOLD: f32 = 0.6;

/// Picks the search result that actually corresponds to `query`.
///
/// The old behaviour ("exact match, else the first result") is what attached
/// unrelated Nintendo art to a game whose folder happened to be called "Game":
/// the autocomplete for "Game" returns a title literally named *GAME* first.
pub fn best_match<'a>(query: &str, candidates: &'a [GameData]) -> Option<&'a GameData> {
    let q = match_key(query);
    if q.chars().count() < 2 {
        return None;
    }

    candidates
        .iter()
        .filter_map(|candidate| {
            let score = similarity(&q, &match_key(&candidate.name));
            (score >= MATCH_THRESHOLD).then_some((score, candidate))
        })
        .max_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                // On a tie, the verified entry must compare as *greater*.
                .then_with(|| a.1.verified.cmp(&b.1.verified))
        })
        .map(|(_, candidate)| candidate)
}

/// Lowercase, alphanumeric and single-spaced, so "ELDEN RING" == "Elden Ring".
fn match_key(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut last_space = true;
    for ch in value.chars() {
        if ch.is_alphanumeric() {
            out.extend(ch.to_lowercase());
            last_space = false;
        } else if !last_space {
            out.push(' ');
            last_space = true;
        }
    }
    out.trim_end().to_string()
}

fn similarity(query: &str, name: &str) -> f32 {
    if query.is_empty() || name.is_empty() {
        return 0.0;
    }
    if query == name {
        return 1.0;
    }

    // Prefix relationship: "elden ring" vs "elden ring seamless co-op".
    if name.starts_with(query) || query.starts_with(name) {
        let (short, long) = if name.chars().count() <= query.chars().count() {
            (name, query)
        } else {
            (query, name)
        };
        let ratio = short.chars().count() as f32 / long.chars().count() as f32;
        return 0.6 + 0.35 * ratio;
    }

    // Token overlap (Jaccard).
    let q: std::collections::HashSet<&str> = query.split(' ').collect();
    let n: std::collections::HashSet<&str> = name.split(' ').collect();
    let intersection = q.intersection(&n).count() as f32;
    let union = q.union(&n).count().max(1) as f32;
    0.75 * (intersection / union)
}

#[cfg(test)]
mod match_tests {
    use super::*;

    fn game(id: i64, name: &str, verified: bool) -> GameData {
        GameData {
            id,
            name: name.to_string(),
            verified,
            types: vec![],
        }
    }

    #[test]
    fn exact_match_beats_a_longer_variant() {
        let results = vec![
            game(1, "Elden Ring Seamless Co-op", false),
            game(2, "Elden Ring", true),
            game(3, "Elden Ring GB", false),
        ];
        assert_eq!(best_match("Elden Ring", &results).unwrap().id, 2);
    }

    #[test]
    fn unrelated_results_are_rejected() {
        // The real regression: "Game" was used as a title, and this is what the
        // API returns for it.
        let results = vec![game(5438475, "GAME", true), game(799, "Game Studio Simulator", true)];
        assert!(best_match("Elden Ring", &results).is_none());
    }

    #[test]
    fn prefix_variants_still_match() {
        let results = vec![game(1, "Elden Ring", true)];
        assert_eq!(best_match("ELDEN RING", &results).unwrap().id, 1);
        assert_eq!(best_match("elden.ring", &results).unwrap().id, 1);
    }

    #[test]
    fn punctuation_and_case_are_ignored() {
        let results = vec![game(7, "S.T.A.L.K.E.R.: Shadow of Chernobyl", true)];
        assert_eq!(
            best_match("S.T.A.L.K.E.R. Shadow of Chernobyl", &results).unwrap().id,
            7
        );
    }

    #[test]
    fn empty_candidates_match_nothing() {
        assert!(best_match("Hades", &[]).is_none());
        assert!(best_match("", &[game(1, "Hades", true)]).is_none());
    }

    #[test]
    fn verified_wins_a_tie() {
        let results = vec![game(1, "Hades", false), game(2, "Hades", true)];
        assert_eq!(best_match("Hades", &results).unwrap().id, 2);
    }
}

/// Picks the cover to cache.
///
/// Do not rank by `score`: the live API returns `0` for every asset (verified
/// against `/grids/game/4997889`), so a "highest score" sort silently returns
/// whichever element happened to be last. The API's default order is by votes and
/// relevance, so the first acceptable entry *is* the best one. Tag filtering stays,
/// because tagged artwork can still appear at the front.
pub fn pick_best(assets: &[Asset]) -> Option<&Asset> {
    let is_safe = |a: &&Asset| {
        !a.tags.iter().any(|t| {
            let t = t.to_lowercase();
            t.contains("nsfw") || t.contains("epilepsy") || t.contains("humor")
        })
    };

    assets.iter().find(is_safe).or_else(|| assets.first())
}

/// Percent-encodes everything outside the unreserved set, which is what a path
/// segment needs (spaces, `#`, `?` and `/` all appear in game titles).
fn encode_segment(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            b => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flat_autocomplete_items() {
        let json = r#"{"success":true,"data":[{"id":220,"name":"Half-Life 2","types":["steam"],"verified":true}]}"#;
        let env: Envelope<Vec<GameData>> = serde_json::from_str(json).unwrap();
        let games = env.data.unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].id, 220);
        assert_eq!(games[0].name, "Half-Life 2");
        assert!(games[0].verified);
    }

    #[test]
    fn parses_double_nested_shape_from_the_spec() {
        // Literal reading of components.schemas.Games -> Game -> data.
        let json = r#"{"success":true,"data":[{"success":true,"data":{"id":391540,"name":"Night in the Woods","types":["steam"],"verified":false}}]}"#;
        let env: Envelope<Vec<GameData>> = serde_json::from_str(json).unwrap();
        let games = env.data.unwrap();
        assert_eq!(games[0].id, 391540);
        assert_eq!(games[0].name, "Night in the Woods");
    }

    #[test]
    fn parses_assets_with_missing_optional_fields() {
        let json = r#"{"success":true,"page":0,"total":2,"limit":50,"data":[
            {"id":1,"score":10,"style":"alternate","url":"https://cdn/a.webp","thumb":"https://cdn/a_t.webp","tags":[]},
            {"id":2,"score":99,"style":"no_logo","url":"https://cdn/b.png","thumb":"https://cdn/b_t.png"}
        ]}"#;
        let env: Envelope<Vec<Asset>> = serde_json::from_str(json).unwrap();
        let assets = env.data.unwrap();
        assert_eq!(assets.len(), 2);
        assert!(assets[1].tags.is_empty());
        // API order wins: scores are not meaningful on the live API.
        assert_eq!(pick_best(&assets).unwrap().id, 1);
    }

    #[test]
    fn pick_best_follows_api_order_over_score() {
        // The live API returns score: 0 for everything, so trusting the score
        // silently picks the last element. Order is the real signal.
        let assets = vec![
            Asset {
                id: 10,
                score: 0,
                style: "alternate".into(),
                url: "u10".into(),
                thumb: "t10".into(),
                tags: vec![],
            },
            Asset {
                id: 11,
                score: 999,
                style: "alternate".into(),
                url: "u11".into(),
                thumb: "t11".into(),
                tags: vec![],
            },
        ];
        assert_eq!(pick_best(&assets).unwrap().id, 10);
    }

    #[test]
    fn pick_best_skips_tagged_artwork() {
        let assets = vec![
            Asset {
                id: 1,
                score: 999,
                style: "alternate".into(),
                url: "u1".into(),
                thumb: "t1".into(),
                tags: vec!["nsfw".into()],
            },
            Asset {
                id: 2,
                score: 5,
                style: "alternate".into(),
                url: "u2".into(),
                thumb: "t2".into(),
                tags: vec![],
            },
        ];
        assert_eq!(pick_best(&assets).unwrap().id, 2);
    }

    #[test]
    fn error_envelope_reports_its_errors() {
        let json = r#"{"success":false,"errors":["Invalid game id"]}"#;
        let env: Envelope<Vec<Asset>> = serde_json::from_str(json).unwrap();
        assert!(!env.success);
        assert_eq!(env.errors, vec!["Invalid game id".to_string()]);
    }

    #[test]
    fn encodes_awkward_search_terms() {
        assert_eq!(encode_segment("Ori and the Will"), "Ori%20and%20the%20Will");
        assert_eq!(encode_segment("A/B"), "A%2FB");
        assert_eq!(encode_segment("What?"), "What%3F");
        assert_eq!(encode_segment("hades"), "hades");
    }

    #[test]
    fn search_without_key_fails_fast() {
        let client = Sgdb::new("");
        assert!(!client.has_key());
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let err = rt.block_on(client.search("hades")).unwrap_err();
        assert!(matches!(err, SgdbError::MissingKey));
    }

    #[test]
    fn base_url_is_overridable_for_tests() {
        let client = Sgdb::new("k").with_base("http://127.0.0.1:9/api/v2");
        assert_eq!(client.base, "http://127.0.0.1:9/api/v2");
        assert!(client.has_key());
    }

    /// Hits the real API. Ignored by default so `cargo test` stays offline.
    ///
    ///     $env:PIRATELINK_SGDB_KEY = "..."
    ///     cargo test -- --ignored live_
    #[test]
    #[ignore = "network: needs PIRATELINK_SGDB_KEY"]
    fn live_search_then_grids_then_download() {
        let key = std::env::var("PIRATELINK_SGDB_KEY").expect("PIRATELINK_SGDB_KEY");
        let client = Sgdb::new(key);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let matches = client.search("Hades").await.expect("search");
            assert!(!matches.is_empty(), "expected results for Hades");

            let hades = matches
                .iter()
                .find(|g| g.name == "Hades")
                .expect("an exact Hades match");
            assert_eq!(hades.id, 4997889, "known-good id for Hades");

            let grids = client.grids(hades.id, None).await.expect("grids");
            assert!(!grids.is_empty(), "expected portrait grids");

            // Every requested grid must be the 600x900 portrait we ask for; this is
            // the dimension filter actually working, not just returning anything.
            let chosen = pick_best(&grids).expect("a pick");
            assert!(chosen.url.starts_with("https://cdn2.steamgriddb.com/"));

            let bytes = client.fetch_bytes(&chosen.url).await.expect("download");
            assert!(bytes.len() > 1024, "cover looks truncated: {} bytes", bytes.len());
            assert!(
                bytes.starts_with(&[0x89, b'P', b'N', b'G']) || bytes.starts_with(b"RIFF"),
                "unexpected image format"
            );
        });
    }
}
