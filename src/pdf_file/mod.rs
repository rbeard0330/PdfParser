mod util;
mod decode;
mod pdf_objects;

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::str;
use std::rc::Rc;

use PDFObj::*;
use util::*;


type PDFResult = Result<SharedObject, PDFError>;
type SharedObject = Rc<PDFObj>;

pub struct PdfFileHandler {
    bytes: Vec<u8>,
    pub version: Option<PDFVersion>,
    trailer: Option<PDFTrailer>,
    index_map: HashMap<ObjectID, usize>,
    object_map: HashMap<ObjectID, Rc<PDFObj>>,
}

impl PdfFileHandler {

    pub fn create_pdf_from_file(path: &str) -> Result<Self, PDFError> {
        let raw_pdf = fs::read(path);
        let bytes = match raw_pdf {
            Ok(data) => data,
            Err(e) => return Err(PDFError{message:format!("File read error: {:?}", e),
                            function: "create_pdf_from_file"})
        };
        let mut pdf = PdfFileHandler{
            bytes, version: None, trailer: None,
            index_map: HashMap::new(), object_map: HashMap::new(),
        };
        pdf.set_version()?;
        let trailer_index = pdf.find_trailer_index()?;
        //println!("trailer starts at: {:?}", trailer_index);
        pdf.trailer = Some(pdf.process_trailer(trailer_index)?);
        match pdf.process_xref_table() {
            None => {},
            Some(e) => return Err(e)
        };
        Ok(pdf)
    }

    pub fn get_root(&mut self) -> Result<Rc<PDFObj>, PDFError> {
        let trailer_dict = match &self.trailer.as_ref().expect("Parse trailer first!").trailer_dict {
            Dictionary(trailer_dict) => trailer_dict,
            _ => return Err(PDFError{ message: "Error processing trailer".to_string(), function: "build_page_tree"})
        };
        
        let catalog_ref = match trailer_dict.get("Root") {
            Some(val) => {
                match **val {
                    ObjectRef(obj_ref) => obj_ref,
                    _ => return Err(PDFError{ message: "/Root entry not ref".to_string(), function: "build_page_tree"})
                }
            },
            _ => return Err(PDFError{ message: "No /Root entry in trailer".to_string(), function: "build_page_tree"})
        };
        drop(trailer_dict);
        self.get_object(&catalog_ref)
    }

    pub fn get_object_once(&mut self, id: &ObjectID) -> Result<Rc<PDFObj>, PDFError> {
        if !self.object_map.contains_key(id) {
            let index = match self.index_map.get(id) {
                None => return Err(PDFError{ message: format!("object {} not found", id), function: "get_object"}),
                Some(i) => i
            };
            let (new_obj, _) = self.parse_object(*index)?;
            self.object_map.insert(*id, Rc::new(new_obj));
        };
        let mut object_to_return = match self.object_map.get(id) {
            Some(obj) => Rc::clone(obj),
            _ => return Err(PDFError {
                message: format!("Expected {} to be an indirect object", id), function: "get_object"
            })
        };

        if let Stream(ref map, ref bytes) = *object_to_return {
            object_to_return = Rc::clone(&(self.decode_stream(map, bytes)?));
        };
        Ok(Rc::clone(&object_to_return))
    }

    pub fn get_object(&mut self, id: &ObjectID) -> Result<Rc<PDFObj>, PDFError> {
        let obj = self.get_object_once(id)?;
        match *obj {
            ObjectRef(nested_id) => self.get_object(&nested_id),
            _ => Ok(obj)
        }
    }

    pub fn get_from_map(&mut self, map: &HashMap<String, Rc<PDFObj>>, key: &str) -> Result<Rc<PDFObj>, PDFError> {
        println!("  fetching {} from\n  {:?}", key, map);
        match map.get(key) {
            None => {
                println!("None found!");
                Err(PDFError{message: format!("No such key: {}", key), function: "get_from_map"})
            },
            Some(obj) => {
                //println!("  found {:?}", obj);
                    match **obj {
                    ObjectRef(id) => self.get_object(&id),
                    _ => Ok(Rc::clone(obj))
                }
            }
        }
    }

