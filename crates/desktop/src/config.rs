//! Bundle paths and explicit development overrides; packaged play needs no shell.

use crate::updates::Build;
use std::{ffi::OsString, path::PathBuf};

pub struct Config {
    pub assets: PathBuf,
    pub profile: PathBuf,
    pub build: Build,
}

impl Config {
    pub fn load(arguments: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let executable_directory = executable.parent().ok_or("The application has no directory")?;
        #[cfg(target_os = "macos")]
        let root = if executable_directory.file_name().is_some_and(|name| name == "MacOS") {
            executable_directory
                .parent()
                .ok_or("The application bundle is incomplete")?
                .join("Resources")
        } else {
            executable_directory.to_owned()
        };
        #[cfg(not(target_os = "macos"))]
        let root = executable_directory.to_owned();
        let mut assets = std::env::var_os("SDR_DESKTOP_CLIENT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("game"));
        let mut profile = std::env::var_os("SDR_DESKTOP_USER_DATA").map(PathBuf::from);
        let mut revision = None;
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            let name = argument.to_str().ok_or("Invalid command-line option")?;
            match name {
                "--assets" => {
                    assets = PathBuf::from(arguments.next().ok_or("--assets needs a directory")?)
                }
                "--data-dir" => {
                    profile =
                        Some(PathBuf::from(arguments.next().ok_or("--data-dir needs a directory")?))
                }
                "--revision" => {
                    revision = Some(
                        arguments
                            .next()
                            .and_then(|value| value.into_string().ok())
                            .ok_or("--revision needs a build revision")?,
                    )
                }
                _ => return Err(format!("Unknown option: {name}")),
            }
        }
        let build = if let Some(revision) = revision {
            Build {
                revision,
                source_timestamp: 0,
                release_repository: "JarrettOneSource/solomon-darker-rust".into(),
            }
        } else {
            let text = match std::env::var("SDR_DESKTOP_BUILD_JSON") {
                Ok(value) => value,
                Err(std::env::VarError::NotPresent) => std::fs::read_to_string(root.join("desktop-build.json"))
                    .map_err(|error| format!("Cannot read the application build: {error}. For development pass --assets and --revision."))?,
                Err(error) => return Err(format!("Invalid build configuration: {error}")),
            };
            serde_json::from_str(&text)
                .map_err(|error| format!("Invalid application build: {error}"))?
        };
        build.validate()?;
        if !assets.join("assets/stock/College.bundle").is_file() {
            return Err(format!("The game assets are missing from {}.", assets.display()));
        }
        Ok(Self {
            assets: assets.canonicalize().map_err(|error| error.to_string())?,
            profile: match profile {
                Some(profile) => profile,
                None => crate::profile::default_directory()?,
            },
            build,
        })
    }
}

pub fn platform() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        os => os,
    }
}
pub fn architecture() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        arch => arch,
    }
}
