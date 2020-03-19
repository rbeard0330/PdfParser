mod decode;
mod util;

use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryInto;
use std::fmt;
use std::fs;
use std::rc::{Rc, Weak};
use std::str;

use crate::errors::*;

pub use super::pdf_objects::*;
use util::*;
use PdfDataType::*;

pub trait PdfFileInterface<T: PdfObjectInterface> {
    fn retrieve_object_ref(&self, id: u32, gen: u32) -> Result<Rc<T>>;
    fn retrieve_root(&self) -> Result<Rc<T>>;
}

#[derive(Debug)]
pub struct ObjectCache(RefCell<HashMap<ObjectId, Rc<PdfObject>>>);

impl ObjectCache {
    fn new() -> Self {
        ObjectCache(RefCell::new(HashMap::new()))
    }
}

impl PdfFileInterface<PdfObject> for ObjectCache {}

#[derive(Debug)]
pub struct PdfFileHandler {
    bytes: Vec<u8>,
    pub version: Option<PDFVersion>,
    trailer: Option<PDFTrailer>,
    index_map: HashMap<ObjectId, usize>,
    object_map: Rc<ObjectCache>,
    weak_ref_to_clone: Weak<ObjectCache>,
}

impl PdfFileInterface<PdfObject> for PdfFileHandler {
    fn retrieve_object_ref(&self, id: u32, gen: u32) -> Result<SharedObject> {
        self.object_map.retrieve_object_ref(id, gen)
    }
    fn retrieve_root(&self) -> Result<SharedObject> {
        Ok(Rc::clone(
            self.trailer
                .as_ref()
                .expect("Parse trailer first!")
                .trailer_dict
                .try_into_map()
                .chain_err(|| "Error processing trailer")?
                .get("Root")
                .chain_err(|| "Could not retrieve Root directory")?,
        ))
    }
}

impl PdfFileHandler {
    pub fn create_pdf_from_file(path: &str) -> Result<Self> {
        let bytes = fs::read(path)?;
        let cache_ref = Rc::new(ObjectCache::new());
        let weak_ref = Rc::downgrade(&cache_ref);
        let mut pdf = PdfFileHandler {
            bytes,
            version: None,
            trailer: None,
            index_map: HashMap::new(),
            object_map: cache_ref,
            weak_ref_to_clone: weak_ref,
        };
        pdf.set_version()?;
        let trailer_index = pdf.find_trailer_index()?;
        //println!("trailer starts at: {:?}", trailer_index);
        pdf.trailer = Some(pdf.process_trailer(trailer_index)?);
        pdf.process_xref_table()?;
        Ok(pdf)
    }

    pub fn get_root(&self) -> Result<Rc<PdfObject>> {
        Ok(Rc::clone(
            self.trailer
                .as_ref()
                .expect("Parse trailer first!")
                .trailer_dict
                .try_into_map()
                .chain_err(|| "Error processing trailer")?
                .get("Root")
                .chain_err(|| "Could not retrieve Root directory")?,
        ))
    }

    fn set_version(&mut self) -> Result<()> {
        let intro = String::from_utf8(
            self.bytes[..12]
                .iter()
                .map(|c| *c)
                .take_while(|c| !is_EOL(*c))
                .collect(),
        );
        let intro = match intro {
            Ok(s) if s.contains("%PDF-") => s,
            _ => {
                return Err(ErrorKind::ParsingError(format!(
                    "Could not find version number in {:?}",
                    intro
                )))?
            }
        };
        self.version = match intro
            .splitn(2, "%PDF-")
            .last()
            .unwrap()
            .split_at(3)
            .0
            .parse::<f32>()
        {
            Ok(1.0) => Some(PDFVersion::V1_0),
            Ok(1.1) => Some(PDFVersion::V1_1),
            Ok(1.2) => Some(PDFVersion::V1_2),
            Ok(1.3) => Some(PDFVersion::V1_3),
            Ok(1.4) => Some(PDFVersion::V1_4),
            Ok(1.5) => Some(PDFVersion::V1_5),
            Ok(1.6) => Some(PDFVersion::V1_6),
            Ok(1.7) => Some(PDFVersion::V1_7),
            Ok(2.0) => Some(PDFVersion::V2_0),
            Ok(x) if x > 2.0 => Err(ErrorKind::ParsingError(format!(
                "Unsupported PDF version: {}",
                x
            )))?,
            _ => Err(ErrorKind::ParsingError(
                "Could not find PDF version".to_string(),
            ))?,
        };
        Ok(())
    }

