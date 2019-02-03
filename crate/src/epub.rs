use crate::error::Result;
use minidom::{Children, Element};
use std::collections::HashMap;
use std::io::{BufReader, Cursor};
use std::path::Path;
use zip::ZipArchive;

struct Descendants<'a> {
    stack: Vec<Children<'a>>,
}

impl<'a> Iterator for Descendants<'a> {
    type Item = &'a Element;

    fn next(&mut self) -> Option<&'a Element> {
        while let Some(iter) = self.stack.last_mut() {
            if let Some(elem) = iter.next() {
                self.stack.push(elem.children());
                return Some(elem);
            } else {
                self.stack.pop();
            }
        }
        None
    }
}

trait Descend {
    fn descendants(&self) -> Descendants;
}

impl Descend for minidom::Element {
    fn descendants(&self) -> Descendants {
        let stack = vec![self.children()];
        Descendants { stack }
    }
}

type ItemId = String;
type Manifest = HashMap<ItemId, ManifestItem>;
type Spine<'a> = Vec<&'a ManifestItem>;
type Zip = ZipArchive<Cursor<Vec<u8>>>;

#[derive(Debug)]
struct ManifestItem {
    id: ItemId,
    href: String,
    media_type: String,
}

#[derive(Debug)]
pub struct Epub {
    manifest: Manifest,
    opf_doc: Element,
    opf_path: String,
    zip: Zip,
}

impl Epub {
    // TODO: clean up. Implement &[u8] constructor.
    pub fn new(bytes: Vec<u8>) -> Result<Epub> {
        let mut zip = ZipArchive::new(Cursor::new(bytes))?;
        let container_doc = parse_xml(&mut zip, "META-INF/container.xml")?;
        let rootfile_node = container_doc
            .descendants()
            .find(|n| n.name() == "rootfile")
            .ok_or("no rootfile in container.xml")?;
        let opf_path = rootfile_node
            .attr("full-path")
            .ok_or("no full-path attribute in rootfile")?
            .to_string();
        let opf_doc = parse_xml(&mut zip, &opf_path)?;
        let manifest_node = opf_doc
            .children()
            .find(|n| n.name() == "manifest")
            .ok_or("manifest element missing in OPF")?;
        let manifest: Manifest = manifest_node
            .children()
            .filter(|c| c.name() == "item")
            .map(|i| {
                let id = i.attr("id").expect("id missing in item").to_string();
                let href = i.attr("href").expect("href missing in item").to_string();
                let media_type = i
                    .attr("media-type")
                    .expect("media_type missing in item")
                    .to_string();
                (
                    id.clone(),
                    ManifestItem {
                        id,
                        href,
                        media_type,
                    },
                )
            })
            .collect();
        let epub = Epub {
            manifest,
            opf_doc,
            opf_path,
            zip,
        };
        Ok(epub)
    }

    pub fn doc_count(&self) -> Result<usize> {
        let spine = self.spine()?;
        Ok(spine.len())
    }

    // TODO: replace img and css with base64 data URLs.
    pub fn chapter(&mut self, item_idx: usize) -> Result<String> {
        let spine = self.spine()?;
        let doc_href = spine
            .get(item_idx)
            .ok_or("item_idx not in spine")?
            .href
            .clone();
        let first_doc = self.parse_xml(&doc_href)?;
        let mut doc_bytes: Vec<u8> = Vec::new();
        first_doc.write_to(&mut doc_bytes)?;
        // let img_nodes: Vec<_> = first_doc.descendants().filter(|n| n.name() == "img").collect();
        // dbg!(img_nodes);
        Ok(String::from_utf8(doc_bytes)?)
    }

    // TODO: memoize spine.
    fn spine(&self) -> Result<Spine> {
        let spine = self
            .spine_node()?
            .children()
            .filter(|c| c.name() == "itemref")
            .map(|i| {
                let idref = i.attr("idref").expect("idref missing in item");
                self.manifest
                    .get(idref)
                    .expect("idref in spine not defined in manifest")
            })
            .collect();
        Ok(spine)
    }

    fn spine_node(&self) -> Result<&Element> {
        let spine_node = self
            .opf_doc
            .children()
            .find(|n| n.name() == "spine")
            .ok_or("spine element missing in OPF")?;
        Ok(spine_node)
    }

    // TODO: find a way to avoid needing `.clone()`.
    fn parse_xml(&mut self, filename: &str) -> Result<Element> {
        parse_xml(&mut self.zip, &relative_path(&filename, &self.opf_path))
    }

    // TODO: the NCX file is superseded and marked for removal in EPUB 3.
    #[allow(dead_code)]
    fn ncx_doc(&mut self) -> Result<Element> {
        let ncx_id = self
            .spine_node()?
            .attr("toc")
            .expect("toc missing in spine");
        let ncx_item = self
            .manifest
            .get(ncx_id)
            .ok_or("toc in spine not defined in manifest")?;
        let ncx_doc = self.parse_xml(&ncx_item.href.clone())?;
        Ok(ncx_doc)
    }
}

fn parse_xml(zip: &mut Zip, filename: &str) -> Result<Element> {
    let buf_reader = BufReader::new(zip.by_name(filename)?);
    let mut xml_reader = quick_xml::Reader::from_reader(buf_reader);
    Ok(Element::from_reader(&mut xml_reader)?)
}

fn relative_path<'p>(path: &'p str, relative_to: &str) -> String {
    match Path::new(relative_to).parent() {
        Some(dir) => dir.join(path).to_str().expect("invalid path").to_string(),
        None => path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Result<T> = std::result::Result<T, Box<std::error::Error>>;

    #[test]
    fn read_first_chapter() -> Result<()> {
        let bytes = std::fs::read("tests/ebooks/mice.epub")?;
        let mut epub = Epub::new(bytes)?;
        let chapter_html = epub.chapter(0)?;
        assert!(chapter_html.contains("epub:type=\"cover\""));
        Ok(())
    }

    #[test]
    fn doc_count() -> Result<()> {
        let bytes = std::fs::read("tests/ebooks/mice.epub")?;
        let epub = Epub::new(bytes)?;
        let count = epub.doc_count()?;
        assert_eq!(15, count);
        Ok(())
    }
}