    fn decode_stream(&mut self, map: &HashMap<String, Rc<PDFObj>>, bytes: &Vec<u8>) -> Result<Rc<PDFObj>, PDFError> {
        //Check size
        let expected_byte_length = self.get_from_map(map, "Length")?
                                       .cast_to_int()
                                       .expect("Length key not a number!") as usize;
        assert_eq!(bytes.len(), expected_byte_length);

        // Classify stream
        let type_and_subtype = (self.get_from_map(&map, "Type"), self.get_from_map(&map, "Subtype"));
        let stream_type = determine_stream_type(type_and_subtype);
        if let StreamType::Image = stream_type { return Ok(Rc::new(DecodedStream{stream_type: StreamType::Image})) };
    
        //Extract filters
        let filters = self.get_from_map(&map, "Filter").unwrap_or(Rc::new(Array(Vec::new())));
        let params = self.get_from_map(&map, "DecodeParms").ok();
        let filter_string_array = match &*filters {
            &Name(ref s) => vec!(Ok(s.clone())),
            Array(ref v) => v.iter().map(|item| match **item {
                PDFObj::Name(ref s) => Ok(s.clone()),
                _ => Err(PDFError{ message: format!("Non-name item in Filter array: {:?}", item),
                                            function: "decode_stream"})
            }).collect(),
            item => return Err(PDFError{ message: format!("Non-name item in Filter array: {:?}", item),
                                        function: "decode_stream"})
        }.into_iter().collect::<Result<Vec<String>, _>>()?;
        let filter_array = filter_string_array
                            .into_iter()
                            .enumerate()
                            // Collect matching params without throwing error if no filters need params
                            .map(|(index, s)| decode::filter_from_string_and_params(
                                &s, params.as_ref().map(
                                            |arr| match **arr {
                                                Array(_) => arr.index(index).unwrap(),
                                                _ => Rc::clone(arr)
                                            })))
                            .collect::<Result<Vec<decode::Filter>, _>>()?;
        let object = filter_array.into_iter().fold(Ok(bytes.clone()), |data, filter| filter.apply(data))?;
    
        Ok(Rc::new(DecodedStream{stream_type: StreamType::Image}))
    }

    fn set_version(&mut self) -> Result<(), PDFError> {
        let intro = String::from_utf8(
                        self.bytes[..12].iter()
                        .map(|c| *c)
                        .take_while(|c| !is_EOL(*c))
                        .collect());
        let intro = match intro {
            Ok(s) if s.contains("%PDF-") => s,
            _ => return Err(PDFError{ message: format!("Could not find version number in {:?}", intro), function:
                                    "set_version"})
        };
        self.version = match intro.splitn(2, "%PDF-")
                            .last().unwrap()
                            .split_at(3).0
                            .parse::<f32>() {
            Ok(1.0) => Some(PDFVersion::V1_0),
            Ok(1.1) => Some(PDFVersion::V1_1),
            Ok(1.2) => Some(PDFVersion::V1_2),
            Ok(1.3) => Some(PDFVersion::V1_3),
            Ok(1.4) => Some(PDFVersion::V1_4),
            Ok(1.5) => Some(PDFVersion::V1_5),
            Ok(1.6) => Some(PDFVersion::V1_6),
            Ok(1.7) => Some(PDFVersion::V1_7),
            Ok(2.0) => Some(PDFVersion::V2_0),
            Ok(x) if x > 2.0 => return Err(PDFError{ message: format!("Unsupported PDF version: {}", x),
                                                    function: "set_version"}),
            _ => return Err(PDFError{ message: "Could not find PDF version".to_string(), function: "set_version"})
        };
        Ok(())
    }

    fn find_trailer_index(&self) -> Result<usize, PDFError> {
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
                _ => 0
            };
    
