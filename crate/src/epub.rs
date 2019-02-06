use crate::error::Result;
use crate::xml::{parse_xml, Descend};
use minidom::Element;
use std::collections::HashMap;
use std::io::{BufReader, Cursor, Read};
use std::path::{Path, PathBuf};
use url::Url;
use zip::ZipArchive;

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
    base_url: Url,
    current_path: PathBuf,
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
        Ok(Epub {
            base_url: Url::parse("https://leedor.jreyes.org")?,
            current_path: opf_path.clone(),
            manifest,
            opf_doc,
            opf_path,
            zip,
        })
    }

    pub fn doc_count(&self) -> Result<usize> {
        let spine = self.spine()?;
        Ok(spine.len())
    }

    pub fn chapter(&mut self, item_idx: usize) -> Result<String> {
        let spine = self.spine()?;
        let item = spine.get(item_idx).ok_or("item_idx not in spine")?;
        self.current_path = resolve_path(&item.href, &self.opf_path);
        self.current_chapter()
    }

    pub fn chapter_by_link(&mut self, link: &str) -> Result<String> {
        let url = Url::options().base_url(Some(&self.base_url)).parse(link)?;
        let path = &url.path()[1..]; // drop the slash
        self.current_path = resolve_path(path, &self.current_path);
        self.current_chapter()
    }

    fn current_chapter(&mut self) -> Result<String> {
        let path_str = self.current_path.to_str().ok_or("invalid path")?;
        let doc_file = self.zip.by_name(path_str)?;
        let mut doc = parse_xml(doc_file)?;
        self.inline_resources(&mut doc)?;
        let mut doc_bytes = vec![];
        doc.write_to(&mut doc_bytes)?;
        Ok(String::from_utf8(doc_bytes)?)
    }

    // TODO: make it non-recursive.
    fn inline_resources(&mut self, elem: &mut Element) -> Result<()> {
        for c in elem.children_mut() {
            self.inline_resources(c)?;
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
        let resource_path = resolve_path(img_href, &self.current_path);
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
            let item_path = resolve_path(&i.href, &self.opf_path);
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

fn resolve_path<'a>(path_str: &'a str, relative_to: &'a Path) -> PathBuf {
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

    lazy_static::lazy_static! {
        static ref BYTES: Vec<u8> = std::fs::read("tests/ebooks/rosa.epub").unwrap();
    }

    type Result<T> = std::result::Result<T, Box<std::error::Error>>;

    #[test]
    fn doc_count() -> Result<()> {
        let epub = Epub::new(BYTES.clone())?;
        let count = epub.doc_count()?;
        assert_eq!(3, count);
        Ok(())
    }

    #[test]
    fn read_first_chapter() -> Result<()> {
        let mut epub = Epub::new(BYTES.clone())?;
        let chapter_html = epub.chapter(0)?;
        assert!(chapter_html.contains("<h1 id=\"pgepubid00000\">BRIEFE AUS DEM GEFÄNGNIS</h1>"));
        Ok(())
    }

    #[test]
    fn images_replaced_with_data_url() -> Result<()> {
        let mut epub = Epub::new(BYTES.clone())?;
        let chapter_html = epub.chapter(0)?;
        assert!(chapter_html.contains("<img alt=\"\" src=\"data:image/png;base64,"));
        Ok(())
    }

    #[test]
    fn read_chapter_by_link() -> Result<()> {
        let mut epub = Epub::new(BYTES.clone())?;
        epub.chapter(0)?; // open the chapter with the link
        let link =
            "@public@vhost@g@gutenberg@html@files@26964@26964-h@26964-h-2.htm.html#Footnote_1_1";
        let chapter_html = epub.chapter_by_link(link)?;
        assert!(chapter_html.contains("id=\"Footnote_1_1\""));
        Ok(())
    }
}
