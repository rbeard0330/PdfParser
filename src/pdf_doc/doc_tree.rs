#[path = "pdf_file/pdf_file.rs"]
mod pdf_file;
#[path = "pdf_objects/pdf_objects.rs"]
mod pdf_objects;

use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::errors::*;
use vec_tree::VecTree;

pub use pdf_file::*;
use pdf_objects::*;

type TreeIndex = vec_tree::Index;
struct DocTree {}

#[derive(Debug)]
struct PdfDoc {
    file: PdfFileHandler,
    page_tree: Node,
    root: SharedObject,
}

// ----------Node-------------

#[derive(Debug, Clone)]
struct Node {
    node_type: NodeType,
    contents: Option<SharedObject>,
    attributes: HashMap<String, SharedObject>,
}


#[derive(Debug, Clone, Copy)]
enum NodeType {
    Root,
    Page,
    PageTreeIntermediate,
    NotImplemented
}

struct PageTree {
    tree: VecTree<Node>,
}

impl PageTree {
    fn new(root: &PdfObject) -> Result<Self> {
        let mut new_tree = PageTree{ tree: VecTree::new() };
        new_tree.add_node(root, None)?;
        Ok(new_tree)
    }

    fn add_node(&mut self, new_node: &PdfObject, target_index: Option<TreeIndex>) -> Result<()> {
        let node_map = new_node.try_into_map()
                               .chain_err(|| ErrorKind::TestingError(
                                   format!("Expected dictionary, got {:?}", new_node))
                                )?;
        let node_type = node_map.get("Type")
                                .map(|obj| PageTree::_get_node_type(obj))
                                .ok_or(ErrorKind::DocTreeError(
                                    format!("No /Type key in node")
                                ))??;
        let kids = node_map.get("Kids");
        let new_node = Node{
            contents: node_map.get("Contents").map(|rc_ref| Rc::clone(rc_ref)),
            node_type,
            attributes: node_map.as_ref().clone()
        };
        
        let this_index = match target_index {
            None => self.tree.insert_root(new_node),
            Some(index) => self.tree.insert(new_node, index)
        };
        let kids = match node_type {
            NodeType::Root => node_map.get("Pages"),
            NodeType::PageTreeIntermediate => node_map.get("Kids"),
            _ => None
        };

                                
        Ok(())
    }

    fn _get_node_type(name: &PdfObject) -> Result<NodeType> {
        use NodeType::*;
        match &name.try_into_string()?[..] {
            "Pages" => Ok(PageTreeIntermediate),
            "Page" => Ok(Page),
            "Catalog" => Ok(Root),
            _ => Ok(NotImplemented)
        }
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
        let trailer_dict = file.retrieve_trailer()?
                               .try_into_map()
                               .unwrap();
        let root = trailer_dict.get("Root").ok_or(ErrorKind::ParsingError("Root not present in trailer!".to_string()))?;
        let mut pdf = PdfDoc {
            file: file,
            page_tree: Node::new(),
            root: Rc::clone(root),
        };
        Ok(pdf)
    }

    fn parse_page_tree(&mut self) -> Result<()> {
        println!("cache: {:?}", Rc::strong_count(&self.file.object_map));
        let pages = self.root.try_to_get("Pages")?;
        println!("Page ref: {:?}", pages);
        if let None = pages { Err(ErrorKind::ParsingError(format!("No Pages in {:?}", self.root)))? };
        let mut page_tree_catalog = Node::new();

        page_tree_catalog.attributes.push(Rc::clone(&pages.unwrap()));
        self.expand_page_tree(&mut page_tree_catalog)?;
        self.page_tree = page_tree_catalog;
        Ok(())
    }

    fn expand_page_tree(&mut self, node: &mut Node) -> Result<()> {
        let node_dict_ref = node
            .attributes
            .last()
            .unwrap()
            .try_into_map()
            .chain_err(|| ErrorKind::ParsingError(format!("No dict in node: {:?}", node)))?;
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
            .chain_err(|| ErrorKind::ParsingError(format!("Invalid Kids dict: {}", kids_result)))?;
        let mut child_nodes = Vec::new();
        for kid in kids.as_ref().into_iter() {
            let mut kid_node = Node::new();
            kid_node.attributes = node.attributes.clone();
            kid_node.attributes.push(Rc::clone(&kid));
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
            assert_eq!(pdf.version, version);
        }
    }

    #[test]
    fn page_trees() {
        let test_pdfs = test_data();
        for (path, _version) in test_pdfs {
            println!("{}", path);
            let mut pdf = PdfDoc::create_pdf_from_file(path).unwrap();
            println!("Current strong: {}", Rc::strong_count(&pdf.file.object_map));
            //println!("{:#?}", pdf.file.object_map);
            pdf.parse_page_tree();
        }
    }
}
