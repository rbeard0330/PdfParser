use std::io::Read;

use flate2;

use super::*;
use crate::errors::*;

pub enum Filter {
    ASCIIHex,
    ASCII85,
    LZW(Option<SharedObject>),
    Flate(Option<SharedObject>),
    RunLength,
    CCITTFax(Option<SharedObject>),
    JBIG2(Option<SharedObject>),
    DCT(Option<SharedObject>),
    JPX,
    Crypt(Option<SharedObject>),
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
            Crypt(_) => write!(f, "Crypt Filter"),
        }
        .expect("Error in write macro!");
        Ok(())
    }
}

impl Filter {
    pub fn apply(self, data: Result<Vec<u8>>) -> Result<Vec<u8>> {
        use Filter::*;
        if data.is_err() {
            return Err(data.unwrap_err());
        };
        let data = data.unwrap();
        let output_data = match self {
            ASCIIHex => Filter::apply_ascii_hex(data),
            ASCII85 => Filter::apply_ascii_85(data),
            LZW(params) => Filter::apply_LZW(data, params),
            Flate(params) => Filter::apply_flate(data, params),
            _ => Err(ErrorKind::FilterError(
                format!("Unsupported filter: {}", self),
                "Filter.apply",
            ))?,
        };
        output_data
    }

    fn apply_ascii_hex(data: Vec<u8>) -> Result<Vec<u8>> {
        const END_OF_DATA: u8 = b'<'; // Standard 7.4.2
        let mut output = Vec::new();
        let mut buffer = Option::None;
        for c in data {
            if !is_hex(c) {
                if !is_whitespace(c) {
                    return Err(ErrorKind::FilterError(
                        format!("Invalid character for ASCIIHexDecode: {}", c as char),
                        "Filter.apply_ascii_hex",
                    ))?;
                };
                if c == END_OF_DATA {
                    break;
                };
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

    fn apply_ascii_85(data: Vec<u8>) -> Result<Vec<u8>> {
        let mut new_data = Vec::new();
        for group in AsciiData(data).ascii85_iter() {
            new_data.extend(Filter::_parse_ascii_85_group(group)?)
        }
        Ok(new_data)
    }

    fn _parse_ascii_85_group(arr: [Option<u8>; 5]) -> Result<Vec<u8>> {
        let mut base_256_value: u32 = 0;
        let vec: Vec<u8> = arr
            .into_iter()
            .filter(|c| c.is_some())
            .map(|c| c.unwrap())
            .collect();
        for &c in &vec {
            if !is_valid_ascii_85_byte(c) {
                return Err(ErrorKind::FilterError(
                    format!("Invalid Ascii85 character: {}", c),
                    "apply_ascii_85",
                ))?;
            };
            if c == b'z' {
                if vec.len() > 1 {
                    return Err(ErrorKind::FilterError(
                        format!("z in middle of group: {:?}", vec),
                        "apply_ascii_85::_parse_ascii_85_group",
                    ))?;
                }
                return Ok(vec![0, 0, 0, 0]);
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

    fn apply_LZW(data: Vec<u8>, params: Option<SharedObject>) -> Result<Vec<u8>> {
        Ok(data)
    }

    fn apply_flate(data: Vec<u8>, params: Option<SharedObject>) -> Result<Vec<u8>> {
        let mut decoder = flate2::read::DeflateDecoder::new(&*data);
        let mut output = Vec::new();
        let decode_result = decoder.read_to_end(&mut output);
        match decode_result {
            Ok(_) => Ok(data),
            Err(e) => Err(ErrorKind::FilterError(
                format!("Error applying flate filter: {:?}", e),
                "apply:apply_flate",
            ))?,
        }
    }
}

pub fn decode_stream(map: &PdfMap, bytes: &Vec<u8>) -> Result<PdfObject> {
    //Check size
    let expected_byte_length = map.get("Length")
                                .ok_or(ErrorKind::ParsingError(
                                    format!("Missing Length in {:?}", map)))?
                                .try_into_int()? as usize;
    assert_eq!(bytes.len(), expected_byte_length);

    // Classify stream
    let type_and_subtype = (map.get("Type"), map.get("Subtype"));
    let stream_type = determine_stream_type(type_and_subtype);
    if let StreamType::Image = stream_type {
        return Ok(Rc::new(DecodedStream {
            stream_type: StreamType::Image,
        }));
    };

    //Extract filters
    let filters = map.get("Filter").unwrap_or(Rc::new(Array(Vec::new())));
    let params = map.get("DecodeParms").ok();
    let filter_string_array = match &*filters {
        &Name(ref s) => vec![Ok(s.clone())],
        Array(ref v) => v
            .iter()
            .map(|item| match **item {
                PDFObj::Name(ref s) => Ok(s.clone()),
                _ => Err(PDFError {
                    message: format!("Non-name item in Filter array: {:?}", item),
                    function: "decode_stream",
                }),
            })
            .collect(),
        item => Err(ErrorKind::FilterError(
            format!("Non-name item in Filter array: {:?}", item),
            "decode stream",
        ))?,
    }
    .into_iter()
    .collect::<Result<Vec<String>, _>>()?;
    let filter_array = filter_string_array
        .into_iter()
        .enumerate()
        // Collect matching params without throwing error if no filters need params
        .map(|(index, s)| {
            filter_from_string_and_params(
                &s,
                params.as_ref().map(|arr| match **arr {
                    Array(_) => arr.index(index).unwrap(),
                    _ => Rc::clone(arr),
                }),
            )
        })
        .collect::<Result<Vec<decode::Filter>, _>>()?;
    let object = filter_array
        .into_iter()
        .fold(Ok(bytes.clone()), |data, filter| filter.apply(data))?;

    Ok(Rc::new(DecodedStream {
        stream_type: StreamType::Image,
    }))
}

fn filter_from_string_and_params(name: &str, params: Option<PdfObject>) -> Result<Filter> {
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
        _ => Err(ErrorKind::FilterError(
            format!("Unsupported filter: {}", name),
            "filter_from_string",
        ))?,
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
    return Unknown;
}

struct Ascii85Iterator {
    data: Vec<u8>,
    data_cursor: usize,
    last_index: usize,
    buffer: [Option<u8>; 5],
    buffer_cursor: usize,
}

impl Iterator for Ascii85Iterator {
    type Item = [Option<u8>; 5];
    fn next(&mut self) -> Option<[Option<u8>; 5]> {
        loop {
            if self.data_cursor > self.last_index {
                return None;
            };
            let next_char = self.data[self.data_cursor];
            self.data_cursor += 1;
            if self.data_cursor == self.last_index {
                break;
            };

            if is_whitespace(next_char) {
                continue;
            };

            if next_char == b'~'
                && self.data_cursor < self.last_index
                && self.data[self.data_cursor + 1] == b'>'
            {
                return None;
            };

            self.buffer[self.buffer_cursor] = Some(next_char);
            self.buffer_cursor += 1;

            if self.buffer_cursor > 4 {
                debug_assert_eq!(self.buffer_cursor, 5);
                break;
            };
            if next_char == b'z' {
                break;
            }
        }
        let return_value = self.buffer;
        self.buffer = [Option::None; 5];
        self.buffer_cursor = 0;
        return Some(return_value);
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
            buffer_cursor: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flate_example() {
        let mut pdf_file = PdfFileHandler::create_pdf_from_file("data/document.pdf").unwrap();
        //TODO: Example
    }
}
