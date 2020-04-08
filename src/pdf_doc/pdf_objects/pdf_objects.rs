use std::collections::HashMap;
use std::convert::Into;
use std::fmt::Debug;
use std::rc::{Rc, Weak};

use super::*;
use crate::errors::*;
use ErrorKind::*;
use crate::doc_tree::pdf_file::decode::*;
use crate::doc_tree::pdf_file::object_cache::ObjectStreamCache;

pub use PdfData::*;

pub type SharedObject = Rc<PdfObject>;
pub type PdfMap = HashMap<String, Rc<PdfObject>>;

pub type PdfArray = Vec<Rc<PdfObject>>;

pub trait PdfObjectInterface: Debug {
    fn get_data_type(&self) -> Result<DataType>;
    fn get_pdf_primitive_type(&self) -> Result<PdfDataType>;
    fn try_to_get<T: AsRef<str> + ?Sized>(&self, key: &T) -> Result<Option<SharedObject>>;
    fn try_to_index(&self, index: usize)  -> Result<SharedObject>;
    fn try_into_map(&self) -> Result<Rc<PdfMap>> {
        Err(UnavailableType(
            "map".to_string(),
            format!("{:?}", &self),
        ))?
    }
    fn try_into_array(&self) -> Result<Rc<PdfArray>> {
        Err(UnavailableType(
            "array".to_string(),
            format!("{:?}", &self),
        ))?
    }
    fn try_into_binary(&self) -> Result<Rc<Vec<u8>>> {
        Err(UnavailableType(
            "binary".to_string(),
            format!("{:?}", &self),
        ))?
    }
    fn try_into_string(&self) -> Result<Rc<String>> {
        Err(UnavailableType(
            "string".to_string(),
            format!("{:?}", &self),
        ))?
    }
    fn try_into_int(&self) -> Result<i32> {
        Err(UnavailableType(
            "int".to_string(),
            format!("{:?}", &self),
        ))?
    }
    fn try_into_float(&self) -> Result<f32> {
        Err(UnavailableType(
            "float".to_string(),
            format!("{:?}", &self),
        ))?
    }
    fn try_into_bool(&self) -> Result<bool> {
        Err(UnavailableType(
            "bool".to_string(),
            format!("{:?}", &self),
        ))?
    }
    fn try_into_content_stream(&self) -> Result<Rc<PdfContentStream>> {
        Err(UnavailableType(
            "content stream".to_string(),
            format!("{:?}", &self),
        ))?
    }
    fn try_into_object_stream(&self) -> Result<Rc<ObjectStreamCache>> {
        Err(UnavailableType(
            "object stream".to_string(),
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
    fn is_name(&self) -> bool {
        false
    }
    fn is_number(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone)]
pub enum PdfData {
    Boolean(bool),
    NumberInt(i32),
    NumberFloat(f32),
    Name(Rc<String>),
    CharString(Rc<String>),
    HexString(Rc<Vec<u8>>),
    Array(Rc<PdfArray>),
    Dictionary(Rc<PdfMap>),
    ContentStream(Rc<PdfContentStream>),
    BinaryStream(Rc<PdfBinaryStream>),
    ObjectStream(Rc<ObjectStreamCache>),
    Comment(Rc<String>),
    Null
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

    pub fn new_content_stream(data: Vec<u8>, attributes: PdfMap) -> PdfObject {
        PdfObject::Actual(ContentStream(Rc::new(
            PdfContentStream::new(data, attributes)
        )))
    }

    pub fn new_binary_stream(data: PdfBinaryStream) -> PdfObject {
        PdfObject::Actual(BinaryStream(Rc::new(data)))
    }
    pub fn new_comment<T: Into<String>>(data: T) -> PdfObject {
        PdfObject::Actual(Comment(Rc::new(data.into())))
    }

    pub fn new_reference<T, S>(id: T, gen: S, data: Weak<ObjectCache>) -> PdfObject
    where
        T: Into<u32>,
        S: Into<u32>,
    {
        PdfObject::Reference(PdfObjectReference { id: ObjectId(id.into(), gen.into()), data })
    }
    pub fn new_object_stream(attributes: PdfMap, data: Vec<u8>, weak_ref: Weak<ObjectCache>) -> Result<PdfObject> {
        debug_assert_eq!(
            *attributes.get("Type").expect("No type in object stream dict!").try_into_string().unwrap(),
            "ObjStm");
        let object_count = attributes.get("N")
            .ok_or(ParsingError(format!("No /N key in object stream dict")))?
            .try_into_int()
            .chain_err(|| ParsingError(format!("/N key in object stream dict not an integer")))? as usize;
        let first_object_start = attributes.get("First")
            .ok_or(ParsingError(format!("No /First key in object stream dict")))?
            .try_into_int()
            .chain_err(|| ParsingError(format!("/First key in object stream dict not an integer")))? as usize;
        // TODO: Implement "Extends"
        assert!(first_object_start > 0);
        let index_slice = &data[..(first_object_start as usize)];
        let index_string = String::from_utf8(Vec::from(index_slice))
            .chain_err(|| ParsingError(format!("Invalid character in object stream index: {:?}", index_slice)))?;
        let mut word_iter = index_string.split_whitespace().into_iter();
        let mut object_index = HashMap::new();
        loop {
            let first_word = word_iter.next();
            let second_word = word_iter.next();
            if first_word.is_none() { break };
            if second_word.is_none() {
                Err(ParsingError(format!("No position for object #{}", first_word.unwrap())))?
            };
            let first_word_as_int: u32 = first_word.unwrap().parse()
                .chain_err(|| ParsingError(format!("Not an integer: {}", first_word.unwrap())))?;
            let second_word_as_int: usize = second_word.unwrap().parse()
                .chain_err(|| ParsingError(format!("Not an integer: {}", second_word.unwrap())))?;
            object_index.insert(ObjectId(first_word_as_int, 0), second_word_as_int + first_object_start);
        };
        debug_assert_eq!(object_index.len(), object_count);
        Ok(PdfObject::Actual(
            ObjectStream(Rc::new(ObjectStreamCache::new(
                object_index, data, weak_ref
            )))
        ))
    }
}

impl PdfObjectInterface for PdfObject {
    fn get_data_type(&self) -> Result<DataType> {
        match self {
            PdfObject::Reference(ref link) => link.get()?.get_data_type(),
            PdfObject::Actual(ref obj) => match obj {
                Boolean(_) => Ok(DataType::Boolean),
                NumberInt(_) => Ok(DataType::I32),
                NumberFloat(_) => Ok(DataType::F32),
                Name(_) => Ok(DataType::String),
                CharString(_) => Ok(DataType::String),
                HexString(_) => Ok(DataType::VecU8),
                Array(_) => Ok(DataType::VecObjects),
                Dictionary(_) => Ok(DataType::HashMap),
                ContentStream(_) => Ok(DataType::String),
                BinaryStream(_) => Ok(DataType::VecU8),
                Comment(_) => Ok(DataType::String),
                ObjectStream(_) => Ok(DataType::VecObjects),
                Null => Ok(DataType::Null)
            }
        }
    }
    fn get_pdf_primitive_type(&self) -> Result<PdfDataType> {
        match self {
            PdfObject::Reference(ref link) => link.get()?.get_pdf_primitive_type(),
            PdfObject::Actual(ref obj) => match obj {
                Boolean(_) => Ok(PdfDataType::Boolean),
                NumberInt(_) => Ok(PdfDataType::Number),
                NumberFloat(_) => Ok(PdfDataType::Number),
                Name(_) => Ok(PdfDataType::Name),
                CharString(_) => Ok(PdfDataType::CharString),
                HexString(_) => Ok(PdfDataType::HexString),
                Array(_) => Ok(PdfDataType::Array),
                Dictionary(_) => Ok(PdfDataType::Dictionary),
                ContentStream(_) => Ok(PdfDataType::Stream),
                BinaryStream(_) => Ok(PdfDataType::Stream),
                Comment(_) => Ok(PdfDataType::Comment),
                ObjectStream(_) => Ok(PdfDataType::Stream),
                Null => Ok(PdfDataType::Null)
            }
        }
    }
    fn try_to_get<T: AsRef<str> + ?Sized>(&self, key: &T) -> Result<Option<SharedObject>> {
        match self {
            PdfObject::Reference(ref link) => link.get()?.try_to_get(key),
            PdfObject::Actual(ref obj) => match obj {
                Dictionary(map) => Ok(map.get(key.as_ref()).map(|result| Rc::clone(result))),
                _ => Err(UnavailableType("map".to_string(), "try_to_get".to_string()))?

            }
        }
    }
    fn try_to_index(&self, index: usize) -> Result<SharedObject> {
        match self {
            PdfObject::Reference(ref link) => link.get()?.try_to_index(index),
            PdfObject::Actual(ref obj) => match obj {
                Array(vec) => Ok(Rc::clone(&vec[index])),
                _ => Err(UnavailableType("vector".to_string(), "try_to_index".to_string()))?

            }
        }
    }
    fn try_into_map(&self) -> Result<Rc<PdfMap>> {
        match self {
            PdfObject::Reference(ref link) => link.get()?.try_into_map(),
            PdfObject::Actual(ref obj) => match obj {
                Dictionary(map) => Ok(Rc::clone(map)),
                BinaryStream(stream) => Ok(Rc::new(stream.attributes.clone())),
                _ => {
                    error!("Data type: {:?}", self.get_data_type()?);
                    Err(UnavailableType("map".to_string(), "try_into_map".to_string()))?
                }
            }
        }
    }
    fn try_into_array(&self) -> Result<Rc<PdfArray>> {
        match self {
            PdfObject::Reference(ref link) => link.get()?.try_into_array(),
            PdfObject::Actual(ref obj) => match obj {
                Array(arr) => Ok(Rc::clone(arr)),
                _ => Err(UnavailableType("array".to_string(), "try_into_array".to_string()))?
            }
        }
    }
    fn try_into_binary(&self) -> Result<Rc<Vec<u8>>> {
        match self {
            PdfObject::Reference(ref link) => link.get()?.try_into_binary(),
            PdfObject::Actual(ref obj) =>  match obj {
                HexString(vec) => Ok(Rc::clone(vec)),
                BinaryStream(stream) => Ok(Rc::clone(&stream.data)),
                _ => Err(UnavailableType("binary".to_string(), "try_into_binary".to_string()))?
            },
        }
    }
    fn try_into_string(&self) -> Result<Rc<String>> {
        match self {
            PdfObject::Reference(ref link) => link.get()?.try_into_string(),
            PdfObject::Actual(obj) => match obj {
                CharString(s) | Name(s) | Comment(s) => Ok(Rc::clone(s)),
                _ => Err(UnavailableType(
                    "string".to_string(),
                    format!("{:?}", &self)))?
            }
        }
    }
    fn try_into_int(&self) -> Result<i32> {
        match self {
            PdfObject::Reference(ref link) => link.get()?.try_into_int(),
            PdfObject::Actual(ref obj) =>  match obj {
                NumberInt(int) => Ok(*int),
                _ => Err(UnavailableType("integer".to_string(), "try_into_int".to_string()))?
            },
        }
    }
    fn try_into_float(&self) -> Result<f32> {
        match self {
            PdfObject::Reference(ref link) => link.get()?.try_into_float(),
            PdfObject::Actual(ref obj) =>  match obj {
                NumberFloat(float) => Ok(*float),
                _ => Err(UnavailableType("float".to_string(), "try_into_float".to_string()))?
            }
        }
    }
    fn try_into_bool(&self) -> Result<bool> {
        match self {
            PdfObject::Reference(ref link) => link.get()?.try_into_bool(),
            PdfObject::Actual(ref obj) =>  match obj {
                Boolean(b) => Ok(*b),
                _ => Err(UnavailableType("boolean".to_string(), "try_into_bool".to_string()))?
            },
        }
    }
    fn try_into_object_stream(&self) -> Result<Rc<ObjectStreamCache>> {
        match self {
            PdfObject::Reference(ref link) => link.get()?.try_into_object_stream(),
            PdfObject::Actual(ref obj) =>  match obj {
                ObjectStream(cache) => Ok(Rc::clone(cache)),
                _ => Err(UnavailableType("object_stream".to_string(), "try_into_object_stream".to_string()))?
            }
        }
    }
    fn is_map(&self) -> bool {
        match self {
            PdfObject::Reference(ref link) => match link.get() {
                Ok(val) => val.is_map(),
                _ => false
            },
            PdfObject::Actual(ref obj) =>  match obj {
                Dictionary(_) => true,
                _ => false
            },
        }
    }
    fn is_array(&self) -> bool {
        match self {
            PdfObject::Reference(ref link) => match link.get() {
                Ok(val) => val.is_array(),
                _ => false
            },
            PdfObject::Actual(ref obj) =>  match obj {
                Array(_) => true,
                _ => false
            },
        }
    }
    fn is_binary(&self) -> bool {
        match self {
            PdfObject::Reference(ref link) => match link.get() {
                Ok(val) => val.is_binary(),
                _ => false
            },
            PdfObject::Actual(ref obj) =>  match obj {
                HexString(_) | BinaryStream(_) => true,
                _ => false
            },
        }
    }
    fn is_string(&self) -> bool {
        match self {
            PdfObject::Reference(ref link) => match link.get() {
                Ok(val) => val.is_string(),
                _ => false
            },
            PdfObject::Actual(ref obj) =>  match obj {
                CharString(_) | Name(_) | Comment (_) => true,
                _ => false
            },
        }
    }
    fn is_int(&self) -> bool {
        match self {
            PdfObject::Reference(ref link) => match link.get() {
                Ok(val) => val.is_int(),
                _ => false
            },
            PdfObject::Actual(ref obj) =>  match obj {
                NumberInt(_) => true,
                _ => false
            },
        }
    }
    fn is_float(&self) -> bool {
        match self {
            PdfObject::Reference(ref link) => match link.get() {
                Ok(val) => val.is_float(),
                _ => false
            },
            PdfObject::Actual(ref obj) =>  match obj {
                NumberFloat(_) => true,
                _ => false
            },
        }
    }
    fn is_bool(&self) -> bool {
        match self {
            PdfObject::Reference(ref link) => match link.get() {
                Ok(val) => val.is_bool(),
                _ => false
            },
            PdfObject::Actual(ref obj) =>  match obj {
                Boolean(_) => true,
                _ => false
            },
        }
    }
    fn is_stream(&self) -> bool {
        match self {
            PdfObject::Reference(ref link) => match link.get() {
                Ok(val) => val.is_stream(),
                _ => false
            },
            PdfObject::Actual(ref obj) =>  match obj {
                BinaryStream(_) | ContentStream(_) => true,
                _ => false
            },
        }
    }
    fn is_name(&self) -> bool {
        match self {
            PdfObject::Reference(ref link) => match link.get() {
                Ok(val) => val.is_name(),
                _ => false
            },
            PdfObject::Actual(ref obj) =>  match obj {
                Name(_) => true,
                _ => false
            },
        }
    }
    fn is_number(&self) -> bool {
        match self {
            PdfObject::Reference(ref link) => match link.get() {
                Ok(val) => val.is_number(),
                _ => false
            },
            PdfObject::Actual(ref obj) =>  match obj {
                NumberFloat(_) | NumberInt(_) => true,
                _ => false
            },
        }
    }
}

impl Clone for PdfObject {
    fn clone(&self) -> PdfObject {
        match &self {
            &PdfObject::Actual(obj) => PdfObject::Actual(obj.clone()),
            &PdfObject::Reference(obj_ref) => PdfObject::Reference(PdfObjectReference{
                id: obj_ref.id, data: Weak::clone(&obj_ref.data)
            })
        }
    }
}


impl fmt::Display for PdfObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PdfObject::Reference(r) => write!(f, "Reference to object #{}", r.id),
            PdfObject::Actual(obj) => match obj {
                Boolean(b) => write!(f, "Boolean: {}", b),
                NumberInt(n) => write!(f, "Number: {}", n),
                NumberFloat(n) => write!(f, "Number: {:.2}", n),
                Name(s) => write!(f, "Name: {}", s),
                CharString(s) => write!(f, "String: {}", s),
                HexString(s) => write!(f, "String: {:?}", s),
                Array(v) => write!(f, "Array: {:#?}", v),
                Dictionary(h) => write!(f, "Dictionary: {:#?}", h),
                ContentStream(d) => write!(f, "Content stream object: {}", d),
                BinaryStream(d) => write!(f, "Binary stream object: {}", d),
                ObjectStream(d) => write!(f, "Object stream object: {}", d),
                Comment(s) => write!(f, "Comment: {:?}", s),
                Null => write!(f, "Null")
            //Keyword(kw) => write!(f, "Keyword: {:?}", kw),
            }
        }?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct PdfObjectReference<T: ParserInterface<PdfObject>> {
    id: ObjectId,
    data: Weak<T>,
}

impl<T: ParserInterface<PdfObject> + Debug> PdfObjectReference<T> {
    fn get(&self) -> Result<SharedObject> {
        let usable_ref = self.data.upgrade().expect("Could not access weak ref in File Interface get");
        usable_ref.retrieve_object_by_ref(self.id)
    }
}

struct PdfFile {}

pub struct Image {}
pub struct ContentStream {}

#[derive(Debug, Clone, Copy)]
pub enum DataType {
    Boolean,
    VecObjects,
    I32,
    F32,
    String,
    VecU8,
    HashMap,
    Null
}

#[derive(Debug, Clone, Copy)]
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
    Null
}
