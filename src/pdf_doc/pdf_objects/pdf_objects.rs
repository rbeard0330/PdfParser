use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::Into;
use std::fmt::Debug;
use std::rc::{Rc, Weak};

use super::*;
use crate::errors::*;

pub use PdfData::*;

pub type SharedObject = Rc<PdfObject>;

pub trait PdfObjectInterface: Debug {
    fn get_data_type(&self) -> Result<DataType>;
    fn get_pdf_primitive_type(&self) -> Result<PdfDataType>;
    fn try_into_map(&self) -> Result<Rc<PdfMap>> {
        Err(ErrorKind::UnavailableType(
            "map".to_string(),
            format!("{:?}", &self),
        ))?
    }
    fn try_into_array(&self) -> Result<Rc<PdfArray>> {
        Err(ErrorKind::UnavailableType(
            "array".to_string(),
            format!("{:?}", &self),
        ))?
    }
    fn try_into_binary(&self) -> Result<Rc<Vec<u8>>> {
        Err(ErrorKind::UnavailableType(
            "binary".to_string(),
            format!("{:?}", &self),
        ))?
    }
    fn try_into_string(&self) -> Result<Rc<String>> {
        Err(ErrorKind::UnavailableType(
            "string".to_string(),
            format!("{:?}", &self),
        ))?
    }
    fn try_into_int(&self) -> Result<i32> {
        Err(ErrorKind::UnavailableType(
            "int".to_string(),
            format!("{:?}", &self),
        ))?
    }
    fn try_into_float(&self) -> Result<f32> {
        Err(ErrorKind::UnavailableType(
            "float".to_string(),
            format!("{:?}", &self),
        ))?
    }
    fn try_into_bool(&self) -> Result<bool> {
        Err(ErrorKind::UnavailableType(
            "bool".to_string(),
            format!("{:?}", &self),
        ))?
    }
    fn try_into_stream(&self) -> Result<Rc<PdfStream>> {
        Err(ErrorKind::UnavailableType(
            "stream".to_string(),
            format!("{:?}", &self),
        ))?
    }
    fn is_map(&self) -> bool {
        false
    }
    fn is_array(&self) -> bool {
        false
    }
    fn is_binary(&self) -> bool {
        false
    }
    fn is_string(&self) -> bool {
        false
    }
    fn is_int(&self) -> bool {
        false
    }
    fn is_float(&self) -> bool {
        false
    }
    fn is_bool(&self) -> bool {
        false
    }
    fn is_stream(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub enum PdfData {
    Boolean(bool),
    NumberInt(i32),
    NumberFloat(f32),
    Name(Rc<String>),
    CharString(Rc<String>),
    HexString(Rc<Vec<u8>>),
    Array(Rc<PdfArray>),
    Dictionary(Rc<PdfMap>),
    Stream(Rc<PdfStream>),
    Comment(Rc<String>),
}

#[derive(Debug)]
pub enum PdfObject {
    Reference(PdfObjectReference<ObjectCache>),
    Actual(PdfData),
}

impl PdfObject {
    pub fn new_boolean(data: bool) -> PdfObject {
        PdfObject::Actual(Boolean(data))
    }

    pub fn new_number_int<T: Into<i32>>(data: T) -> PdfObject {
        PdfObject::Actual(NumberInt(data.into()))
    }

    pub fn new_number_float<T: Into<f32>>(data: T) -> PdfObject {
        PdfObject::Actual(NumberFloat(data.into()))
    }

    pub fn new_name<T: Into<String>>(data: T) -> PdfObject {
        PdfObject::Actual(Name(Rc::new(data.into())))
    }

    pub fn new_char_string<T: Into<String>>(data: T) -> PdfObject {
        PdfObject::Actual(CharString(Rc::new(data.into())))
    }

    pub fn new_hex_string(data: Vec<u8>) -> PdfObject {
        PdfObject::Actual(HexString(Rc::new(data)))
    }

    pub fn new_array(data: Rc<PdfArray>) -> PdfObject {
        PdfObject::Actual(Array(data))
    }

    pub fn new_dictionary(data: Rc<PdfMap>) -> PdfObject {
        PdfObject::Actual(Dictionary(data))
    }

    //TODO: Stream constructor

    pub fn new_comment<T: Into<String>>(data: T) -> PdfObject {
        PdfObject::Actual(Comment(Rc::new(data.into())))
    }

    pub fn new_reference<T, S, D>(id: T, gen: S, data: Weak<ObjectCache>) -> PdfObject
    where
        T: Into<u32>,
        S: Into<u32>,
    {
        PdfObject::Reference(PdfObjectReference { id:id.into(), gen:gen.into(), data })
    }
}

impl PdfObjectInterface for PdfObject {
    fn get_data_type(&self) -> Result<DataType> {
        match self {
            PdfObject::Reference(ref link) => link.get().get_data_type(),
            PdfObject::Actual(ref obj) => obj.get_data_type(),
        }
    }
    fn get_pdf_primitive_type(&self) -> Result<PdfDataType> {
        match self {
            PdfObject::Reference(ref link) => link.get().get_pdf_primitive_type(),
            PdfObject::Actual(ref obj) => obj.get_pdf_primitive_type(),
        }
    }
    fn try_into_map(&self) -> Result<Rc<PdfMap>> {
        match self {
            PdfObject::Reference(ref link) => link.get().try_into_map(),
            PdfObject::Actual(ref obj) => obj.try_into_map(),
        }
    }
    fn try_into_array(&self) -> Result<Rc<PdfArray>> {
        match self {
            PdfObject::Reference(ref link) => link.get().try_into_array(),
            PdfObject::Actual(ref obj) => obj.try_into_array(),
        }
    }
    fn try_into_binary(&self) -> Result<Rc<Vec<u8>>> {
        match self {
            PdfObject::Reference(ref link) => link.get().try_into_binary(),
            PdfObject::Actual(ref obj) => obj.try_into_binary(),
        }
    }
    fn try_into_string(&self) -> Result<Rc<String>> {
        match self {
            PdfObject::Reference(ref link) => link.get().try_into_string(),
            PdfObject::Actual(ref obj) => obj.try_into_string(),
        }
    }
    fn try_into_int(&self) -> Result<i32> {
        match self {
            PdfObject::Reference(ref link) => link.get().try_into_int(),
            PdfObject::Actual(ref obj) => obj.try_into_int(),
        }
    }
    fn try_into_float(&self) -> Result<f32> {
        match self {
            PdfObject::Reference(ref link) => link.get().try_into_float(),
            PdfObject::Actual(ref obj) => obj.try_into_float(),
        }
    }
    fn try_into_bool(&self) -> Result<bool> {
        match self {
            PdfObject::Reference(ref link) => link.get().try_into_bool(),
            PdfObject::Actual(ref obj) => obj.try_into_bool(),
        }
    }
    // TODO: Implement is_ methods
}

impl Clone for PdfObject {
    fn clone(&self) -> PdfObject {
        match self {
            &PdfObject::Actual(ref obj) => PdfObject::Actual(obj.clone()),
            &PdfObject::Reference(ref obj_ref) => match obj_ref.get() {
                Ok(obj) => obj,
                Err(_) => PdfObject::Reference(obj_ref),
            },
        }
    }
}

impl fmt::Display for PdfObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Boolean(b) => write!(f, "Boolean: {}", b),
            NumberInt(n) => write!(f, "Number: {}", n),
            NumberFloat(n) => write!(f, "Number: {:.2}", n),
            Name(s) => write!(f, "Name: {}", s),
            CharString(s) => write!(f, "String: {}", s),
            HexString(s) => write!(f, "String: {:?}", s),
            Array(v) => write!(f, "Array: {:#?}", v),
            Dictionary(h) => write!(f, "Dictionary: {:#?}", h),
            Stream(d, _) => write!(f, "Stream object: {:#?}", d),
            Comment(s) => write!(f, "Comment: {:?}", s),
            //Keyword(kw) => write!(f, "Keyword: {:?}", kw),
        }
        .expect("Error in write macro!");
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct PdfObjectReference<T: PdfFileInterface<PdfObject>> {
    id: u32,
    gen: u32,
    data: Weak<T>,
}

impl<T> PdfObjectReference<T> {
    fn get(&self) -> Result<PdfObject> {
        self.data.retrieve_object_ref(self.id, self.gen)
    }
}

pub trait PdfMapInterface {
    fn get<T: Into<String>>(&self, key: T) -> Option<SharedObject>;
    fn insert<T: Into<String>>(&mut self, key: T, value: SharedObject) -> Option<SharedObject>;
}

#[derive(Debug, Clone)]
pub struct PdfMap(HashMap<String, SharedObject>);

impl PdfMapInterface for PdfMap {
    fn get<T: Into<str>>(&self, key: T) -> Option<SharedObject> {
        self.0.get(key).map(|result| Rc::clone(result))
    }

