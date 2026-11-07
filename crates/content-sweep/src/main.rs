//! `solomon-content-sweep` checks that the mods and Boneyards published on the Website load in
//! the Rust game. It reads mod packages as the Website stores them, builds the session content
//! the Website would provision for a player who enabled each one, and runs that content through
//! the hosted authority's admission, Lua compilation, College preparation, Boneyard loading and
//! ten seconds of play in each Boneyard. With `--api` it reads the Website's public mod listing and
//! sweeps the version each listed mod would load, matched to a package file by SHA-256.
//!
//! The JSON summary on stdout counts outcomes per kind and identifies every failure by id,
//! version and package SHA-256, with the step that failed and its error. It never contains
//! package contents. The exit status is 0 when every item loaded, 1 when any item failed or
//! had no package, and 2 when the sweep itself could not run.

mod catalog;
mod package;
mod play;

use std::any::Any;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use serde_json::{Value, json};

use catalog::PublishedMod;
use package::{Package, PackageFile};

const USAGE: &str = "\
usage: solomon-content-sweep --assets DIRECTORY [--packages DIRECTORY] [--api URL]
                             [--ticks COUNT] [--delay-ms MILLISECONDS]

  --assets    the client directory the host serves, which holds assets/navigation
  --packages  mod packages (.zip or .sdmod), searched recursively
  --api       a Website mount such as https://solomondarker.com/game-rust/; sweeps the
              version each public mod loads, matched to --packages by SHA-256
  --ticks     session ticks played in each Boneyard, 100 a second (default 1000)
  --delay-ms  pause between Website requests (default 500)";

/// The step a package failed at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Reading the archive and building the Website's session content row.
    Package,
    /// The archive's content digest differs from the one the Website published for it.
    Identity,
    /// The hosted authority's validation of the session content.
    Admission,
    /// Compiling each mod's Lua definition.
    Definitions,
    /// Compiling assets and the content catalog, and starting the mods' Lua runtime.
    Preparation,
    /// Materializing a Boneyard choice and entering it.
    Boneyard,
    /// Ticking the session and the mods' rules, then projecting and saving mod state.
    Session,
}

impl Stage {
    fn name(self) -> &'static str {
        match self {
            Self::Package => "package",
            Self::Identity => "identity",
            Self::Admission => "admission",
            Self::Definitions => "definitions",
            Self::Preparation => "preparation",
            Self::Boneyard => "boneyard",
            Self::Session => "session",
        }
    }
}

/// A published Boneyard is a package with Boneyard overlays and no Lua; everything else is a
/// mod, including Lua mods that bring their own Boneyards.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Mod,
    Boneyard,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Mod => "mod",
            Self::Boneyard => "boneyard",
        }
    }
}

struct Options {
    assets: PathBuf,
    packages: Option<PathBuf>,
    api: Option<String>,
    ticks: u64,
    delay: Duration,
}

/// One package or published mod and what happened to it.
struct Item {
    kind: Kind,
    id: Option<String>,
    slug: String,
    version: Option<String>,
    sha256: Option<String>,
    content_sha256: Option<String>,
    outcome: Outcome,
}

enum Outcome {
    Ok(Vec<play::Play>),
    Failed {
        stage: Stage,
        error: String,
    },
    /// A published mod the sweep has no package for, with the reason.
    Unavailable(String),
}

#[derive(Default)]
struct Summary {
    items: Vec<Item>,
    /// With `--api`, the number of public mods listed.
    published: Option<usize>,
    /// With `--api`, packages that are not the version any listed mod loads.
    ignored: Vec<(String, String)>,
}

#[derive(Default)]
struct Counts {
    ok: u64,
    failed: u64,
    unavailable: u64,
}

