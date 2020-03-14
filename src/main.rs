mod pdf_file;
use self::pdf_file::*;
use std::rc;
use pdf_file::{PDFError, ObjectID};

struct PdfDoc {
    file: PdfFileHandler,
}

impl PdfDoc {
    fn parse_page_tree(&mut self) -> Result<(), PDFError> {
        let mut objects_to_visit = Vec::new();
        let root = self.file.get_root()?;
        let pages = root.get("Pages")?.expect("No pages dict!");
        let page_ref = match **pages {
            PDFObj::ObjectRef(id) => id,
            _ => return Err(PDFError{message: "No reference to Pages".to_string(), function: "parse_page_tree"})
        };
        drop(root);
        objects_to_visit.push(Ok(page_ref));
        
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
    let mut pdf_file = PdfFileHandler::create_pdf_from_file("data/document.pdf").unwrap();
    //let mut pdf_file = PdfFileHandler::create_pdf_from_file("data/treatise.pdf").unwrap();
    let mut pdf_doc = PdfDoc{file: pdf_file};
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
    fn basic_imports() {
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
            let pdf_file = PdfFileHandler::create_pdf_from_file(path).unwrap();
            let mut pdf = PdfDoc {file: pdf_file};
            pdf.parse_page_tree();
        }
    }
}