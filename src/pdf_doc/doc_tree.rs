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
pub struct PdfDoc {
    file: Parser,
    page_tree: PageTree,
    root: SharedObject,
}

//TODO: Reimplement here
fn get_version(bytes: &Vec<u8>) -> Result<PDFVersion> {
    let intro = String::from_utf8(
        bytes[..12]
            .iter()
            .map(|c| *c)
            //.take_while(|c| !is_eol(*c))
            .collect(),
    );
    let intro = match intro {
        Ok(s) if s.contains("%PDF-") => s,
        _ => {
            return Err(ErrorKind::ParsingError(format!(
                "Could not find version number in {:?}",
                intro
            )))?
        }
    };
    match intro // Syntax: %PDF-x.y
        .splitn(2, "%PDF-")  // Trim leading text
        .last()
        .ok_or(ErrorKind::ParsingError(format!(
            "Missing '%PDF-' marker")))?
        .split_at(3)  // Trim everything after the 3 version characters
        .0
        .split_at(1)  // Split out two two-character strings
    {
        ("1", ".0") => Ok(PDFVersion::V1_0),
        ("1", ".1") => Ok(PDFVersion::V1_1),
        ("1", ".2") => Ok(PDFVersion::V1_2),
        ("1", ".3") => Ok(PDFVersion::V1_3),
        ("1", ".4") => Ok(PDFVersion::V1_4),
        ("1", ".5") => Ok(PDFVersion::V1_5),
        ("1", ".6") => Ok(PDFVersion::V1_6),
        ("1", ".7") => Ok(PDFVersion::V1_7),
        ("2", ".0") => Ok(PDFVersion::V2_0),
        (x, y) => Err(ErrorKind::ParsingError(format!(
            "Unsupported PDF version: {}.{}",
            x, y
        )))?,
    }
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

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let display_contents = match self.contents {
            None => "with no contents".to_string(),
            Some(_) => "with contents".to_string()
        };
        writeln!(f, "Node of type {:?} {} and these attributes:", self.node_type, display_contents)?;
        for key in self.attributes.keys() {
            writeln!(f, "  {:?}", key)?
        }
        Ok(())
    }
}

#[derive(Debug)]
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
        println!("Adding {:?} to tree", new_node);
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
        // Verify required entries for node type
        match node_type {
            NodeType::Root => {
                let page_parent = node_map.get("Pages")
                        .ok_or(ErrorKind::DocTreeError(format!("Root node missing /Pages entry")))?;
                self.add_node(page_parent, Some(this_index))
            },
            NodeType::PageTreeIntermediate => {
                let kids_array = node_map.get("Kids")
                                     .ok_or(ErrorKind::DocTreeError(format!("Page tree node missing /Kids entry")))?;
                for kid in kids_array.try_into_array()
                                .chain_err(||
                                    ErrorKind::DocTreeError(
                                        format!("Could not resolve /Kids object into array: {:?}", kids)
                                    ))?
                                .as_ref() {
                    self.add_node(kid.as_ref(), Some(this_index))?;
                };
                Ok(())
            },
            _ => Ok(())
        }
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

impl fmt::Display for PageTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let root = self.tree.get_root_index().unwrap();
        for node in self.tree.descendants(root) {
            writeln!(f, "{}", self.tree.get(node).unwrap())?
        };
        Ok(())
    }
}

impl PdfDoc {
    pub fn create_pdf_from_file(path: &str) -> Result<Self> {
        let file = Parser::create_pdf_from_file(path)?;
        let trailer_dict = file.retrieve_trailer()?
                               .try_into_map()
                               .unwrap();
        let root = trailer_dict.get("Root").ok_or(ErrorKind::ParsingError("Root not present in trailer!".to_string()))?;
        let pdf = PdfDoc {
            file: file,
            page_tree: PageTree::new(&root)?,
            root: Rc::clone(root),
        };
        Ok(pdf)
    }
}

impl fmt::Display for PdfDoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.page_tree)?;
        Ok(())
    }
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
        // TODO: Add version checks too
        for (path, _version) in test_pdfs {
            let pdf = Parser::create_pdf_from_file(path);
            let pdf = match pdf {
                Ok(val) => val,
                Err(e) => {
                    println!("{:?}", e);
                    panic!();
                }
            };
        }
    }

    #[test]
    fn page_trees() {
        let test_pdfs = test_data();
        for (path, _version) in test_pdfs {
            println!("{}", path);
            PdfDoc::create_pdf_from_file(path).unwrap();
        }
    }
}
