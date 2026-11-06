//! Owner-authenticated editor draft operations from Website `lib/api.ts`.
//! The same WebsiteApi transport owns mount, token, status and response handling.
use super::{ApiError, WebsiteApi};
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoneyardDraftSummary {
    pub id: u64,
    pub name: String,
    pub updated_at: String,
    pub document_size: u64,
    pub compiled_size: Option<u64>,
}
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
