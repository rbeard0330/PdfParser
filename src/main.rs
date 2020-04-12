#![recursion_limit = "1024"]
#[path = "pdf_doc/pdf_doc.rs"]
mod pdf_doc;
mod util;

#[macro_use]
extern crate error_chain;

extern crate pretty_env_logger;
#[macro_use]
extern crate log;

mod errors {
    error_chain! {

        foreign_links {
            Fmt(::std::fmt::Error);
            Io(::std::io::Error);
            ParseFloat(::std::num::ParseFloatError);
            ParseInt(::std::num::ParseIntError);
        }
        errors {
            UnavailableType(req: String, thrower: String) {
                description("Cannot provide requested type")
                display("Unavailable type {} requested from: {}", req, thrower)
            }
            FilterError(description: String, function: &'static str) {
                description("Error applying/decoding filter")
                display("{} encountered an error applying/decoding filter {}", function, description)
            }
            ParsingError(problem: String) {
                description("Error parsing PDF file")
                display("{}", problem)
            }
            ReferenceError(problem: String) {
                description("Bad reference")
                display("{}", problem)
            }
            TestingError(text: String) {
                description("Custom error")
                display("{}", text)
            }
            DocTreeError(text: String) {
                description("Doc tree error")
                display("{}", text)
            }
        }
    }
}

mod test_utils {

}

//use errors::*;

fn main() {
    pretty_env_logger::init_timed();
    //let pdf_doc = doc_tree::PdfDoc::create_pdf_from_file("data/document.pdf").unwrap();
    //let pdf_doc = doc_tree::PdfDoc::create_pdf_from_file("data/treatise.pdf").unwrap();
    let _pdf_doc = pdf_doc::PdfDoc::create_pdf_from_file("data/PDF32000_2008.pdf").unwrap();
    //println!("{}", pdf_doc);
}



