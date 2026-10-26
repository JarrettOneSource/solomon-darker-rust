//! `pages/Game.tsx` native transfer controller: browser files at the edge, one save writer.

use crate::platform::files::SelectedFile;
use solomon_assets::native_save::{
    self as native_save, GameSaveImportPreview, PortableExportResult,
};

use super::*;

pub(super) enum TransferRequest {
    Inspect { request_id: u64, files: Vec<SelectedFile> },
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

async fn inspect_files(files: Vec<SelectedFile>) -> Result<GameSaveImportPreview, String> {
    let mut contents = Vec::with_capacity(files.len());
    for file in files {
        let name = file.name();
        let relative_path = file.relative_path()?;
        let bytes = file.read(solomon_assets::website_save::MAX_SAVE_BYTES).await?;
        contents.push((name, relative_path, bytes));
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
