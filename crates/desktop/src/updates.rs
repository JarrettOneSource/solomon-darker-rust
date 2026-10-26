//! Published desktop updates, preserving the release owner and timestamp checks.
//! Download opens the qualified archive in the system browser; updates never gate play.

use serde::Deserialize;
use std::time::Duration;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Build {
    pub revision: String,
    pub source_timestamp: u64,
    pub release_repository: String,
}

impl Build {
    pub fn validate(&self) -> Result<(), String> {
        if self.revision != "development" && !revision(&self.revision) {
            return Err("The application build revision is invalid.".into());
        }
        let parts: Vec<_> = self.release_repository.split('/').collect();
        if parts.len() != 2
            || parts.iter().any(|part| {
                part.is_empty()
                    || !part
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
            })
        {
            return Err("The application release repository is invalid.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvailableUpdate {
    pub revision: String,
    pub url: String,
}

fn revision(value: &str) -> bool {
    value.len() == 40
        && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u32,
    revision: String,
    source_timestamp: u64,
}

fn candidate(
    build: &Build,
    platform: &str,
    arch: &str,
    release: Release,
) -> Option<AvailableUpdate> {
    let next = release.tag_name.strip_prefix("desktop-")?;
    if !revision(next) || release.draft || release.prerelease || next == build.revision {
        return None;
    }
    let base =
        format!("https://github.com/{}/releases/download/desktop-{next}", build.release_repository);
    let filename = format!("Solomon-Darker-{platform}-{arch}.zip");
    let url = format!("{base}/{filename}");
    for (name, url) in [
        ("desktop-release.json", format!("{base}/desktop-release.json")),
        (filename.as_str(), url.clone()),
    ] {
        if !release
            .assets
            .iter()
            .any(|asset| asset.name == name && asset.browser_download_url == url)
        {
            return None;
        }
    }
    Some(AvailableUpdate { revision: next.into(), url })
}

pub fn check(build: &Build, platform: &str, arch: &str) -> Result<Option<AvailableUpdate>, String> {
    build.validate()?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .http_status_as_error(false)
        .build()
        .into();
    let url = format!("https://api.github.com/repos/{}/releases/latest", build.release_repository);
    let mut response = agent
        .get(&url)
        .header("accept", "application/json")
        .header("user-agent", "Solomon-Darker-native")
        .call()
        .map_err(|error| error.to_string())?;
    if response.status() == 404 {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(format!("Update check failed (HTTP {}).", response.status()));
    }
    let body = response
        .body_mut()
        .with_config()
        .limit(4 * 1024 * 1024)
        .read_to_string()
        .map_err(|error| error.to_string())?;
    let release = match serde_json::from_str(&body) {
        Ok(release) => release,
        Err(_) => return Ok(None),
    };
    let Some(update) = candidate(build, platform, arch, release) else { return Ok(None) };
    let manifest_url = format!(
        "https://github.com/{}/releases/download/desktop-{}/desktop-release.json",
        build.release_repository, update.revision
    );
    let mut response = agent
        .get(&manifest_url)
        .header("accept", "application/json")
        .header("user-agent", "Solomon-Darker-native")
        .call()
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("Update manifest failed (HTTP {}).", response.status()));
    }
    let body = response
        .body_mut()
        .with_config()
        .limit(1024 * 1024)
        .read_to_string()
        .map_err(|error| error.to_string())?;
    let manifest: Manifest = match serde_json::from_str(&body) {
        Ok(manifest) => manifest,
        Err(_) => return Ok(None),
    };
    Ok((manifest.schema_version == 1
        && manifest.revision == update.revision
        && manifest.source_timestamp <= 9_007_199_254_740_991
        && manifest.source_timestamp > build.source_timestamp)
        .then_some(update))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn build() -> Build {
        Build {
            revision: "a".repeat(40),
            source_timestamp: 100,
            release_repository: "JarrettOneSource/solomon-darker-rust".into(),
        }
    }
    fn release(build: &Build) -> Release {
        let revision = "b".repeat(40);
        let base = format!(
            "https://github.com/{}/releases/download/desktop-{revision}",
            build.release_repository
        );
        Release {
            tag_name: format!("desktop-{revision}"),
            draft: false,
            prerelease: false,
            assets: ["desktop-release.json", "Solomon-Darker-darwin-arm64.zip"]
                .into_iter()
                .map(|name| Asset {
                    name: name.into(),
                    browser_download_url: format!("{base}/{name}"),
                })
                .collect(),
        }
    }
    #[test]
    fn only_the_owned_published_platform_archive_is_an_update_candidate() {
        let build = build();
        assert!(candidate(&build, "darwin", "arm64", release(&build)).is_some());
        assert!(candidate(&build, "win32", "x64", release(&build)).is_none());
        for variant in 0..5 {
            let mut release = release(&build);
            match variant {
                0 => release.draft = true,
                1 => release.prerelease = true,
                2 => release.tag_name = format!("desktop-{}", build.revision),
                3 => {
                    release.assets[0].browser_download_url =
                        "https://other.test/manifest.json".into()
                }
                _ => release.assets[1].browser_download_url = "https://other.test/game.zip".into(),
            }
            assert!(candidate(&build, "darwin", "arm64", release).is_none());
        }
    }
}
