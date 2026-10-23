//! Account, content and admission I/O from Website `lib/api.ts` and `game-bootstrap.ts`.
//! The Website remains the control plane; this adapter never manufactures a game ticket,
//! score receipt or authoritative checkpoint.

mod boneyards;
mod types;
pub use boneyards::{BoneyardDraft, BoneyardDraftSummary};
pub use types::*;

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use solomon_sim::hall_of_fame::HallOfFameBoard;
use solomon_sim::social::GameSessionKind;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, Response, Window};

const TOKEN_KEY: &str = "sdr.token";

/// Deployment-owned mount. A staging page uses its own API and storage namespace while
/// the packaged launcher and the ordinary static preview keep their existing local mode.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WebsiteMount {
    prefix: String,
}

impl WebsiteMount {
    pub fn new(prefix: &str) -> Result<Self, String> {
        let prefix = prefix.trim_end_matches('/');
        if (!prefix.is_empty() && !prefix.starts_with('/'))
            || prefix.starts_with("//")
            || prefix.contains(['?', '#', '\\'])
            || prefix.split('/').any(|part| matches!(part, "." | ".."))
        {
            return Err("The game has an invalid Website mount".into());
        }
        Ok(Self { prefix: prefix.to_owned() })
    }

    pub fn configured(window: &Window) -> Result<Option<Self>, String> {
        let document = window.document().ok_or("The game has no document")?;
        let Some(configuration) = document
            .query_selector("meta[name='solomon-website-mount']")
            .map_err(|_| "Could not read the Website configuration")?
        else {
            return Ok(None);
        };
        Self::new(&configuration.get_attribute("content").unwrap_or_default()).map(Some)
    }

    #[must_use]
    pub fn path(&self, path: &str) -> String {
        format!("{}{path}", self.prefix)
    }

    #[must_use]
    pub fn storage_key(&self, key: &str) -> String {
        if self.prefix.is_empty() { key.into() } else { format!("{key}:{}", self.prefix) }
    }

    /// C# builds root-absolute supervisor URLs. A staging deployment routes those same
    /// paths below its mount; an explicitly separate endpoint origin keeps its own path.
    pub fn endpoint_url(&self, window: &Window, endpoint: &str) -> Result<String, String> {
        let url = web_sys::Url::new(endpoint)
            .map_err(|_| "The game session provisioner returned an invalid endpoint")?;
        if url.protocol() != "wss:" || !url.username().is_empty() || !url.password().is_empty() {
            return Err("The game session provisioner returned an invalid endpoint".into());
        }
        let host = window.location().host().map_err(|_| "Could not read the game host")?;
        if url.host() == host && !self.prefix.is_empty() {
            let path = url.pathname();
            if !path.starts_with(&format!("{}/", self.prefix)) {
                url.set_pathname(&self.path(&path));
            }
        }
        Ok(url.href())
    }
}

#[derive(Debug)]
pub struct ApiError {
    pub status: u16,
    pub message: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ApiError {}

impl From<String> for ApiError {
    fn from(message: String) -> Self {
        Self { status: 0, message }
    }
}

#[derive(Clone)]
pub struct WebsiteApi {
    window: Window,
    mount: WebsiteMount,
}

/// A gallery image retains its browser Blob URL until its owning view is removed.
pub struct OwnedImageUrl(String);

impl OwnedImageUrl {
    #[must_use]
    pub fn url(&self) -> &str {
        &self.0
    }
}

impl Drop for OwnedImageUrl {
    fn drop(&mut self) {
        let _ = web_sys::Url::revoke_object_url(&self.0);
    }
}

#[derive(Default)]
pub struct ModQuery {
    pub mine: bool,
    pub search: String,
    pub tags: Vec<String>,
    pub sort: Option<ModSort>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

pub enum BrowserAdmission<'a> {
    GlobalHub,
    PrivateCollege,
    Party { intent_id: &'a str },
    Resume { party_rejoin_token: Option<&'a str>, document: &'a str, shared_hub: bool },
}

impl WebsiteApi {
    #[must_use]
    pub fn new(window: Window, mount: WebsiteMount) -> Self {
        Self { window, mount }
    }

    #[must_use]
    pub fn mount(&self) -> &WebsiteMount {
        &self.mount
    }

    pub fn token(&self) -> Result<Option<String>, ApiError> {
        self.window
            .local_storage()
            .map_err(|_| String::from("The browser blocked account storage"))?
            .ok_or_else(|| String::from("The browser has no account storage"))?
            .get_item(&self.mount.storage_key(TOKEN_KEY))
            .map_err(|_| String::from("Could not read the account sign-in").into())
    }