    fn insert<T: Into<str>>(&mut self, key: T, value: SharedObject) -> Option<SharedObject> {
        self.0.insert(key, Rc::clone(value))
    }
}

impl<'a> IntoIterator for &'a PdfMap {
    type Item = (&'a String, SharedObject);
    fn into_iter(&'a self) -> Iter<'_, &'a String, SharedObject> {
        self.0.iter().map(|k, v| (k, Rc::clone(v)))
    }
}

struct PdfFile {}

pub struct Image {}
pub struct ContentStream {}

pub enum DataType {
    VecObjects,
    I32,
    F32,
    String,
    VecU8,
    HashMap,
}

pub enum PdfDataType {
    Boolean,
    Number,
    Name,
    CharString,
    HexString,
    Array,
    Dictionary,
    Stream,
    Comment,
}

#[derive(Debug, PartialEq, Clone)]
pub struct PdfArray(Rc<PdfArray>);

impl PdfObjectInterface for PdfArray {
    fn get_data_type(&self) -> Result<DataType> {
        Ok(DataType::VecObjects)
    }
    fn get_pdf_primitive_type(&self) -> Result<PdfDataType> {
        Ok(PdfDataType::Array)
    }
    fn try_into_array(&self) -> Result<Rc<PdfArray>> {
        Ok(Rc::clone(self.0))
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
struct PdfBool(bool);

impl PdfObjectInterface for PdfBool {
    fn get_data_type(&self) -> Result<DataType> {
        Ok(DataType::Boolean)
    }

    fn get_pdf_primitive_type(&self) -> Result<PdfDataType> {
        Ok(PdfDataType::Boolean)
    }

    fn try_into_bool(&self) -> Result<bool> {
        Ok(self.0)
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
struct PdfInt(i32);

impl PdfObjectInterface for PdfInt {
    fn get_data_type(&self) -> Result<DataType> {
        Ok(DataType::I32)
    }

    fn get_pdf_primitive_type(&self) -> Result<PdfDataType> {
        Ok(PdfDataType::Number)
    }

    fn try_into_bool(&self) -> Result<i32> {
        Ok(self.0)
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
struct PdfFloat(f32);

impl PdfObjectInterface for PdfFloat {
    fn get_data_type(&self) -> Result<DataType> {
        Ok(DataType::F32)
    }

    fn get_pdf_primitive_type(&self) -> Result<PdfDataType> {
        Ok(PdfDataType::Number)
    }

    fn try_into_float(&self) -> Result<f32> {
        Ok(self.0)
    }
}

#[derive(Debug, PartialEq, Clone)]
struct PdfName(String);

impl PdfObjectInterface for PdfName {
    fn get_data_type(&self) -> Result<DataType> {
        Ok(DataType::String)
    }

    fn get_pdf_primitive_type(&self) -> Result<PdfDataType> {
        Ok(PdfDataType::Name)
    }

    fn try_into_string(&self) -> Result<String> {
        Ok(self.0.clone())
    }
}

#[derive(Debug, PartialEq, Clone)]
struct PdfComment(String);

impl PdfObjectInterface for PdfComment {
    fn get_data_type(&self) -> Result<DataType> {
        Ok(DataType::String)
    }

    fn get_pdf_primitive_type(&self) -> Result<PdfDataType> {
        Ok(PdfDataType::Comment)
    }

    fn try_into_string(&self) -> Result<String> {
        Ok(self.0.clone())
    }
}

#[derive(Debug, PartialEq, Clone)]
struct PdfCharString(String);

impl PdfObjectInterface for PdfCharString {
    fn get_data_type(&self) -> Result<DataType> {
        Ok(DataType::String)
    }

    fn get_pdf_primitive_type(&self) -> Result<PdfDataType> {
        Ok(PdfDataType::CharString)
    }

    fn try_into_string(&self) -> Result<String> {
        Ok(self.0.clone())
    }
}

#[derive(Debug, PartialEq, Clone)]
struct PdfHexString(Vec<u8>);

impl PdfObjectInterface for PdfHexString {
    fn get_data_type(&self) -> Result<DataType> {
        Ok(DataType::VecU8)
    }

    fn get_pdf_primitive_type(&self) -> Result<PdfDataType> {
        Ok(PdfDataType::HexString)
    }

    fn try_into_binary(&self) -> Result<Vec<u8>> {
        Ok(self.0.clone())
    }
}

#[derive(Debug, PartialEq, Clone)]
struct PdfDictionary(Rc<PdfMap>);

impl PdfObjectInterface for PdfDictionary {
    fn get_data_type(&self) -> Result<DataType> {
        Ok(DataType::HashMap)
    }

    fn get_pdf_primitive_type(&self) -> Result<PdfDataType> {
        Ok(PdfDataType::Dictionary)
    }

    fn try_into_map(&self) -> Result<Rc<PdfMap>> {
        Ok(Rc::clone(self.0))
    }
}

#[derive(Debug, PartialEq, Clone)]
struct PdfStream(Rc<PdfMap>, Vec<u8>);

impl PdfObjectInterface for PdfStream {
    fn get_data_type(&self) -> Result<DataType> {
        Ok(DataType::VecU8)
    }

    fn get_pdf_primitive_type(&self) -> Result<PdfDataType> {
        Ok(PdfDataType::HexString)
    }
    //TODO: figure out try_into
}

enum PdfKeyword {}
