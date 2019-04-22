use crate::epub::{Epub, TocItem};
use crate::utils;
use futures::Future;
use js_sys::{ArrayBuffer, Promise, Uint8Array};
use std::cell::RefCell;
use std::cmp::{max, min};
use std::rc::Rc;
use url::Url;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{future_to_promise, JsFuture};
use web_sys::{
    Document, Element, Event, EventTarget, FileReader, HtmlElement, HtmlInputElement, Response,
    ShadowRootInit, ShadowRootMode,
};

const FONT_SIZE_DEFAULT: isize = 20;
const FONT_SIZE_INCREMENT: isize = 2;
const FONT_SIZE_MIN: isize = 6;
const FONT_SIZE_MAX: isize = 60;
type EpubRef = Rc<RefCell<Option<Epub>>>;
type JsResult<T> = std::result::Result<T, JsValue>;
type EventHandler = Box<FnMut(Event) -> JsResult<()>>;
trait OnceEventHandler: FnOnce(Event) -> JsResult<()> + 'static {}
impl<T: FnOnce(Event) -> JsResult<()> + 'static> OnceEventHandler for T {}

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
    epub: EpubRef,
}

impl LeedorApp {
    pub fn new() -> LeedorApp {
        LeedorApp {
            epub: Rc::new(RefCell::new(None)),
        }
    }

    pub fn run(&self) -> JsResult<()> {
        utils::set_panic_hook();
        let document = document()?;
        let file_input = document.get_element_by_id("file").ok_or("no #file")?;
        let prev_button = document.get_element_by_id("prev").ok_or("no #prev")?;
        let next_button = document.get_element_by_id("next").ok_or("no #next")?;
        let smaller_button = document.get_element_by_id("smaller").ok_or("no #smaller")?;
        let larger_button = document.get_element_by_id("larger").ok_or("no #larger")?;
        let toggle_toc = document
            .get_element_by_id("toggle-toc")
            .ok_or("no #toggle-toc")?;
        let toc = document.get_element_by_id("toc").ok_or("no #toc")?;
        let content = document.get_element_by_id("content").ok_or("no #content")?;
        let shadow_root = content.attach_shadow(&ShadowRootInit::new(ShadowRootMode::Open))?;
        let samples = document.get_element_by_id("samples").ok_or("no #samples")?;
        add_event_listener(file_input, "change", self.handle_file_change())?;
        add_event_listener(prev_button, "click", self.handle_arrows(Cmp::Less))?;
        add_event_listener(next_button, "click", self.handle_arrows(Cmp::More))?;
        add_event_listener(smaller_button, "click", self.handle_font(Cmp::Less))?;
        add_event_listener(larger_button, "click", self.handle_font(Cmp::More))?;
        add_event_listener(toggle_toc, "click", self.handle_toggle_toc())?;
        add_event_listener(toc, "click", self.handle_click(true))?;
        add_event_listener(shadow_root, "click", self.handle_click(false))?;
        add_once_event_listener(samples, "click", self.handle_sample_click())?;
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
            let shadow_root = document()?
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
            let elem: HtmlElement = document()?
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

    fn handle_toggle_toc(&self) -> EventHandler {
        let handler = move |_| -> JsResult<()> {
            let toc_nav = document()?
                .get_element_by_id("toc-nav")
                .ok_or("no #toc-nav")?;
            toc_nav.class_list().toggle("hidden")?;
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
            let array_buffer: ArrayBuffer = file_reader.result()?.into();
            load_from_buffer(&epub_ref, &array_buffer)
        };
        Box::new(handler)
    }

    fn handle_sample_click(&self) -> impl OnceEventHandler {
        let epub_ref = self.epub.clone();
        |e: Event| -> JsResult<()> {
            e.prevent_default();
            let clicked_elem: Element = e.target().ok_or("no event target")?.dyn_into()?;
            let href = clicked_elem.get_attribute("href").ok_or("no href")?;
            let window = web_sys::window().ok_or("no window")?;
            let fetch_promise = window.fetch_with_str(&format!("static/{}.epub", &href[1..]));
            let future = JsFuture::from(fetch_promise)
                .and_then(|response_val: JsValue| -> JsResult<Promise> {
                    let response: Response = response_val.into();
                    response.array_buffer()
                })
                .and_then(JsFuture::from)
                .and_then(move |array_buffer_val: JsValue| -> JsResult<JsValue> {
                    let array_buffer: ArrayBuffer = array_buffer_val.into();
                    load_from_buffer(&epub_ref, &array_buffer)?;
                    Ok(JsValue::from(0))
                });
            future_to_promise(future);
            Ok(())
        }
    }
}

fn document() -> JsResult<Document> {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            return Ok(document);
        }
    }
    Err(JsValue::from("no document"))
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

fn add_once_event_listener<T, H>(target: T, event: &str, handler: H) -> JsResult<()>
where
    T: Into<EventTarget>,
    H: OnceEventHandler,
{
    let handler_cl = Closure::once(handler);
    let event_target = target.into();
    event_target.add_event_listener_with_callback(event, handler_cl.as_ref().unchecked_ref())?;
    handler_cl.forget();
    Ok(())
}

fn load_from_buffer(epub_ref: &EpubRef, array_buffer: &ArrayBuffer) -> JsResult<()> {
    let mut bytes = vec![0; array_buffer.byte_length() as usize];
    Uint8Array::new(&array_buffer).copy_to(&mut bytes);
    let mut epub_option = epub_ref.borrow_mut();
    *epub_option = Some(Epub::new(bytes)?);
    let epub = epub_option.as_mut().ok_or("no epub")?;
    let first_chapter = epub.chapter(0)?;
    render_toc(&epub.toc()?)?;
    render_content(&first_chapter)
}

fn render_toc(toc: &[TocItem]) -> JsResult<()> {
    let document = document()?;
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
    let document = document()?;
    let welcome = document.get_element_by_id("welcome").ok_or("no #welcome")?;
    welcome.class_list().add_1("hidden")?;
    let content_div = document.get_element_by_id("content").ok_or("no #content")?;
    let shadow_root = content_div.shadow_root().ok_or("no shadow root")?;
    shadow_root.set_inner_html(content);
    content_div.scroll_with_x_and_y(0.0, 0.0);
    Ok(())
}
