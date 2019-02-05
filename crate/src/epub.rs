use crate::error::Result;
use minidom::{Children, Element};
use std::collections::HashMap;
use std::io::{BufReader, Cursor, Read};
use std::path::{Path, PathBuf};
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
type Spine<'a> = Vec<&'a ManifestItem>;
type Zip = ZipArchive<Cursor<Vec<u8>>>;
type Manifest = HashMap<ItemId, ManifestItem>;

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
    opf_path: PathBuf,
    zip: Zip,
}

impl Epub {
    // TODO: clean up. Implement &[u8] constructor.
    pub fn new(bytes: Vec<u8>) -> Result<Epub> {
        let mut zip = ZipArchive::new(Cursor::new(bytes))?;
        let container_doc = parse_xml(zip.by_name("META-INF/container.xml")?)?;
        let rootfile_node = container_doc
            .descendants()
            .find(|n| n.name() == "rootfile")
            .ok_or("no rootfile in container.xml")?;
        let opf_str_path = rootfile_node
            .attr("full-path")
            .ok_or("no full-path attribute in rootfile")?;
        let opf_file = zip.by_name(opf_str_path)?;
        let opf_path = opf_file.sanitized_name();
        let opf_doc = parse_xml(opf_file)?;
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

    pub fn chapter(&mut self, item_idx: usize) -> Result<String> {
        let spine = self.spine()?;
        let doc_href = spine
            .get(item_idx)
            .ok_or("item_idx not in spine")?
            .href
            .clone();
        let doc_path = relative_path(&doc_href, &self.opf_path);
        let doc_file = self.zip.by_name(doc_path.to_str().ok_or("invalid path")?)?;
        let mut doc = parse_xml(doc_file)?;
        self.inline_resources(&mut doc, &doc_path)?;
        let mut doc_bytes = Vec::new();
        doc.write_to(&mut doc_bytes)?;
        Ok(String::from_utf8(doc_bytes)?)
    }

    // TODO: make it non-recursive.
    fn inline_resources(&mut self, elem: &mut Element, doc_path: &Path) -> Result<()> {
        for c in elem.children_mut() {
            self.inline_resources(c, doc_path)?;
        }

        let attr_name = match elem.name() {
            "img" => "src",
            "image" => "xlink:href",
            "link" => "href",
            _ => return Ok(()),
        };
        let img_href = match elem.attr(attr_name) {
            Some(s) => s,
            None => return Ok(()),
        };
        let resource_path = relative_path(img_href, doc_path);
        let media_type = self.media_type(&resource_path).unwrap_or_default();
        let mut attr_value = format!("data:{};base64,", media_type);
        let path_str = resource_path.to_str().ok_or("invalid path")?;
        let img_file = self.zip.by_name(path_str)?;
        let mut bytes = vec![];
        let mut buf_reader = BufReader::new(img_file);
        buf_reader.read_to_end(&mut bytes)?;
        base64::encode_config_buf(&bytes, base64::STANDARD, &mut attr_value);
        elem.set_attr(attr_name, attr_value);
        Ok(())
    }

    fn media_type(&self, path: &Path) -> Option<&str> {
        let item_opt = self.manifest.values().find(|i| {
            let item_path = relative_path(&i.href, &self.opf_path);
            item_path == path
        });
        if let Some(item) = item_opt {
            return Some(&item.media_type);
        }
        None
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
        let ncx_doc = parse_xml(self.zip.by_name(&ncx_item.href)?)?;
        Ok(ncx_doc)
    }
}

fn parse_xml<R>(contents: R) -> Result<Element>
where
    R: std::io::Read,
{
    let buf_reader = BufReader::new(contents);
    let mut xml_reader = quick_xml::Reader::from_reader(buf_reader);
    Ok(Element::from_reader(&mut xml_reader)?)
}

fn relative_path<'a>(path_str: &'a str, relative_to: &'a Path) -> PathBuf {
    let mut built_path = PathBuf::from(relative_to);
    built_path.pop(); // remove relative_to file name
    for segment in Path::new(path_str) {
        if segment == ".." {
            built_path.pop();
        } else {
            built_path.push(segment);
        }
    }
    built_path
}

#[cfg(test)]
mod tests {
    use super::*;

    type Result<T> = std::result::Result<T, Box<std::error::Error>>;
    const BOOK_PATH: &'static str = "tests/ebooks/rosa.epub";

    #[test]
    fn read_first_chapter() -> Result<()> {
        let bytes = std::fs::read(BOOK_PATH)?;
        let mut epub = Epub::new(bytes)?;
        let chapter_html = epub.chapter(0)?;
        assert!(chapter_html.contains("<h1 id=\"pgepubid00000\">BRIEFE AUS DEM GEFÄNGNIS</h1>"));
        Ok(())
    }

    #[test]
    fn images_replaced_with_data_url() -> Result<()> {
        let bytes = std::fs::read(BOOK_PATH)?;
        let mut epub = Epub::new(bytes)?;
        let chapter_html = epub.chapter(0)?;
        assert!(chapter_html.contains("<img alt=\"\" src=\"data:image/png;base64,"));
        Ok(())
    }

    #[test]
    fn doc_count() -> Result<()> {
        let bytes = std::fs::read(BOOK_PATH)?;
        let epub = Epub::new(bytes)?;
        let count = epub.doc_count()?;
        assert_eq!(3, count);
        Ok(())
    }
}
