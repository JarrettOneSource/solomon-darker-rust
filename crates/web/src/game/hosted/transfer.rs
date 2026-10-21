//! `pages/Game.tsx` native transfer controller: browser files at the edge, one save writer.

use solomon_assets::native_save::{
    self as native_save, GameSaveImportPreview, PortableExportResult,
};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use super::*;

pub(super) enum TransferRequest {
    Inspect { request_id: u64, files: Vec<web_sys::File> },
    Export { request_id: u64, document: String },
}

pub(super) enum TransferCompletion {
    Inspected { request_id: u64, result: Result<Box<GameSaveImportPreview>, String> },
    Exported { request_id: u64, result: Result<Box<PortableExportResult>, String> },
}

impl TransferRequest {
    pub async fn execute(self) -> TransferCompletion {
        match self {
            Self::Inspect { request_id, files } => TransferCompletion::Inspected {
                request_id,
                result: inspect_files(files).await.map(Box::new),
            },
            Self::Export { request_id, document } => TransferCompletion::Exported {
                request_id,
                result: native_save::export_web_game_save_to_native_archive(&document)
                    .map(Box::new),
            },
        }
    }
}

async fn inspect_files(files: Vec<web_sys::File>) -> Result<GameSaveImportPreview, String> {
    let mut contents = Vec::with_capacity(files.len());
    for file in files {
        let name = file.name();
        let relative_path = js_sys::Reflect::get(&file, &JsValue::from_str("webkitRelativePath"))
            .map_err(|_| format!("The path for {name} could not be read."))?
            .as_string()
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| name.clone());
        let buffer = JsFuture::from(file.array_buffer())
            .await
            .map_err(|_| format!("The save file {name} could not be read."))?;
        contents.push((name, relative_path, js_sys::Uint8Array::new(&buffer).to_vec()));
    }
    let selection = contents
        .iter()
        .map(|(name, relative_path, bytes)| native_save::GameSaveInputFile {
            name,
            relative_path,
            bytes,
        })
        .collect::<Vec<_>>();
    native_save::read_game_save_file_selection(&selection)
}

impl Game {
    pub(super) fn hosted_transfer_action(
        &mut self,
        action: &HostedMenuAction,
    ) -> Result<bool, String> {
        let hosted = self.hosted.as_mut().unwrap();
        let request = match action {
            HostedMenuAction::InspectSaveFiles { request_id, files } => {
                TransferRequest::Inspect { request_id: *request_id, files: files.clone() }
            }
            HostedMenuAction::ExportSave { request_id } => {
                let Some(record) = &hosted.saves.record else {
                    self.menu.complete_save_export(
                        *request_id,
                        Err("Create or import a wizard before exporting.".into()),
                    )?;
                    return Ok(true);
                };
                TransferRequest::Export {
                    request_id: *request_id,
                    document: record.document.clone(),
                }
            }
            HostedMenuAction::ReplaceImportedSave { request_id, document } => {
                hosted.import_checkpoint =
                    Some((*request_id, hosted.saves.replace(document.clone())));
                return Ok(true);
            }
            _ => return Ok(false),
        };
        hosted.pending.push(HostedRequest::Transfer(request));
        Ok(true)
    }

    pub(super) fn complete_hosted_transfer(
        &mut self,
        completion: TransferCompletion,
    ) -> Result<(), String> {
        match completion {
            TransferCompletion::Inspected { request_id, result } => {
                self.menu.complete_save_inspection(request_id, result.map(|preview| *preview))?;
            }
            TransferCompletion::Exported { request_id, result } => {
                self.menu.complete_save_export(request_id, result.map(|exported| *exported))?;
            }
        }
        Ok(())
    }
}