            if current_index + state <= 6 {
                return Result::Err(PDFError {
                    message: "Could not find trailer".to_string(), function: "find_trailer_index"
                })
            };
        }
        Result::Ok(current_index)
    }

    fn process_trailer(&mut self, start_index: usize) -> Result<PDFTrailer, PDFError> {
        assert_eq!(&(String::from_utf8(Vec::from(&self.bytes[start_index..start_index + 7])).unwrap()), "trailer");
        let (trailer_dict, next_index) = self.parse_object(start_index + 7)?;
        let trailer_string = String::from_utf8(self.bytes[(next_index + 1)..].to_vec())
                            .expect("Could not convert trailer to string!");
        let mut trailer_lines = trailer_string
                                .lines()
                                .filter(|l| !l.trim().is_empty());
        let first_line = trailer_lines.next().expect("No line after trailer dict!");
        //println!("{}", trailer_string);
        if first_line != "startxref" {
            return Err(PDFError {
                message: format!("startxref keyword not found at {}", next_index), function: "process_trailer"
            })
        };
        let second_line = trailer_lines.next().expect("No xref location in trailer");
        let xref_index = second_line.trim().parse().expect("Invalid xref index in trailer");
        let third_line = trailer_lines.next().expect("Missing file terminator!");
        assert_eq!(third_line, "%%EOF");
        assert_eq!(trailer_lines.next(), None);
        return Ok(PDFTrailer {start_index, trailer_dict, xref_index})
    }

    fn process_xref_table(&mut self) -> Option<PDFError> {
        let trailer = self.trailer.as_ref().expect("Parse trailer before parsing xref table!");
        let start_index = trailer.xref_index;
        let end_index = trailer.start_index - 1;
        let table = String::from_utf8(self.bytes[start_index..end_index].to_vec()).expect("Invalid xref table");
        //println!("{}", table);
        let mut line_iter = table.lines();
        let mut obj_number = 0;
        assert_eq!(line_iter.next().unwrap(), "xref");
        loop {
            let line = match line_iter.next() {
                Some(s) => s,
                None => return None
            };
            //println!("{:?}", line);
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() == 3 {
                if parts[2] == "f" { obj_number += 1} else {
                    self.index_map.insert(
                        ObjectID(obj_number, parts[1].parse().expect("Could not parse gen number")),
                        parts[0].parse().expect("Could not parse offset")
                    );
                    obj_number += 1;
                }
             } else if parts.len() == 2 {
                    obj_number = parts[0].parse().expect("Could not parse object number");
                } else {
                    println!("{:?}", parts);
                    return Some(PDFError{
                        message: format!("Invalid line in xref table: {:?}", parts), function: "process_xref_table"
                    })
            }
        };
    }
    
    fn parse_object(&mut self, start_index: usize) -> Result<(PDFObj, usize), PDFError> {
        let mut state = ParserState::Neutral;
        let mut index = start_index;
        let mut this_object_type = PDFComplexObject::Unknown;
        let length = self.bytes.len();
        if index > length { return Err(PDFError{
            message: format!("index {} out of range", index), function: "parse_object"
        })};
        let mut char_buffer = Vec::new();
        let mut object_buffer = Vec::new();
        loop {
            if index > length {
                return Err(PDFError{message: "end of file while parsing object".to_string(), function: "parse_object" })
            };
            let c = self.bytes[index];
            state = match state {
                ParserState::Neutral => match c {
                    b'[' if this_object_type == PDFComplexObject::Unknown => {
                        this_object_type = PDFComplexObject::Array;
                        state
                    },
                    b'[' => {
                        let (new_array, end_index) = self.parse_object(index)?;
                        index = end_index;
                        object_buffer.push(new_array);
                        state
                    },
                    b']' => {
                        if this_object_type == PDFComplexObject::Array {
                            return make_array_from_object_buffer(object_buffer, index)
                        } else {
                            return Err(PDFError {
                                message: format!("Invalid terminator for {:?} at {}: ]", this_object_type, index),
                                function: "parse_object"
                            }) 
                        }
                    },
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
                    },
                    b'<' => { ParserState::HexString },
                    b'>' if (peek_ahead_by_n(&self.bytes, index, 1) == Some(b'>')) => {
                        if this_object_type == PDFComplexObject::Dict {
                            //println!("Dictionary ended at {}", index + 1);
                            return make_dict_from_object_buffer(object_buffer, index + 1)
                        } else {
                            println!("-------Dictionary ended but I'm a {:?}", this_object_type);
                            println!("Buffer: {:#?}", object_buffer);
                            return Err(PDFError {
                                message: format!("Invalid terminator for {:?} at {}: >>", this_object_type, index),
                                function: "parse_object"
                            }) 
                        }
                    },
                    b'(' => { ParserState::CharString(0) },
                    b'/' => { ParserState::Name },
                    b'R' => {
                        let object_buffer_length = object_buffer.len();
                        if object_buffer_length <= 1 {
                            return Err(PDFError{
                                message: format!("Could not parse reference to object at {}", index),
                                function: "parse_object" })
                        };
                        let new_object = match object_buffer[(object_buffer_length - 2)..object_buffer_length] {
                            [PDFObj::NumberInt(n1), PDFObj::NumberInt(n2)] if n1 >= 0 && n2 >= 0 =>
                                PDFObj::ObjectRef(ObjectID(n1 as u32, n2 as u16)),
                            _ => return Err(PDFError{
                                message: format!("Could not parse reference to object at {}", index),
                                function: "parse_object" })
                        };
                        object_buffer.truncate(object_buffer_length - 2);
                        object_buffer.push(new_object);
                        state
                    },
                    b's' | b'e' | b'o' | b'n' | b't' | b'f' => {
                        char_buffer.push(c);
                        ParserState::Keyword
                    },
                    b'0'..= b'9' | b'+' | b'-' => { index -= 1; ParserState::Number },
                    _ if is_whitespace(c) => state,
                    _ => return Err(PDFError {
                        message:format!("Invalid character at {}: {}", index, c as char), function: "parse_object"
                    }) 
                },
                ParserState::HexString => match c {
                    b'>' => {
                        object_buffer.push( flush_buffer_to_object(&state, &mut char_buffer)? );
                        ParserState::Neutral
                    },
                    b'0'..=b'9' | b'A'..=b'F' => {
                        char_buffer.push(c);
                        state
                    },
                    _ if is_whitespace(c) => state,
                    _ => return Err(PDFError{
                        message: format!("invalid character in hexstring at {}: {}", index, c as char),
                        function: "parse_object"
                    })
                },
                ParserState::CharString(depth) => match c {
                    b')' if depth == 0 => {
                        //println!("Making a string at {}", index);
                        object_buffer.push( flush_buffer_to_object(&state, &mut char_buffer)? );
                        ParserState::Neutral
                    },
                    b')' if depth > 0 => ParserState::CharString(depth - 1),
                    b'(' => ParserState::CharString(depth + 1),
                    b'\\' if index + 1 < length => {
                        match self.bytes[index + 1] {
                            15 => {
                                index += 1; // Skip carriage return
                                if index + 1 < length && self.bytes[index + 1] == 12 { index += 1}; // Skip linefeed too
                                state
                            },
                            12 => {index += 1; state}, // Escape naked LF
                            b'\\' => {
                                index += 1;
                                char_buffer.push(b'\\');
                                state
                            },
                            b'(' => {
                                index += 1;
                                char_buffer.push(b'(');
                                state
                            },
                            b')' => {
                                index += 1;
                                char_buffer.push(b')');
                                state
                            },
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
                            },
                            _ => state // Other escaped characters do not require special treatment
                        }
                    },
                    _ => { char_buffer.push(c); state}
                },
                ParserState::Name => {
                    if c != b'%' && (is_whitespace(c) || is_delimiter(c)) {
                        object_buffer.push( flush_buffer_to_object(&state, &mut char_buffer)? );
                        index -= 1; // Need to parse delimiter character on next iteration
                        ParserState::Neutral
                    } else {
                        char_buffer.push(c);
                        state
                    }
                },
                ParserState::Number => match c {
                    b'0'..=b'9' => {
                        char_buffer.push(c);
                        state
                    },
                    b'-' | b'+' if char_buffer.len() == 0 => {
                        char_buffer.push(c);
                        state
                    },
                    b'.' => {
                        if char_buffer.contains(&b'.') {
                            return Err(PDFError{
                                message: "two decimal points in number".to_string(), function: "parse_object"
                            })};
                        char_buffer.push(c);
                        state
                    },
                    _ if is_whitespace(c) || is_delimiter(c) => {
                        object_buffer.push( flush_buffer_to_object(&state, &mut char_buffer)? );
                        index -= 1; // Need to parse delimiter character on next iteration
                        ParserState::Neutral
                    },
                    _ => return Err(PDFError {
                        message: format!("invalid character in number at {}: {}", index, c as char),
                        function: "parse_object"
                    }) 
                },
                ParserState::Comment => {
                    if is_EOL(c) {
                        object_buffer.push( flush_buffer_to_object(&state, &mut char_buffer)? );
                        ParserState::Neutral
                    } else {
                        char_buffer.push(c);
                        state
                    }
                },
                ParserState::Keyword => {
                    if !is_body_keyword_letter(c) {
                        if !(is_delimiter(c) || is_whitespace(c)) {
                            return Err(PDFError {
                                message: format!("invalid character in keyword at {}: {}", index, c as char),
                                function: "parse_object"
                            }) 
                        };
                        let this_keyword = flush_buffer_to_object(&state, &mut char_buffer)?;
                        match this_keyword {
                            PDFObj::Keyword(PDFKeyword::EndObj) => {
                                if this_object_type == PDFComplexObject::IndirectObj {
                                    return make_object_from_object_buffer(object_buffer, index)
                                } else {
                                    return Err(PDFError{
                                        message: format!("Encountered endobj outside indirect object at {}", index),
                                        function: "parse_object"
                                    })
                                };
                            },
                            PDFObj::Keyword(PDFKeyword::Stream) => {
                                return self.make_stream_object(object_buffer, index)
                            },
                            PDFObj::Keyword(PDFKeyword::Obj) if this_object_type != PDFComplexObject::Unknown => 
                                return Err(
                                    PDFError{ message: format!("Encountered nested obj declaration at {}", index),
                                    function: "parse_object"
                                }),
                            PDFObj::Keyword(PDFKeyword::Obj) => {
                                this_object_type = PDFComplexObject::IndirectObj;
                                index -= 1;
                                ParserState::Neutral
                            },
                            PDFObj::Keyword(_) => {
                                object_buffer.push(this_keyword);
                                index -= 1;
                                ParserState::Neutral
                            },
                            _ =>{
                                return Err(PDFError{
                                    message: format!("Unrecognized keyword at {}: {:?}", index, this_keyword),
                                    function: "parse_object"
                                })
                            }
                        }
                    } else {
                        char_buffer.push(c);
                        state
                    }
                }
            };
            index += 1;
        };
    }

    fn make_stream_object(&mut self, mut object_buffer: Vec<PDFObj>, index: usize)
            -> Result<(PDFObj, usize), PDFError> {
        if object_buffer.len() != 3 {
            return Err(PDFError{
                message: format!("Expected stream at {} to be preceded by a sole dictionary", index),
                function: "make_stream_object"
        })};
        let binary_start_index = match self.bytes[index] {
            b'\n' => index + 1,
            b'\r' => {
                if let Some(b'\n') = peek_ahead_by_n(&self.bytes, index, 1) {
                    index + 2} else {
                        return Err(PDFError{
                            message: format!("Stream tag not followed by appropriate spacing at {}", index),
                            function: "make_stream_object"
                        })
                    }
                },
            _ => return Err(PDFError{
                message: format!("Stream tag not followed by appropriate spacing at {}", index),
                function: "make_stream_object"
            })};
        let wrapped_stream_dict = match object_buffer.pop().unwrap() {
            PDFObj::Dictionary(hash_map) => hash_map,
            _ => return Err(PDFError{
                message: format!("Stream at {} preceded by a non-dictionary object", index),
                function: "make_stream_object" })};
        let mut stream_dict = HashMap::new();
        for (key, value) in wrapped_stream_dict {
            stream_dict.insert(key, value);
        }
        //println!("{:#?}", stream_dict);
        let id_number = match object_buffer[0] {
            PDFObj::NumberInt(i) => i as u32,
            _ => return Err(PDFError{ message: "invalid object number".to_string(), function: "make_stream_object"})
        };
        let gen_number = match object_buffer[1] {
            PDFObj::NumberInt(i) => i as u16,
            _ => return Err(PDFError{ message: "invalid generation number".to_string(), function: "make_stream_object"})
        };
        let binary_length = match stream_dict.get("Length") {
            Some(obj) => match **obj {

            
                PDFObj::NumberInt(binary_length) => binary_length as usize,
                PDFObj::ObjectRef(id) => match &*(self.get_object(&id)?) {
                    &PDFObj::NumberInt(binary_length) => binary_length as usize,
                    obj @ _ => {
                        println!("{:?}", obj);
                        return Err(PDFError{ message: "Could not find valid /Length key by ref".to_string(),
                                function: "make_stream_object" })
                    }
                },
                _ =>return Err(PDFError{ message: "Could not find valid /Length key".to_string(),
                function: "make_stream_object" })
            },
            _ => return Err(PDFError{ message: "Could not find valid /Length key".to_string(),
                            function: "make_stream_object" })
        };
        // TODO: Confirm endstream included
        if binary_start_index + binary_length >= self.bytes.len() {
            return Err(PDFError{ message: format!("Reported binary content length ({}) too long", binary_length),
                        function: "make_stream_object"})
        };
        Ok((PDFObj::Stream(stream_dict,
                          Vec::from(&self.bytes[binary_start_index..(binary_start_index + binary_length)])),
            binary_start_index + binary_length + 9))
    }
    
}

