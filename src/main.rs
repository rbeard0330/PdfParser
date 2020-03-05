use std::fs;
use std::fmt;

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
enum PDFCharacter {
    //Regular
    Regular(char),
    //White space
    InlineWhitespace,
    CarriageReturn,
    LineFeed,
    //Delimiters
    OpenParen,
    CloseParen,
    OpenAngle,
    CloseAngle,
    OpenBracket,
    CloseBracket,
    OpenBrace,
    CloseBrace,
    Solidus,
    PercentSymbol,
}

#[derive(Debug)]
#[derive(PartialEq)]
enum PDFToken {
    Word(String),
    StreamStart,
    Newline,
    Bytes(Vec<u8>),
}

impl fmt::Display for PDFToken {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PDFToken::Word(s) => write!(f, "{}", &s),
            PDFToken::StreamStart => write!(f, "stream"),
            PDFToken::Newline => write!(f, "Newline"),
            PDFToken::Bytes(_v) => write!(f, "Bytes")
        }
    }
}

#[derive(Debug)]
#[derive(PartialEq)]
enum PDFObj {
    Boolean(bool),
    NumberInt(i32),
    NumberFloat(f32),
    Name(String),
    PDFString(String),
    Array(PDFArray),
    Dictionary(PDFDict),
    Stream(PDFDict, Box<Vec<u8>>),
    Comment(String),
    Keyword(PDFKeyword),
    ObjectRef{ id_number: u32, gen_number: u16 },
}

#[derive(Debug)]
#[derive(PartialEq)]
enum PDFComplexObject {
    Unknown,
    Dict,
    Array,
    IndirectObj
}

#[derive(Debug)]
#[derive(PartialEq)]
struct PDFDict {
    start_index: usize,
    end_index: usize
}

#[derive(Debug)]
#[derive(PartialEq)]
struct PDFArray {
    start_index: usize,
    end_index: usize,
    items: Box<Vec<PDFObj>>
}

struct PDF {
    version: PDFVersion,
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
    let raw_pdf = fs::read("data/CCI01212020.pdf").expect("Could not read data!");
    println!("{}", raw_pdf.len());
    println!("trailer starts at: {:?}", _find_trailer_index(&raw_pdf).expect("!"));
    
}



fn _find_trailer_index(bytes: &Vec<u8>) -> Result<usize, PDFError> {
    let mut state: usize = 0;
    let mut current_index = bytes.len() as usize;
    while state < 7 {
        current_index -= 1;
        let c = bytes[current_index] as char;
        println!("char {} with {}", c, state);
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

fn _process_trailer(bytes: &Vec<u8>, start_index: usize) {

}

#[derive(Debug)]
#[derive(PartialEq)]
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

fn parse_object(bytes: &Vec<u8>, start_index: usize) -> Result<PDFObj, PDFError> {
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
        let state = match state {
            ParserState::Neutral => match c {
                b'[' if this_object_type == PDFComplexObject::Unknown => {
                    this_object_type = PDFComplexObject::Array;
                    state
                },
                b'[' => {
                    let new_array = parse_object(&bytes, index)?;
                    index = new_array.end_index;
                    object_buffer.push(new_array);
                    state
                },
                b'<' if peek_ahead_by_n(&bytes, index, 1) == Some(b'<') => {
                    if let this_object_type = PDFComplexObject::Unknown {
                        this_object_type = PDFComplexObject::Dict;
                        index += 1;
                    } else {
                        let new_dict = parse_object(&bytes, index)?;
                        index = new_dict.end_index;
                        object_buffer.push(new_dict);
                        state
                    }
                },
                b'<' => { ParserState::HexString },
                b'(' => { ParserState::CharString(0) },
                b'R' => {
                    let object_buffer_length = object_buffer.len();
                    if object_buffer_length <= 1 {
                        return Err(PDFError{ message: "Could not parse reference to object", location: index })
                    };
                    let new_object = match object_buffer.slice(object_buffer_length - 2: object_buffer_length) {
                        [PDFObj::NumberInt(n1), PDFObj::NumberInt(n2)] => PDFObj::ObjectRef {
                            id_number: n1, gen_number: n2 as u16
                        },
                        _ => return Err(PDFError{ message: "Could not parse reference to object", location: index })
                    };
                    object_buffer.truncate(object_buffer_length - 2);
                    object_buffer.push(new_object);
                    state
                },
                let keyword_list = ["stream", "endstream", "obj", "endobj", "null", "true", "false"];
                let xref_kws = ["f\n", "n\n", "xref"];
                let trailer_kws = ["trailer", "startxref"];
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
                _ => return Err(PDFError{ message: &format!("invalid character {} in hexstring", c), location: index })
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
                        12 => {index + 1; state}, // Escape naked LF
                        b'\\' => {
                            index + 1;
                            char_buffer.push(b'\\');
                            state
                        },
                        b'(' => {
                            index + 1;
                            char_buffer.push(b'(');
                            state
                        },
                        b')' => {
                            index + 1;
                            char_buffer.push(b')');
                            state
                        },
                        d @ b'0'..=b'7' => {
                            index += 1;
                            let mut code = d - 48; // ASCII 0 = 48
                            if index + 1 < length && is_octal(bytes[index + 1]) {
                                code = code * 8 + bytes[index + 1] - 48;
                                index += 1;
                                if index + 1 < length && is_octal(bytes[index + 1]) {
                                    code = code * 8 + bytes[index + 1] - 48;
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
                _ => return Err(PDFError { message: &format!("invalid character {} in number", c), location: index }) 
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
                if !is_letter(c) {
                    let this_keyword = flush_buffer_to_object(&state, &mut char_buffer)?;
                    match this_keyword {
                        PDFKeyword::EndObj => ()
                    }

                    object_buffer.push(this_keyword);
                    ParserState::Neutral
                } else {
                    char_buffer.push(c);
                    state
                }
            }
        };
        index += 1;
    };
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
    c == 40 || c == 41 || c == 60 || c == 62 || c == 91 || c == 93 || c == 123 || c == 125 || c == 47 || c == 37
}

fn is_EOL(c: u8) -> bool {
    c == 10 || c == 15
}

fn is_letter(c: u8) -> bool {
    (b'a' <= c && c <= b'z') || (b'A' <= c || c <= b'Z')
}

fn flush_buffer_to_object (state: &ParserState , buffer: &mut Vec<u8>) -> Result<PDFObj, PDFError> {
    match state {
        Neutral,
        HexString,
        CharString(u8),
        Name,
        Number,
        Comment,
        Keyword
    }

}


fn create_array(object_buffer: &Vec<PDFObj>) -> Result<PDFObj, PDFError> {

}

fn create_dict(object_buffer: &Vec<PDFObj>) -> Result<PDFObj, PDFError> {
    
}

fn create_object(object_buffer: &Vec<PDFObj>) -> Result<PDFObj, PDFError> {
    
}

fn _parse_indirect_object(bytes: &Vec<u8> ,start_index: usize) -> (Result<PDFObj, PDFError>, usize) {
    let mut char_buffer = Vec::new();
    let mut object_buffer = Vec::new();
    let mut context_stack = Vec::new();
    let mut state: u8 = 0;
    let mut index = start_index;
    loop {
        let next_char = _char_from_u8(bytes[index]);
    }
}

