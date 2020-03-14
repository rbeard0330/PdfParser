use std::io::Read;

use flate2;

use super::*;

pub enum Filter{
    ASCIIHex,
    ASCII85,
    LZW(Rc<PDFObj>),
    Flate(Rc<PDFObj>),
    RunLength,
    CCITTFax(Rc<PDFObj>),
    JBIG2(Rc<PDFObj>),
    DCT(Rc<PDFObj>),
    JPX,
    Crypt(Rc<PDFObj>)
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
        Ok(data)
    }

    fn apply_ascii_85(data: Vec<u8>) -> Result<Vec<u8>, PDFError> {
        Ok(data)
    }

    fn apply_LZW(data: Vec<u8>, params: Rc<PDFObj>) -> Result<Vec<u8>, PDFError> {
        Ok(data)
    }

    fn apply_flate(data: Vec<u8>, params: Rc<PDFObj>) -> Result<Vec<u8>, PDFError> {
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


pub fn filter_from_string_and_params(name: &str, params: Result<Rc<PDFObj>, PDFError>) -> Result<Filter, PDFError> {
    use Filter::*;
    match name {
        "ASCIIHexDecode" => Ok(ASCIIHex),
        "ASCII85Decode" => Ok(ASCII85),
        "JPXDecode" => Ok(JPX),
        "RunLengthDecode" => Ok(RunLength),
        "LZWDecode" => Ok(LZW(params?)),
        "FlateDecode" => Ok(Flate(params?)),
        "CCITTFaxDecode" => Ok(CCITTFax(params?)),
        "JBIG2Decode" => Ok(JBIG2(params?)),
        "DCTDecode" => Ok(DCT(params?)),
        "Crypt" => Ok(Crypt(params?)),
        _ => Err(PDFError{message: format!("Unsupported filter: {}", name), function: "filter_from_string"})
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