use std::io::Read;

use flate2;

use super::*;

pub enum Filter{
    ASCIIHex,
    ASCII85,
    LZW(Option<SharedObject>),
    Flate(Option<SharedObject>),
    RunLength,
    CCITTFax(Option<SharedObject>),
    JBIG2(Option<SharedObject>),
    DCT(Option<SharedObject>),
    JPX,
    Crypt(Option<SharedObject>)
}

impl std::fmt::Display for Filter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Filter::*;
        match self {
            ASCIIHex => write!(f, "ASCIIHex Filter"),
            ASCII85 => write!(f, "ASCII85 Filter"),
            LZW(_) => write!(f, "LZW Filter"),
            Flate(_) => write!(f, "Flate Filter"),
            RunLength => write!(f, "RunLength Filter"),
            CCITTFax(_) => write!(f, "CCITTFax Filter"),
            JBIG2(_) => write!(f, "JBIG2 Filter"),
            DCT(_) => write!(f, "DCT Filter"),
            JPX => write!(f, "JPX Filter"),
            Crypt(_) => write!(f, "Crypt Filter")
        }.expect("Error in write macro!");
        Ok(())
    }
}

impl Filter {
    pub fn apply(self, data: Result<Vec<u8>, PDFError>) -> Result<Vec<u8>, PDFError> {
        use Filter::*;
        if data.is_err() {return Err(data.unwrap_err())};
        let data = data.unwrap();
        let output_data = match self {
            ASCIIHex => Filter::apply_ascii_hex(data),
            ASCII85 => Filter::apply_ascii_85(data),
            LZW(params) => Filter::apply_LZW(data, params),
            Flate(params) => Filter::apply_flate(data, params),
            _ => Err(PDFError{message: format!("Unsupported filter: {}", self), function: "Filter.apply"})
        };
        output_data
    }

    fn apply_ascii_hex(data: Vec<u8>) -> Result<Vec<u8>, PDFError> {
        const END_OF_DATA: u8 = b'<';  // Standard 7.4.2
        let mut output = Vec::new();
        let mut buffer = Option::None;
        for c in data {
            if !is_hex(c) {
                if !is_whitespace(c) {return Err(PDFError{
                    message: format!("Invalid character for ASCIIHexDecode: {}", c as char),
                    function: "Filter.apply_ascii_hex"})
                };
                if c == END_OF_DATA { break };
            };
            match buffer {
                None => buffer = Some(c as char),
                Some(old_c) => {
                    let hex_pair: String = [old_c, c as char].iter().collect();
                    let value = u8::from_str_radix(&hex_pair, 16).unwrap(); // Valid hex confirmed already
                    output.push(value);
                }
            }
        }
        if let Some(final_char) = buffer {
            // Per spec 7.4.2, unpaired digit is followed by an implicit 0
            output.push(16 * (final_char.to_digit(16).unwrap() as u8)); 
        }
        Ok(output)
    }

    fn apply_ascii_85(data: Vec<u8>) -> Result<Vec<u8>, PDFError> {
        let mut new_data = Vec::new();
        for group in AsciiData(data).ascii85_iter() {
            new_data.extend(Filter::_parse_ascii_85_group(group)?)
        }
        Ok(new_data)
    }

    fn _parse_ascii_85_group(arr: [Option<u8>; 5]) -> Result<Vec<u8>, PDFError> {

        let mut base_256_value: u32 = 0;
        let vec: Vec<u8> = arr.into_iter().filter(|c| c.is_some()).map(|c| c.unwrap()).collect();
        for &c in &vec {
            if !is_valid_ascii_85_byte(c) {
                return Err(PDFError{message: format!("Invalid Ascii85 character: {}", c),
                                    function: "apply_ascii_85"})
            };
            if c == b'z' {
                if vec.len() > 1 {
                    return Err(PDFError{message: format!("z in middle of group: {:?}", vec),
                                function: "apply_ascii_85::_parse_ascii_85_group"})
                }
                return Ok(vec!(0, 0, 0, 0))
                }
            base_256_value = base_256_value * 85 + (c - b'!') as u32; // See spec 7.4.3
        }
        let mut data = Vec::new();
        for exp in (0..3).into_iter().rev() {
            let place_value = base_256_value.pow(exp);
            let digit = (base_256_value / place_value) as u8;
            data.push(digit);
            base_256_value %= place_value;
        }
        Ok(data)
    }

    fn apply_LZW(data: Vec<u8>, params: Option<SharedObject>) -> Result<Vec<u8>, PDFError> {
        Ok(data)
    }

    fn apply_flate(data: Vec<u8>, params: Option<SharedObject>) -> Result<Vec<u8>, PDFError> {
        let mut decoder = flate2::read::DeflateDecoder::new(&*data);
        let mut output = Vec::new();
        let decode_result = decoder.read_to_end(&mut output);
        match decode_result {
            Ok(_) => Ok(data),
            Err(e) => Err(PDFError{message: format!("Error applying flate filter: {:?}", e),
                                   function: "apply:apply_flate"})
        }
    }
}


pub fn filter_from_string_and_params(name: &str, params: Option<Rc<PDFObj>>) -> Result<Filter, PDFError> {
    use Filter::*;
    match name {
        "ASCIIHexDecode" => Ok(ASCIIHex),
        "ASCII85Decode" => Ok(ASCII85),
        "JPXDecode" => Ok(JPX),
        "RunLengthDecode" => Ok(RunLength),
        "LZWDecode" => Ok(LZW(params)),
        "FlateDecode" => Ok(Flate(params)),
        "CCITTFaxDecode" => Ok(CCITTFax(params)),
        "JBIG2Decode" => Ok(JBIG2(params)),
        "DCTDecode" => Ok(DCT(params)),
        "Crypt" => Ok(Crypt(params)),
        _ => Err(PDFError{message: format!("Unsupported filter: {}", name), function: "filter_from_string"})
    }
}

struct Ascii85Iterator {
    data: Vec<u8>,
    data_cursor: usize,
    last_index: usize,
    buffer: [Option<u8>; 5],
    buffer_cursor: usize
}

impl Iterator for Ascii85Iterator {
    type Item = [Option<u8>; 5];
    fn next(&mut self) -> Option<[Option<u8>; 5]> {
        loop {
            if self.data_cursor > self.last_index { return None };
            let next_char = self.data[self.data_cursor];
            self.data_cursor += 1;
            if self.data_cursor == self.last_index { break };

            if is_whitespace(next_char) { continue };

            if next_char == b'~'
               && self.data_cursor < self.last_index
               && self.data[self.data_cursor + 1] == b'>' {
                    return None
            };

            self.buffer[self.buffer_cursor] = Some(next_char);
            self.buffer_cursor += 1;

            if self.buffer_cursor > 4 {
                debug_assert_eq!(self.buffer_cursor, 5);
                break
            };
            if next_char == b'z' { break }
        }
        let return_value = self.buffer;
        self.buffer = [Option::None; 5];
        self.buffer_cursor = 0;
        return Some(return_value)
    }
}

struct AsciiData(Vec<u8>);

impl AsciiData {
    fn ascii85_iter(self) -> Ascii85Iterator {
        let len = self.0.len();
        Ascii85Iterator {
            data: self.0,
            data_cursor: 0,
            last_index: len,
            buffer: [Option::None; 5],
            buffer_cursor: 0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flate_example() {
        let mut pdf_file = PdfFileHandler::create_pdf_from_file("data/document.pdf").unwrap();
        println!("Object: {:?}", pdf_file.get_object(&ObjectID(80, 0)));
    }

}