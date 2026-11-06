use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{Event, HtmlAnchorElement, HtmlInputElement};

use crate::platform::Surface;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedFile(pub(crate) web_sys::File);

impl SelectedFile {
    pub fn name(&self) -> String {
        self.0.name()
    }

    pub fn relative_path(&self) -> Result<String, String> {
        Ok(js_sys::Reflect::get(&self.0, &JsValue::from_str("webkitRelativePath"))
            .map_err(|_| format!("The path for {} could not be read.", self.name()))?
            .as_string()
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| self.name()))
    }

    pub async fn read(&self, maximum_bytes: usize) -> Result<Vec<u8>, String> {
        if self.0.size() > maximum_bytes as f64 {
            return Err(format!("{} exceeds the supported file size", self.name()));
        }
        let buffer = wasm_bindgen_futures::JsFuture::from(self.0.array_buffer())
            .await
            .map_err(|_| format!("The file {} could not be read.", self.name()))?;
        Ok(js_sys::Uint8Array::new(&buffer).to_vec())
    }
}

pub struct FilePicker {
    input: HtmlInputElement,
    selected: Rc<RefCell<Vec<Vec<SelectedFile>>>>,
    change: Closure<dyn FnMut(Event)>,
}

impl FilePicker {
    pub fn new(surface: &Surface, extensions: &[&str], multiple: bool) -> Result<Self, String> {
        let document = surface.canvas().owner_document().ok_or("The game has no document")?;
        let input: HtmlInputElement = document
            .create_element("input")
            .map_err(|_| "Could not create the file picker")?
            .dyn_into()
            .map_err(|_| "Invalid file picker")?;
        input.set_type("file");
        input.set_accept(
            &extensions.iter().map(|value| format!(".{value}")).collect::<Vec<_>>().join(","),
        );
        input.set_multiple(multiple);
        let selected = Rc::new(RefCell::new(Vec::new()));
        let queue = selected.clone();
        let target = input.clone();
        let change = Closure::<dyn FnMut(Event)>::new(move |_| {
            if let Some(files) = target.files()
                && files.length() > 0
            {
                queue.borrow_mut().push(
                    (0..files.length())
                        .filter_map(|index| files.item(index))
                        .map(SelectedFile)
                        .collect(),
                );
            }
            target.set_value("");
        });
        input
            .add_event_listener_with_callback("change", change.as_ref().unchecked_ref())
            .map_err(|_| "Could not listen for selected files")?;
        Ok(Self { input, selected, change })
    }

    pub fn choose(&self) -> Result<(), String> {
        self.input.click();
        Ok(())
    }
    pub fn take(&self) -> Vec<Vec<SelectedFile>> {
        self.selected.take()
    }
}

impl Drop for FilePicker {
    fn drop(&mut self) {
        if let Err(error) = self
            .input
            .remove_event_listener_with_callback("change", self.change.as_ref().unchecked_ref())
        {
            web_sys::console::error_1(&error);
        }
    }
}

pub async fn save_file(
    surface: &Surface,
    name: &str,
    bytes: Vec<u8>,
    mime: &str,
) -> Result<bool, String> {
    let document = surface.canvas().owner_document().ok_or("The game has no document")?;
    let anchor: HtmlAnchorElement = document
        .create_element("a")
        .map_err(|_| "Could not create the download")?
        .dyn_into()
        .map_err(|_| "Invalid download link")?;
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(mime);
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(bytes.as_slice()));
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
        .map_err(|_| "The file could not be exported")?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| "The download could not be opened")?;
    anchor.set_href(&url);
    anchor.set_download(name);
    anchor.click();
    web_sys::Url::revoke_object_url(&url).map_err(|_| "The download could not be released")?;
    Ok(true)
}
