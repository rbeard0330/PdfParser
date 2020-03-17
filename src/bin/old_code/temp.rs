fn _parse_header(header_slice: &Vec<PDFCharacter>) -> Result<PDFVersion, PDFError>{
    let index: usize = 0;
    let max_length = header_slice.len();
    while header_slice[index] == PDFCharacter::InlineWhitespace || header_slice[index] == PDFCharacter::EOLWhitespace {
        index += 1;
        if index == max_length {
            return Err(PDFError {message: "PDF Version not found in header"})
        };
    }
    if !(
        header_slice[index]     == PDFCharacter::PercentSymbol &&
        header_slice[index + 1] == PDFCharacter::Character('P') &&
        header_slice[index + 2] == PDFCharacter::Character('D') &&
        header_slice[index + 3] == PDFCharacter::Character('F') &&
        header_slice[index + 5] == PDFCharacter::Character('.')
    ) {
        return Err(PDFError {message: "Invalid PDF Version specification"})
    } else {
        match header_slice[index + 4] {
            PDFCharacter::Character('1') => match header_slice[index + 6] {
                PDFCharacter::Character('0') => Ok(PDFVersion::V1_0),
                PDFCharacter::Character('1') => Ok(PDFVersion::V1_1),
                PDFCharacter::Character('2') => Ok(PDFVersion::V1_2),
                PDFCharacter::Character('3') => Ok(PDFVersion::V1_3),
                PDFCharacter::Character('4') => Ok(PDFVersion::V1_4),
                PDFCharacter::Character('5') => Ok(PDFVersion::V1_5),
                PDFCharacter::Character('6') => Ok(PDFVersion::V1_6),
                PDFCharacter::Character('7') => Ok(PDFVersion::V1_7),
            },
            PDFCharacter::Character('1') if header_slice[index + 6] == PDFCharacter::Character('0') => Ok(PDFVersion::V2_0),
            _ => Err(PDFError {message: "Invalid PDF Version specification"})
    }
}

"""
state machine

object_context:
    IN_OBJECT, IN_DICT, IN_STRING, IN_NAME, etc.
    Contexts are saved in a stack.
    When reader encounters an end-context token that is not escaped in the current top context, it pops the top context
    off the stack and dispatches an object (or throws an error if it doesn't fit (<<...])).  Other characters and
    sub-objects are saved in a buffer.
    Objects are built from the buffer when the context terminates, then added to the buffer for the containing
    context.  If the buffer does not contain the appropriate data, throw an error.

parsing_state:
    on a character level, there is a simple state machine for determining context transitions:
    <<  => push dict context. Ignore in string context.
    / => push name context. Ignore in string context.
    [ => push array context. Ignore in string context.
    ( => push string context. Ignore in string context.
    /d+ /d+ obj => push object context
    /d+ /d+ R => push and immediately pop indirect-object context
    stream => invoke binary method binary data is handled outside the character context using the /Length metadata
    /w/d+.?/d*/w => push and immediately pop number context (but maintain state to trigger object context)
    >> => Ignore in string context.  Else, Pop all name and number contexts until a dict context is reached.  Reaching
    any other context or the bottom of the stack is an error. TODO: Confirm about errors
    ] => See dict, but for arrays
    /w => pop name context, if active, else ignore
    ) => pop string context.  An error if not in a string context.  TODO: Confirm
    TODO: Braces, comments, #
    

"""

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


fn parse_ext_ascii(c: u8) -> char {
    match c {
        0..=127 => c as char,
        128 => 'Ç',
        129 => 'ü',
        130 => 'é',
        131 => 'â',
        132 => 'ä',
        133 => 'à',
        134 => 'å',
        135 => 'ç',
        136 => 'ê',
        137 => 'è',
        138 => 'è',
        139 => 'ï',
        140 => 'î',
        141 => 'ì',
        142 => 'Ä',
        143 => 'Å',
        144 => 'É',
        145 => 'æ',
        146 => 'Æ',
        147 => 'ô',
        148 => 'ö',
        149 => 'ò',
        150 => 'û',
        151 => 'ù',
        152 => 'ÿ',
        153 => 'Ö',
        154 => 'Ü',
        155 => '¢',
        156 => '£',
        157 => '¥',
        158 => '₧',
        159 => 'ƒ',
        160 => 'á',
        161 => 'í',
        162 => 'ó',
        163 => 'ú',
        164 => 'ñ',
        165 => 'Ñ',
        166 => 'ª',
        167 => 'º',
        _ => '?'
    }
}


fn create_node_from_object(&mut self, node_id: ObjectID) -> Result<PageNode, PDFError> {
    let node_dict = match self.get_object(&node_id) {
        Ok(Dictionary(map)) => map,
        _ => return Err(PDFError{ 
            message: "Could not find node dictionary".to_string(), function: "create_node_from_object"
        })
    };
    let parent = node_dict.get("Parent");
    let kids = node_dict.get("Kids");
    let count = node_dict.get("Count");
    match (parent, kids) {
        (Some(ObjectRef(p_id)), None) => Ok(),
        (Some(ObjectRef(p_id)), Some(kid_array)) => Ok(),
        (None, Some(kid_array)) => Ok(),
        (None, None) => return Err(PDFError{
            message: format!("Page node {} must have either Parent or Kids", node_id),
            function: "create_node_from_object"
        })
        

        
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum PDFObj {
    Boolean(bool),
    NumberInt(i32),
    NumberFloat(f32),
    Name(String),
    CharString(String),
    HexString(Vec<u8>),
    Array(Vec<SharedObject>),
    Dictionary(HashMap<String, SharedObject>),
    Stream(HashMap<String, SharedObject>, Vec<u8>),
    Comment(String),
    Keyword,
    ObjectRef,
    DecodedStream
}