fn determine_stream_type(tup: (PDFResult, PDFResult)) -> StreamType {
    use StreamType::*;
    if let Ok(result) = tup.1 {
        match *result {
            Name(ref s) if s == "Image" => return Image,
            _ => {}
        };
    }
    return Unknown
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



#[derive(Debug, PartialEq, Clone)]
pub enum PDFObj {
    Boolean(bool),
    NumberInt(i32),
    NumberFloat(f32),
    Name(String),
    CharString(String),
    HexString(Vec<u8>),
    Array(Vec<Rc<PDFObj>>),
    Dictionary(HashMap<String, Rc<PDFObj>>),
    Stream(HashMap<String, Rc<PDFObj>>, Vec<u8>),
    Comment(String),
    Keyword(PDFKeyword),
    ObjectRef(ObjectID),
    DecodedStream{stream_type: StreamType}
}

impl PDFObj {
    pub fn get(&self, key: &str) -> Result<Option<&Rc<PDFObj>>, PDFError> {
        match self {
            Dictionary(map) => Ok(map.get(key)),
            _ => Err(PDFError{message: format!("No dictionary in provided type: {}", self), function: "get_dict_ref" })
        }
    }

    pub fn index(&self, index: usize) -> Result<Rc<PDFObj>, PDFError> {
        match self {
            Array(vec) => Ok(Rc::clone(&vec[index])),
            _ => Err(PDFError{message: format!("Attempted to index ({}) a non-Array object: {:?}", index, self),
                              function: "PDFObj.index"})
        }
    }
    
    fn cast_to_int(&self) -> Result<i32, PDFError> {
        match self {
            NumberInt(n) => Ok(*n),
            _ => return Err(PDFError{message: format!("Not an integer: {:?}", self), function: "cast_to_int"})
        }
    }

    pub fn get_as_object_id(&self) -> Option<ObjectID> {
        match self {
            ObjectRef(id) => Some(*id),
            _ => None
        }
    }

    pub fn get_dict_ref(&self) -> Option<&HashMap<String, Rc<PDFObj>>> {
        match self {
            Dictionary(map) => Some(map),
            _ => None
        }
    }
}

impl fmt::Display for PDFObj {
    
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
            Keyword(kw) => write!(f, "Keyword: {:?}", kw),
            ObjectRef(ObjectID(id_number, gen_number)) => 
                write!(f, "Reference to obj with id {} and gen {}", id_number, gen_number),
            DecodedStream{..} => write!(f, "Stream object")
        }.expect("Error in write macro!");
        Ok(())
    }
}

