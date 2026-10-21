//! `DarkCloudScene.tsx` requests against the existing Website control plane.
//! Subscription receipts carry the actual refreshed set, including automatic disablement.

use super::*;
use crate::hosted::{
    ConnectedGamePlayer, DeveloperGameMatch, ModComments, ModDetail, ModQuery, ModSort,
    ModSubscription, ModSummary, OwnedImageUrl, SharedMobileUiLayout,
};
use crate::menu::ModSubscriptionAction;
use crate::mod_content::{GameContentDownloadProgress, ModContentCache};

pub(super) struct CloudRequest {
    pub action: HostedMenuAction,
    signed_in: bool,
    mobile_layout: Option<serde_json::Value>,
}

pub(super) struct CloudCatalog {
    pub mods: Result<Vec<ModSummary>, String>,
    pub subscriptions: Result<Vec<ModSubscription>, String>,
}

pub(super) enum SubscriptionUpdate {
    ActiveMods(Vec<ActiveWebMod>),
    Catalog { request_id: u64, catalog: CloudCatalog },
}

pub(super) enum CloudCompletion {
    Catalog(CloudCatalog),
    Detail(Result<Box<ModDetail>, String>),
    Comments(Result<ModComments, String>),
    CommentMutation(Result<(), String>),
    Subscription(Result<(), String>),
    Image(Result<OwnedImageUrl, String>),
    Presence(Result<(Vec<ConnectedGamePlayer>, Vec<DeveloperGameMatch>), String>),
    Observe(Result<ProvisionedGameEndpoint, String>),
    MobileLayout(Result<SharedMobileUiLayout, String>),
}

