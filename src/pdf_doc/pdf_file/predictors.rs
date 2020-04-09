pub fn png_up(data: &Vec<u8>, line_length: usize) -> Vec<u8> {
    let data_length = data.len();
    //println!("data length: {}, line length: {}", data_length, line_length);
    //assert_eq!(data_length % line_length, 0);
    // copy first line
    let mut new_data = Vec::from(&data[..line_length]);
    new_data.reserve(data_length - line_length);
    for index in line_length..data_length {
        let prior_line_index = index - line_length;
        new_data.push(data[index].wrapping_add(data[prior_line_index]));
    }
    //    new_data
    data.clone()



}