    fn find_trailer_index(&self) -> Result<usize> {
        let mut state: usize = 0;
        let mut current_index = self.bytes.len() as usize;
        while state < 7 {
            current_index -= 1;
            let c = self.bytes[current_index] as char;
            //println!("char {} with {}", c, state);
            state = match state {
                1 if c == 'e' => 2,
                2 if c == 'l' => 3,
                3 if c == 'i' => 4,
                4 if c == 'a' => 5,
                5 if c == 'r' => 6,
                6 if c == 't' => 7,
                _ if c == 'r' => 1,
                _ => 0,
            };

            if current_index + state <= 6 {
                return Err(ErrorKind::ParsingError(
                    "Could not find trailer".to_string(),
                ))?;
            };
        }
        Result::Ok(current_index)
    }

    fn process_trailer(&mut self, start_index: usize) -> Result<PDFTrailer> {
        assert_eq!(
            &(String::from_utf8(Vec::from(&self.bytes[start_index..start_index + 7])).unwrap()),
            "trailer"
        );
        let (trailer_dict, next_index) = self.parse_object(start_index + 7)?;
        let trailer_string = String::from_utf8(self.bytes[(next_index + 1)..].to_vec())
            .expect("Could not convert trailer to string!");
        let mut trailer_lines = trailer_string.lines().filter(|l| !l.trim().is_empty());
        let first_line = trailer_lines.next().expect("No line after trailer dict!");
        //println!("{}", trailer_string);
        if first_line != "startxref" {
            Err(ErrorKind::ParsingError(format!(
                "startxref keyword not found at {}",
                next_index
            )))?
        };
        let second_line = trailer_lines.next().expect("No xref location in trailer");
        let xref_index = second_line
            .trim()
            .parse()
            .expect("Invalid xref index in trailer");
        let third_line = trailer_lines.next().expect("Missing file terminator!");
        assert_eq!(third_line, "%%EOF");
        assert_eq!(trailer_lines.next(), None);
        return Ok(PDFTrailer {
            start_index,
            trailer_dict,
            xref_index,
        });
    }

    fn process_xref_table(&mut self) -> Result<()> {
        let trailer = self
            .trailer
            .as_ref()
            .expect("Parse trailer before parsing xref table!");
        let start_index = trailer.xref_index;
        let end_index = trailer.start_index - 1;
        let table = String::from_utf8(self.bytes[start_index..end_index].to_vec())
            .expect("Invalid xref table");
        //println!("{}", table);
        let mut line_iter = table.lines();
        let mut obj_number = 0;
        assert_eq!(line_iter.next().unwrap(), "xref");
        loop {
            let line = line_iter.next();
            if let None = line {
                return Ok(());
            };
            //println!("{:?}", line);
            let parts: Vec<&str> = line.unwrap().split_whitespace().collect();
            if parts.len() == 3 {
                if parts[2] == "f" {
                    obj_number += 1
                } else {
                    self.index_map.insert(
                        ObjectId(
                            obj_number,
                            parts[1].parse().expect("Could not parse gen number"),
                        ),
                        parts[0].parse().expect("Could not parse offset"),
                    );
                    obj_number += 1;
                }
            } else if parts.len() == 2 {
                obj_number = parts[0].parse().expect("Could not parse object number");
            } else {
                println!("{:?}", parts);
                return Err(ErrorKind::ParsingError(format!(
                    "Invalid line in xref table: {:?}",
                    parts
                )))?;
            }
        }
    }