    pub fn logout(&self) -> Result<(), ApiError> {
        self.set_token(None)
    }

    fn set_token(&self, token: Option<&str>) -> Result<(), ApiError> {
        let storage = self
            .window
            .local_storage()
            .map_err(|_| String::from("The browser blocked account storage"))?
            .ok_or_else(|| String::from("The browser has no account storage"))?;
        match token {
            Some(token) => storage.set_item(&self.mount.storage_key(TOKEN_KEY), token),
            None => storage.remove_item(&self.mount.storage_key(TOKEN_KEY)),
        }
        .map_err(|_| String::from("Could not save the account sign-in").into())
    }

    pub async fn account(&self) -> Result<Option<Account>, ApiError> {
        if self.token()?.is_none() {
            return Ok(None);
        }
        let response: AccountResponse = self.get("/api/auth/me").await?;
        Ok(Some(response.user))
    }

    pub async fn login(
        &self,
        username_or_email: &str,
        password: &str,
    ) -> Result<Account, ApiError> {
        let response: AuthResponse = self
            .send(
                "POST",
                "/api/auth/login",
                Some(json!({"usernameOrEmail": username_or_email, "password": password})),
                None,
            )
            .await?;
        self.set_token(Some(&response.token))?;
        Ok(response.user)
    }

    pub async fn register(
        &self,
        username: &str,
        email: &str,
        password: &str,
    ) -> Result<Account, ApiError> {
        let response: AuthResponse = self
            .send(
                "POST",
                "/api/auth/register",
                Some(json!({"username": username, "email": email, "password": password})),
                None,
            )
            .await?;
        self.set_token(Some(&response.token))?;
        Ok(response.user)
    }

    pub async fn mods(&self, query: &ModQuery) -> Result<ModList, ApiError> {
        let parameters = web_sys::UrlSearchParams::new()
            .map_err(|_| String::from("Could not create the Library search"))?;
        if query.mine {
            parameters.set("mine", "true");
        }
        if !query.search.is_empty() {
            parameters.set("search", &query.search);
        }
        for tag in &query.tags {
            parameters.append("tag", tag);
        }
        if let Some(sort) = query.sort {
            parameters.set("sort", sort.as_str());
        }
        if let Some(page) = query.page {
            parameters.set("page", &page.to_string());
        }
        if let Some(size) = query.page_size {
            parameters.set("pageSize", &size.to_string());
        }
        self.get(&format!("/api/mods?{}", String::from(parameters.to_string()))).await
    }

    pub async fn mod_detail(&self, slug: &str) -> Result<ModDetail, ApiError> {
        self.get(&format!("/api/mods/{}", component(slug))).await
    }

    /// `AuthenticatedImage.tsx`: protected thumbnails and screenshots carry the
    /// account header, then become view-owned Blob URLs.
    pub async fn image(&self, source: &str) -> Result<OwnedImageUrl, ApiError> {
        let location = self.window.location();
        let base =
            location.href().map_err(|_| String::from("Could not read the Website address"))?;
        let url = web_sys::Url::new_with_base(source, &base)
            .map_err(|_| String::from("The Website returned an invalid image address"))?;
        if url.origin()
            != location.origin().map_err(|_| String::from("Could not read the Website address"))?
        {
            return Err(String::from("The image does not belong to this Website").into());
        }
        let mut path = format!("{}{}", url.pathname(), url.search());
        if !self.mount.prefix.is_empty() && path.starts_with(&format!("{}/", self.mount.prefix)) {
            path = path[self.mount.prefix.len()..].to_owned();
        }
        let response =
            self.request("GET", &path, None, None, web_sys::RequestCache::Default).await?;
        if !response.ok() {
            return Err(ApiError {
                status: response.status(),
                message: format!("Image request failed ({})", response.status()),
            });
        }
        let blob =
            JsFuture::from(response.blob().map_err(|_| String::from("Could not read the image"))?)
                .await
                .map_err(|_| String::from("Could not read the image"))?
                .dyn_into::<web_sys::Blob>()
                .map_err(|_| String::from("The image response was not a Blob"))?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)
            .map_err(|_| String::from("Could not display the image"))?;
        Ok(OwnedImageUrl(url))
    }

    pub async fn mod_comments(&self, slug: &str) -> Result<ModComments, ApiError> {
        self.get(&format!("/api/mods/{}/comments", component(slug))).await
    }