//TODO: Remove pub fields
#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone)]
pub struct ObjectID(pub u32, pub u16);

impl fmt::Display for ObjectID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Object {} {}", self.0, self.1)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct PDFStreamObject {
    object_type: StreamType
}

#[derive(Debug, PartialEq, Clone)]
enum StreamType {
    Content,
    Object,
    XRef,
    Image,
    Unknown
}

#[derive(Debug, PartialEq)]
enum PDFComplexObject {
    Unknown,
    Dict,
    Array,
    IndirectObj
}

#[derive(Debug)]
struct PDFTrailer {
    start_index: usize,
    trailer_dict: PDFObj,
    xref_index: usize
}

pub struct PDFError {
    pub message: String,
    pub function: &'static str
}

impl fmt::Display for PDFError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PDF Processing Error: {} in function {}", self.message, self.function)
    }
}

impl fmt::Debug for PDFError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PDF Processing Error: {} in function {}", self.message, self.function)
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum ParserState {
    Neutral,
    HexString,
    CharString(u8),
    Name,
    Number,
    Comment,
    Keyword
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
    StartXRef
}

fn flush_buffer_to_object (state: &ParserState , buffer: &mut Vec<u8>) -> Result<PDFObj, PDFError> {
    let new_obj = match state {
        ParserState::Neutral => return Err(PDFError {message: "Called flush buffer in Neutral context".to_string(),
                function: "flush_buffer_to_object"}),
        ParserState::HexString => {
            //TODO: ADD PADDING
            for c in buffer.iter() {
                if !is_hex(*c) { return Err(PDFError { message: format!("Invalid character in hex string: {}", c),
                    function: "flush_buffer_to_object"}) };
            };
            PDFObj::HexString(buffer.clone() as Vec<u8>)
        },
        ParserState::CharString(0) => {
            PDFObj::CharString(String::from_utf8_lossy(buffer).into_owned())
        },
        ParserState::CharString(_c) => {
            return Err(PDFError{ message: "Asked to create string with unclosed parentheses".to_string(),
                function: "flush_buffer_to_object" });
        },
        ParserState::Name => PDFObj::Name(str::from_utf8(buffer).unwrap().to_string()),
        ParserState::Number => {
            if buffer.contains(&b'.') {
                PDFObj::NumberFloat(str::from_utf8(buffer).unwrap().parse().unwrap())
            } else {
                PDFObj::NumberInt(str::from_utf8(buffer).unwrap().parse().unwrap())
            }
        },
        ParserState::Comment => PDFObj::Comment(str::from_utf8(buffer).unwrap().to_string()),
        ParserState::Keyword => {
            let s = str::from_utf8(buffer).unwrap();
            match s {
                "obj" => PDFObj::Keyword(PDFKeyword::Obj),
                "endobj" => PDFObj::Keyword(PDFKeyword::EndObj),
                "stream" => PDFObj::Keyword(PDFKeyword::Stream),
                "endstream" => PDFObj::Keyword(PDFKeyword::EndStream),
                "null" => PDFObj::Keyword(PDFKeyword::Null),
                "false" => PDFObj::Keyword(PDFKeyword::False),
                "true" => PDFObj::Keyword(PDFKeyword::True),
                _ => return Err(PDFError{ message: format!("Invalid PDF keyword: {}", s),
                        function: "flush_buffer_to_object"})
            }
        }
    };
    buffer.clear();
    return Ok(new_obj)
}

