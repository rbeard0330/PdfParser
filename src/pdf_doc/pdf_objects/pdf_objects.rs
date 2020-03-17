use std::error;
use std::rc::Rc;
use std::convert::TryInto;
use std::cell::RefCell;
use std::fmt::Debug;

use crate::errors::*;

pub type SharedObject = Rc<dyn PdfObjectInterface>;

pub trait PdfObjectInterface: Debug {
    fn get_data_type(&self) -> Result<DataType>;
    fn get_pdf_primitive_type(&self) -> Result<PdfDataType>;
    fn try_into_map(&self) -> Result<Rc<PdfMap>> {
        Err(ErrorKind::UnavailableType("map".to_string(), format!("{:?}", &self)))?
    }
    fn try_into_array(&self) -> Result<Rc<PdfArray>> {
        Err(ErrorKind::UnavailableType("arry".to_string(), format!("{:?}", &self)))?
    }
    fn try_into_binary(&self) -> Result<Rc<Vec<u8>>> {
        Err(ErrorKind::UnavailableType("binary".to_string(), format!("{:?}", &self)))?
    }
    fn try_into_string(&self) -> Result<Rc<String>> {
        Err(ErrorKind::UnavailableType("string".to_string(), format!("{:?}", &self)))?
    }
    fn try_into_int(&self) -> Result<i32> {
        Err(ErrorKind::UnavailableType("int".to_string(), format!("{:?}", &self)))?
    }
    fn try_into_float(&self) -> Result<f32> {
        Err(ErrorKind::UnavailableType("float".to_string(), format!("{:?}", &self)))?
    }
    fn try_into_bool(&self) -> Result<bool> {
        Err(ErrorKind::UnavailableType("bool".to_string(), format!("{:?}", &self)))?
    }
}

trait PdfObjectReference: PdfObjectInterface + Debug {
    fn retrieve(self) -> Result<SharedObject>;
    fn as_ref(&self) -> Result<&SharedObject>;
}

#[derive(Debug)]
pub enum PdfObject {
    Reference(Box<dyn PdfObjectReference>),
    Actual(SharedObject)
}

impl PdfObjectInterface for PdfObject {
    fn get_data_type(&self) -> Result<DataType> {
        match self {
            PdfObject::Reference(ref link) => {
                link.as_ref().get_data_type()
            },
            PdfObject::Actual(ref obj) => obj.get_data_type()
        }
    }
    fn get_pdf_primitive_type(&self) -> Result<PdfDataType> {
        match self {
            PdfObject::Reference(ref link) => {
                link.as_ref().get_pdf_primitive_type()
            },
            PdfObject::Actual(ref obj) => obj.get_pdf_primitive_type()
        }
    }
    fn try_into_map(&self) -> Result<Rc<PdfMap>> {
        match self {
            PdfObject::Reference(ref link) => {
                link.as_ref().try_into_map()
            },
            PdfObject::Actual(ref obj) => obj.try_into_map()
        }
    }
    fn try_into_array(&self) -> Result<Rc<PdfArray>> {
        match self {
            PdfObject::Reference(ref link) => {
                link.as_ref().try_into_array()
            },
            PdfObject::Actual(ref obj) => obj.try_into_array()
        }
    }
    fn try_into_binary(&self) -> Result<Rc<Vec<u8>>> {
        match self {
            PdfObject::Reference(ref link) => {
                link.as_ref().try_into_binary()
            },
            PdfObject::Actual(ref obj) => obj.try_into_binary()
        }
    }
    fn try_into_string(&self) -> Result<Rc<String>> {
        match self {
            PdfObject::Reference(ref link) => {
                link.as_ref().try_into_string()
            },
            PdfObject::Actual(ref obj) => obj.try_into_string()
        }
    }
    fn try_into_int(&self) -> Result<i32> {
        match self {
            PdfObject::Reference(ref link) => {
                link.as_ref().try_into_int()
            },
            PdfObject::Actual(ref obj) => obj.try_into_int()
        }
    }
    fn try_into_float(&self) -> Result<f32> {
        match self {
            PdfObject::Reference(ref link) => {
                link.as_ref().try_into_float()
            },
            PdfObject::Actual(ref obj) => obj.try_into_float()
        }
    }
    fn try_into_bool(&self) -> Result<bool> {
        match self {
            PdfObject::Reference(ref link) => {
                link.as_ref().try_into_bool()
            },
            PdfObject::Actual(ref obj) => obj.try_into_bool()
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

#[derive(Debug)]
pub struct PdfArray(Rc<Vec<SharedObject>>);

impl PdfObjectInterface for PdfArray {
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