    pub async fn post_mod_comment(&self, slug: &str, body: &str) -> Result<ModComment, ApiError> {
        self.send(
            "POST",
            &format!("/api/mods/{}/comments", component(slug)),
            Some(json!({"body":body})),
            None,
        )
        .await
    }

    pub async fn delete_mod_comment(&self, slug: &str, id: u64) -> Result<(), ApiError> {
        self.send::<Value>(
            "DELETE",
            &format!("/api/mods/{}/comments/{id}", component(slug)),
            None,
            None,
        )
        .await?;
        Ok(())
    }

    pub async fn publish_mobile_layout(
        &self,
        layout: &Value,
    ) -> Result<SharedMobileUiLayout, ApiError> {
        self.send("POST", "/api/game/layouts", Some(json!({"layout":layout})), None).await
    }

    pub async fn mobile_layout(&self, code: &str) -> Result<SharedMobileUiLayout, ApiError> {
        self.get(&format!("/api/game/layouts/{}", component(code))).await
    }

    pub async fn subscriptions(&self) -> Result<Vec<ModSubscription>, ApiError> {
        Ok(self.get::<Items<ModSubscription>>("/api/mods/subscriptions").await?.items)
    }

    pub async fn active_mods(&self) -> Result<ActiveWebModSet, ApiError> {
        self.get("/api/mods/active").await
    }

    /// Fetches immutable content. The content cache verifies the descriptor's length and
    /// SHA-256 before preparing any image, audio, script or scene consumer.
    pub async fn content_download(&self, sha256: &str) -> Result<Response, ApiError> {
        if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(String::from("The content digest is invalid").into());
        }
        let response = self
            .request(
                "GET",
                &format!("/api/game/content/{sha256}"),
                None,
                None,
                web_sys::RequestCache::ForceCache,
            )
            .await?;
        if !response.ok() {
            return Err(ApiError {
                status: response.status(),
                message: format!("Content download failed ({})", response.status()),
            });
        }
        Ok(response)
    }

    pub async fn subscribe(&self, slug: &str) -> Result<(), ApiError> {
        self.send::<Value>(
            "PUT",
            &format!("/api/mods/{}/subscription", component(slug)),
            None,
            None,
        )
        .await?;
        Ok(())
    }

    pub async fn set_mod_enabled(&self, slug: &str, enabled: bool) -> Result<(), ApiError> {
        self.send::<Value>(
            "PATCH",
            &format!("/api/mods/{}/subscription", component(slug)),
            Some(json!({"enabled": enabled})),
            None,
        )
        .await?;
        Ok(())
    }

    pub async fn unsubscribe(&self, slug: &str) -> Result<(), ApiError> {
        self.send("DELETE", &format!("/api/mods/{}/subscription", component(slug)), None, None)
            .await
    }

    pub async fn sync_mods(&self, mods: &[PartyJoinMod]) -> Result<(), ApiError> {
        let mods: Vec<_> = mods
            .iter()
            .map(|entry| {
                json!({
                    "contentSha256": entry.content_sha256, "id": entry.id,
                    "slug": entry.slug, "version": entry.version,
                })
            })
            .collect();
        self.send::<Value>(
            "POST",
            "/api/mods/subscriptions/sync",
            Some(json!({"mods": mods})),
            None,
        )
        .await?;
        Ok(())
    }

    pub async fn disable_all_mods(&self) -> Result<(), ApiError> {
        self.send::<Value>("POST", "/api/mods/subscriptions/disable-all", Some(json!({})), None)
            .await?;
        Ok(())
    }

    pub async fn parties(&self) -> Result<Vec<PublicGameParty>, ApiError> {
        Ok(self.get::<Items<PublicGameParty>>("/api/game/parties").await?.items)
    }

    pub async fn connected_players(&self) -> Result<Vec<ConnectedGamePlayer>, ApiError> {
        Ok(self.get::<Items<ConnectedGamePlayer>>("/api/game/players").await?.items)
    }

    pub async fn developer_matches(&self) -> Result<Vec<DeveloperGameMatch>, ApiError> {
        Ok(self.get::<Items<DeveloperGameMatch>>("/api/game/matches").await?.items)
    }

    pub async fn resolve_party_code(&self, code: &str) -> Result<PartyJoinResolution, ApiError> {
        self.send("POST", "/api/game/join/resolve", Some(json!({"code": code})), None).await
    }

