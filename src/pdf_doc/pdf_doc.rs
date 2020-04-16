#[path = "pdf_file/pdf_file.rs"]
mod pdf_file;
#[path = "pdf_objects/pdf_objects.rs"]
mod pdf_objects;
mod page;

use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::errors::*;
use ErrorKind::*;
use vec_tree;

pub use pdf_file::*;
use page::Page;

type TreeIndex = vec_tree::Index;
struct DocTree {}

#[derive(Debug)]
pub struct PdfDoc {
    file: Parser,
    page_tree: PageTree,
    root: SharedObject,
}

impl PdfDoc {
    pub fn pages(&self) -> Pages {
        Pages {
            page_count: self.page_tree.page_count().unwrap_or_default(),
            tree: &self.page_tree,
            current_page: 0
        }
    }
    pub fn page_count(&self) -> usize {
        self.page_tree.page_count().unwrap()
    }
}

//TODO: Reimplement here
pub fn get_version(bytes: &Vec<u8>) -> Result<PDFVersion> {
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

impl Node {
    pub fn get(&self, key: &str) -> Option<SharedObject> {
        self.attributes.get(key).map(|obj| Rc::clone(obj))
    }
    pub fn is_page(&self) -> bool {
        match self.node_type {
            NodeType::Page => true,
            _ => false
        }
    }
    pub fn has_contents(&self) -> bool {
        self.contents.is_some()
    }
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
pub struct PageTree {
    tree: vec_tree::VecTree<Node>,
}

impl PageTree {
    fn new(root: &PdfObject) -> Result<Self> {
        let mut new_tree = PageTree{ tree: vec_tree::VecTree::new() };
        new_tree.add_node(root, None)?;
        Ok(new_tree)
    }

    fn add_node(&mut self, new_node: &PdfObject, target_index: Option<TreeIndex>) -> Result<()> {
        //println!("Adding {:?} to tree", new_node);
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

    fn root(&self) -> Option<&Node> {
        let root_index = self.tree.get_root_index();
        match root_index {
            None => None,
            Some(ix) => Some(&self.tree[ix])
        }

    }

    fn page_count(&self) -> Result<usize> {
        match self.root() {
            None => Err(ParsingError(format!("No root !")))?,
            Some(node) => {
                match node.get("Count") {
                    None => Err(ParsingError(format!("No /Count entry in root!")))?,
                    Some(obj) => Ok(obj.try_into_int()? as usize)
                }
            }
        }
    }
    
    pub fn get_page(&self, page_number: usize) -> Option<page::Page> {
        if page_number > self.page_count().ok()? { return None };
        let mut current_node = self.tree.get_root_index()?;
        let mut pages_passed = 0;
        loop {
            let starting_node = current_node;
            for child in self.tree.children(current_node) {
                let this_child = &self.tree[child];
                //println!("Current node: {}", this_child);
                if this_child.is_page() {
                    if pages_passed == page_number - 1 {
                        return Some(page::Page::new_from_index(child, &self))
                    };
                    pages_passed += 1;
                    continue
                }
                let this_child_pages = this_child
                    .get("Count")
                    .map(
                        |obj| obj.try_into_int().unwrap_or(0)
                    )
                    .unwrap_or(0) as usize;
                let next_pages = pages_passed + this_child_pages;
                if next_pages >= page_number {
                    current_node = child;
                    break
                } else {
                    pages_passed = next_pages;
                }
            }
            if starting_node == current_node { return None };
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

struct Pages<'a> {
    page_count: usize,
    tree: &'a PageTree,
    current_page: usize
}

impl<'a> Iterator for Pages<'a> {
    type Item = Page<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        self.current_page += 1;
        self.tree.get_page(self.current_page)
    }
}

impl PdfDoc {
    pub fn create_pdf_from_file(path: &str) -> Result<Self> {
        let file = Parser::create_pdf_from_file(path)?;
        let trailer_dict = file.retrieve_trailer()?
                               .try_into_map()
                               .unwrap();
        let root = trailer_dict.get("Root").ok_or(ErrorKind::ParsingError("Root not present in trailer!".to_string()))?;
        let root_dict = root.try_into_map()
                .chain_err(|| ErrorKind::ParsingError("Root not a dictionary!".to_string()))?;
        let pages_root = root_dict.get("Pages")
                .ok_or(ErrorKind::ParsingError("Pages not present in root!".to_string()))?;
        let pdf = PdfDoc {
            file: file,
            page_tree: PageTree::new(pages_root)?,
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
        data.insert("data/f1120.pdf", PDFVersion::V1_7);
        data.insert("data/PDF32000_2008.pdf", PDFVersion::V1_6);
        //data.insert("data/who.pdf", PDFVersion::V1_7);
        data.insert("data/treatise.pdf", PDFVersion::V1_6);
        data
    }

    #[test]
    fn object_imports() {
        let test_pdfs = test_data();
        // TODO: Add version checks too
        let mut had_errors = false;
        for (path, _version) in test_pdfs {
            
            let pdf = Parser::create_pdf_from_file(path);
            match pdf {
                Ok(_val) => {},
                Err(e) => {
                    error!("Error processing {}: {:?}", path, e);
                    had_errors = true;
                }
            };
        };
        if had_errors { panic!() };
    }

    #[test]
    fn page_trees() {
        let test_pdfs = test_data();
        for (path, _version) in test_pdfs {
            println!("{}", path);
            PdfDoc::create_pdf_from_file(path).unwrap();
        }
    }

    #[test]
    fn page_iter() {
        let test_pdfs = test_data();
        for (path, _version) in test_pdfs {
            println!("{}", path);
            let doc = PdfDoc::create_pdf_from_file(path).unwrap();
            println!("Root: {}", doc.page_tree.root().unwrap());
            println!("Page count: {:?}", doc.page_count());
            let mut counter = 0;
            for (page_num, page) in doc.pages().enumerate() {
                //println!("Page {}: {}", page_num + 1, page);
                counter += 1;
            }
            assert_eq!(doc.page_count(), counter);
        }
    }

    #[test]
    fn get_page() {
        let doc = PdfDoc::create_pdf_from_file("data/PDF32000_2008.pdf").unwrap();
        let page_count = 756;
        for page in 1..page_count {
            doc.page_tree.get_page(page).unwrap();
            //println!("Page {}: {}", page, doc.page_tree.get_page(page).unwrap());
        }
    }

    fn get_specific_page() {
        let source = "data/PDF32000_2008.pdf";
        let doc = PdfDoc::create_pdf_from_file(source).unwrap();
        doc.page_tree.get_page(1).unwrap();
        //println!("Page {}: {}", page, doc.page_tree.get_page(page).unwrap());
    }
}
