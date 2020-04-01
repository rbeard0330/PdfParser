pub mod decode;
mod util;
mod file_reader;
pub mod object_cache;

use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::fs;
use std::io::{Seek, SeekFrom};
use std::ops::DerefMut;
use std::rc::{Rc, Weak};
use std::str;

use crate::errors::*;
use ErrorKind::*;
pub use object_cache::{ObjectCache, ObjectLocation};

pub use super::pdf_objects::*;
use util::*;
use file_reader::{PdfFileReader, PdfFileReaderInterface};

pub trait ParserInterface<T: PdfObjectInterface> {
    fn retrieve_object_by_ref(&self, id: ObjectId) -> Result<Rc<T>>;
    fn retrieve_trailer(&self) -> Result<&PdfObject>;
}

#[derive(Debug)]
pub struct Parser {
    trailer: Option<PdfObject>,
    pub object_map: Rc<ObjectCache>,
}

enum XrefType {
    Standard,
    Stream
}

impl ParserInterface<PdfObject> for Parser {
    fn retrieve_object_by_ref(&self, id: ObjectId) -> Result<SharedObject> {
        self.object_map.retrieve_object_by_ref(id)
    }
    fn retrieve_trailer(&self) -> Result<&PdfObject> {
        match self.trailer {
            None => Err(UnavailableType("trailer".to_string(), "new parser".to_string()))?,
            Some(ref dict) => Ok(dict)
        }
    }
}

impl Parser {
    pub fn create_pdf_from_file(path: &str) -> Result<Self> {
        //TODO: Fix the index
        let mut reader = PdfFileReader::new(path)?;
        let (xref_start, xref_type) = Parser::find_xref_start_and_type(&mut reader)?;

        let null_ref = Weak::new();
        let cache_ref = Rc::new(ObjectCache::new(reader, HashMap::new(), null_ref.clone()));
        let weak_ref = Rc::downgrade(&cache_ref);
        cache_ref.update_reference(Weak::clone(&weak_ref));
        let mut pdf = Parser {
            trailer: None,
            object_map: cache_ref,
        };
        let (index, file_trailer) = match xref_type {
            XrefType::Standard =>  {
                let xrefs = Parser::process_standard_xref_table(pdf.object_map.reader(), xref_start)?;
                let (trailer, _) = Parser::get_standard_trailer(pdf.object_map.reader(), &weak_ref)?;
                (xrefs, Some(trailer))
            },
            XrefType::Stream => {
                let (xrefs, trailer) = pdf.process_xref_stream(xref_start, &weak_ref)?;
                (xrefs, Some(trailer))
            }
        };
        
        pdf.trailer = file_trailer;
        pdf.object_map.update_index(index);
        Ok(pdf)
    }

    fn find_xref_start_and_type(reader: &mut PdfFileReader) -> Result<(usize, XrefType)> {
        reader.seek(SeekFrom::End(-1))?;
        assert_eq!(str::from_utf8(reader.peek_current_line()).expect("Internal error: line not ascii"), "%%EOF");
        let steps = reader.step_to_end_of_prior_line();
        debug_assert!(steps != 0);
        let xref_start: usize = str::from_utf8(reader.peek_current_line())
                                .chain_err(|| ErrorKind::ParsingError(format!("Xref start contains non-ASCII")))?
                                .parse()
                                .chain_err(|| ErrorKind::ParsingError(format!("Xref start not an integer")))?;
        let steps = reader.step_to_end_of_prior_line();
        debug_assert!(steps != 0);
        assert_eq!(str::from_utf8(reader.peek_current_line()).expect("Internal error: line not ascii"), "startxref");
        reader.seek(SeekFrom::Start(xref_start as u64))?;
        match reader.peek_current_line() {
            &[b'x', b'r', b'e', b'f'] => Ok((xref_start, XrefType::Standard)),
            line @ _ => {
                let slice_length = line.len();
                if slice_length < 7 {
                    Err(ErrorKind::ParsingError(format!("No valid xref table at {}: {:?}", xref_start, line)))?
                };
                match line[(slice_length - 3)..] {
                    [b'o', b'b', b'j'] => return Ok((xref_start, XrefType::Stream)),
                    _ => Err(ErrorKind::ParsingError(format!("No valid xref table at {}: {:?}", xref_start, line)))?
                }
            }
        }
    }