    fn parse_object(&mut self, start_index: usize) -> Result<(PdfObject, usize)> {
        let mut state = ParserState::Neutral;
        let mut index = start_index;
        let mut this_object_type = PDFComplexObject::Unknown;
        let length = self.bytes.len();
        if index > length {
            return Err(ErrorKind::ParsingError(format!(
                "index {} out of range",
                index
            )))?;
        }
        let mut char_buffer = Vec::new();
        let mut object_buffer = Vec::new();
        loop {
            if index > length {
                return Err(ErrorKind::ParsingError(
                    "end of file while parsing object".to_string(),
                ))?;
            };
            let c = self.bytes[index];
            state = match state {
                ParserState::Neutral => match c {
                    b'[' if this_object_type == PDFComplexObject::Unknown => {
                        this_object_type = PDFComplexObject::Array;
                        state
                    }
                    b'[' => {
                        let (new_array, end_index) = self.parse_object(index)?;
                        index = end_index;
                        object_buffer.push(new_array);
                        state
                    }
                    b']' => {
                        if this_object_type == PDFComplexObject::Array {
                            return make_array_from_object_buffer(object_buffer, index);
                        } else {
                            return Err(ErrorKind::ParsingError(format!(
                                "Invalid terminator for {:?} at {}: ]",
                                this_object_type, index
                            )))?;
                        }
                    }
                    b'<' if peek_ahead_by_n(&self.bytes, index, 1) == Some(b'<') => {
                        if this_object_type == PDFComplexObject::Unknown {
                            this_object_type = PDFComplexObject::Dict;
                            index += 1;
                        //println!("Dict started at: {}", index);
                        } else {
                            //println!("Nested dict in {:?} at {}", this_object_type, index);
                            let (new_dict, end_index) = self.parse_object(index)?;
                            index = end_index;
                            //println!("Nested dict closed at {}", index);
                            object_buffer.push(new_dict);
                        };
                        state
                    }
                    b'<' => ParserState::HexString,
                    b'>' if (peek_ahead_by_n(&self.bytes, index, 1) == Some(b'>')) => {
                        if this_object_type == PDFComplexObject::Dict {
                            //println!("Dictionary ended at {}", index + 1);
                            return make_dict_from_object_buffer(object_buffer, index + 1);
                        } else {
                            println!("-------Dictionary ended but I'm a {:?}", this_object_type);
                            println!("Buffer: {:#?}", object_buffer);
                            return Err(ErrorKind::ParsingError(format!(
                                "Invalid terminator for {:?} at {}: >>",
                                this_object_type, index
                            )))?;
                        }
                    }
                    b'(' => ParserState::CharString(0),
                    b'/' => ParserState::Name,
                    b'R' => {
                        let object_buffer_length = object_buffer.len();
                        if object_buffer_length <= 1
                            || !object_buffer[object_buffer_length - 2].is_int()
                            || !object_buffer[object_buffer_length - 1].is_int()
                            || object_buffer[object_buffer_length - 2]
                                .try_into_int()
                                .unwrap()
                                < 0
                            || object_buffer[object_buffer_length - 1]
                                .try_into_int()
                                .unwrap()
                                < 0
                        {
                            return Err(ErrorKind::ParsingError(format!(
                                "Could not parse reference to object at {}",
                                index
                            )))?;
                        };
                        let new_object = PdfObject::new_reference(
                            object_buffer[object_buffer_length - 2]
                                .try_into_int()
                                .unwrap()
                                .try_into().unwrap(),
                            object_buffer[object_buffer_length - 1]
                                .try_into_int()
                                .unwrap()
                                .try_into().unwrap(),
                            Weak::clone(&self.weak_ref_to_clone),
                        );

                        object_buffer.truncate(object_buffer_length - 2);
                        object_buffer.push(new_object);
                        state
                    }
                    b's' | b'e' | b'o' | b'n' | b't' | b'f' => {
                        char_buffer.push(c);
                        ParserState::Keyword
                    }
                    b'0'..=b'9' | b'+' | b'-' => {
                        index -= 1;
                        ParserState::Number
                    }
                    _ if is_whitespace(c) => state,
                    _ => {
                        return Err(ErrorKind::ParsingError(format!(
                            "Invalid character at {}: {}",
                            index, c as char
                        )))?
                    }
                },
                ParserState::HexString => match c {
                    b'>' => {
                        object_buffer.push(flush_buffer_to_object(&state, &mut char_buffer)?);
                        ParserState::Neutral
                    }
                    b'0'..=b'9' | b'A'..=b'F' => {
                        char_buffer.push(c);
                        state
                    }
                    _ if is_whitespace(c) => state,
                    _ => {
                        return Err(ErrorKind::ParsingError(format!(
                            "invalid character in hexstring at {}: {}",
                            index, c as char
                        )))?
                    }
                },
                ParserState::CharString(depth) => match c {
                    b')' if depth == 0 => {
                        //println!("Making a string at {}", index);
                        object_buffer.push(flush_buffer_to_object(&state, &mut char_buffer)?);
                        ParserState::Neutral
                    }
                    b')' if depth > 0 => ParserState::CharString(depth - 1),
                    b'(' => ParserState::CharString(depth + 1),
                    b'\\' if index + 1 < length => {
                        match self.bytes[index + 1] {
                            15 => {
                                index += 1; // Skip carriage return
                                if index + 1 < length && self.bytes[index + 1] == 12 {
                                    index += 1
                                }; // Skip linefeed too
                                state
                            }
                            12 => {
                                index += 1;
                                state
                            } // Escape naked LF
                            b'\\' => {
                                index += 1;
                                char_buffer.push(b'\\');
                                state
                            }
                            b'(' => {
                                index += 1;
                                char_buffer.push(b'(');
                                state
                            }
                            b')' => {
                                index += 1;
                                char_buffer.push(b')');
                                state
                            }
                            d @ b'0'..=b'7' => {
                                index += 1;
                                let mut code = d - b'0';
                                if index + 1 < length && is_octal(self.bytes[index + 1]) {
                                    code = code * 8 + (self.bytes[index + 1] - b'0');
                                    index += 1;
                                    if index + 1 < length && is_octal(self.bytes[index + 1]) {
                                        code = code * 8 + (self.bytes[index + 1] - b'0');
                                        index += 1;
                                    }
                                };
                                char_buffer.push(code);
                                state
                            }
                            _ => state, // Other escaped characters do not require special treatment
                        }
                    }
                    _ => {
                        char_buffer.push(c);
                        state
                    }
                },
                ParserState::Name => {
                    if c != b'%' && (is_whitespace(c) || is_delimiter(c)) {
                        object_buffer.push(flush_buffer_to_object(&state, &mut char_buffer)?);
                        index -= 1; // Need to parse delimiter character on next iteration
                        ParserState::Neutral
                    } else {
                        char_buffer.push(c);
                        state
                    }
                }
                ParserState::Number => match c {
                    b'0'..=b'9' => {
                        char_buffer.push(c);
                        state
                    }
                    b'-' | b'+' if char_buffer.len() == 0 => {
                        char_buffer.push(c);
                        state
                    }
                    b'.' => {
                        if char_buffer.contains(&b'.') {
                            return Err(ErrorKind::ParsingError(
                                "two decimal points in number".to_string(),
                            ))?;
                        };
                        char_buffer.push(c);
                        state
                    }
                    _ if is_whitespace(c) || is_delimiter(c) => {
                        object_buffer.push(flush_buffer_to_object(&state, &mut char_buffer)?);
                        index -= 1; // Need to parse delimiter character on next iteration
                        ParserState::Neutral
                    }
                    _ => {
                        return Err(ErrorKind::ParsingError(format!(
                            "invalid character in number at {}: {}",
                            index, c as char
                        )))?
                    }
                },
                ParserState::Comment => {
                    if is_EOL(c) {
                        object_buffer.push(flush_buffer_to_object(&state, &mut char_buffer)?);
                        ParserState::Neutral
                    } else {
                        char_buffer.push(c);
                        state
                    }
                }
                ParserState::Keyword => {
                    if !is_body_keyword_letter(c) {
                        if !(is_delimiter(c) || is_whitespace(c)) {
                            return Err(ErrorKind::ParsingError(format!(
                                "invalid character in keyword at {}: {}",
                                index, c as char
                            )))?;
                        };
                        let s = str::from_utf8(&char_buffer).unwrap();
                        char_buffer.clear();
                        let this_keyword = match s {
                            "obj" => PDFKeyword::Obj,
                            "endobj" => PDFKeyword::EndObj,
                            "stream" => PDFKeyword::Stream,
                            "endstream" => PDFKeyword::EndStream,
                            "null" => PDFKeyword::Null,
                            "false" => PDFKeyword::False,
                            "true" => PDFKeyword::True,
                            _ => Err(ErrorKind::ParsingError(format!(
                                "Invalid PDF keyword: {}",
                                s
                            )))?,
                        };
                        match this_keyword {
                            PDFKeyword::EndObj => {
                                if this_object_type == PDFComplexObject::IndirectObj {
                                    return make_object_from_object_buffer(object_buffer, index);
                                } else {
                                    return Err(ErrorKind::ParsingError(format!(
                                        "Encountered endobj outside indirect object at {}",
                                        index
                                    )))?;
                                };
                            }
                            PDFKeyword::Stream => {
                                return self.make_stream_object(object_buffer, index)
                            }
                            PDFKeyword::Obj if this_object_type != PDFComplexObject::Unknown => {
                                return Err(ErrorKind::ParsingError(format!(
                                    "Encountered nested obj declaration at {}",
                                    index
                                )))?
                            }
                            PDFKeyword::Obj => {
                                this_object_type = PDFComplexObject::IndirectObj;
                                index -= 1;
                                ParserState::Neutral
                            }
                            PDFKeyword::True => {
                                object_buffer.push(PdfObject::new_boolean(true));
                                index -= 1;
                                ParserState::Neutral
                            }
                            _ => {
                                return Err(ErrorKind::ParsingError(format!(
                                    "Unrecognized keyword at {}: {:?}",
                                    index, this_keyword
                                )))?
                            }
                        }
                    } else {
                        char_buffer.push(c);
                        state
                    }
                }
            };
            index += 1;
        }
    }

