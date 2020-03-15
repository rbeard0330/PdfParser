mod pdf_file;
use self::pdf_file::*;
use std::rc::Rc;
use pdf_file::{PDFError, ObjectID};

struct PdfDoc {
    file: PdfFileHandler,
    page_tree: Node,
    root: SharedObject
}

type Link = Option<Vec<Box<Node>>>;
type SharedObject = Rc<PDFObj>;

#[derive(Debug, Clone, PartialEq)]
struct Node {
    children: Link,
    contents: Option<SharedObject>,
    attributes: Vec<SharedObject>
}

impl Node {
    fn new() -> Node {
        Node{
            children: None,
            contents: None,
            attributes: Vec::new()
        }
    }

    fn extend(&mut self, counter: u32) {

    }
}

impl PdfDoc {
    fn create_pdf_from_file(path: &str) -> Result<Self, PDFError> {
        let mut file = PdfFileHandler::create_pdf_from_file(path)?;
        let root = file.get_root()?;
        let mut pdf = PdfDoc {
            file: file,
            page_tree: Node::new(),
            root: root
        };
        Ok(pdf)
    }

    fn parse_page_tree(&mut self) -> Result<(), PDFError> {
        let pages = self.root.get("Pages")?.expect("No pages dict!");
        let mut page_tree_catalog = Node::new();

        let page_ref = pages.get_as_object_id()
                            .ok_or(PDFError{message: "No reference to Pages".to_string(),
                                        function: "parse_page_tree"})?;
        page_tree_catalog.attributes.push(self.file.get_object(&page_ref)?);
        self.expand_tree(&mut page_tree_catalog);
        Ok(())
    }

    fn expand_tree(&mut self, node: &mut Node) -> Result<(), PDFError> {
        let node_dict_ref = node.attributes
                                .last()
                                .unwrap()
                                .get_dict_ref()
                                .ok_or(PDFError{message: format!("No dict in node: {:?}", node),
                                                function: "parse_page_tree::expand_tree"})?;
        let kids = node_dict_ref.get("Kids")
                                .map();
    


        
        loop {
            let this_obj = objects_to_visit.pop();
            let this_ref = match this_obj {
                None => {break},
                Some(id_result) => id_result?
            };
            let node = self.file.get_object(&this_ref)?;
            println!("{:?}", node.get("Type"));
            if let &PDFObj::Dictionary{..} = &*node {} else {continue};
            let kids_array = node.get("Kids")?;
            if kids_array.is_none() {continue}
            match &**(kids_array.unwrap()) {
                    &PDFObj::Array(ref v) => {
                        objects_to_visit.extend(
                            v.iter().map(|item| match **item {
                                PDFObj::ObjectRef(id) => Ok(id),
                                _ => Err(PDFError{ message: format!("Invalid item in Kids array: {:?}", item),
                                                            function: "parse_page_tree"})
                            }));
                        },
                    _ => {}
                    
                };
        }
        Ok(())
    }

}

fn main() {
    let mut pdf_doc = PdfDoc::create_pdf_from_file("data/document.pdf").unwrap();
    //let mut pdf_file = PdfFileHandler::create_pdf_from_file("data/treatise.pdf").unwrap();
    pdf_doc.parse_page_tree();
}




#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_data () -> HashMap<&'static str, PDFVersion> {
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
                Err(e) => {println!("{:?}", e); panic!();}
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