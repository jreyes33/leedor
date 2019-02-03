use crate::epub::Epub;
use crate::utils;
use js_sys::{ArrayBuffer, Uint8Array};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    console, Document, Event, FileReader, HtmlElement, HtmlInputElement, ShadowRootInit,
    ShadowRootMode,
};

type JsResult<T> = std::result::Result<T, JsValue>;

#[wasm_bindgen]
pub fn run() -> JsResult<()> {
    let app = LeedorApp::new();
    app.run()
}

struct LeedorApp {
    epub: Rc<RefCell<Option<Epub>>>,
}

impl LeedorApp {
    pub fn new() -> LeedorApp {
        LeedorApp {
            epub: Rc::new(RefCell::new(None)),
        }
    }

    pub fn run(&self) -> JsResult<()> {
        utils::set_panic_hook();
        let document = document().ok_or("no document")?;

        let content_div = document
            .get_element_by_id("book-content")
            .ok_or("no #book-content")?;
        let shadow_root_init = ShadowRootInit::new(ShadowRootMode::Open);
        content_div.attach_shadow(&shadow_root_init)?;

        let file_handler = Closure::wrap(self.handle_file_change());
        let file_input = document.get_element_by_id("file").ok_or("no #file")?;
        file_input
            .add_event_listener_with_callback("change", file_handler.as_ref().unchecked_ref())?;
        file_handler.forget();

        let chapter_handler = Closure::wrap(self.handle_chapter_change());
        let chapter_input = document.get_element_by_id("chapter").ok_or("no #chapter")?;
        chapter_input
            .add_event_listener_with_callback("change", chapter_handler.as_ref().unchecked_ref())?;
        chapter_handler.forget();

        console::log_1(&"ready".into());
        Ok(())
    }

    fn handle_chapter_change(&self) -> Box<FnMut(Event) -> JsResult<()>> {
        let epub_ref = self.epub.clone();
        let handler = move |e: Event| -> JsResult<()> {
            let target: HtmlInputElement = e.target().ok_or("no event target")?.dyn_into()?;
            let chapter_number: usize = target.value().parse().or(Err("not a valid number"))?;
            let mut epub_option = epub_ref.borrow_mut();
            let epub = epub_option.as_mut().ok_or("no epub loaded yet")?;
            let first_chapter = epub.chapter(chapter_number.saturating_sub(1))?;
            render_content(&first_chapter)?;
            Ok(())
        };
        Box::new(handler)
    }

    fn handle_file_change(&self) -> Box<FnMut(Event) -> JsResult<()>> {
        let onload_rc = Rc::new(Closure::wrap(self.handle_file_load()));
        let handler = move |e: Event| -> JsResult<()> {
            let target: HtmlInputElement = e.target().ok_or("no event target")?.dyn_into()?;
            let file_list = target.files().ok_or("no files in target")?;
            let file = file_list.get(0).ok_or("no files")?;
            let reader = FileReader::new()?;
            let onload_handler = onload_rc.as_ref();
            reader.set_onload(Some(onload_handler.as_ref().unchecked_ref()));
            reader.read_as_array_buffer(&file)?;
            Ok(())
        };
        Box::new(handler)
    }

    // TODO: fix memory leaks when loading new epubs.
    fn handle_file_load(&self) -> Box<FnMut(Event) -> JsResult<()>> {
        let epub_ref = self.epub.clone();
        let handler = move |e: Event| -> JsResult<()> {
            let target: FileReader = e.target().ok_or("no event target")?.dyn_into()?;
            let contents_buf: ArrayBuffer = target.result()?.into();
            let mut bytes = vec![0; contents_buf.byte_length() as usize];
            Uint8Array::new(&contents_buf).copy_to(&mut bytes);
            *epub_ref.borrow_mut() = Some(Epub::new(bytes)?);
            let mut epub_option = epub_ref.borrow_mut();
            let epub = epub_option.as_mut().ok_or("no epub")?;
            let first_chapter = epub.chapter(0)?;
            render_count(epub.doc_count()?)?;
            render_content(&first_chapter)?;
            Ok(())
        };
        Box::new(handler)
    }
}

fn document() -> Option<Document> {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            return Some(document);
        }
    }
    None
}

fn render_count(count: usize) -> JsResult<()> {
    let count_div: HtmlElement = document()
        .ok_or("no document")?
        .get_element_by_id("doc-count")
        .ok_or("no #doc-count")?
        .dyn_into()?;
    count_div.set_inner_text(&count.to_string());
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
