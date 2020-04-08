use std::io::Read;
use std::fmt::Display;

use flate2;

use super::*;
use crate::errors::*;
use crate::doc_tree::pdf_objects::PdfObjectInterface;

#[derive(Debug)]
pub struct PdfContentStream {
    attributes: PdfMap,
    data: String
}

impl Display for PdfContentStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Attributes: {:#?}, Content: {}", self.attributes, self.data)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct PdfBinaryStream {
    pub attributes: PdfMap,
    pub data: Rc<Vec<u8>>
}

impl Display for PdfBinaryStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Attributes: {:#?}, Content length: {}", self.attributes, self.data.len())?;
        Ok(())
    }
}

impl PdfContentStream {
    pub fn new(data: Vec<u8>, attributes: PdfMap) -> Self {
        PdfContentStream{
            data: to_ascii(data), attributes
        }
    }
}

#[derive(Debug, Clone)]
enum Filter {
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

fn to_ascii(data: Vec<u8>) -> String {
    data.iter().map(|i| *i as char).collect()

}

impl Filter {
    pub fn apply(self, data: Result<Vec<u8>>) -> Result<Vec<u8>> {
        use Filter::*;
        if data.is_err() {
            return Err(data.unwrap_err());
        };
        
        // if let Ok(ref v) = data {
        //     println!("Data length as vec: {}", v.len());
        //     let data_string_start = v.iter().take(15).map(|n| *n as char).collect::<String>();
        //     let data_string_end = v.iter().rev().take(15).map(|n| *n as char).collect::<Vec<_>>().iter().rev().collect::<String>();
        //     println!("data start: {}\ndata end: {}", data_string_start, data_string_end);
            

        // };
        let data = data.unwrap();
        let output_data = match self {
            ASCIIHex => Filter::apply_ascii_hex(data),
            ASCII85 => Filter::apply_ascii_85(data),
            LZW(params) => Filter::apply_lzw(data, params),
            Flate(params) => Filter::apply_flate(data, params),
            _ => Err(ErrorKind::FilterError(
                format!("Unsupported filter: {}", self),
                "Filter.apply",
            ))?,
        };
        //println!("output data_success: {:?}", !output_data.is_err());
        let output_data_clone = output_data.unwrap();
        //fs::write("decoded_stream.txt", to_ascii(output_data_clone.clone()));
        Ok(output_data_clone)
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
        let vec: Vec<u8> = arr.iter()
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

    fn apply_lzw(data: Vec<u8>, _params: Option<SharedObject>) -> Result<Vec<u8>> {
        Ok(data)
    }

    fn apply_flate(data: Vec<u8>, params: Option<SharedObject>) -> Result<Vec<u8>> {
        let mut flate_decoder = flate2::read::ZlibDecoder::new(&*data);
        let mut output = Vec::new();
        let flate_result = flate_decoder.read_to_end(&mut output);
        let decode_result = match params {
            None => output,
            Some(map) => _apply_predictors(map, output)?
        };
        match flate_result {
            Ok(_) => Ok(decode_result),
            Err(e) => Err(ErrorKind::FilterError(
                format!("Error applying flate filter: {:?}", e),
                "apply:apply_flate",
            ))?,
        }
    }
}

fn _apply_predictors(shared_map: SharedObject, data: Vec<u8>) -> Result<Vec<u8>> {
    let map = shared_map.try_into_map()?;
    let algorithm = match map.get("Predictor") {
        None => { return Ok(data) }, // Technically an error
        Some(obj) => obj.try_into_int()?
    };
    let output_data = match algorithm {
        12 => png_predictor(&data,
                            map.get("Columns").unwrap().try_into_int()? as usize,
                            Some(PngAlgorithm::Up)),
        _ => data
    };
    Ok(output_data)

}

pub fn parse_stream(map: PdfMap, bytes: &[u8], weak_ref: Weak<ObjectCache>) -> Result<PdfObject> {
    //Check size
    let expected_byte_length = map
        .get("Length")
        .ok_or(ErrorKind::ParsingError(format!(
            "Missing Length in {:?}",
            map
        )))?
        .try_into_int()? as usize;
    assert_eq!(bytes.len(), expected_byte_length);
    //println!("expected byte length: {}, actual: {}", expected_byte_length, bytes.len());
    //println!("first bytes {:?}", &bytes[..2]);
    //println!("last bytes: {:?}", &bytes[(expected_byte_length - 2)..]);
    //std::fs::write("compressed_file.gz", bytes);

    // Classify stream
    let type_and_subtype = (map.get("Type"), map.get("Subtype"));
    let stream_type = determine_stream_type(type_and_subtype);
    match stream_type {
        StreamType::Image => 
            Ok(PdfObject::new_binary_stream(PdfBinaryStream{
                                attributes: map, data: Rc::new(Vec::from(bytes))
                            })),
        StreamType::XRef => {
            let data = Rc::new(decode_stream(&map, bytes)?);
            Ok(PdfObject::new_binary_stream(PdfBinaryStream{ attributes: map, data }))
        },
        StreamType::Object => {
            let data = decode_stream(&map, bytes)?;
            PdfObject::new_object_stream(map, data, weak_ref)
        },
        StreamType::Content => {
            let data = decode_stream(&map, bytes)?;
            //println!("{}", to_ascii(data.clone()));
            Ok(PdfObject::new_content_stream(data, map))
        }
        _ => Err(TestingError(format!("{:?} not implemented", stream_type)))?
    }
}

fn decode_stream(stream_dict: &PdfMap, bytes: &[u8]) -> Result<Vec<u8>> {
    let params = stream_dict.get("DecodeParms");
    let filter_object_array = match stream_dict.get("Filter") {
        None => Vec::new(),
        Some(obj) if obj.is_string() => vec![Rc::new(obj.as_ref().clone())],
        Some(obj) if obj.is_array() => (*obj.try_into_array().unwrap()).to_owned(),
        Some(obj) => Err(ErrorKind::FilterError(
            format!("Non-name item in Filter array: {:?}", obj),
            "decode stream",
        ))?,
    }
    .into_iter()
    .collect::<Vec<SharedObject>>();
    let filter_array = filter_object_array
        .into_iter()
        .enumerate()
        // Collect matching params without throwing error if no filters need params
        .map(|(index, s)| {
            filter_from_string_and_params(
                s.try_into_string()?.as_ref(),
                params.as_ref()
                      .map(|arr| {
                          if arr.is_array() {
                              arr.try_to_index(index).unwrap()
                            } else {Rc::clone(arr)}
                      }))
        })
        .collect::<Result<Vec<decode::Filter>>>()?;
    //println!("Filters to apply: {:?}", filter_array);
    //println!("Data length: {}", bytes.len());
        let filtered_data = filter_array
        .into_iter()
        .fold(Ok(Vec::from(bytes)), |data, filter| filter.apply(data))?;
    //let data_string: String = filtered_data.iter().map(|v| *v as char).collect();

    // for i in 0..20 {
    //     let s = &filtered_data[5*i..(5*i + 5)];
    //     let ix:u64 = s[1] as u64 * 256 * 256 + s[2] as u64 * 256 + s[3] as u64;
    //     println!("{} {} {}", s[0], ix, s[4]);
    // }

    //println!("data length: {}", filtered_data.len());
    Ok(filtered_data)
}

fn filter_from_string_and_params<T: AsRef<str> + Display>(name: T, params: Option<Rc<PdfObject>>) -> Result<Filter> {
    use Filter::*;
    match name.as_ref() {
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

fn determine_stream_type(tup: (Option<&Rc<PdfObject>>, Option<&Rc<PdfObject>>)) -> StreamType {
    use StreamType::*;
    if let Some(object) = tup.0 {
        match object.try_into_string() {
            Ok(s) if *s == "ObjStm" => return Object,
            Ok(s) if *s == "XRef" => return XRef,
            Ok(s) if *s == "Metadata" => return Metadata,
            _ => {}
        }
    };
    if let Some(object) = tup.1 {
        match object.try_into_string() {
            Ok(s) if *s == "Image" => return Image,
            _ => {}
        }
    };
    if let None = tup.0 {
        if let None = tup.1 {
            return Content 
        };
    };
    //println!("{:?}", tup);
    return Unknown
}

enum PngAlgorithm {
    Sub,
    Up,
    Paeth,
    None,
    Average

}

fn png_predictor(data: &Vec<u8>, line_length: usize, _expected_algorithm: Option<PngAlgorithm>) -> Vec<u8> {
    let data_length = data.len();
    //println!("data length: {}, line length: {}", data_length, line_length);
    assert_eq!(data_length % (line_length + 1), 0);
    let lines = data_length / (line_length + 1);
    
    let mut new_data = Vec::from(&data[1..(line_length + 1)]);
    new_data.reserve(line_length * (lines - 1));
    for line_ix in 1..lines {
        let _filter_byte = data[line_ix * line_length];
        
        for col_ix in 0..line_length {
            let index = line_ix * (line_length + 1) + col_ix + 1;
            let prior_line_index = (line_ix - 1) * line_length + col_ix;
            let new_val = data[index] as u16 + new_data[prior_line_index] as u16;
            new_data.push(new_val as u8);
        }        
    }
    new_data



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
        let _pdf_file = Parser::create_pdf_from_file("data/document.pdf").unwrap();
        //TODO: Example
    }
}