    fn make_stream_object(
        &mut self,
        mut object_buffer: Vec<PdfObject>,
        index: usize,
    ) -> Result<(PdfObject, usize)> {
        if object_buffer.len() != 3 {
            Err(ErrorKind::ParsingError(format!(
                "Expected stream at {} to be preceded by a sole dictionary",
                index
            )))?;
        };
        let binary_start_index = match self.bytes[index] {
            b'\n' => index + 1,
            b'\r' => {
                if let Some(b'\n') = peek_ahead_by_n(&self.bytes, index, 1) {
                    index + 2
                } else {
                    Err(ErrorKind::ParsingError(format!(
                        "Stream tag not followed by appropriate spacing at {}",
                        index
                    )))?
                }
            }
            _ => Err(ErrorKind::ParsingError(format!(
                "Stream tag not followed by appropriate spacing at {}",
                index
            )))?,
        };
        let stream_dict = object_buffer
            .pop()
            .unwrap()
            .try_into_map()
            .chain_err(|| {
                ErrorKind::ParsingError(format!(
                    "Stream at {} preceded by a non-dictionary object",
                    index
                ))
            })?
            .clone();
        //println!("{:#?}", stream_dict);
        let id_number = object_buffer[0]
            .try_into_int()
            .chain_err(|| ErrorKind::ParsingError("Invalid object number".to_string()))?;
        let gen_number = object_buffer[0]
            .try_into_int()
            .chain_err(|| ErrorKind::ParsingError("Invalid gen number".to_string()))?;
        let binary_length = stream_dict
            .get("Length")
            .ok_or(ErrorKind::ParsingError(format!(
                "No Length value for stream {}",
                id_number
            )))?
            .try_into_int()
            .chain_err(|| ErrorKind::ParsingError("Invalid gen number".to_string()))?
            as usize;
        // TODO: Confirm endstream included
        if binary_start_index + binary_length >= self.bytes.len() {
            Err(ErrorKind::ParsingError(format!(
                "Reported binary content length for Obj#{} ({}) too long",
                id_number, binary_length
            )))?
        };
        Ok((
            PDFObj::Stream(
                stream_dict,
                Vec::from(&self.bytes[binary_start_index..(binary_start_index + binary_length)]),
            ),
            binary_start_index + binary_length + 9,
        ))
    }
}

