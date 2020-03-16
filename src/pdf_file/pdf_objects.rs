use std::error;
use std::rc::Rc;
use std::convert::TryInto;

pub type Result<T> = std::result::Result<T, PdfFileError>;
pub type SharedObject = Rc<dyn PdfObject>;
pub struct PdfFileError;
pub struct PdfTypeError;

pub trait PdfObject {
    fn get_data_type(&self) -> DataType;
    fn get_pdf_primitive_type(&self) -> PdfDataType;
}


pub struct PdfMap {
    
}

pub struct PdfArray(Rc<Vec<SharedObject>>);

impl PdfObject for PdfArray {
    fn get_data_type(&self) -> DataType {
        DataType::Vec
    }
    fn get_pdf_primitive_type(&self) -> PdfDataType {
        PdfDataType::Array
    }
}

impl TryInto<&Vec<SharedObject>> for PdfArray {
    type Error = PdfFileError;
    fn try_into(obj: PdfArray) -> Result<&Vec<SharedObject>> {
        
    }

}

pub struct Image {}
pub struct ContentStream {}

pub enum DataType {
    Vec

}

pub enum PdfDataType {
    Array

}

struct Null {}
