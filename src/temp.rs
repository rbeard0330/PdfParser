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