    pub async fn resolve_public_party(
        &self,
        listing_id: &str,
    ) -> Result<PartyJoinResolution, ApiError> {
        self.send("POST", "/api/game/join/public", Some(json!({"listingId": listing_id})), None)
            .await
    }

    pub async fn request_party_join(
        &self,
        listing_id: &str,
        display_name: &str,
        requester_id: &str,
    ) -> Result<PartyJoinRequestReceipt, ApiError> {
        self.send(
            "POST",
            "/api/game/join/requests",
            Some(json!({
                "listingId": listing_id, "displayName": display_name, "requesterId": requester_id,
            })),
            None,
        )
        .await
    }

    pub async fn party_request_status(
        &self,
        token: &str,
    ) -> Result<PartyJoinRequestStatus, ApiError> {
        self.get(&format!("/api/game/join/requests/{}", component(token))).await
    }

    pub async fn admit(
        &self,
        admission: BrowserAdmission<'_>,
    ) -> Result<ProvisionedGameEndpoint, ApiError> {
        let response = match admission {
            BrowserAdmission::GlobalHub => {
                self.send("POST", "/api/game/hub", None, Some("enter-hub")).await?
            }
            BrowserAdmission::PrivateCollege => {
                self.send("POST", "/api/game/sessions", None, Some("provision")).await?
            }
            BrowserAdmission::Party { intent_id } => {
                self.send(
                    "POST",
                    "/api/game/join/admit",
                    Some(json!({"intentId": intent_id})),
                    None,
                )
                .await?
            }
            BrowserAdmission::Resume { party_rejoin_token, document, shared_hub } => {
                if let Some(token) = party_rejoin_token {
                    match self
                        .send(
                            "POST",
                            "/api/game/rejoin",
                            Some(json!({"token": token, "save": document})),
                            None,
                        )
                        .await
                    {
                        Ok(endpoint) => return self.accept_endpoint(endpoint),
                        Err(error) if error.status == 404 => {}
                        Err(error) => return Err(error),
                    }
                }
                let (path, header) = if shared_hub {
                    ("/api/game/hub", "enter-hub")
                } else {
                    ("/api/game/sessions", "provision")
                };
                self.send("POST", path, None, Some(header)).await?
            }
        };
        self.accept_endpoint(response)
    }

    pub async fn admit_editor_test(
        &self,
        name: &str,
        bytes_base64: &str,
    ) -> Result<ProvisionedGameEndpoint, ApiError> {
        let endpoint = self
            .send(
                "POST",
                "/api/game/editor-test",
                Some(json!({"name":name,"bytesBase64":bytes_base64})),
                Some("editor-test"),
            )
            .await?;
        self.accept_endpoint(endpoint)
    }

    pub async fn observe(&self, match_id: &str) -> Result<ProvisionedGameEndpoint, ApiError> {
        let endpoint = self
            .send("POST", "/api/game/observe", Some(json!({"matchId": match_id})), None)
            .await?;
        let endpoint = self.accept_endpoint(endpoint)?;
        if !endpoint.observer {
            return Err(
                String::from("The game observer provisioner returned an invalid endpoint").into()
            );
        }
        Ok(endpoint)
    }

    fn accept_endpoint(
        &self,
        mut endpoint: ProvisionedGameEndpoint,
    ) -> Result<ProvisionedGameEndpoint, ApiError> {
        if endpoint.credential.is_empty()
            || endpoint.credential.len() > 512
            || endpoint.session_kind == GameSessionKind::Standalone
        {
            return Err(
                String::from("The game session provisioner returned an invalid endpoint").into()
            );
        }
        endpoint.url = self.mount.endpoint_url(&self.window, &endpoint.url)?;
        Ok(endpoint)
    }