    fn get_standard_trailer(mut reader: PdfFileReader, weak_ref: &Weak<ObjectCache>)
            -> Result<(PdfObject, PdfFileReader)> {
        reader.seek(SeekFrom::End(-1)).unwrap();
        loop {
            let line = String::from_utf8_lossy(reader.peek_current_line()).trim().to_owned();
            if line == "trailer" {
                reader.step_to_start_of_next_line();
                let pos = reader.position();
                return parse_object_at(reader.spawn_clone(), pos, &Weak::clone(&weak_ref))
                        .chain_err(|| ParsingError("invalid trailer".to_string()))
            };
            if reader.position() == 0 {
                Err(ParsingError("Reached beginning of file without finding trailer".to_string()))?
            };
            reader.step_to_end_of_prior_line();
        }
    }

    fn process_standard_xref_table(mut reader: PdfFileReader, start_index: usize)
            -> Result<HashMap<ObjectId, ObjectLocation>> {
        reader.seek(SeekFrom::Start(start_index as u64))?;
        debug_assert_eq!(reader.read_current_line(), &[b'x', b'r', b'e', b'f']);
        let mut index_map = HashMap::new();
        let mut free_objects = Vec::new();
        let mut obj_number = 0;
        let mut objects_to_go = 0;
        loop {
            let line = String::from_utf8_lossy(reader.read_current_line()).trim().to_owned();
            //println!("Reading line {}", line);
            
            if !(line.chars().last().unwrap() == 'n' || line.chars().last().unwrap() == 'f') {
                if line == "trailer" {break};
                let line_components: Result<Vec<u32>> =
                    line.split_whitespace()
                        .map(|s| s.parse().chain_err(||ParsingError(format!("Could not parse {:?}", s))))
                        .collect();
                let line_components = line_components?;
                if line_components.len() != 2 {
                    Err(ParsingError(format!("Invalid line format: {:#?}", line_components)))?
                };
                if objects_to_go != 0 {
                    Err(ParsingError(
                        format!("Expected {} more objects at pos :{}", objects_to_go, 0)
                    ))? 
                };
                obj_number = line_components[0];
                objects_to_go = line_components[1];
                continue
            };
            if objects_to_go == 0 { break };
            let line_components: Vec<_> = line.split_whitespace().collect();
            if line_components.len() != 3 {
                Err(ParsingError(format!("Invalid line format: {:#?}", line_components)))?
            };
            let first_number = line_components[0]
                                .parse()
                                .chain_err(
                                    ||ParsingError(format!("Non-integer as object identifier: {}", line_components[0]))
                                )?;
            let second_number = line_components[1]
                                .parse()
                                .chain_err(
                                    ||ParsingError(format!("Non-integer as object identifier: {}", line_components[1]))
                                )?;
            match line_components[2] {
                "n" => { index_map.insert(ObjectId(obj_number, second_number),
                         ObjectLocation::Uncompressed(first_number)); },
                "f" => free_objects.push(first_number),
                _ => Err(ParsingError(format!("Could not resolve line-end: {}", line_components[2])))?
            };
            obj_number += 1;
            objects_to_go -= 1;
        }
        let _sink = free_objects;
        Ok(index_map)
    }

    fn process_xref_stream(&mut self, start_index: usize, weak_ref: &Weak<ObjectCache>)
            -> Result<(HashMap<ObjectId, ObjectLocation>, PdfObject)> {
        let (stream, _) = parse_object_at(self.object_map.reader(), start_index, weak_ref)?;
        let map = stream.try_into_map().unwrap();
        let v: Vec<_> = map.get("W")
                             .ok_or(ParsingError(format!("Missing W entry in crossref stream")))?
                             .try_into_array()?
                             .iter()
                             .map(|obj| obj.try_into_int().unwrap() as usize)
                             .collect::<Vec<_>>();
        let data = stream.try_into_binary()?;
        let line_length = v[0] + v[1] + v[2];
        assert_eq!(data.len() % line_length, 0);
        let line_count = data.len() / line_length;
        for line_ix in 0..line_count {
            let line_start = line_ix * line_length as usize;
            let field1 = u8_slice_as_int(&data[line_start..(line_start + v[0])]);
            let field2 = u8_slice_as_int(&data[(line_start + v[0])..(line_start + v[0] + v[1])]);
            let field3 = u8_slice_as_int(&data[(line_start + v[1])..(line_start + v[0] + v[2])]);
        }



        Err(ParsingError(format!("Not implemented")))?

    }
}


