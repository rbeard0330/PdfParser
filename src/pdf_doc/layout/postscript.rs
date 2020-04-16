use pest::Parser;
use data_string::DataString;

#[derive(Parser)]
#[grammar = "pdf_doc/layout/postscript.pest"]
pub struct PSParser;

#[cfg(test)]
mod parse_tests {
    use super::*;
    use crate::pdf_doc::PdfDoc;
    use std::fs;
    
    #[test]
    fn simple_parse() {
        let data = "/PlacedGraphic /MC0 BDC
        EMC";
        let result = PSParser::parse(Rule::block, data);
        if result.is_err() {
            println!("result: {}", result.unwrap_err());
        };
    }
    #[test]
    fn parse_test_file() {
        let mut data = fs::read_to_string("data/test.txt").expect("Bad file");
        let result = PSParser::parse(Rule::text_block, &data).unwrap();
        //println!("{:#?}", result);
        
    }
    #[test]
    fn real_parse() {
        let doc = PdfDoc::create_pdf_from_file("data/f1120.pdf").unwrap();
        for page_num in 1..doc.page_count() {
            let page = doc.page_tree.get_page(page_num).unwrap();
            
            let contents = match page.contents_as_binary() {
                Some(contents) => contents,
                None => vec!()
            };
            let mut content_string = DataString::from_vec(contents);
            let result = PSParser::parse(Rule::block, content_string.as_ref());
            if result.is_err() { 
                println!("Page {}: {:#?}", page_num, result);
                println!("{}", result.unwrap_err());
                let borrow = content_string.take_data().unwrap();
                // for index in 200..300 {
                //     println!("{}: {}", borrow[index] as char, borrow[index]);
                // }
                let letter_vec: Vec<char> = borrow.iter().map(|r| *r as char).collect();
                for index in 4500..5500 {
                    print!("{}", letter_vec[index]);
                }
                panic!();
                //println!("{}", content_string);
            }
        }

    }
}