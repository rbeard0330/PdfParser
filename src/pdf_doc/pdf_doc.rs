#[path = "pdf_file/pdf_file.rs"]
mod pdf_file;
#[path = "pdf_objects/pdf_objects.rs"]
mod pdf_objects;

use std::fmt;
use std::rc::Rc;

use crate::errors::*;
pub use pdf_file::*;
use pdf_objects::*;

struct PdfDoc {
    file: PdfFileHandler,
    page_tree: Node,
    root: SharedObject,
}

// ----------Node-------------

#[derive(Debug, Clone)]
struct Node {
    children: Vec<Node>,
    contents: Option<SharedObject>,
    attributes: Vec<SharedObject>,
}

impl Node {
    fn new() -> Node {
        Node {
            children: Vec::new(),
            contents: None,
            attributes: Vec::new(),
        }
    }

    fn _display(&self, indent: usize, f: &mut fmt::Formatter<'_>) -> Result<()> {
        _write_indented_line(f, format!("Contents: {:?}", self.contents), indent)?;
        _write_indented_line(f, format!("Attributes: {:?}", self.attributes), indent)?;
        _write_indented_line(f, format!("Children({}):", self.children.len()), indent)?;
        for child in &self.children {
            child._display(indent + 2, f)?;
        }
        Ok(())
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<()> {
        write!(
            f,
            "Node\nContents: {:?}\nAttributes:{:?}\nChildren({}):",
            self.contents,
            self.attributes,
            self.children.len()
        )?;
        for child in &self.children {
            child._display(2, f)?;
        }
        Ok(())
    }
}

fn _write_indented_line(f: &mut fmt::Formatter<'_>, s: String, indent: usize) -> fmt::Result {
    let indent = String::from_utf8(vec![b' '; indent]).unwrap();
    write!(f, "{}{}\n", indent, s)?;
    Ok(())
}

impl PdfDoc {
    fn create_pdf_from_file(path: &str) -> Result<Self> {
        let mut file = PdfFileHandler::create_pdf_from_file(path)?;
        let root = file.get_root()?;
        let mut pdf = PdfDoc {
            file: file,
            page_tree: Node::new(),
            root: root,
        };
        Ok(pdf)
    }

    fn parse_page_tree(&mut self) -> Result<()> {
        let pages = self.root.get("Pages")?;
        let mut page_tree_catalog = Node::new();
        println!("Page ref: {:?}", pages);
        page_tree_catalog
            .attributes
            .push(pages);
        self.expand_page_tree(&mut page_tree_catalog)?;
        self.page_tree = page_tree_catalog;
        Ok(())
    }

    fn expand_page_tree(&mut self, node: &mut Node) -> Result<()> {
        let node_dict_ref =
            node.attributes
                .last()
                .unwrap()
                .try_into_map()
                .chain_err(|| ErrorKind::ParsingError(format!(
                    "No dict in node: {:?}",
                    node
                )))?;
        println!("attributes: {:?}", node_dict_ref);
        node_dict_ref.get("Contents");
        println!("contents: {:?}", node.contents);
        let kids_result = match node_dict_ref.get("Kids") {
            None => return Ok(()),
            Some(obj) => obj,
        };
        println!("kids: {:?}", kids_result);
        let kids = kids_result
            .try_into_array()
            .chain_err(|| ErrorKind::ParsingError(format!(
                "Invalid Kids dict: {}",
                kids_result
            )))?;
        let mut child_nodes = Vec::new();
        for kid in kids.into_iter() {
            let mut kid_node = Node::new();
            kid_node.attributes = node.attributes.clone();
            kid_node.attributes.push(Rc::clone(kid));
            self.expand_page_tree(&mut kid_node)?;
            child_nodes.push(kid_node);
        }
        //println!("child nodes: {:?}", child_nodes);
        node.children = child_nodes;

        Ok(())
    }
}

fn main() {
    let mut pdf_doc = PdfDoc::create_pdf_from_file("data/document.pdf").unwrap();
    //let mut pdf_file = PdfFileHandler::create_pdf_from_file("data/treatise.pdf").unwrap();
    pdf_doc.parse_page_tree().expect("Error");
    println!("{}", pdf_doc.page_tree);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_data() -> HashMap<&'static str, PDFVersion> {
        let mut data = HashMap::new();
        data.insert("data/simple_pdf.pdf", PDFVersion::V1_7);
        data.insert("data/CCI01212020.pdf", PDFVersion::V1_3);
        data.insert("data/document.pdf", PDFVersion::V1_4);
        data.insert("data/2018W2.pdf", PDFVersion::V1_4);
        data
    }

    #[test]
    fn object_imports() {
        let test_pdfs = test_data();
        for (path, version) in test_pdfs {
            let pdf = PdfFileHandler::create_pdf_from_file(path);
            let pdf = match pdf {
                Ok(val) => val,
                Err(e) => {
                    println!("{:?}", e);
                    panic!();
                }
            };
            assert_eq!(pdf.version, Some(version));
        }
    }

    #[test]
    fn page_trees() {
        let test_pdfs = test_data();
        for (path, _version) in test_pdfs {
            println!("{}", path);
            let mut pdf = PdfDoc::create_pdf_from_file(path).unwrap();
            pdf.parse_page_tree();
        }
    }
}