fn make_array_from_object_buffer(object_buffer: Vec<PDFObj>, end_index: usize) -> Result<(PDFObj, usize), PDFError> {
    let mut new_vec = Vec::new();
    for obj in object_buffer {
        new_vec.push(Rc::new(obj));
    }
    Ok((PDFObj::Array(new_vec), end_index))
}

fn make_dict_from_object_buffer(object_buffer: Vec<PDFObj>, end_index: usize) -> Result<(PDFObj, usize), PDFError> {
    let mut dict = HashMap::new();
    let mut object_it = object_buffer.into_iter();
    loop {
        let key = match object_it.next() {
            None => {
                //println!("completed a dict: {:?}", dict);
                return Ok((PDFObj::Dictionary(dict), end_index))
            },
            Some(PDFObj::Name(s)) => s,
            Some(obj) => return Err(PDFError{ message: format!("Dictionary key ({:?}) was not a Name", obj),
                            function: "make_dict_from_object_buffer"})
        };
        let value = match object_it.next() {
            None => return Err(PDFError{ message: "No object for key".to_string(),
                            function: "make_dict_from_object_buffer" }),
            Some(v) => v
        };
        dict.insert(key, Rc::new(value));
    } 
}

fn make_object_from_object_buffer(mut object_buffer: Vec<PDFObj>, end_index: usize)
        -> Result<(PDFObj, usize), PDFError> {
    if object_buffer.len() != 3 {
        return Err(PDFError{ message: format!("Object tags contained {} objects", object_buffer.len()),
                    function: "make_object_from_object_buffer"})
    };
    let id_number = match object_buffer[0] {
        PDFObj::NumberInt(i) => i as u32,
        _ => return Err(PDFError{ message: "invalid object number".to_string(),
                        function: "make_object_from_object_buffer"})
    };
    let gen_number = match object_buffer[1] {
        PDFObj::NumberInt(i) => i as u16,
        _ => return Err(PDFError{ message: "invalid generation number".to_string(),
                        function: "make_object_from_object_buffer"})
    }; 
    return Ok((object_buffer.pop().unwrap(), end_index))
}



