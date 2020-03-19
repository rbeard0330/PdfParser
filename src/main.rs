#![recursion_limit = "1024"]
#[path = "pdf_doc/pdf_doc.rs"]
mod pdf_doc;

#[macro_use]
extern crate error_chain;

mod errors {
    error_chain! {

        foreign_links {
            Fmt(::std::fmt::Error);
            Io(::std::io::Error);
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
            FileError
            ReferenceError
        }
    }
}

use errors::*;

fn main() {}
