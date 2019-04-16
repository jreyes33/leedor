use crate::epub::{Epub, TocItem};
use crate::utils;
use js_sys::{ArrayBuffer, Uint8Array};
use std::cell::RefCell;
use std::cmp::{max, min};
use std::rc::Rc;
use url::Url;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    Document, Element, Event, EventTarget, FileReader, HtmlElement, HtmlInputElement,
    ShadowRootInit, ShadowRootMode,
};

const FONT_SIZE_DEFAULT: isize = 20;
const FONT_SIZE_INCREMENT: isize = 2;
const FONT_SIZE_MIN: isize = 6;
const FONT_SIZE_MAX: isize = 60;
type JsResult<T> = std::result::Result<T, JsValue>;
type EventHandler = Box<FnMut(Event) -> JsResult<()>>;

enum Cmp {
    More,
    Less,
}

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
        let file_input = document.get_element_by_id("file").ok_or("no #file")?;
        let chapter_input = document.get_element_by_id("chapter").ok_or("no #chapter")?;
        let prev_button = document.get_element_by_id("prev").ok_or("no #prev")?;
        let next_button = document.get_element_by_id("next").ok_or("no #next")?;
        let smaller_button = document.get_element_by_id("smaller").ok_or("no #smaller")?;
        let larger_button = document.get_element_by_id("larger").ok_or("no #larger")?;
        let toc = document.get_element_by_id("toc").ok_or("no #toc")?;
        let content = document.get_element_by_id("content").ok_or("no #content")?;
        let shadow_root = content.attach_shadow(&ShadowRootInit::new(ShadowRootMode::Open))?;
        add_event_listener(file_input, "change", self.handle_file_change())?;
        add_event_listener(chapter_input, "change", self.handle_chapter_change())?;
        add_event_listener(prev_button, "click", self.handle_arrows(Cmp::Less))?;
        add_event_listener(next_button, "click", self.handle_arrows(Cmp::More))?;
        add_event_listener(smaller_button, "click", self.handle_font(Cmp::Less))?;
        add_event_listener(larger_button, "click", self.handle_font(Cmp::More))?;
        add_event_listener(toc, "click", self.handle_click(true))?;
        add_event_listener(shadow_root, "click", self.handle_click(false))?;
        Ok(())
    }

    fn handle_click(&self, is_toc: bool) -> EventHandler {
        let epub_ref = self.epub.clone();
        let handler = move |e: Event| -> JsResult<()> {
            let clicked_elem: Element = e.target().ok_or("no event target")?.dyn_into()?;
            let anchor;
            if clicked_elem.tag_name() == "A" {
                anchor = clicked_elem;
            } else if let Some(c) = clicked_elem.closest("a")? {
                anchor = c;
            } else {
                return Ok(());
            }
            let href = match anchor.get_attribute("href") {
                Some(s) => s,
                None => return Ok(()),
            };
            // open external links in a new tab
            if Url::parse(&href).is_ok() {
                anchor.set_attribute("target", "_blank")?;
                return Ok(());
            }
            e.prevent_default();
            let mut epub_option = epub_ref.borrow_mut();
            let epub = epub_option.as_mut().ok_or("no epub loaded yet")?;
            let content = if is_toc {
                epub.chapter_by_toc_link(&href)?
            } else {
                epub.chapter_by_link(&href)?
            };
            render_content(&content)?;
            let url = utils::parse_relative_url(&href)?;
            let fragment = match url.fragment() {
                Some(s) => s,
                None => return Ok(()),
            };
            let shadow_root = document()
                .ok_or("no document")?
                .get_element_by_id("content")
                .ok_or("no #content")?
                .shadow_root()
                .ok_or("no shadow root")?;
            if let Some(elem) = shadow_root.get_element_by_id(fragment) {
                elem.scroll_into_view();
            }
            Ok(())
        };
        Box::new(handler)
    }

    fn handle_arrows(&self, cmp: Cmp) -> EventHandler {
        let epub_ref = self.epub.clone();
        let handler = move |_| -> JsResult<()> {
            let mut epub_option = epub_ref.borrow_mut();
            let epub = epub_option.as_mut().ok_or("no epub loaded yet")?;
            let content = match cmp {
                Cmp::Less => epub.prev_chapter()?,
                Cmp::More => epub.next_chapter()?,
            };
            render_content(&content)?;
            Ok(())
        };
        Box::new(handler)
    }

    fn handle_font(&self, cmp: Cmp) -> EventHandler {
        let handler = move |_| -> JsResult<()> {
            let elem: HtmlElement = document()
                .ok_or("no document")?
                .get_element_by_id("content")
                .ok_or("no #content")?
                .dyn_into()?;
            let style = elem.style();
            let str_val = style.get_property_value("font-size")?;
            let old_val = str_val[0..str_val.len().saturating_sub(2)]
                .parse()
                .unwrap_or(FONT_SIZE_DEFAULT);
            let delta = match cmp {
                Cmp::Less => -FONT_SIZE_INCREMENT,
                Cmp::More => FONT_SIZE_INCREMENT,
            };
            let new_val = min(max(old_val + delta, FONT_SIZE_MIN), FONT_SIZE_MAX);
            style.set_property("font-size", &format!("{}px", new_val))?;
            Ok(())
        };
        Box::new(handler)
    }

    fn handle_chapter_change(&self) -> EventHandler {
        let epub_ref = self.epub.clone();
        let handler = move |e: Event| -> JsResult<()> {
            let input: HtmlInputElement = e.target().ok_or("no event target")?.dyn_into()?;
            let chapter_number: usize = input.value().parse().or(Err("not a valid number"))?;
            let mut epub_option = epub_ref.borrow_mut();
            let epub = epub_option.as_mut().ok_or("no epub loaded yet")?;
            let content = epub.chapter(chapter_number.saturating_sub(1))?;
            render_content(&content)?;
            Ok(())
        };
        Box::new(handler)
    }

    fn handle_file_change(&self) -> EventHandler {
        let onload_rc = Rc::new(Closure::wrap(self.handle_file_load()));
        let handler = move |e: Event| -> JsResult<()> {
            let input: HtmlInputElement = e.target().ok_or("no event target")?.dyn_into()?;
            let file_list = input.files().ok_or("no files in input")?;
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
    fn handle_file_load(&self) -> EventHandler {
        let epub_ref = self.epub.clone();
        let handler = move |e: Event| -> JsResult<()> {
            let file_reader: FileReader = e.target().ok_or("no event target")?.dyn_into()?;
            let contents_buf: ArrayBuffer = file_reader.result()?.into();
            let mut bytes = vec![0; contents_buf.byte_length() as usize];
            Uint8Array::new(&contents_buf).copy_to(&mut bytes);
            let mut epub_option = epub_ref.borrow_mut();
            *epub_option = Some(Epub::new(bytes)?);
            let epub = epub_option.as_mut().ok_or("no epub")?;
            let first_chapter = epub.chapter(0)?;
            render_count(epub.doc_count()?)?;
            render_toc(&epub.toc()?)?;
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

fn add_event_listener<T>(target: T, event: &str, handler: EventHandler) -> JsResult<()>
where
    T: Into<EventTarget>,
{
    let handler_cl = Closure::wrap(handler);
    let event_target = target.into();
    event_target.add_event_listener_with_callback(event, handler_cl.as_ref().unchecked_ref())?;
    handler_cl.forget();
    Ok(())
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

fn render_toc(toc: &[TocItem]) -> JsResult<()> {
    let document = document().ok_or("no document")?;
    let ul = document.get_element_by_id("toc").ok_or("no #toc")?;
    ul.set_inner_html("");
    for item in toc {
        let li = document.create_element("li")?;
        let anchor: HtmlElement = document.create_element("a")?.dyn_into()?;
        anchor.set_attribute("href", &item.href)?;
        anchor.set_inner_text(&item.text);
        li.append_child(&anchor)?;
        ul.append_child(&li)?;
    }
    Ok(())
}

fn render_content(content: &str) -> JsResult<()> {
    let content_div = document()
        .ok_or("no document")?
        .get_element_by_id("content")
        .ok_or("no #content")?;
    let shadow_root = content_div.shadow_root().ok_or("no shadow root")?;
    shadow_root.set_inner_html(content);
    content_div.scroll_with_x_and_y(0.0, 0.0);
    Ok(())
}
