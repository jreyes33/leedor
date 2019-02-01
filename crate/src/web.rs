use crate::epub::read_opf;
use crate::utils;
use js_sys::{ArrayBuffer, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    console, Document, Event, FileReader, HtmlInputElement, ShadowRootInit, ShadowRootMode,
};

type JsResult<T> = std::result::Result<T, JsValue>;

#[wasm_bindgen]
pub fn run() -> JsResult<()> {
    utils::set_panic_hook();
    let document = document().ok_or("no document")?;
    let file_input = document.get_element_by_id("file").ok_or("no #file")?;
    let content_div = document
        .get_element_by_id("book-content")
        .ok_or("no #book-content")?;
    let init = ShadowRootInit::new(ShadowRootMode::Open);
    content_div.attach_shadow(&init)?;
    let handler = Closure::wrap(Box::new(handle_file_change) as Box<Fn(_) -> JsResult<()>>);
    file_input.add_event_listener_with_callback("change", handler.as_ref().unchecked_ref())?;
    handler.forget();
    console::log_1(&"ready".into());
    Ok(())
}

fn handle_file_change(e: Event) -> JsResult<()> {
    let target: HtmlInputElement = e.target().ok_or("no event target")?.dyn_into()?;
    let file_list = target.files().ok_or("no files in target")?;
    let file = file_list.get(0).ok_or("no files")?;
    let reader = FileReader::new()?;
    let onload_handler = Closure::wrap(Box::new(handle_file_load) as Box<Fn(_) -> JsResult<()>>);
    reader.set_onload(Some(onload_handler.as_ref().unchecked_ref()));
    onload_handler.forget();
    reader.read_as_array_buffer(&file)?;
    Ok(())
}

fn handle_file_load(e: Event) -> JsResult<()> {
    let target: FileReader = e.target().ok_or("no event target")?.dyn_into()?;
    let contents_buf: ArrayBuffer = target.result()?.into();
    let mut bytes = vec![0; contents_buf.byte_length() as usize];
    Uint8Array::new(&contents_buf).copy_to(&mut bytes);
    // TODO: fix error conversion to use the ? operator instead.
    let first_chapter =
        read_opf(&bytes).unwrap_or_else(|err| format!("failed in read_opf: {}", err));
    render_content(&first_chapter)?;
    Ok(())
}

fn render_content(content: &str) -> JsResult<()> {
    let root = document()
        .ok_or("no document")?
        .get_element_by_id("book-content")
        .ok_or("no #book-content")?
        .shadow_root()
        .ok_or("no shadow root")?;
    root.set_inner_html(content);
    Ok(())
}

fn document() -> Option<Document> {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            return Some(document);
        }
    }
    None
}
