use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::Path;
use zip::ZipArchive;

type ItemId = String;
type Result<T> = std::result::Result<T, Box<std::error::Error>>;
type Spine<'s> = Vec<&'s ManifestItem>;

#[derive(Debug)]
struct ManifestItem {
    id: ItemId,
    href: String,
    media_type: String,
}

pub fn read_opf(bytes: &[u8]) -> Result<String> {
    let mut zip = ZipArchive::new(Cursor::new(bytes))?;
    let mut buf = String::new();
    let container_doc = parse_xml(&mut zip, "META-INF/container.xml", &mut buf)?;
    let rootfile_node = container_doc
        .descendants()
        .find(|n| n.tag_name().name() == "rootfile")
        .ok_or("no rootfile found in container.xml")?;
    let opf_path = rootfile_node
        .attribute("full-path")
        .ok_or("no full-path attribute in rootfile")?;
    let mut buf = String::new();
    let opf_doc = parse_xml(&mut zip, opf_path, &mut buf)?;
    let manifest_node = opf_doc
        .descendants()
        .find(|n| n.tag_name().name() == "manifest")
        .ok_or("manifest element missing in OPF")?;
    let manifest: HashMap<ItemId, ManifestItem> = manifest_node
        .children()
        .filter(|c| c.tag_name().name() == "item")
        .map(|i| {
            let id = i.attribute("id").expect("id missing in item").to_string();
            let href = i
                .attribute("href")
                .expect("href missing in item")
                .to_string();
            let media_type = i
                .attribute("media-type")
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
    let spine_node = opf_doc
        .descendants()
        .find(|n| n.tag_name().name() == "spine")
        .ok_or("spine element missing in OPF")?;
    let spine: Spine = spine_node
        .children()
        .filter(|c| c.tag_name().name() == "itemref")
        .map(|i| {
            let idref = i.attribute("idref").expect("idref missing in item");
            manifest
                .get(idref)
                .expect("idref in spine not defined in manifest")
        })
        .collect();
    let ncx_id = spine_node.attribute("toc").expect("toc missing in spine");
    // TODO: the NCX file is superseded and marked for removal in EPUB 3
    let ncx_item = manifest
        .get(ncx_id)
        .expect("toc in spine not defined in manifest");
    let mut buf = String::new();
    let _ncx_doc = parse_xml(&mut zip, &relative_path(&ncx_item.href, opf_path), &mut buf)?;
    let first_item = spine.first().ok_or("empty spine")?;
    let mut buf = String::new();
    let first_doc = parse_xml(
        &mut zip,
        &relative_path(&first_item.href, opf_path),
        &mut buf,
    )?;
    let _body_node = first_doc
        .descendants()
        .find(|n| n.tag_name().name() == "body");
    Ok(buf)
}

fn relative_path<'p>(path: &'p str, relative_to: &str) -> String {
    match Path::new(relative_to).parent() {
        Some(dir) => dir.join(path).to_str().expect("invalid path").to_string(),
        None => path.to_string(),
    }
}

fn parse_xml<'a>(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    filename: &str,
    buf: &'a mut String,
) -> Result<roxmltree::Document<'a>> {
    archive.by_name(filename)?.read_to_string(buf)?;
    let doc = roxmltree::Document::parse(buf)?;
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_epub() -> Result<()> {
        let bytes = std::fs::read("tests/ebooks/mice.epub")?;
        let contents = read_opf(&bytes);
        dbg!(contents?);
        Ok(assert!(false))
    }
}
