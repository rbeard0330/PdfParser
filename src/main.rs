mod pdf_file;
use self::pdf_file::*;
use std::rc::Rc;
use pdf_file::{PDFError};

struct PdfDoc {
    file: PdfFileHandler,
}

impl PdfDoc {
    fn parse_page_tree(&mut self) -> Result<(), PDFError> {
        let mut objects_to_visit = Vec::new();
        let root = self.file.get_root()?;
        let pages = root.get_dict_ref()?.get("Pages").unwrap();
        let page_ref = match **pages {
            PDFObj::ObjectRef(id) => id,
            _ => return Err(PDFError{message: "No reference to Pages".to_string(), function: "parse_page_tree"})
        };
        drop(root);
        objects_to_visit.push(self.file.get_object(&page_ref).unwrap());
        
        loop {
            let this_obj = objects_to_visit.pop();
            let this_ref = match this_obj {
                None => {break},
                Some(ref obj) => match **obj {
                    PDFObj::ObjectRef(id) => id,
                    _ => {panic!();}
                }
            };
            let kids_array = match &*(self.file.get_object(&this_ref).unwrap()) {
                &PDFObj::Dictionary(ref hash_map) => Rc::clone(hash_map.get("Kids").unwrap()),
                o @ _ => {
                    println!("{:?}", o);
                    panic!();},
            };
            match &*kids_array {
                    &PDFObj::Array(ref v) => {objects_to_visit.extend(v.clone());},
                    _ => {}
                    
                }
        }
        Ok(())


    }

}

fn main() {
    let mut pdf_file = PdfFileHandler::create_pdf_from_file("data/document.pdf").unwrap();
    let mut pdf_doc = PdfDoc{file: pdf_file};
    pdf_doc.parse_page_tree();



}




#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PDFS: [&str; 3] = ["data/simple_pdf.pdf",// "data/CCI01212020.pdf",
        "data/document.pdf", "data/2018W2.pdf"];
    
    #[test]
    fn basic_imports() {
        for path in &TEST_PDFS {
            let pdf = PdfFileHandler::create_pdf_from_file(path);
        }
    }
}