fn main() -> ExitCode {
    let options = match parse_options(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match sweep(&options) {
        Ok(summary) => {
            let document = summary.document(&options);
            println!("{}", serde_json::to_string_pretty(&document).expect("the summary is JSON"));
            if summary.all_loaded() { ExitCode::SUCCESS } else { ExitCode::from(1) }
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

fn parse_options(mut arguments: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut assets = None;
    let mut packages = None;
    let mut api = None;
    let mut ticks = 1_000;
    let mut delay_ms = 500;
    while let Some(flag) = arguments.next() {
        let value = arguments.next().ok_or_else(|| format!("{flag} needs a value."))?;
        match flag.as_str() {
            "--assets" => assets = Some(PathBuf::from(value)),
            "--packages" => packages = Some(PathBuf::from(value)),
            "--api" => api = Some(website_base(&value)?),
            "--ticks" => ticks = value.parse().map_err(|_| "--ticks must be a whole number.")?,
            "--delay-ms" => {
                delay_ms = value.parse().map_err(|_| "--delay-ms must be a whole number.")?;
            }
            _ => return Err(format!("Unknown argument: {flag}")),
        }
    }
    let assets: PathBuf = assets.ok_or("--assets is required.")?;
    if !assets.join("assets/navigation").is_dir() {
        return Err(format!("{} has no assets/navigation directory.", assets.display()));
    }
    if packages.is_none() && api.is_none() {
        return Err("Give --packages, --api or both.".into());
    }
    Ok(Options { assets, packages, api, ticks, delay: Duration::from_millis(delay_ms) })
}

/// A Website mount as a base URL ending in `/`, so `api/mods` resolves inside it.
fn website_base(url: &str) -> Result<String, String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err(format!("--api must be an http or https URL: {url}"));
    }
    Ok(if url.ends_with('/') { url.to_owned() } else { format!("{url}/") })
}

fn sweep(options: &Options) -> Result<Summary, String> {
    let packages = match &options.packages {
        Some(directory) => package::find_packages(directory)?,
        None => Vec::new(),
    };
    let mut summary = Summary::default();
    let Some(base) = &options.api else {
        for file in &packages {
            summary.items.push(sweep_package(file, None, options));
        }
        return Ok(summary);
    };
    let published = catalog::published_mods(base, options.delay)?;
    summary.published = Some(published.len());
    let mut matched = vec![false; packages.len()];
    for listed in &published {
        let local = listed.latest.as_ref().ok().and_then(|latest| {
            packages.iter().position(|file| file.sha256 == latest.package_sha256)
        });
        match local {
            Some(index) => {
                matched[index] = true;
                summary.items.push(sweep_package(&packages[index], Some(listed), options));
            }
            None => summary.items.push(unavailable(listed, options)),
        }
    }
    for (file, matched) in packages.iter().zip(matched) {
        if !matched {
            summary.ignored.push((file.relative_path.clone(), file.sha256.clone()));
        }
    }
    Ok(summary)
}

fn sweep_package(file: &PackageFile, listed: Option<&PublishedMod>, options: &Options) -> Item {
    let package = Package::read(&file.bytes);
    let readable = package.as_ref().ok();
    let kind = match (readable, listed) {
        (Some(package), _) if package.is_boneyard_publication() => Kind::Boneyard,
        (None, Some(listed)) if listed.boneyard_tag => Kind::Boneyard,
        _ => Kind::Mod,
    };
    let version = readable.and_then(Package::version).map(str::to_owned).or_else(|| {
        listed.and_then(|listed| listed.latest.as_ref().ok()).map(|latest| latest.version.clone())
    });
    let slug = match listed {
        Some(listed) => listed.slug.clone(),
        None => local_slug(file, version.as_deref()),
    };
    let outcome = match &package {
        Ok(package) => play(package, &slug, listed, options),
        Err(error) => Outcome::Failed { stage: Stage::Package, error: error.clone() },
    };
    Item {
        kind,
        id: readable
            .and_then(Package::id)
            .map(str::to_owned)
            .or_else(|| listed.and_then(|listed| listed.package_id.clone())),
        slug,
        version,
        sha256: Some(file.sha256.clone()),
        content_sha256: readable.map(|package| package.content_sha256.clone()),
        outcome,
    }
}

fn play(
    package: &Package,
    slug: &str,
    listed: Option<&PublishedMod>,
    options: &Options,
) -> Outcome {
    if let Some(published) = listed.and_then(|listed| listed.latest.as_ref().ok())
        && package.content_sha256 != published.content_sha256
    {
        return Outcome::Failed {
            stage: Stage::Identity,
            error: format!(
                "The package's content digest {} differs from the published {}.",
                package.content_sha256, published.content_sha256
            ),
        };
    }
    let name = listed
        .map(|listed| listed.name.as_str())
        .or_else(|| package.name())
        .or_else(|| package.id())
        .unwrap_or("Unnamed mod");
    let content = match package.session_content(slug, name) {
        Ok(content) => content,
        Err(error) => return Outcome::Failed { stage: Stage::Package, error },
    };
    // A panic inside the game is a failure of this package, not of the sweep.
    let stage = Cell::new(Stage::Admission);
    let played = catch_unwind(AssertUnwindSafe(|| {
        play::play_package(&content, &options.assets, options.ticks, &stage)
    }));
    match played {
        Ok(Ok(plays)) => Outcome::Ok(plays),
        Ok(Err(error)) => Outcome::Failed { stage: stage.get(), error },
        Err(panic) => Outcome::Failed {
            stage: stage.get(),
            error: format!("The game panicked: {}", panic_message(panic.as_ref())),
        },
    }
}

fn unavailable(listed: &PublishedMod, options: &Options) -> Item {
    let latest = listed.latest.as_ref().ok();
    let reason = match (&listed.latest, &options.packages) {
        (Err(reason), _) => reason.clone(),
        (Ok(_), None) => "No --packages directory was given.".into(),
        (Ok(_), Some(_)) => "No package under --packages has the published SHA-256.".into(),
    };
    Item {
        kind: if listed.boneyard_tag { Kind::Boneyard } else { Kind::Mod },
        id: listed.package_id.clone(),
        slug: listed.slug.clone(),
        version: latest.map(|latest| latest.version.clone()),
        sha256: latest.map(|latest| latest.package_sha256.clone()),
        content_sha256: latest.map(|latest| latest.content_sha256.clone()),
        outcome: Outcome::Unavailable(reason),
    }
}

/// The Website stores each version at `SLUG/VERSION.zip`; another file is named by its stem.
fn local_slug(file: &PackageFile, version: Option<&str>) -> String {
    let path = Path::new(&file.relative_path);
    let stem = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("package");
    let parent = path.parent().and_then(Path::file_name).and_then(|parent| parent.to_str());
    match parent {
        Some(parent) if version == Some(stem) => parent.to_owned(),
        _ => stem.to_owned(),
    }
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "no message".into()
    }
}

impl Summary {
    fn all_loaded(&self) -> bool {
        self.items.iter().all(|item| matches!(item.outcome, Outcome::Ok(_)))
    }

    fn document(&self, options: &Options) -> Value {
        let mut counts =
            BTreeMap::from([("boneyard", Counts::default()), ("mod", Counts::default())]);
        let mut ok = Vec::new();
        let mut failed = Vec::new();
        let mut unavailable = Vec::new();
        for item in &self.items {
            let mut row = json!({
                "kind": item.kind.name(),
                "id": item.id,
                "slug": item.slug,
                "version": item.version,
                "sha256": item.sha256,
                "contentSha256": item.content_sha256,
            });
            let count = counts.get_mut(item.kind.name()).expect("both kinds are counted");
            match &item.outcome {
                Outcome::Ok(plays) => {
                    count.ok += 1;
                    row["boneyards"] = plays
                        .iter()
                        .map(|play| {
                            json!({
                                "choiceId": play.choice_id,
                                "name": play.name,
                                "source": if play.stock { "stock" } else { "mod" },
                                "sourceSha256": play.source_sha256,
                                "ticks": play.ticks,
                            })
                        })
                        .collect();
                    ok.push(row);
                }
                Outcome::Failed { stage, error } => {
                    count.failed += 1;
                    row["stage"] = json!(stage.name());
                    row["error"] = json!(error);
                    failed.push(row);
                }
                Outcome::Unavailable(reason) => {
                    count.unavailable += 1;
                    row["reason"] = json!(reason);
                    unavailable.push(row);
                }
            }
        }
        let counts: BTreeMap<&str, Value> = counts
            .into_iter()
            .map(|(kind, count)| {
                (kind, json!({ "ok": count.ok, "failed": count.failed, "unavailable": count.unavailable }))
            })
            .collect();
        let ignored: Vec<Value> = self
            .ignored
            .iter()
            .map(|(path, sha256)| json!({ "path": path, "sha256": sha256 }))
            .collect();
        json!({
            "api": options.api,
            "packages": options.packages.as_ref().map(|directory| directory.display().to_string()),
            "published": self.published,
            "ticksPerBoneyard": options.ticks,
            "counts": counts,
            "ok": ok,
            "failed": failed,
            "unavailable": unavailable,
            "ignoredPackages": ignored,
        })
    }
}
