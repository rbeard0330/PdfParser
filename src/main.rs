use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::str;

enum PDFVersion {
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


#[derive(Debug)]
#[derive(PartialEq)]
enum PDFObj {
    Boolean(bool),
    NumberInt(i32),
    NumberFloat(f32),
    Name(String),
    CharString(String),
    HexString(Vec<u8>),
    Array(Vec<PDFObj>),
    Dictionary(Box<HashMap<String, PDFObj>>),
    Stream(Box<HashMap<String, PDFObj>>, Vec<u8>),
    Comment(String),
    Keyword(PDFKeyword),
    ObjectRef{id_number: u32, gen_number: u16 },
    IndirectObject{ id_number: u32, gen_number: u16, object:Box<PDFObj> },
}

impl PDFObj {
    fn unwrap(obj: PDFObj) -> Result<PDFObj, PDFError> {
        if let PDFObj::ObjectRef{ id_number, gen_number } = obj {
            return Err(PDFError {message: "Indirect references not implemented", location: 0})
        }
        Ok(obj)

    }
}

impl fmt::Display for PDFObj {
    
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use PDFObj::*;
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
            ObjectRef{id_number, gen_number} => 
                write!(f, "Reference to obj with id {} and gen {}", id_number, gen_number),
            IndirectObject{id_number, gen_number, object} => 
                write!(f, "Object with id {} and gen {}: {}", id_number, gen_number, object)
        };
        Ok(())
    }

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
    trailer_dict: PDFObj,
    xref_index: usize
}

struct PDF {
    //version: PDFVersion,
    trailer: PDFTrailer,
}

struct PDFError {
    message: &'static str,
    location: usize
}


impl fmt::Display for PDFError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PDF Processing Error: {} at {}", self.message, self.location)
    }
}

impl fmt::Debug for PDFError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PDF Processing Error: {} at {}", self.message, self.location)
    }
}

fn main(){
    
    //let raw_pdf = fs::read("data/simple_pdf.pdf").expect("Could not read data!");
    //let raw_pdf = fs::read("data/CCI01212020.pdf").expect("Could not read data!");
    let raw_pdf = fs::read("data/document.pdf").expect("Could not read data!");
    //let raw_pdf = fs::read("data/2018W2.pdf").expect("Could not read data!");
    println!("{}", raw_pdf.len());
    let pdf = parse_pdf(raw_pdf).expect("Processing error");
    

    
}

fn parse_pdf(bytes: Vec<u8>) -> Result<PDF, PDFError> {
    let trailer_index = find_trailer_index(&bytes)?;
    println!("trailer starts at: {:?}", trailer_index);
    let trailer = process_trailer(&bytes, trailer_index)?;
    let xref_table = process_xref_table(&bytes, trailer.xref_index, trailer_index)?;
    for (object_number, index) in xref_table {
        match parse_object(&bytes, index) {
            Ok((obj, _)) => println!("Obj #{}: {}", object_number, obj),
            Err(e) => {
                println!("Obj #{}: {:?}", object_number, e);
                let prev_characters: Vec<char> = bytes[(e.location - 10)..e.location].iter().map(|c| *c as char).collect();
                let next_characters: Vec<char> = bytes[e.location..(e.location + 10)].iter().map(|c| *c as char).collect();
                println!("{:?} | {:?}", prev_characters, next_characters);
            }
        };
    }
    Ok(PDF{ trailer })

}

fn find_trailer_index(bytes: &Vec<u8>) -> Result<usize, PDFError> {
    let mut state: usize = 0;
    let mut current_index = bytes.len() as usize;
    while state < 7 {
        current_index -= 1;
        let c = bytes[current_index] as char;
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
            return Result::Err(PDFError {message: "Could not find trailer", location: current_index})
        };
    }
    Result::Ok(current_index)
}