    pub async fn read_save(&self) -> Result<Option<WebGameSave>, ApiError> {
        #[derive(Deserialize)]
        struct SaveResponse {
            save: Option<WebGameSave>,
        }
        match self.get::<SaveResponse>("/api/game/saves/0").await {
            Ok(response) => Ok(response.save),
            Err(error) if error.status == 404 => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub async fn write_save(
        &self,
        document: &str,
        expected_revision: u64,
    ) -> Result<WebGameSave, ApiError> {
        self.send(
            "PUT",
            "/api/game/saves/0",
            Some(json!({"document": document, "expectedRevision": expected_revision})),
            None,
        )
        .await
    }

    pub async fn remove_save(&self, expected_revision: u64) -> Result<(), ApiError> {
        self.send(
            "DELETE",
            &format!("/api/game/saves/0?expectedRevision={expected_revision}"),
            None,
            None,
        )
        .await
    }

    pub async fn leaderboard(
        &self,
        board: HallOfFameBoard,
    ) -> Result<Vec<GlobalLeaderboardEntry>, ApiError> {
        Ok(self
            .get::<Items<GlobalLeaderboardEntry>>(&format!(
                "/api/game/leaderboards?board={}",
                board.as_str()
            ))
            .await?
            .items)
    }

    pub async fn submit_score(&self, receipt: &str) -> Result<GlobalLeaderboardEntry, ApiError> {
        self.send("POST", "/api/game/leaderboards", Some(json!({"receipt": receipt})), None).await
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        self.send("GET", path, None, None).await
    }

    async fn send<T: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        session_header: Option<&str>,
    ) -> Result<T, ApiError> {
        let response = self
            .request(method, path, body, session_header, web_sys::RequestCache::Default)
            .await?;
        let status = response.status();
        let text = JsFuture::from(
            response.text().map_err(|_| String::from("Could not read the Website response"))?,
        )
        .await
        .map_err(|_| String::from("Could not read the Website response"))?
        .as_string()
        .ok_or_else(|| String::from("The Website response was not text"))?;
        decode_response(status, &text)
    }

    async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        session_header: Option<&str>,
        cache: web_sys::RequestCache,
    ) -> Result<Response, ApiError> {
        let options = RequestInit::new();
        options.set_method(method);
        options.set_cache(cache);
        options.set_credentials(web_sys::RequestCredentials::SameOrigin);
        if let Some(body) = body.as_ref() {
            options.set_body(&JsValue::from_str(&body.to_string()));
        }
        let request = Request::new_with_str_and_init(&self.mount.path(path), &options)
            .map_err(|_| String::from("Could not create the Website request"))?;
        let headers = request.headers();
        headers
            .set("Accept", "application/json")
            .map_err(|_| String::from("Could not set request headers"))?;
        if body.is_some() {
            headers
                .set("Content-Type", "application/json")
                .map_err(|_| String::from("Could not set request headers"))?;
        }
        if let Some(token) = self.token()? {
            headers
                .set("Authorization", &format!("Bearer {token}"))
                .map_err(|_| String::from("Could not set account authorization"))?;
        }
        if let Some(value) = session_header {
            headers
                .set("X-Solomon-Dark-Session", value)
                .map_err(|_| String::from("Could not set game admission headers"))?;
        }
        JsFuture::from(self.window.fetch_with_request(&request))
            .await
            .map_err(|_| String::from("Cannot reach the server. Check your connection."))?
            .dyn_into()
            .map_err(|_| String::from("The Website returned an invalid response").into())
    }
}

fn component(text: &str) -> String {
    js_sys::encode_uri_component(text).into()
}

fn decode_response<T: DeserializeOwned>(status: u16, text: &str) -> Result<T, ApiError> {
    if !(200..300).contains(&status) {
        #[derive(Deserialize)]
        struct ErrorResponse {
            error: String,
        }
        let message = serde_json::from_str::<ErrorResponse>(text)
            .map_or_else(|_| format!("Request failed ({status})"), |body| body.error);
        return Err(ApiError { status, message });
    }
    serde_json::from_str(if status == 204 { "null" } else { text }).map_err(|_| ApiError {
        status,
        message: "The Website returned an invalid response".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staging_api_and_saved_identity_use_the_deployment_mount() {
        let staging = WebsiteMount::new("/game-stg/").unwrap();
        assert_eq!(staging.path("/api/game/hub"), "/game-stg/api/game/hub");
        assert_eq!(staging.storage_key("resume"), "resume:/game-stg");
        assert_eq!(WebsiteMount::new("/").unwrap().storage_key("resume"), "resume");
        for invalid in ["game-stg", "//other.example", "/../game", "/game?host=1"] {
            assert!(WebsiteMount::new(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn cloud_conflicts_and_inactive_rejoins_keep_their_distinct_status() {
        let conflict = decode_response::<WebGameSave>(
            409,
            r#"{"error":"The browser game save changed in another session.","currentRevision":9}"#,
        )
        .err()
        .unwrap();
        assert_eq!(conflict.status, 409);
        assert_eq!(conflict.message, "The browser game save changed in another session.");
        assert_eq!(decode_response::<WebGameSave>(404, "not found").err().unwrap().status, 404);
        assert!(decode_response::<WebGameSave>(200, "not json").is_err());
        assert!(decode_response::<()>(204, "").is_ok());
    }
}