impl CloudRequest {
    pub async fn execute(
        self,
        api: &WebsiteApi,
        progress: impl FnMut(GameContentDownloadProgress<'_>),
        subscription_update: impl FnMut(SubscriptionUpdate),
    ) -> (HostedMenuAction, CloudCompletion) {
        let result = match &self.action {
            HostedMenuAction::LoadDarkCloudCatalog { .. } => {
                CloudCompletion::Catalog(load_catalog(api, self.signed_in).await)
            }
            HostedMenuAction::LoadModDetail { slug, .. } => CloudCompletion::Detail(
                api.mod_detail(slug).await.map(Box::new).map_err(|e| e.to_string()),
            ),
            HostedMenuAction::LoadModComments { slug, .. } => {
                CloudCompletion::Comments(api.mod_comments(slug).await.map_err(|e| e.to_string()))
            }
            HostedMenuAction::PostModComment { slug, body, .. } => {
                CloudCompletion::CommentMutation(
                    api.post_mod_comment(slug, body).await.map(|_| ()).map_err(|e| e.to_string()),
                )
            }
            HostedMenuAction::DeleteModComment { slug, comment_id, .. } => {
                CloudCompletion::CommentMutation(
                    api.delete_mod_comment(slug, *comment_id).await.map_err(|e| e.to_string()),
                )
            }
            HostedMenuAction::ChangeModSubscription { request_id, slug, action } => {
                CloudCompletion::Subscription(
                    mutate_subscription(
                        api,
                        slug,
                        *action,
                        *request_id,
                        progress,
                        subscription_update,
                    )
                    .await,
                )
            }
            HostedMenuAction::LoadModImage { url, .. } => {
                CloudCompletion::Image(api.image(url).await.map_err(|e| e.to_string()))
            }
            HostedMenuAction::LoadDeveloperPresence { .. } => {
                let result = async {
                    let players = api.connected_players().await.map_err(|e| e.to_string())?;
                    let matches = api.developer_matches().await.map_err(|e| e.to_string())?;
                    Ok((players, matches))
                }
                .await;
                CloudCompletion::Presence(result)
            }
            HostedMenuAction::ObserveMatch { match_id, .. } => {
                CloudCompletion::Observe(api.observe(match_id).await.map_err(|e| e.to_string()))
            }
            HostedMenuAction::LoadMobileLayout { code, .. } => CloudCompletion::MobileLayout(
                api.mobile_layout(code).await.map_err(|e| e.to_string()),
            ),
            HostedMenuAction::PublishMobileLayout { .. } => CloudCompletion::MobileLayout(
                api.publish_mobile_layout(
                    self.mobile_layout.as_ref().expect("publishing requires a saved layout"),
                )
                .await
                .map_err(|e| e.to_string()),
            ),
            _ => unreachable!("only Dark Cloud requests enter its I/O owner"),
        };
        (self.action, result)
    }
}

async fn load_catalog(api: &WebsiteApi, signed_in: bool) -> CloudCatalog {
    let mods = async {
        let first = api
            .mods(&ModQuery {
                sort: Some(ModSort::Newest),
                page_size: Some(50),
                ..ModQuery::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        let count = first.total.div_ceil(first.page_size);
        let mut mods = first.items;
        for page in 2..=count {
            mods.extend(
                api.mods(&ModQuery {
                    sort: Some(ModSort::Newest),
                    page_size: Some(50),
                    page: Some(page),
                    ..ModQuery::default()
                })
                .await
                .map_err(|e| e.to_string())?
                .items,
            );
        }
        Ok(mods)
    }
    .await;
    let subscriptions = if signed_in {
        api.subscriptions().await.map_err(|e| e.to_string())
    } else {
        Ok(Vec::new())
    };
    CloudCatalog { mods, subscriptions }
}

async fn mutate_subscription(
    api: &WebsiteApi,
    slug: &str,
    action: ModSubscriptionAction,
    request_id: u64,
    progress: impl FnMut(GameContentDownloadProgress<'_>),
    mut update: impl FnMut(SubscriptionUpdate),
) -> Result<(), String> {
    match action {
        ModSubscriptionAction::Subscribe => api.subscribe(slug).await,
        ModSubscriptionAction::Enable => api.set_mod_enabled(slug, true).await,
        ModSubscriptionAction::Disable => api.set_mod_enabled(slug, false).await,
        ModSubscriptionAction::Unsubscribe => api.unsubscribe(slug).await,
    }
    .map_err(|e| e.to_string())?;
    let mods = api.active_mods().await.map_err(|e| e.to_string())?.mods;
    update(SubscriptionUpdate::ActiveMods(mods.clone()));
    update(SubscriptionUpdate::Catalog { request_id, catalog: load_catalog(api, true).await });
    if !matches!(action, ModSubscriptionAction::Subscribe | ModSubscriptionAction::Enable) {
        return Ok(());
    }
    let assets: Vec<_> = mods.iter().flat_map(|content| content.assets.iter()).cloned().collect();
    let cache = ModContentCache::open(api.clone()).await.map_err(|e| e.to_string())?;
    if let Err(error) = cache.prefetch(&assets, progress).await {
        let Some(failed) = mods.iter().find(|content| content.id == error.mod_id) else {
            return Err(error.to_string());
        };
        let (slug, name) = (failed.slug.clone(), failed.name.clone());
        let disable = async {
            api.set_mod_enabled(&slug, false).await?;
            update(SubscriptionUpdate::ActiveMods(api.active_mods().await?.mods));
            update(SubscriptionUpdate::Catalog {
                request_id,
                catalog: load_catalog(api, true).await,
            });
            Ok::<_, crate::hosted::ApiError>(())
        }
        .await;
        return match disable {
            Ok(()) => Err(format!(
                "{name} was disabled because its content could not be loaded or verified."
            )),
            Err(_) => Err(format!("{error} The mod could not be disabled automatically.")),
        };
    }
    Ok(())
}

impl Game {
    pub(super) fn apply_hosted_subscription_updates(&mut self) -> Result<(), String> {
        let Some(hosted) = &mut self.hosted else { return Ok(()) };
        for update in std::mem::take(&mut *hosted.subscription_updates.borrow_mut()) {
            match update {
                SubscriptionUpdate::ActiveMods(mods) => hosted.active_mods = mods,
                SubscriptionUpdate::Catalog { request_id, catalog } => {
                    self.menu.complete_dark_cloud_catalog(
                        request_id,
                        catalog.mods,
                        catalog.subscriptions,
                    )?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn hosted_cloud_action(
        &mut self,
        action: &HostedMenuAction,
    ) -> Result<bool, String> {
        let hosted = self.hosted.as_mut().unwrap();
        match action {
            HostedMenuAction::SignIn => {
                web_sys::window()
                    .ok_or("no window")?
                    .location()
                    .assign(&hosted.api.mount().path("/login"))
                    .map_err(|_| "Could not open account sign-in")?;
            }
            HostedMenuAction::SignOut => {
                hosted.api.logout().map_err(|e| e.to_string())?;
                web_sys::window()
                    .ok_or("no window")?
                    .location()
                    .reload()
                    .map_err(|_| "Could not return to guest play")?;
            }
            HostedMenuAction::LoadDarkCloudCatalog { .. }
            | HostedMenuAction::LoadModDetail { .. }
            | HostedMenuAction::LoadModComments { .. }
            | HostedMenuAction::PostModComment { .. }
            | HostedMenuAction::DeleteModComment { .. }
            | HostedMenuAction::ChangeModSubscription { .. }
            | HostedMenuAction::LoadModImage { .. }
            | HostedMenuAction::LoadDeveloperPresence { .. }
            | HostedMenuAction::ObserveMatch { .. }
            | HostedMenuAction::LoadMobileLayout { .. }
            | HostedMenuAction::PublishMobileLayout { .. } => {
                let mobile_layout =
                    if let HostedMenuAction::PublishMobileLayout { request_id } = action {
                        match self.mobile_layout_store.current_document() {
                            Ok(document) => {
                                Some(serde_json::to_value(document).map_err(|e| e.to_string())?)
                            }
                            Err(error) => {
                                self.menu.complete_mobile_layout(*request_id, Err(error))?;
                                return Ok(true);
                            }
                        }
                    } else {
                        None
                    };
                hosted.pending.push(HostedRequest::Cloud(CloudRequest {
                    action: action.clone(),
                    signed_in: hosted.account.is_some(),
                    mobile_layout,
                }));
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(super) fn complete_hosted_cloud(
        &mut self,
        action: HostedMenuAction,
        completion: CloudCompletion,
    ) -> Result<(), String> {
        match (action, completion) {
            (
                HostedMenuAction::LoadDarkCloudCatalog { request_id },
                CloudCompletion::Catalog(catalog),
            ) => {
                self.menu.complete_dark_cloud_catalog(
                    request_id,
                    catalog.mods,
                    catalog.subscriptions,
                )?;
            }
            (
                HostedMenuAction::LoadModDetail { request_id, .. },
                CloudCompletion::Detail(result),
            ) => {
                self.menu.complete_mod_detail(request_id, result.map(|detail| *detail))?;
            }
            (
                HostedMenuAction::LoadModComments { request_id, .. },
                CloudCompletion::Comments(result),
            ) => {
                self.menu.complete_mod_comments(request_id, result)?;
            }
            (
                HostedMenuAction::PostModComment { request_id, .. }
                | HostedMenuAction::DeleteModComment { request_id, .. },
                CloudCompletion::CommentMutation(result),
            ) => {
                self.menu.complete_mod_comment_mutation(request_id, result)?;
            }
            (
                HostedMenuAction::ChangeModSubscription { request_id, .. },
                CloudCompletion::Subscription(result),
            ) => {
                self.menu.complete_mod_subscription(request_id, result)?;
            }
            (HostedMenuAction::LoadModImage { request_id, .. }, CloudCompletion::Image(result)) => {
                self.menu.complete_mod_image(request_id, result)?;
            }
            (
                HostedMenuAction::LoadDeveloperPresence { request_id },
                CloudCompletion::Presence(result),
            ) => {
                self.menu.complete_developer_presence(request_id, result)?;
            }
            (
                HostedMenuAction::ObserveMatch { request_id, .. },
                CloudCompletion::Observe(result),
            ) => match result {
                Ok(endpoint) if self.menu.complete_observe_match(request_id, Ok(()))? => {
                    self.begin_hosted_observer(endpoint)?;
                }
                Err(error) => {
                    self.menu.complete_observe_match(request_id, Err(error))?;
                }
                _ => {}
            },
            (
                HostedMenuAction::LoadMobileLayout { request_id, .. },
                CloudCompletion::MobileLayout(result),
            ) => {
                let result = result.and_then(|shared| {
                    self.mobile_layout_store.install_shared(&shared.layout)?;
                    Ok(shared)
                });
                self.menu.complete_mobile_layout(request_id, result)?;
                self.menu
                    .set_mobile_layout_customized(self.mobile_layout_store.read()?.customized)?;
            }
            (
                HostedMenuAction::PublishMobileLayout { request_id },
                CloudCompletion::MobileLayout(result),
            ) => {
                self.menu.complete_mobile_layout(request_id, result)?;
            }
            _ => unreachable!("Dark Cloud completion belongs to its request"),
        }
        Ok(())
    }
}