fn process_trailer(bytes: &Vec<u8>, start_index: usize) -> Result<PDFTrailer, PDFError> {
    assert_eq!(&(String::from_utf8(Vec::from(&bytes[start_index..start_index + 8])).unwrap()), "trailer\n");
    let (trailer_dict, next_index) = parse_object(&bytes, start_index + 7)?;
    let trailer_string = String::from_utf8(bytes[(next_index + 1)..].to_vec())
                        .expect("Could not convert trailer to string!");
    let mut trailer_lines = trailer_string
                            .lines()
                            .filter(|l| !l.trim().is_empty());
    let first_line = trailer_lines.next().expect("No line after trailer dict!");
    //println!("{}", trailer_string);
    if first_line != "startxref" {
        return Err(PDFError {message: "startxref keyword not found", location: next_index})
    };
    let second_line = trailer_lines.next().expect("No xref location in trailer");
    let xref_index = second_line.trim().parse().expect("Invalid xref index in trailer");
    let third_line = trailer_lines.next().expect("Missing file terminator!");
    assert_eq!(third_line, "%%EOF");
    assert_eq!(trailer_lines.next(), None);
    return Ok(PDFTrailer {trailer_dict, xref_index})
}

fn process_xref_table(bytes: &Vec<u8>, start_index:usize, end_index: usize) -> Result<HashMap<u32, usize>, PDFError> {
    let mut table = String::from_utf8(bytes[start_index..end_index].to_vec()).expect("Invalid xref table");
    let mut line_iter = table.lines();
    let mut object_number_to_index = HashMap::new();
    let mut obj_number = 0;
    assert_eq!(line_iter.next().unwrap(), "xref");
    loop {
        let line = match line_iter.next() {
            Some(s) => s,
            None => return Ok(object_number_to_index)
        };
        println!("{:?}", line);
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() == 3 {
            if parts[2] == "f" { obj_number += 1} else {
                object_number_to_index.insert(
                    obj_number,
                    parts[0].parse().expect("Could not parse offset")
                );
                obj_number += 1;
            }
         } else if parts.len() == 2 {
                obj_number = parts[0].parse().expect("Could not parse object number");
            } else {
                println!("{:?}", parts);
                return Err(PDFError{ message: "Invalid line in xref table", location: 0})
        }
    };
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

#[derive(Debug)]
#[derive(PartialEq)]
enum PDFKeyword {
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

fn parse_object(bytes: &Vec<u8>, start_index: usize) -> Result<(PDFObj, usize), PDFError> {
    let mut state = ParserState::Neutral;
    let mut index = start_index;
    let mut this_object_type = PDFComplexObject::Unknown;
    let length = bytes.len();
    if index > length { return Err(PDFError { message: "index out of range" , location: index })};
    let mut char_buffer = Vec::new();
    let mut object_buffer = Vec::new();
    loop {
        if index > length {
            return Err(PDFError { message: "end of file while parsing object" , location: index })
        };
        let c = bytes[index];
        state = match state {
            ParserState::Neutral => match c {
                b'[' if this_object_type == PDFComplexObject::Unknown => {
                    this_object_type = PDFComplexObject::Array;
                    state
                },
                b'[' => {
                    let (new_array, end_index) = parse_object(&bytes, index)?;
                    index = end_index;
                    object_buffer.push(new_array);
                    state
                },
                b']' => {
                    if this_object_type == PDFComplexObject::Array {
                        return make_array_from_object_buffer(object_buffer, index)
                    } else {
                        println!("-------Array ended but I'm a {:?}", this_object_type);
                        return Err(PDFError {
                            message: "Invalid terminator: ]", location: index
                        }) 
                    }
                },
                b'<' if peek_ahead_by_n(&bytes, index, 1) == Some(b'<') => {
                    if this_object_type == PDFComplexObject::Unknown {
                        this_object_type = PDFComplexObject::Dict;
                        index += 1;
                        println!("Dict started at: {}", index);
                    } else {
                        println!("Nested dict in {:?} at {}", this_object_type, index);
                        let (new_dict, end_index) = parse_object(&bytes, index)?;
                        index = end_index;
                        //println!("Nested dict closed at {}", index);
                        object_buffer.push(new_dict);
                    };
                    state
                },
                b'<' => { ParserState::HexString },
                b'>' if (peek_ahead_by_n(&bytes, index, 1) == Some(b'>')) => {
                    if this_object_type == PDFComplexObject::Dict {
                        return make_dict_from_object_buffer(object_buffer, index + 1)
                    } else {
                        println!("-------Dictionary ended but I'm a {:?}", this_object_type);
                        println!("Buffer: {:#?}", object_buffer);
                        return Err(PDFError {
                            message: "Invalid terminator: >>", location: index
                        }) 
                    }
                },
                b'(' => { ParserState::CharString(0) },
                b'/' => { ParserState::Name },
                b'R' => {
                    let object_buffer_length = object_buffer.len();
                    if object_buffer_length <= 1 {
                        return Err(PDFError{ message: "Could not parse reference to object", location: index })
                    };
                    let new_object = match object_buffer[(object_buffer_length - 2)..object_buffer_length] {
                        [PDFObj::NumberInt(n1), PDFObj::NumberInt(n2)] if n1 >= 0 && n2 >= 0 => PDFObj::ObjectRef {
                            id_number: n1 as u32, gen_number: n2 as u16
                        },
                        _ => return Err(PDFError{ message: "Could not parse reference to object", location: index })
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
                _ => { println!("Invalid character: {}", c as char); return Err(PDFError {
                    message: "Invalid character", location: index
                }) }
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
                _ => return Err(PDFError{ message: "invalid character in hexstring", location: index })
            },
            ParserState::CharString(depth) => match c {
                b')' if depth == 0 => {
                    object_buffer.push( flush_buffer_to_object(&state, &mut char_buffer)? );
                    ParserState::Neutral
                },
                b')' if depth > 0 => ParserState::CharString(depth - 1),
                b'(' => ParserState::CharString(depth + 1),
                b'\\' if index + 1 < length => {
                    match bytes[index + 1] {
                        15 => {
                            index += 1; // Skip carriage return
                            if index + 1 < length && bytes[index + 1] == 12 { index += 1}; // Skip linefeed too
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
                            if index + 1 < length && is_octal(bytes[index + 1]) {
                                code = code * 8 + (bytes[index + 1] - b'0');
                                index += 1;
                                if index + 1 < length && is_octal(bytes[index + 1]) {
                                    code = code * 8 + (bytes[index + 1] - b'0');
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
                        return Err(PDFError { message: "two decimal points in number", location: index }) };
                    char_buffer.push(c);
                    state
                },
                _ if is_whitespace(c) || is_delimiter(c) => {
                    object_buffer.push( flush_buffer_to_object(&state, &mut char_buffer)? );
                    index -= 1; // Need to parse delimiter character on next iteration
                    ParserState::Neutral
                },
                _ => return Err(PDFError { message: "invalid character in number", location: index }) 
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
                        return Err(PDFError{ message: "Invalid character in keyword", location: index})
                    };
                    let this_keyword = flush_buffer_to_object(&state, &mut char_buffer)?;
                    match this_keyword {
                        PDFObj::Keyword(PDFKeyword::EndObj) => {
                            if this_object_type == PDFComplexObject::IndirectObj {
                                return make_object_from_object_buffer(object_buffer, index)
                            } else {
                                return Err(
                                    PDFError{ message: "Encountered endobj outside indirect object", location: index})
                            };
                        },
                        PDFObj::Keyword(PDFKeyword::Stream) => {
                            return make_stream_object(&bytes, object_buffer, index)
                        },
                        PDFObj::Keyword(PDFKeyword::Obj) if this_object_type != PDFComplexObject::Unknown => 
                            return Err(
                                PDFError{ message: "Encountered nested obj declaration", location: index}
                            ),
                        PDFObj::Keyword(PDFKeyword::Obj) => {
                            this_object_type = PDFComplexObject::IndirectObj;
                            index -= 1;
                            println!("parsing an object");
                            ParserState::Neutral
                        },
                        PDFObj::Keyword(_) => {
                            object_buffer.push(this_keyword);
                            index -= 1;
                            ParserState::Neutral
                        },
                        _ =>{
                            return Err(PDFError{
                                message: "Not a recognized keyword", location: index
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


fn flush_buffer_to_object (state: &ParserState , buffer: &mut Vec<u8>) -> Result<PDFObj, PDFError> {
    let new_obj = match state {
        ParserState::Neutral => return Err(PDFError {message: "called flush buffer in Neutral context", location: 0}),
        ParserState::HexString => {
            for c in buffer.iter() {
                if !is_hex(*c) { return Err(PDFError { message: "invalid character in hex string", location: 0}) };
            };
            PDFObj::HexString(buffer.clone() as Vec<u8>)
        },
        ParserState::CharString(0) => {
            println!("Vec: {:?}", buffer);

            PDFObj::CharString(str::from_utf8(buffer).unwrap().to_string())
        },
        ParserState::CharString(_c) => {
            return Err(PDFError{ message: "Asked to create string with unclosed parens", location: 0 });
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
                _ => return Err(PDFError{ message: "Invalid PDF keyword", location: 0})
            }
        }
    };
    buffer.clear();
    return Ok(new_obj)
}


fn make_array_from_object_buffer(object_buffer: Vec<PDFObj>, end_index: usize) -> Result<(PDFObj, usize), PDFError> {
    Ok((PDFObj::Array(object_buffer), end_index))
}

fn make_dict_from_object_buffer(object_buffer: Vec<PDFObj>, end_index: usize) -> Result<(PDFObj, usize), PDFError> {
    let mut dict = HashMap::new();
    let mut object_it = object_buffer.into_iter();
    loop {
        let key = match object_it.next() {
            None => {
                println!("completed a dict: {:?}", dict);
                return Ok((PDFObj::Dictionary(Box::new(dict)), end_index))
            },
            Some(PDFObj::Name(s)) => s,
            _ => return Err(PDFError{ message: "Dictionary key was not a Name", location: end_index})
        };
        let value = match object_it.next() {
            None => return Err(PDFError{ message: "No object for key", location: end_index }),
            Some(v) => v
        };
        dict.insert(key, value);
    } 
}

fn make_object_from_object_buffer(mut object_buffer: Vec<PDFObj>, end_index: usize) -> Result<(PDFObj, usize), PDFError> {
    if object_buffer.len() != 3 {
        return Err(PDFError{ message: "Object tags contained multiple or no objects", location: end_index })
    };
    let id_number = match object_buffer[0] {
        PDFObj::NumberInt(i) => i as u32,
        _ => return Err(PDFError{ message: "invalid object number", location: 0})
    };
    let gen_number = match object_buffer[1] {
        PDFObj::NumberInt(i) => i as u16,
        _ => return Err(PDFError{ message: "invalid generation number", location: 0})
    }; 
    return Ok((PDFObj::IndirectObject{
        id_number, gen_number, object: Box::new(object_buffer.pop().unwrap())
    }, end_index))
}

fn make_stream_object(bytes: &Vec<u8>, mut object_buffer: Vec<PDFObj>, index: usize)
        -> Result<(PDFObj, usize), PDFError> {
    if object_buffer.len() != 3 {
        return Err(PDFError{ message: "Expected stream to be preceded by a sole dictionary", location: index })
    };
    let binary_start_index = match bytes[index] {
        b'\n' => index + 1,
        b'\r' => {
            if let Some(b'\n') = peek_ahead_by_n(&bytes, index, 1) {
                index + 2} else {
                    return Err(PDFError{ message: "Stream tag not followed by appropriate spacing", location: index })
                }
            },
        _ => return Err(PDFError{ message: "Stream tag not followed by appropriate spacing", location: index })
    };
    let stream_dict = match object_buffer.pop().unwrap() {
        PDFObj::Dictionary(hash_map) => hash_map,
        _ => return Err(PDFError{ message: "Stream preceded by a non-dictionary object", location: index })
    };
    let id_number = match object_buffer[0] {
        PDFObj::NumberInt(i) => i as u32,
        _ => return Err(PDFError{ message: "invalid object number", location: 0})
    };
    let gen_number = match object_buffer[1] {
        PDFObj::NumberInt(i) => i as u16,
        _ => return Err(PDFError{ message: "invalid generation number", location: 0})
    };
    let binary_length = match stream_dict.get("Length") {
        Some(PDFObj::NumberInt(binary_length)) => *binary_length as usize,
        _ => return Err(PDFError{ message: "Could not find valid /Length key", location: index })
    };
    // TODO: Confirm endstream included
    if binary_start_index + binary_length >= bytes.len() {
        return Err(PDFError{ message: "Reported binary content length too long", location: binary_start_index})
    };
    Ok((PDFObj::IndirectObject{
        id_number, gen_number, object: Box::new(
            PDFObj::Stream(
                stream_dict,
                Vec::from(&bytes[binary_start_index..(binary_start_index + binary_length + 1)])))
        },
        binary_start_index + binary_length + 9))
}


fn peek_ahead_by_n(bytes: &Vec<u8>, index: usize, n: usize) -> Option<u8> {
    if index + n >= bytes.len() {return None};
    return Some(bytes[index + n])
}

fn is_octal(c: u8) -> bool {
    b'0' <= c && c <= b'7'
}

fn is_whitespace(c: u8) -> bool {
    c == 0 || c== 9 || c== 12 || c == 32 || is_EOL(c)
}

fn is_delimiter(c: u8) -> bool {
    match c {
        b'<' | b'>' | b'(' | b')' | b'[' | b']' | b'{' | b'}' | b'/' | b'%' => true,
        _ => false
    }
}

fn is_hex(c: u8) -> bool {
    (b'0' <= c && c <= b'9') || (b'A' <= c && c <= b'F')
}

fn is_EOL(c: u8) -> bool {
    c == b'\n' || c == b'\r'
}

fn is_letter(c: u8) -> bool {
    (b'a' <= c && c <= b'z') || (b'A' <= c || c <= b'Z')
}

fn is_body_keyword_letter(c: u8) -> bool {
    match c {
        b'e' | b'n' | b'd' | b's' | b't' | b'r' | b'a' | b'm' | b'o' | b'b' | b'j' | b'u' | b'l' | b'u' | b'f' => true,
        _ => false
    }
}

#[test]
fn test_body_keyword_letters() {
    let keywords = "stream_endstream_obj_endobj_null_true_false";
    for letter in (b'a'..b'z').chain(b'A'..b'Z') {
        println!("{}", letter as char);
        if keywords.contains(letter as char) {
            assert_eq!(is_body_keyword_letter(letter), true);
        } else {
            assert_eq!(is_body_keyword_letter(letter), false);
        }
    };
}

fn is_trailer_keyword_letter(c: u8) -> bool {
    match c {
        b't' | b'r' | b'a' | b'i' | b'l' | b'e' | b's' | b'x' | b'f' => true,
        _ => false
    }
}

#[test]
fn test_trailer_keyword_letters() {
    let keywords = "trailer_startxref";
    for letter in (b'a'..b'z').chain(b'A'..b'Z') {
        println!("{}", letter as char);
        if keywords.contains(letter as char) {
            assert_eq!(is_trailer_keyword_letter(letter), true);
        } else {
            assert_eq!(is_trailer_keyword_letter(letter), false);
        }
    };
}

fn is_xref_table_keyword_letter(c: u8) -> bool {
    match c {
        b'x' | b'r' | b'e' | b'f' | b'n' | b'\n' | b'\r' => true,
        _ => false
    }
}

#[test]
fn test_xref_table_keyword_letters() {
    let keywords = "xref_f\r\n_n\r\n";
    for letter in (b'a'..b'z').chain(b'A'..b'Z') {
        println!("{}", letter as char);
        if keywords.contains(letter as char) {
            assert_eq!(is_xref_table_keyword_letter(letter), true);
        } else {
            assert_eq!(is_xref_table_keyword_letter(letter), false);
        }
    };
}

