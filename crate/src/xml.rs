use crate::error::Result;
use minidom::{Children, Element};
use std::io::{BufReader, Read};

pub trait Descend {
    fn descendants(&self) -> Descendants;
}

pub struct Descendants<'a> {
    stack: Vec<Children<'a>>,
}

impl Descend for minidom::Element {
    fn descendants(&self) -> Descendants {
        let stack = vec![self.children()];
        Descendants { stack }
    }
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

pub fn parse_xml<R: Read>(contents: R) -> Result<Element> {
    let buf_reader = BufReader::new(contents);
    let mut xml_reader = quick_xml::Reader::from_reader(buf_reader);
    Ok(Element::from_reader(&mut xml_reader)?)
}
