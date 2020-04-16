use crate::errors::*;


pub fn is_octal(c: u8) -> bool {
    b'0' <= c && c <= b'7'
}

pub fn is_whitespace(c: u8) -> bool {
    c == 0 || c == 9 || c == 12 || c == 32 || is_eol(c)
}

pub fn is_delimiter(c: u8) -> bool {
    match c {
        b'<' | b'>' | b'(' | b')' | b'[' | b']' | b'{' | b'}' | b'/' | b'%' => true,
        _ => false,
    }
}

pub fn is_hex(c: u8) -> bool {
    (b'0' <= c && c <= b'9') || (b'A' <= c && c <= b'F') || (b'a' <= c && c <= b'f')
}

pub fn is_eol(c: u8) -> bool {
    c == b'\n' || c == b'\r'
}

// pub fn is_letter(c: u8) -> bool {
//     (b'a' <= c && c <= b'z') || (b'A' <= c || c <= b'Z')
// }

pub fn to_binary_vec(s: &str) -> Result<Vec<u8>> {
    let mut output_vec = Vec::new();
    for c in s.bytes() {
        output_vec.push(c);
    };
    Ok(output_vec)
}

pub fn is_body_keyword_letter(c: u8) -> bool {
    match c {
        b'e' | b'n' | b'd' | b's' | b't' | b'r' | b'a' | b'm' | b'o' | b'b' | b'j' | b'u'
        | b'l' | b'f' => true,
        _ => false,
    }
}

pub fn is_trailer_keyword_letter(c: u8) -> bool {
    match c {
        b't' | b'r' | b'a' | b'i' | b'l' | b'e' | b's' | b'x' | b'f' => true,
        _ => false,
    }
}

pub fn is_xref_table_keyword_letter(c: u8) -> bool {
    match c {
        b'x' | b'r' | b'e' | b'f' | b'n' | b'\n' | b'\r' => true,
        _ => false,
    }
}

/// Is c a valid character for ASCII85Decode Filter described in spec 7.4.3
pub fn is_valid_ascii_85_byte(c: u8) -> bool {
    match c {
        b'z' => true,
        _ if b'!' <= c && c <= b'u' => true,
        _ => false,
    }
}

pub fn to_ascii(data: Vec<u8>) -> String {
    data.iter().map(|i| *i as char).collect()
}

pub fn u8_slice_as_int(slice: &[u8]) -> u32 {
    let mut acc = 0;
    for d in slice {
        acc = 256 * acc + *d as u32;
    }
    //println!("slice: {:?}, acc: {}", slice, acc);
    acc
}

#[cfg(test)]
mod tests {
    use super::*;

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
        }
    }
}
