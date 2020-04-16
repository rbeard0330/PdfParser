use super::vec_tree;
extern crate data_string;
//use crate::errors::*;

use std::rc::Rc;

use data_string::DataString;

use super::PageTree;
use crate::pdf_doc::pdf_objects::{SharedObject, PdfObjectInterface, DataType};

#[path = "layout/postscript.rs"]
mod postscript;

pub struct Page<'a> {
    index: vec_tree::Index,
    tree: &'a PageTree,
}

impl<'a> Page<'a> {
    pub fn new_from_index(index: vec_tree::Index, tree: &'a PageTree) -> Self {
        Page { index, tree }
    }
    pub fn get_attribute(&self, key: String) -> Option<SharedObject> {
        let mut current_index = Some(self.index);
        // Check attribute dictionary at self and each parent
        while let Some(index) = current_index {
            let current_node = &self.tree.tree[index];
            let current_result = current_node.attributes.get(&key);
            if let Some(object) = current_result {
                return Some(Rc::clone(object))
            };
            current_index = self.tree.tree.parent(index);
        }
        None
    }

    fn contents_as_binary(&self) -> Option<Vec<u8>> {
        let contents_ref = self.tree.tree[self.index].contents.as_ref();
        match contents_ref.unwrap().get_data_type().unwrap() {
            DataType::VecObjects => {
                let content_stream_vec = contents_ref.unwrap().try_into_array().unwrap();
                let mut output_data: Vec<u8> = Vec::new();
                for stream_ref in content_stream_vec.iter() {
                    let stream_data = stream_ref.try_into_binary().unwrap();
                    output_data.extend(stream_data.iter());
                }
                Some(output_data)
            },
            DataType::VecU8 => { return Some(contents_ref.unwrap().try_into_binary().unwrap().as_ref().clone())},
            _ => None
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use super::super::PdfDoc;

    #[test]
    fn get_page() {
        //let data = test_utils::test_data();
        let doc = PdfDoc::create_pdf_from_file("data/f1120.pdf").unwrap();
        for page_num in 1..doc.page_count() {
            let page = doc.page_tree.get_page(page_num).unwrap();
            //println!("Page unwrapped");
            if page_num < 5 {
                let contents = match page.contents_as_binary() {
                    Some(contents) => contents,
                    None => vec!()
                };
                let content_string = DataString::from_vec(contents);
                println!("Page {}: {}", page_num, content_string);
            }
        }
    }

}

