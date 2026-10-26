//! One application owns a profile. A second launch focuses it through loopback IPC.
//! The native documents live beside, and never overwrite, the previous Electron data.

use std::{
    fs::{File, OpenOptions, TryLockError},
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub struct Profile {
    pub root: PathBuf,
    pub documents: PathBuf,
    listener: TcpListener,
    _lock: File,
}

pub enum OpenProfile {
    Primary(Profile),
    AlreadyRunning,
}

pub fn default_directory() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    let root = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let root = std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join("Library/Application Support"));
    #[cfg(all(unix, not(target_os = "macos")))]
    let root = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));
    root.map(|root| root.join("Solomon Darker"))
        .ok_or_else(|| "The operating system did not provide an application data directory.".into())
}

impl Profile {
    pub fn has_previous_app_data(&self) -> Result<bool, String> {
        if self.documents.join("game-save.json").try_exists().map_err(|error| error.to_string())? {
            return Ok(false);
        }
        let entries = match std::fs::read_dir(self.root.join("IndexedDB")) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(format!("Cannot inspect previous saved-game data: {error}")),
        };
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            if entry.file_name().to_string_lossy().ends_with(".indexeddb.leveldb") {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn open(root: &Path) -> Result<OpenProfile, String> {
        std::fs::create_dir_all(root)
            .map_err(|error| format!("Cannot create the save directory: {error}"))?;
        let root = root.canonicalize().map_err(|error| error.to_string())?;
        let mut options = OpenOptions::new();
        options.create(true).truncate(false).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options
            .open(root.join("native-instance.lock"))
            .map_err(|error| format!("Cannot open the application lock: {error}"))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                focus_existing(&root.join("native-instance.port"))?;
                return Ok(OpenProfile::AlreadyRunning);
            }
            Err(TryLockError::Error(error)) => {
                return Err(format!("Cannot lock the save directory: {error}"));
            }
        }
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .map_err(|error| format!("Cannot open the application focus channel: {error}"))?;
        listener.set_nonblocking(true).map_err(|error| error.to_string())?;
        // Windows exclusive file locks deny reads through another handle. Keep
        // discoverable endpoint metadata separate from the lifetime lock.
        let mut endpoint = options
            .truncate(true)
            .open(root.join("native-instance.port"))
            .map_err(|error| format!("Cannot publish the application focus channel: {error}"))?;
        writeln!(endpoint, "{}", listener.local_addr().map_err(|error| error.to_string())?.port())
            .map_err(|error| error.to_string())?;
        endpoint.sync_all().map_err(|error| error.to_string())?;
        let documents = root.join("native-v1");
        Ok(OpenProfile::Primary(Self { root, documents, listener, _lock: lock }))
    }

    pub fn focus_requested(&self) -> Result<bool, String> {
        let mut requested = false;
        loop {
            match self.listener.accept() {
                Ok(_) => requested = true,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    return Ok(requested);
                }
                Err(error) => return Err(format!("Application focus channel: {error}")),
            }
        }
    }
}

fn focus_existing(endpoint: &Path) -> Result<(), String> {
    // A concurrent first launch can hold its lock just before publishing its port.
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let mut port = String::new();
        match File::open(endpoint) {
            Ok(file) => {
                file.take(32).read_to_string(&mut port).map_err(|error| error.to_string())?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!("Cannot read the application focus channel: {error}"));
            }
        }
        if let Ok(port) = port.trim().parse::<u16>() {
            let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
            if TcpStream::connect_timeout(&address, Duration::from_millis(100)).is_ok() {
                return Ok(());
            }
        }
        if Instant::now() >= deadline {
            return Err("Solomon Darker is already using this save directory, but its window could not be focused.".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_second_launch_focuses_the_owner_and_a_closed_profile_can_be_reopened() {
        let path = std::env::temp_dir().join(format!(
            "solomon-profile-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let OpenProfile::Primary(profile) = Profile::open(&path).unwrap() else {
            panic!("first owner");
        };
        assert!(!profile.focus_requested().unwrap());
        assert!(!profile.has_previous_app_data().unwrap());
        let previous = path.join("IndexedDB/solomon-darker_app_0.indexeddb.leveldb");
        std::fs::create_dir_all(&previous).unwrap();
        std::fs::write(previous.join("CURRENT"), b"previous data must remain untouched").unwrap();
        assert!(profile.has_previous_app_data().unwrap());
        std::fs::create_dir_all(&profile.documents).unwrap();
        std::fs::write(profile.documents.join("game-save.json"), b"native slot").unwrap();
        assert!(!profile.has_previous_app_data().unwrap());
        assert_eq!(
            std::fs::read(previous.join("CURRENT")).unwrap(),
            b"previous data must remain untouched"
        );
        assert!(matches!(Profile::open(&path).unwrap(), OpenProfile::AlreadyRunning));
        let deadline = Instant::now() + Duration::from_secs(1);
        while !profile.focus_requested().unwrap() {
            assert!(Instant::now() < deadline, "the second launch did not request focus");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!profile.focus_requested().unwrap());
        drop(profile);
        assert!(matches!(Profile::open(&path).unwrap(), OpenProfile::Primary(_)));
        std::fs::remove_dir_all(path).unwrap();
    }
}