#[derive(Debug, PartialEq)]
pub enum PDFVersion {
    V1_0,
    V1_1,
    V1_2,
    V1_3,
    V1_4,
    V1_5,
    V1_6,
    V1_7,
    V2_0,
}
pub mod PdfTypes {
    use super::super::pdf_objects::{DataType, PdfDataType};
    use super::*;
}

//TODO: Remove pub fields
#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone)]
pub struct ObjectId(pub u32, pub u16);

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Object {} {}", self.0, self.1)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct PDFStreamObject {
    object_type: StreamType,
}

#[derive(Debug, PartialEq, Clone)]
enum StreamType {
    Content,
    Object,
    XRef,
    Image,
    Unknown,
}

#[derive(Debug, PartialEq)]
enum PDFComplexObject {
    Unknown,
    Dict,
    Array,
    IndirectObj,
}

#[derive(Debug)]
struct PDFTrailer {
    start_index: usize,
    trailer_dict: PDFObj,
    xref_index: usize,
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum ParserState {
    Neutral,
    HexString,
    CharString(u8),
    Name,
    Number,
    Comment,
    Keyword,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum PDFKeyword {
    Stream,
    EndStream,
    Obj,
    EndObj,
    R,
    Null,
    True,
    False,
    XRef,
    F,
    N,
    Trailer,
    StartXRef,
}

fn flush_buffer_to_object(state: &ParserState, buffer: &mut Vec<u8>) -> Result<PDFObj> {
    let new_obj = match state {
        ParserState::Neutral => Err(ErrorKind::ParsingError(
            "Called flush buffer in Neutral context".to_string(),
        ))?,
        ParserState::HexString => {
            //TODO: ADD PADDING
            for c in buffer.iter() {
                if !is_hex(*c) {
                    Err(ErrorKind::ParsingError(format!("Invalid character in hex string: {}", c)))?
                };
            }
            PDFObj::HexString(buffer.clone() as Vec<u8>)
        }
        ParserState::CharString(0) => {
            PDFObj::CharString(String::from_utf8_lossy(buffer).into_owned())
        }
        ParserState::CharString(_c) => {
            return Err(PDFError {
                message: "Asked to create string with unclosed parentheses".to_string(),
                function: "flush_buffer_to_object",
            });
        }
        ParserState::Name => PDFObj::Name(str::from_utf8(buffer).unwrap().to_string()),
        ParserState::Number => {
            if buffer.contains(&b'.') {
                PDFObj::NumberFloat(str::from_utf8(buffer).unwrap().parse().unwrap())
            } else {
                PDFObj::NumberInt(str::from_utf8(buffer).unwrap().parse().unwrap())
            }
        }
        ParserState::Comment => PDFObj::Comment(str::from_utf8(buffer).unwrap().to_string()),
        ParserState::Keyword => {}
    };
    buffer.clear();
    return Ok(new_obj);
}

fn make_array_from_object_buffer(
    object_buffer: Vec<PDFObj>,
    end_index: usize,
) -> Result<(PDFObj, usize)> {
    let mut new_vec = Vec::new();
    for obj in object_buffer {
        new_vec.push(Rc::new(obj));
    }
    Ok((PDFObj::Array(new_vec), end_index))
}

fn make_dict_from_object_buffer(
    object_buffer: Vec<PDFObj>,
    end_index: usize,
) -> Result<(PDFObj, usize)> {
    let mut dict = HashMap::new();
    let mut object_it = object_buffer.into_iter();
    loop {
        let key = match object_it.next() {
            None => {
                //println!("completed a dict: {:?}", dict);
                return Ok((PDFObj::Dictionary(dict), end_index));
            }
            Some(PDFObj::Name(s)) => s,
            Some(obj) => {
                return Err(PDFError {
                    message: format!("Dictionary key ({:?}) was not a Name", obj),
                    function: "make_dict_from_object_buffer",
                })
            }
        };
        let value = match object_it.next() {
            None => {
                return Err(PDFError {
                    message: "No object for key".to_string(),
                    function: "make_dict_from_object_buffer",
                })
            }
            Some(v) => v,
        };
        dict.insert(key, Rc::new(value));
    }
}

fn make_object_from_object_buffer(
    mut object_buffer: Vec<PDFObj>,
    end_index: usize,
) -> Result<(PDFObj, usize), PDFError> {
    if object_buffer.len() != 3 {
        return Err(PDFError {
            message: format!("Object tags contained {} objects", object_buffer.len()),
            function: "make_object_from_object_buffer",
        });
    };
    let id_number = match object_buffer[0] {
        PDFObj::NumberInt(i) => i as u32,
        _ => {
            return Err(PDFError {
                message: "invalid object number".to_string(),
                function: "make_object_from_object_buffer",
            })
        }
    };
    let gen_number = match object_buffer[1] {
        PDFObj::NumberInt(i) => i as u16,
        _ => {
            return Err(PDFError {
                message: "invalid generation number".to_string(),
                function: "make_object_from_object_buffer",
            })
        }
    };
    return Ok((object_buffer.pop().unwrap(), end_index));
}

// -----------Utility functions----------------

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PDFS: [&str; 4] = [
        "data/simple_pdf.pdf",
        "data/CCI01212020.pdf",
        "data/document.pdf",
        "data/2018W2.pdf",
    ];

    #[test]
    fn test_sample_pdfs_sensitive() {
        let mut results = Vec::new();
        for path in &TEST_PDFS {
            println!("{}", path);
            let mut pdf = PdfFileHandler::create_pdf_from_file(path).unwrap();
            results.push(add_all_objects(&mut pdf));
        }
        let results: Vec<PDFError> = results
            .into_iter()
            .filter(|result| result.is_err())
            .map(|err| err.unwrap_err())
            .collect();
        if results.len() > 0 {
            for err in results {
                println!("ERROR: {:#?}", err);
            }
            panic!();
        }
    }

    #[test]
    fn test_sample_pdfs_stoic() {
        for path in &TEST_PDFS {
            println!("{}", path);
            let mut pdf = PdfFileHandler::create_pdf_from_file(path).unwrap();
            add_all_objects(&mut pdf);
        }
    }

    fn add_all_objects(pdf: &mut PdfFileHandler) -> Result<(), PDFError> {
        let objects_to_add: Vec<(ObjectId, usize)> =
            pdf.index_map.iter().map(|(a, b)| (*a, *b)).collect();
        for (object_number, _index) in objects_to_add {
            println!("Retrieving Obj #{}", object_number);
            match pdf.get_object(&object_number) {
                Ok(obj) => {} //println!("Obj #{} successfully retrieved: {}", object_number, obj);},
                Err(e) => {
                    println!("**Obj #{} ERROR**: {:?}", object_number, e);
                    return Err(e);
                }
            };
        }
        Ok(())
    }
}
