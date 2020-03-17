use std::error;
use std::rc::Rc;
use std::convert::TryInto;
use std::cell::RefCell;

pub type Result<T> = std::result::Result<T, PdfTypeError>;
pub type SharedObject = Rc<dyn PdfData>;
pub struct PdfFileError;
pub struct PdfTypeError;

pub trait PdfData {
    fn get_data_type(&self) -> Result<DataType>;
    fn get_pdf_primitive_type(&self) -> Result<PdfDataType>;
    fn try_into_map(&self) -> Result<Rc<PdfMap>> {
        Err(PdfTypeError)
    }
    fn try_into_array(&self) -> Result<Rc<PdfArray>> {
        Err(PdfTypeError)
    }
    fn try_into_binary(&self) -> Result<Rc<Vec<u8>>> {
        Err(PdfTypeError)
    }
    fn try_into_string(&self) -> Result<Rc<String>> {
        Err(PdfTypeError)
    }
    fn try_into_int(&self) -> Result<i32> {
        Err(PdfTypeError)
    }
    fn try_into_float(&self) -> Result<f32> {
        Err(PdfTypeError)
    }
    fn try_into_bool(&self) -> Result<bool> {
        Err(PdfTypeError)
    }
}

trait PdfObjectReference: PdfData {
    fn retrieve(self) -> Result<SharedObject>;
    fn as_ref(&self) -> Result<&SharedObject>;
}

enum LazyObject {
    Reference(Box<dyn PdfObjectReference>),
    Actual(SharedObject)
}

impl PdfData for LazyObject {
    fn get_data_type(&self) -> Result<DataType> {
        match self {
            LazyObject::Reference(ref link) => {
                link.as_ref().get_data_type()
            },
            LazyObject::Actual(ref obj) => obj.get_data_type()
        }
    }
    fn get_pdf_primitive_type(&self) -> Result<PdfDataType> {
        match self {
            LazyObject::Reference(ref link) => {
                link.as_ref().get_pdf_primitive_type()
            },
            LazyObject::Actual(ref obj) => obj.get_pdf_primitive_type()
        }
    }
    fn try_into_map(&self) -> Result<Rc<PdfMap>> {
        match self {
            LazyObject::Reference(ref link) => {
                link.as_ref().try_into_map()
            },
            LazyObject::Actual(ref obj) => obj.try_into_map()
        }
    }
    fn try_into_array(&self) -> Result<Rc<PdfArray>> {
        match self {
            LazyObject::Reference(ref link) => {
                link.as_ref().try_into_array()
            },
            LazyObject::Actual(ref obj) => obj.try_into_array()
        }
    }
    fn try_into_binary(&self) -> Result<Rc<Vec<u8>>> {
        match self {
            LazyObject::Reference(ref link) => {
                link.as_ref().try_into_binary()
            },
            LazyObject::Actual(ref obj) => obj.try_into_binary()
        }
    }
    fn try_into_string(&self) -> Result<Rc<String>> {
        match self {
            LazyObject::Reference(ref link) => {
                link.as_ref().try_into_string()
            },
            LazyObject::Actual(ref obj) => obj.try_into_string()
        }
    }
    fn try_into_int(&self) -> Result<i32> {
        match self {
            LazyObject::Reference(ref link) => {
                link.as_ref().try_into_int()
            },
            LazyObject::Actual(ref obj) => obj.try_into_int()
        }
    }
    fn try_into_float(&self) -> Result<f32> {
        match self {
            LazyObject::Reference(ref link) => {
                link.as_ref().try_into_float()
            },
            LazyObject::Actual(ref obj) => obj.try_into_float()
        }
    }
    fn try_into_bool(&self) -> Result<bool> {
        match self {
            LazyObject::Reference(ref link) => {
                link.as_ref().try_into_bool()
            },
            LazyObject::Actual(ref obj) => obj.try_into_bool()
        }
    }
}

struct IndirectObj {
    id: u32,
    data: Rc<RefCell<PdfFile>>
}

pub struct PdfMap {
    
}

struct PdfFile {}

pub struct PdfArray(Rc<Vec<SharedObject>>);

impl PdfData for PdfArray {
    fn get_data_type(&self) -> Result<DataType> {
        Ok(DataType::Vec)
    }
    fn get_pdf_primitive_type(&self) -> Result<PdfDataType> {
        Ok(PdfDataType::Array)
    }
}



pub struct Image {}
pub struct ContentStream {}

pub enum DataType {
    Vec

}

pub enum PdfDataType {
    Boolean,
    Number,
    Name,
    CharString,
    HexString,
    Array,
    Dictionary,
    Stream
}

struct Null {}
