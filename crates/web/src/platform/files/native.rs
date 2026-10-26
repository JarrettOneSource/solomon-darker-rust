use std::cell::{Cell, RefCell};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::rc::Rc;

use crate::platform::Surface;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedFile(PathBuf);

impl SelectedFile {
    pub fn name(&self) -> String {
        self.0.file_name().unwrap_or_default().to_string_lossy().into_owned()
    }

    pub fn relative_path(&self) -> Result<String, String> {
        Ok(self.name())
    }

    pub async fn read(&self, maximum_bytes: usize) -> Result<Vec<u8>, String> {
        let path = self.0.clone();
        blocking::unblock(move || {
            let file = std::fs::File::open(&path)
                .map_err(|error| format!("Cannot open {}: {error}", path.display()))?;
            let mut bytes = Vec::new();
            file.take(maximum_bytes as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|error| format!("Cannot read {}: {error}", path.display()))?;
            if bytes.len() > maximum_bytes {
                return Err(format!("{} exceeds the supported file size", path.display()));
            }
            Ok(bytes)
        })
        .await
    }
}

pub struct FilePicker {
    surface: Surface,
    extensions: Vec<String>,
    multiple: bool,
    active: Rc<Cell<bool>>,
    selected: Rc<RefCell<Vec<Vec<SelectedFile>>>>,
}

impl FilePicker {
    pub fn new(surface: &Surface, extensions: &[&str], multiple: bool) -> Result<Self, String> {
        Ok(Self {
            surface: surface.clone(),
            extensions: extensions.iter().map(|value| (*value).to_owned()).collect(),
            multiple,
            active: Rc::new(Cell::new(false)),
            selected: Rc::new(RefCell::new(Vec::new())),
        })
    }

    pub fn choose(&self) -> Result<(), String> {
        if self.active.replace(true) {
            return Ok(());
        }
        let window = self.surface.window().clone();
        let dialog = rfd::AsyncFileDialog::new()
            .set_parent(window.as_ref())
            .add_filter("Supported files", &self.extensions);
        let active = Rc::downgrade(&self.active);
        let selected = Rc::downgrade(&self.selected);
        let multiple = self.multiple;
        crate::platform::spawn_local(async move {
            let files = if multiple {
                dialog.pick_files().await
            } else {
                dialog.pick_file().await.map(|file| vec![file])
            };
            if let Some(selected) = selected.upgrade()
                && let Some(files) = files
                && !files.is_empty()
            {
                selected.borrow_mut().push(
                    files.into_iter().map(|file| SelectedFile(file.path().to_owned())).collect(),
                );
            }
            if let Some(active) = active.upgrade() {
                active.set(false);
            }
            drop(window);
        });
        Ok(())
    }

    pub fn take(&self) -> Vec<Vec<SelectedFile>> {
        self.selected.take()
    }
}

pub async fn save_file(
    surface: &Surface,
    name: &str,
    bytes: Vec<u8>,
    _mime: &str,
) -> Result<bool, String> {
    let window = surface.window().clone();
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(window.as_ref())
        .set_file_name(name)
        .save_file()
        .await
    else {
        return Ok(false);
    };
    let path = file.path().to_owned();
    blocking::unblock(move || {
        let mut file = std::fs::File::create(&path)
            .map_err(|error| format!("Cannot create {}: {error}", path.display()))?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("Cannot save {}: {error}", path.display()))?;
        Ok(true)
    })
    .await
}
