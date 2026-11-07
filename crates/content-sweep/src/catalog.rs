//! The Website's published mod listing, read with the anonymous requests a signed-out Dark
//! Cloud visitor makes: `GET api/mods` page by page, as the game's own catalog loads it, then
//! `GET api/mods/SLUG` for each mod's versions. Only public mods are listed; unlisted and
//! private ones need their slug or their author's account.
//!
//! The requests carry no credentials or cookies, run one at a time, and wait between each
//! other so a sweep of production stays light.

use std::thread;
use std::time::Duration;

use serde::Deserialize;
use serde::de::DeserializeOwned;

/// Identifies the sweep in the Website's request logs.
const USER_AGENT: &str = "solomon-darker-content-sweep";
const PAGE_SIZE: u64 = 50;

/// One public mod and the version a session would load.
pub struct PublishedMod {
    pub slug: String,
    pub name: String,
    pub package_id: Option<String>,
    /// The Boneyard editor tags every Boneyard it publishes `boneyard`.
    pub boneyard_tag: bool,
    /// The newest version with a package, or why there is none.
    pub latest: Result<PublishedVersion, String>,
}

/// A published version's identity: the Website's version label and both package digests.
pub struct PublishedVersion {
    pub version: String,
    pub package_sha256: String,
    pub content_sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModPage {
    items: Vec<ModSummary>,
    total: u64,
    page_size: u64,
}

#[derive(Deserialize)]
struct ModSummary {
    slug: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModDetail {
    name: String,
    package_id: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    versions: Vec<ModVersion>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModVersion {
    manifest_version: Option<String>,
    package_sha256: Option<String>,
    content_sha256: Option<String>,
}

/// Every public mod listed under `base`, a Website mount such as
/// `https://solomondarker.com/game-rust/`, with a trailing slash.
pub fn published_mods(base: &str, delay: Duration) -> Result<Vec<PublishedMod>, String> {
    let mut website = Website { agent: agent(), delay, requests: 0 };
    let first: ModPage = website.get(&listing_url(base, 1))?;
    if first.page_size == 0 {
        return Err("The mod listing reports a page size of zero.".into());
    }
    let pages = first.total.div_ceil(first.page_size);
    let mut slugs: Vec<String> = first.items.into_iter().map(|item| item.slug).collect();
    for page in 2..=pages {
        let listing: ModPage = website.get(&listing_url(base, page))?;
        slugs.extend(listing.items.into_iter().map(|item| item.slug));
    }
    let mut published = Vec::with_capacity(slugs.len());
    for slug in slugs {
        published.push(website.published_mod(base, slug));
    }
    Ok(published)
}

fn listing_url(base: &str, page: u64) -> String {
    format!("{base}api/mods?sort=newest&pageSize={PAGE_SIZE}&page={page}")
}

struct Website {
    agent: ureq::Agent,
    delay: Duration,
    requests: u32,
}

impl Website {
    /// A mod whose details cannot be read stays in the sweep as unavailable, with the reason.
    fn published_mod(&mut self, base: &str, slug: String) -> PublishedMod {
        let detail =
            if slug.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte)) {
                self.get::<ModDetail>(&format!("{base}api/mods/{slug}"))
            } else {
                Err(format!("The listing returned an unexpected slug: {slug}"))
            };
        match detail {
            Ok(detail) => PublishedMod {
                boneyard_tag: detail.tags.iter().any(|tag| tag == "boneyard"),
                latest: latest_version(&detail.versions),
                name: detail.name,
                package_id: detail.package_id,
                slug,
            },
            Err(error) => PublishedMod {
                name: slug.clone(),
                slug,
                package_id: None,
                boneyard_tag: false,
                latest: Err(error),
            },
        }
    }

    fn get<T: DeserializeOwned>(&mut self, url: &str) -> Result<T, String> {
        if self.requests > 0 {
            thread::sleep(self.delay);
        }
        self.requests += 1;
        let mut response = self
            .agent
            .get(url)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/json")
            .call()
            .map_err(|error| format!("GET {url} failed: {error}"))?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("GET {url} returned {status}"));
        }
        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|error| format!("GET {url} could not be read: {error}"))?;
        serde_json::from_str(&body)
            .map_err(|error| format!("GET {url} is not the expected JSON: {error}"))
    }
}

/// The version the Website's content service loads: the newest with a manifest version and
/// both digests. The detail lists versions newest first.
fn latest_version(versions: &[ModVersion]) -> Result<PublishedVersion, String> {
    for version in versions {
        let (Some(label), Some(package_sha256), Some(content_sha256)) =
            (&version.manifest_version, &version.package_sha256, &version.content_sha256)
        else {
            continue;
        };
        if label.is_empty() || package_sha256.is_empty() || content_sha256.is_empty() {
            continue;
        }
        return Ok(PublishedVersion {
            version: label.clone(),
            package_sha256: package_sha256.to_lowercase(),
            content_sha256: content_sha256.to_lowercase(),
        });
    }
    Err("The Website lists no runnable package for this mod.".into())
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(60)))
        .build()
        .into()
}
