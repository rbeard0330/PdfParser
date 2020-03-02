use std::fs;
use std::fmt;
use std::collections::HashMap;

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
    Character(char),
    //White space
    InlineWhitespace,
    EOLWhitespace,
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

enum PDFData {
    Boolean(bool),
    NumberInt(i32),
    NumberFloat(f32),
    Name(String),
    String(String),
    Array(PDFArray),
    Dictionary(PDFDict),
    Stream(PDFDict, Vec<u8>),
}

struct PDFDict {}

struct PDFObj {
    id_number: u16,
    gen_number: u16,
    data: PDFData,
}

struct PDFArray {}



struct PDF {
    version: PDFVersion,
}

struct PDFError {
    message: &'static str
}


impl fmt::Display for PDFError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PDF Processing Error: {}", self.message)
    }
}

impl fmt::Debug for PDFError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PDF Processing Error: {}", self.message)
    }
}

fn main(){
    
    //let raw_pdf = fs::read("data/simple_pdf.pdf").expect("Could not read data!");
    let raw_pdf = fs::read("data/CCI01212020.pdf").expect("Could not read data!");
    println!("{}", raw_pdf.len());
    let tokens = get_tokens_from_chars(raw_pdf).expect("!!");
    for t in tokens {
        println!("{}", t);
    }
    
}

fn process_raw_pdf(&bytes: &Vec<u8>) -> Result<Vec<PDFData>, PDFError> {
    // find trailer
    let current_index = bytes.len() - 1;

}

fn get_tokens_from_chars(bytes: Vec<u8>) -> Result<Vec<PDFToken>, PDFError> {
    let char_count = bytes.len() - 1;
    let mut index = 0;
    let mut tokens = Vec::new();
    while index < char_count {
        let (token, delimiter, new_index) = _get_next_token(&bytes, index).unwrap();
        index = new_index;
        println!("----------------{:?}", token);
        println!("next index: {}", index);
        match token {
            PDFToken::Word(s) if &s == "" => {},
            PDFToken::Word(s) if s.len() > 2 && &s[s.len() - 2..s.len()] == ">>" => {
                println!("splitting");
                let mut word1 = String::new();
                word1.push_str(&s);
                let word2 = word1.split_off(word1.len() - 2);
                tokens.push(PDFToken::Word(word1));
                tokens.push(PDFToken::Word(word2));
            },
            _ => tokens.push(token)
        }
        if let PDFToken::StreamStart = &tokens[tokens.len() - 1] {
            if delimiter == PDFCharacter::EOLWhitespace {
                tokens.push(PDFToken::Newline);
            }
            println!("Going down the steam path!");
            let mut search_index = tokens.len() - 2;
            while tokens[search_index] != PDFToken::Word("/Length".to_string()) &&
            tokens[search_index] != PDFToken::Word("<</Length".to_string()) {
                if search_index == 0 {
                    return Result::Err(PDFError{ message: "Binary content without length"})
                };
                search_index -= 1;                
            }
            search_index += 1;
            let current_tokens = tokens.len();
            while tokens[search_index] == PDFToken::Newline {
                search_index += 1;
                if search_index >= char_count{
                    return Result::Err(PDFError{ message: "Could not find value for /Length"})
                };
            }
            let binary_length: usize = match &tokens[search_index] {
                PDFToken::Word(s) => match s.parse() {
                    Result::Ok(i) => i,
                    _ => return Result::Err(PDFError { message: "Invalid value for /Length" })
                },
                _ => return Result::Err(PDFError { message: "Invalid value for /Length" })
            };
            let old_index = index;
            index += binary_length;
            let mut these_bytes = vec![0; binary_length];
            println!("{:?}", index);
            println!("{:?}", old_index);
            these_bytes.copy_from_slice(&bytes[old_index..index]);
            tokens.push(PDFToken::Bytes(these_bytes));
        };
        if delimiter == PDFCharacter::EOLWhitespace {
            tokens.push(PDFToken::Newline);
        }
    }
    Result::Ok(tokens)
}



fn _get_next_character(bytes: &Vec<u8>, index: usize) -> (PDFCharacter, usize) {
    let length = bytes.len();
    let mut this_index = index;
    let mut char_type = _char_from_u8(bytes[this_index]);
    while this_index + 2 < length && (char_type == PDFCharacter::InlineWhitespace || char_type == PDFCharacter::EOLWhitespace) {
        this_index += 1;
        char_type = match _char_from_u8(bytes[this_index]) {
            PDFCharacter::InlineWhitespace if char_type == PDFCharacter::InlineWhitespace => PDFCharacter::InlineWhitespace,
            PDFCharacter::InlineWhitespace | PDFCharacter::EOLWhitespace => PDFCharacter::EOLWhitespace,
            _ => {
                this_index -= 1;
                break}
        };
    }
    (char_type, this_index + 1)
}

fn _char_from_u8(val: u8) -> PDFCharacter {
    match val {
        0 | 9 | 12 | 32 => PDFCharacter::InlineWhitespace,
        10 | 13 => PDFCharacter::EOLWhitespace,
        40 => PDFCharacter::OpenParen,
        41 => PDFCharacter::CloseParen,
        60 => PDFCharacter::OpenAngle,
        62 => PDFCharacter::CloseAngle,
        91 => PDFCharacter::OpenBracket,
        93 => PDFCharacter::CloseBracket,
        123 => PDFCharacter::OpenBrace,
        125 => PDFCharacter::CloseBrace,
        92 => PDFCharacter::Solidus,
        37 => PDFCharacter::PercentSymbol,
        c @ _ => PDFCharacter::Character(c as char),
    }
}

fn _get_next_token(bytes: &Vec<u8>, index: usize) -> Result<(PDFToken, PDFCharacter, usize), PDFError> {
    let mut this_index = index;
    let end_index = bytes.len() - 1;
    let (mut next_char, new_index) = _get_next_character(&bytes, this_index);
    this_index = new_index;
    
    let mut word = Vec::new();
    while next_char != PDFCharacter::EOLWhitespace && next_char != PDFCharacter::InlineWhitespace {
        let value = match next_char {
            PDFCharacter::OpenParen => Some('(' as u8),
            PDFCharacter::CloseParen => Some(')' as u8),
            PDFCharacter::OpenAngle => Some('<' as u8),
            PDFCharacter::CloseAngle => Some('>' as u8),
            PDFCharacter::OpenBracket => Some('[' as u8),
            PDFCharacter::CloseBracket => Some(']' as u8),
            PDFCharacter::OpenBrace => Some('{' as u8),
            PDFCharacter::CloseBrace => Some('}' as u8),
            PDFCharacter::Solidus => Some('/' as u8),
            PDFCharacter::PercentSymbol => Some('%' as u8),
            PDFCharacter::Character(c) => Some(c as u8),
            _ => None
        };
        word.push(value.unwrap());
        if word == b"<<" { break };
        let (new_next_char, new_this_index) = _get_next_character(&bytes, this_index);
        //println!("{:?} char at {}", next_char, this_index);
        this_index = new_this_index;
        next_char = new_next_char;
        if this_index > end_index { break };
    }
    let delimiter = match next_char {
        PDFCharacter::InlineWhitespace => PDFCharacter::InlineWhitespace,
        _ => PDFCharacter::EOLWhitespace
    };
    let token = match String::from_utf8(word) {
        Ok(s) if &s == "stream" => PDFToken::StreamStart,
        Ok(s) => PDFToken::Word(s),
        Err(_e) => return Result::Err(PDFError { message: "invalid text content" })
    };
    Result::Ok((token, delimiter, this_index))

}