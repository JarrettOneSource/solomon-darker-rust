//! Protected Website media has one view-owned Blob URL. The coordinator fetches
//! with the account/mount policy; stale completion drops and revokes its result.

use std::collections::{BTreeMap, BTreeSet};

use wasm_bindgen::JsCast;
use web_sys::{Document, HtmlElement, HtmlImageElement};

use crate::hosted::OwnedImageUrl;
use crate::menu::HostedMenuAction;
use crate::ui::{append, element};

enum Image {
    Pending(u64),
    Ready(OwnedImageUrl),
    Failed,
}

#[derive(Default)]
pub(super) struct CloudImages {
    entries: BTreeMap<String, Image>,
    next_request: u64,
    actions: Vec<HostedMenuAction>,
}

impl CloudImages {
    /// Called after repainting both the list and the detail view. Media removed
    /// by filtering, tab changes or gallery closure releases its URL immediately.
    pub fn finish_paint(&mut self, parent: &HtmlElement) -> Result<(), String> {
        let nodes = parent
            .query_selector_all("[data-cloud-image-url]")
            .map_err(|_| "visible mod media selector")?;
        let mut used = BTreeSet::new();
        for index in 0..nodes.length() {
            let node = nodes.item(index).ok_or("visible mod media disappeared")?;
            let source = node
                .dyn_ref::<web_sys::Element>()
                .and_then(|node| node.get_attribute("data-cloud-image-url"))
                .ok_or("visible mod media has no source")?;
            used.insert(source);
        }
        self.entries.retain(|url, _| used.contains(url));
        self.actions.retain(|action| match action {
            HostedMenuAction::LoadModImage { url, .. } => used.contains(url),
            _ => true,
        });
        Ok(())
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.actions.clear();
    }

    pub fn media(
        &mut self,
        document: &Document,
        source: Option<&str>,
        alt: &str,
        class: &str,
        contain: bool,
        eager: bool,
    ) -> Result<HtmlElement, String> {
        let root = element(document, "span", &format!("dark-cloud-media {class}"))?;
        let art = element(document, "span", "native-ui-control-panel-art")?;
        append(&root, &art)?;
        let Some(source) = source else {
            self.placeholder(document, &root, alt)?;
            return Ok(root);
        };
        if !self.entries.contains_key(source) {
            self.next_request += 1;
            self.entries.insert(source.to_owned(), Image::Pending(self.next_request));
            self.actions.push(HostedMenuAction::LoadModImage {
                request_id: self.next_request,
                url: source.to_owned(),
            });
        }
        root.set_attribute("data-cloud-image-url", source).map_err(|_| "mod media source")?;
        if matches!(self.entries.get(source), Some(Image::Failed)) {
            self.placeholder(document, &root, alt)?;
            return Ok(root);
        }
        let image: HtmlImageElement =
            element(document, "img", "")?.dyn_into().map_err(|_| "mod image element")?;
        image.set_alt(alt);
        image.set_attribute("decoding", "async").map_err(|_| "mod image decoding")?;
        image
            .set_attribute("loading", if eager { "eager" } else { "lazy" })
            .map_err(|_| "mod image loading")?;
        image
            .style()
            .set_property("object-fit", if contain { "contain" } else { "cover" })
            .map_err(|_| "mod image fitting")?;
        if let Some(Image::Ready(url)) = self.entries.get(source) {
            image.set_src(url.url());
        }
        root.append_child(&image).map_err(|_| "mod image parent")?;
        Ok(root)
    }

    pub fn complete(
        &mut self,
        document: &Document,
        parent: &HtmlElement,
        request_id: u64,
        result: Result<OwnedImageUrl, String>,
    ) -> Result<bool, String> {
        let source = self.entries.iter().find_map(|(source, state)| {
            matches!(state, Image::Pending(id) if *id == request_id).then(|| source.clone())
        });
        let Some(source) = source else {
            return Ok(false);
        };
        self.entries.insert(source.clone(), result.map_or(Image::Failed, Image::Ready));
        self.paint_source(document, parent, &source)?;
        Ok(true)
    }

    pub fn take_actions(&mut self) -> Vec<HostedMenuAction> {
        std::mem::take(&mut self.actions)
    }

    pub fn failed(
        &mut self,
        document: &Document,
        parent: &HtmlElement,
        source: &str,
    ) -> Result<(), String> {
        if self.entries.contains_key(source) {
            self.entries.insert(source.to_owned(), Image::Failed);
            self.paint_source(document, parent, source)?;
        }
        Ok(())
    }

    fn paint_source(
        &self,
        document: &Document,
        parent: &HtmlElement,
        source: &str,
    ) -> Result<(), String> {
        let nodes = parent
            .query_selector_all("[data-cloud-image-url]")
            .map_err(|_| "resolved mod media selector")?;
        for index in 0..nodes.length() {
            let node: HtmlElement = nodes
                .item(index)
                .ok_or("resolved media disappeared")?
                .dyn_into()
                .map_err(|_| "resolved mod media target")?;
            if node.get_attribute("data-cloud-image-url").as_deref() != Some(source) {
                continue;
            }
            let Some(image) = node.query_selector("img").map_err(|_| "resolved mod image")? else {
                continue;
            };
            let image: HtmlImageElement =
                image.dyn_into().map_err(|_| "resolved mod image target")?;
            match self.entries.get(source) {
                Some(Image::Ready(url)) => image.set_src(url.url()),
                Some(Image::Failed) => {
                    let alt = image.alt();
                    image.remove();
                    self.placeholder(document, &node, &alt)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn placeholder(
        &self,
        document: &Document,
        parent: &HtmlElement,
        alt: &str,
    ) -> Result<(), String> {
        let placeholder = element(document, "span", "dark-cloud-media-placeholder")?;
        placeholder.set_attribute("role", "img").map_err(|_| "missing image role")?;
        placeholder
            .set_attribute("aria-label", &format!("No image available for {alt}"))
            .map_err(|_| "missing image label")?;
        // NativeUiText centers each line inside the provisional flow width.
        let mut spec = solomon_sim::native_ui_text::NativeUiTextSpec {
            placement: solomon_sim::native_ui_text::NativeUiTextPlacement::Box,
            scale: 0.85,
            tint: 0xa99a70,
            ..solomon_sim::native_ui_text::NativeUiTextSpec::new(
                solomon_sim::native_ui_text::NativeUiFontName::Medium,
                "NO\nIMAGE",
                0.0,
                0.0,
            )
        };
        spec.x = solomon_sim::native_ui_text::layout_native_ui_text(&spec).width / 2.0;
        spec.align = solomon_sim::native_ui_text::NativeUiTextAlign::Center;
        append(&placeholder, &crate::ui::text::native_text(document, &spec, "")?)?;
        append(parent, &placeholder)
    }
}
