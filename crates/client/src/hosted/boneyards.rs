//! Owner-authenticated Boneyard draft operations of the Website API (`/api/boneyards`).
//! The same WebsiteApi transport owns mount, token, status and response handling.
use super::{ApiError, WebsiteApi};
use serde::Deserialize;
use serde_json::Value;

/// A cloud Boneyard draft's listing: ID, name, update time and sizes.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoneyardDraftSummary {
    pub id: u64,
    pub name: String,
    pub updated_at: String,
    pub document_size: u64,
    pub compiled_size: Option<u64>,
}
/// A complete cloud Boneyard draft: its summary, editor document, compiled Boneyard and
/// creation time.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoneyardDraft {
    #[serde(flatten)]
    pub summary: BoneyardDraftSummary,
    pub document: Value,
    pub compiled_boneyard: Option<String>,
    pub created_at: String,
}
impl WebsiteApi {
    pub async fn boneyard_drafts(&self) -> Result<Vec<BoneyardDraftSummary>, ApiError> {
        self.get("/api/boneyards").await
    }
    pub async fn boneyard_draft(&self, id: u64) -> Result<BoneyardDraft, ApiError> {
        self.get(&format!("/api/boneyards/{id}")).await
    }
    pub async fn remove_boneyard_draft(&self, id: u64) -> Result<(), ApiError> {
        self.send("DELETE", &format!("/api/boneyards/{id}"), None, None).await
    }
}