fn parse_object_at(input_reader: PdfFileReader, start_index: usize, weak_ref: &Weak<ObjectCache>)
        -> Result<(PdfObject, PdfFileReader)> {
    let mut state = ParserState::Neutral;
    let mut reader = input_reader.spawn_clone();
    reader.seek(SeekFrom::Start(start_index as u64))
          .chain_err(|| ParsingError(format!("Index {} out of bounds", start_index)))?;
    let mut this_object_type = PDFComplexObject::Unknown;
    let mut char_buffer = Vec::new();
    let mut object_buffer = Vec::new();
    loop {
        let slice = reader.read_and_copy_n(1); // This advances the reader by 1, so current position is *after* c
        if slice.len() == 0 {
            return Err(ErrorKind::ParsingError(
                "end of file while parsing object".to_string(),
            ))?;
        };
        debug_assert_eq!(slice.len(), 1);
        let c = slice[0];
        state = match state {
            ParserState::Neutral => match c {
                b'[' if this_object_type == PDFComplexObject::Unknown => {
                    this_object_type = PDFComplexObject::Array;
                    state
                }
                b'[' => {
                    let pos = reader.position() - 1;
                    //println!("Recursive call in array: {}", String::from_utf8_lossy(reader.peek_current_line()));
                    let (new_array, returned_reader) = parse_object_at(reader, pos, weak_ref)?;
                    reader = returned_reader;
                    object_buffer.push(new_array);
                    state
                }
                b']' => {
                    if this_object_type == PDFComplexObject::Array {
                        return Ok((make_array_from_object_buffer(object_buffer)?, reader));
                    } else {
                        return Err(ErrorKind::ParsingError(format!(
                            "Invalid terminator for {:?} at {}: ]\ncontext: {}",
                            this_object_type, reader.position() - 1, String::from_utf8_lossy(reader.peek_current_line())
                        )))?;
                    }
                }
                b'<' if reader.peek_ahead_n(1) == &[b'<'] => {
                    if this_object_type == PDFComplexObject::Unknown {
                        this_object_type = PDFComplexObject::Dict;
                        reader.seek(SeekFrom::Current(1)).unwrap();
                    } else {
                        let pos = reader.position() - 1;
                        //println!("Recursive call in dict: {}", String::from_utf8_lossy(reader.peek_current_line()));
                        let (new_dict, returned_reader) = parse_object_at(reader, pos, weak_ref)?;
                        reader = returned_reader;
                        object_buffer.push(new_dict);
                    };
                    state
                }
                b'<' => ParserState::HexString,
                b'>' if reader.peek_ahead_n(1) == &[b'>'] => {
                    if this_object_type == PDFComplexObject::Dict {
                        reader.seek(SeekFrom::Current(1)).unwrap();
                        return Ok((make_dict_from_object_buffer(object_buffer)?, reader));
                    } else {
                        error!("-------Dictionary ended but I'm a {:?}", this_object_type);
                        error!("Buffer: {:#?}", object_buffer);
                        return Err(ErrorKind::ParsingError(format!(
                            "Invalid terminator for {:?} at {}: >>\ncontext: {}",
                            this_object_type, reader.position(), String::from_utf8_lossy(reader.peek_current_line())
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
                        let e = ErrorKind::ParsingError(format!(
                            "Could not parse reference to object at {}\ncontext: {}",
                            reader.position() - 1, String::from_utf8_lossy(reader.peek_current_line())
                        ));
                        error!("object buffer: {:#?}\nerror: {:?}", object_buffer, e);
                        Err(e)?
                    };
                    let new_object = PdfObject::new_reference(
                        <u32>::try_from(
                            object_buffer[object_buffer_length - 2]
                            .try_into_int()
                            .unwrap()
                        ).map_err(|_e| ErrorKind::ParsingError("Invalid id".to_string()))?,
                        <u32>::try_from(
                            object_buffer[object_buffer_length - 1]
                            .try_into_int()
                            .unwrap()
                        ).map_err(|_e| ErrorKind::ParsingError("Invalid gen".to_string()))?,
                        Weak::clone(weak_ref),
                    );

                    // Remove objects that went into the ObjectId, then push the ObjectId
                    object_buffer.truncate(object_buffer_length - 2);
                    object_buffer.push(new_object);
                    state
                }
                b's' | b'e' | b'o' | b'n' | b't' | b'f' => {
                    char_buffer.push(c);
                    ParserState::Keyword
                }
                b'0'..=b'9' | b'+' | b'-' => {
                    // These digits indicate the start of a number, so step back to reparse them in that state
                    reader.seek(SeekFrom::Current(-1)).unwrap();
                    ParserState::Number
                }
                _ if is_whitespace(c) => state,
                _ => {
                    return Err(ErrorKind::ParsingError(format!(
                        "Invalid character at {}: {}\ncontext: {}",
                        reader.position() - 1, c as char, String::from_utf8_lossy(reader.peek_current_line())
                    )))?
                }
            },
            ParserState::HexString => match c {
                b'>' => {
                    object_buffer.push(flush_buffer_to_object(&state, &mut char_buffer)?);
                    ParserState::Neutral
                }
                b'0'..=b'9' | b'A'..=b'F' | b'a'..=b'f' => {
                    // TODO: Could add verification that a consistent case is used, but 
                    char_buffer.push(c);
                    state
                }
                _ if is_whitespace(c) => state,
                _ => {
                    return Err(ErrorKind::ParsingError(format!(
                        "invalid character in hexstring at {}: {}\ncontext: {}",
                        reader.position() - 1, c as char, String::from_utf8_lossy(reader.peek_current_line())
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
                b'\\' => match reader.read_n(1) {
                    &[15] => { // Skip carriage return
                        if reader.peek_ahead_n(1) == &[12] { // Skip linefeed too
                            reader.seek(SeekFrom::Current(1)).unwrap();
                        }; 
                        state
                    }
                    &[12] => state, // Escape naked LF
                    &[b'\\'] => {
                        char_buffer.push(b'\\');
                        state
                    }
                    &[b'('] => {
                        char_buffer.push(b'(');
                        state
                    }
                    &[b')'] => {
                        char_buffer.push(b')');
                        state
                    }
                    &[d@ b'0'..=b'7'] => {
                        // Parse up to three digits as octal
                        let mut code = d - b'0';
                        let peek_next_digits = reader.peek_ahead_n(2);
                        debug_assert!(peek_next_digits.len() < 3);
                        if peek_next_digits.len() > 0 && is_octal(peek_next_digits[0]) {
                            code = code * 8 + (peek_next_digits[0] - b'0');
                        };
                        if peek_next_digits.len() == 2 && is_octal(peek_next_digits[1]) {
                            code = code * 8 + (peek_next_digits[1] - b'0');
                            reader.seek(SeekFrom::Current(2)).unwrap();
                        } else { reader.seek(SeekFrom::Current(1)).unwrap(); };
                        char_buffer.push(code);
                        state
                    }
                    _ => state, // Other escaped characters do not require special treatment, so we ignore the escape
                                // character
                }
                _ => {
                    char_buffer.push(c);
                    state
                }
            }
            ParserState::Name => {
                if c != b'%' && (is_whitespace(c) || is_delimiter(c)) {
                    object_buffer.push(flush_buffer_to_object(&state, &mut char_buffer)?);
                    reader.seek(SeekFrom::Current(-1)).unwrap(); // Need to parse delimiter character on next iteration
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
                        Err(ErrorKind::ParsingError(
                            format!("Two decimal points in number.  Context: {:?}",
                                   String::from_utf8_lossy(reader.peek_current_line()))
                        ))?
                    };
                    char_buffer.push(c);
                    state
                }
                _ if is_whitespace(c) || is_delimiter(c) => {
                    object_buffer.push(flush_buffer_to_object(&state, &mut char_buffer)?);
                    reader.seek(SeekFrom::Current(-1)).unwrap(); // Need to parse delimiter character on next iteration
                    ParserState::Neutral
                }
                _ => {
                    return Err(ErrorKind::ParsingError(
                        format!(
                        "invalid character in number at {}: {}\nContext: {:?}",
                        reader.position(), c as char, String::from_utf8_lossy(reader.peek_current_line())
                    )))?
                }
            }
            ParserState::Comment => {
                if is_eol(c) {
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
                        Err(ErrorKind::ParsingError(format!(
                            "invalid character in keyword at {}: {}\nContext: {}",
                            reader.position() - 1, c as char, String::from_utf8_lossy(reader.peek_current_line())
                        )))?;
                    };
                    let s = String::from_utf8_lossy(&char_buffer);
                    let this_keyword = match &s[..] {
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
                    char_buffer.clear();
                    match this_keyword {
                        PDFKeyword::EndObj => {
                            if this_object_type == PDFComplexObject::IndirectObj {
                                return Ok((make_object_from_object_buffer(object_buffer)?, reader));
                            } else {
                                return Err(ErrorKind::ParsingError(format!(
                                    "Encountered endobj outside indirect object at {}\nContext: {}",
                                    reader.position() - 1, String::from_utf8_lossy(reader.peek_current_line())
                                )))?;
                            };
                        }
                        PDFKeyword::Stream => {
                            return Ok((make_stream_object(object_buffer, &mut reader)?, reader))
                        }
                        PDFKeyword::Obj if this_object_type != PDFComplexObject::Unknown => {
                            Err(ErrorKind::ParsingError(format!(
                                "Encountered nested obj declaration at {}\nContext: {}",
                                reader.position() - 1, String::from_utf8_lossy(reader.peek_current_line())
                            )))?
                        }
                        PDFKeyword::Obj => {
                            this_object_type = PDFComplexObject::IndirectObj;
                            reader.seek(SeekFrom::Current(-1)).unwrap();
                            ParserState::Neutral
                        }
                        PDFKeyword::True => {
                            object_buffer.push(PdfObject::new_boolean(true));
                            reader.seek(SeekFrom::Current(-1)).unwrap();
                            ParserState::Neutral
                        },
                        PDFKeyword::Null => {
                            object_buffer.push(PdfObject::Actual(Null));
                            reader.seek(SeekFrom::Current(-1)).unwrap();
                            ParserState::Neutral
                        }
                        _ => {
                            Err(ErrorKind::ParsingError(format!(
                                "Unrecognized keyword at {}: {:?}",
                                reader.position() - 1, this_keyword
                            )))?
                        }
                    }
                    
                } else {
                    char_buffer.push(c);
                    state
                }
            }
        }
    }
}

fn make_stream_object(mut object_buffer: Vec<PdfObject>,reader: &mut PdfFileReader) -> Result<PdfObject> {
    if object_buffer.len() != 3 {
        Err(ErrorKind::ParsingError(format!(
            "Expected stream at {} to be preceded by a sole dictionary\nContext: {}",
            reader.position() - 1, String::from_utf8_lossy(reader.peek_current_line())
        )))?;
    };
    let stream_dict = object_buffer
        .pop()
        .unwrap()
        .try_into_map()
        .chain_err(|| {
            ErrorKind::ParsingError(format!(
                "Stream at {} preceded by a non-dictionary object",
                reader.position() - 1
            ))
        })?;

    println!("{:?}", reader.peek_current_line());
    reader.seek(SeekFrom::Current(-3));
    reader.step_to_start_of_next_line();
    println!("beginning read at position {}", reader.position());
    
    trace!("Stream dict: {:#?}", stream_dict);
    let id_number = object_buffer[0]
        .try_into_int()
        .chain_err(|| ErrorKind::ParsingError("Invalid object number".to_string()))?;
    let gen_number = object_buffer[0]
        .try_into_int()
        .chain_err(|| ErrorKind::ParsingError("Invalid gen number".to_string()))?;
    let binary_length = stream_dict
        .get("Length")
        .ok_or(ErrorKind::ParsingError(format!(
            "No Length value for stream {} {}",
            id_number,
            gen_number
        )))?
        .try_into_int()
        .chain_err(|| ErrorKind::ParsingError("Invalid gen number".to_string()))?
        as usize;

    let binary_data = Vec::from(reader.read_n(binary_length));
    if binary_data.len() != binary_length {
        Err(ParsingError(format!("Encountered EOF before reading {} bytes", binary_length)))?
    };
    println!("{:#?}", stream_dict);
    #[cfg(debug)]
    {
        let start_index = reader.position();
        let current_line = reader.read_current_line();
        assert!(current_line.len() == 9);
        assert_eq!(
            String::from_utf8_lossy(current_line[..9]), "endstream"
        );
        reader.seek(SeekFrom::Start(start_index)).unwrap();
    }
    //Step past endstream declaration
    reader.step_to_start_of_next_line();

    Ok(decode::decode_stream(
        Rc::try_unwrap(stream_dict).expect("Could not unwrap Rc in make_stream_object call to decode_stream"),
        &binary_data
    )?)
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


//TODO: Remove pub fields
#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone)]
pub struct ObjectId(pub u32, pub u32);

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

fn flush_buffer_to_object(state: &ParserState, buffer: &mut Vec<u8>) -> Result<PdfObject> {
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
            PdfObject::new_hex_string(buffer.clone() as Vec<u8>)
        }
        ParserState::CharString(0) => {
            PdfObject::new_char_string(String::from_utf8_lossy(buffer).to_owned())
        }
        ParserState::CharString(_c) => {
            Err(ErrorKind::ParsingError(format!("String contains unclosed parentheses: {:?}", buffer)))?
        }
        ParserState::Name => PdfObject::new_name(str::from_utf8(buffer)
                .chain_err(|| ErrorKind::ParsingError(format!("Name contains invalid UTF-8: {:?}", buffer)))?),
        ParserState::Number => {
            if buffer.contains(&b'.') {
                PdfObject::new_number_float(
                    str::from_utf8(buffer)
                        .chain_err(|| ErrorKind::ParsingError(format!("Number contains invalid UTF-8: {:?}", buffer)))?
                        .parse::<f32>()?
                )
            } else {
                PdfObject::new_number_int(
                    str::from_utf8(buffer)
                        .chain_err(|| ErrorKind::ParsingError(format!("Number contains invalid UTF-8: {:?}", buffer)))?
                        .parse::<i32>()?
                )
            }
        }
        ParserState::Comment => PdfObject::new_comment(str::from_utf8(buffer)
                .chain_err(|| ErrorKind::ParsingError(format!("Comment contains invalid UTF-8: {:?}", buffer)))?),
        ParserState::Keyword => {panic!("Entered Keyword match arm in flush_buffer_to_object--keywords expected to be
                                         handled by parse_object")}
    };
    buffer.clear();
    return Ok(new_obj);
}

fn make_array_from_object_buffer(object_buffer: Vec<PdfObject>) -> Result<PdfObject> {
    Ok(PdfObject::new_array(Rc::new(object_buffer.into_iter().map(|obj| Rc::new(obj)).collect())))
}

fn make_dict_from_object_buffer(object_buffer: Vec<PdfObject>) -> Result<PdfObject> {
    let mut dict = HashMap::new();
    let mut object_it = object_buffer.into_iter();
    loop {
        let key = match object_it.next() {
            None =>  return Ok(PdfObject::new_dictionary(Rc::new(dict))),
            Some(obj) => obj
        };
        if !key.is_name() {
            Err(ErrorKind::ParsingError(format!("Dictionary key ({:?}) was not a Name", key)))?
        };

        let value = match object_it.next() {
            None => Err(ErrorKind::ParsingError(format!("No object for key: {:?}", key)))?,
            Some(obj) => obj
        };
        dict.insert(key.try_into_string().unwrap().to_string(), Rc::new(value));
    }
}

fn make_object_from_object_buffer(mut object_buffer: Vec<PdfObject>) -> Result<PdfObject> {
    if object_buffer.len() != 3 {
        Err(ErrorKind::ParsingError(format!("Object tags contained {} objects", object_buffer.len())))?
    };
    if !object_buffer[0].is_int()
        || !object_buffer[1].is_int() {
        Err(ErrorKind::ParsingError("Invalid indirect object format".to_string()))?
    };
    return Ok(object_buffer.pop().unwrap());
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
            let mut pdf = Parser::create_pdf_from_file(path).unwrap();
            results.push(add_all_objects(&mut pdf));
        }
        let results: Vec<_> = results
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
            let mut pdf = Parser::create_pdf_from_file(path).unwrap();
            match add_all_objects(&mut pdf) {
                Ok(_) => println!("Ok!"),
                Err(_) => println!("Err!")
            };
        }
    }

    fn add_all_objects(pdf: &mut Parser) -> Result<()> {
        let objects_to_add: Vec<ObjectId> = pdf.object_map.get_object_list();
        for object_number in objects_to_add {
            println!("Retrieving Obj #{}:", object_number);
            match pdf.retrieve_object_by_ref(object_number) {
                Ok(obj) => {},// println!("Obj #{} successfully retrieved: {}", object_number, obj); },
                Err(e) => {
                    //println!("**Obj #{} ERROR**: {}", object_number, e);
                    Err(e.chain_err(|| ErrorKind::TestingError(format!("**Obj #{} ERROR**", object_number))))?;
                }
            };
        }
        Ok(())
    }
}