// -----------Utility functions----------------



#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PDFS: [&str; 4] = ["data/simple_pdf.pdf", "data/CCI01212020.pdf",
                                      "data/document.pdf", "data/2018W2.pdf"];


    #[test]
    fn test_sample_pdfs_sensitive() {
        let mut results = Vec::new();
        for path in &TEST_PDFS {
            println!("{}", path);
            let mut pdf = PdfFileHandler::create_pdf_from_file(path).unwrap();
            results.push(add_all_objects(&mut pdf));
        }
        let results: Vec<PDFError> = results.into_iter()
               .filter(|result| result.is_err())
               .map(|err| err.unwrap_err())
               .collect();
        if results.len() > 0 {
            for err in results {
                println!("ERROR: {:#?}", err);
            };
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
        let objects_to_add: Vec<(ObjectID, usize)> = pdf.index_map.iter().map(|(a, b)| (*a, *b)).collect();
        for (object_number, _index) in objects_to_add {
            println!("Retrieving Obj #{}", object_number);
            match pdf.get_object(&object_number) {
                Ok(obj) => {}//println!("Obj #{} successfully retrieved: {}", object_number, obj);},
                Err(e) => {println!("**Obj #{} ERROR**: {:?}", object_number, e); return Err(e)}
            };
        };
        Ok(())
    }

}