use std::io::{Cursor, BufRead, Seek, SeekFrom};
use std::collections::HashSet;
use std::cmp;
use std::convert::TryInto;

use crate::errors::*;

const PDF_EOL_MARKERS: [u8; 2] = [b'\n', b'\r'];
const PDF_DELIMITERS: [u8; 17] = [
    b' ', b'\n', b'\r', b'\\', b'\t', b'<', b'>', b'(', b')', b'[', b']', b'{', b'}', b'/', b'%', 0, 12
];



struct PdfFileReader {
    data: Vec<u8>,
    cursor: usize,
    delimiters: HashSet<u8>,
    eol_markers: HashSet<u8>,
}


pub trait PdfFileReaderInterface: Sized {
    /// Return a new reader over the provided file. The reader will read the entire file into memory.
    fn new(path: &str) -> Result<Self>;

    /// Advance the current position by n and return the data (including current position and excluding end position) as a &str.  Any invalid ASCII characters are an error.
    fn get_n(&mut self, n: usize) -> &[u8];
    /// Return the next n characters (including current position) as a &str without advancing current position.  Any invalid ASCII characters are an error.
    fn peek_ahead_n(&self, n: usize) -> &[u8];
    /// Return the preceding n characters (not including current position) as a &str without changing current position.  Any invalid ASCII characters are an error.
    fn peek_behind_n(&self, n: usize) -> &[u8];

    /// Advance to the next PDF standard delimiter and return characters as a &str.
    fn get_until_delimiter(&mut self) -> &[u8];
    /// Advance to the next PDF standard delimiter and return characters from last previous delimiter up to that point.  Returns an empty str if the current position is a delimiter.
    fn get_current_word(&mut self) -> &[u8];
    /// Advance past the next non-delimiter character to the next subsequent delimiter and return characters between teh delimiters.  This method works the same as get_current_word if the current position is not a delimiter.
    fn get_next_word(&mut self) -> &[u8];

    /// Advance until a character that is not in the provided set is reached, and return the characters.  Returns an empty slice if the current position is not in the set.
    fn get_in_charset(&mut self, valid_set: &HashSet<u8>) -> &[u8];
    /// Advance until a character that is in the provided set is reached, and return the characters.  Returns an empty slice if the current position is in the set.
    fn get_until_charset(&mut self, delimiter_set: &HashSet<u8>) -> &[u8];
    
    /// Advance to the first character of the next line and return characters from start of current line.  EOL markers are stripped out.
    fn get_current_line(&mut self) -> &[u8];
    /// Advance to the first character of the next line and return characters from (and including) the current position.  EOL markers are stripped out.
    fn get_rest_of_line(&mut self) -> &[u8];
    /// Return characters from beginning of current line through (but excluding) the current position.  
    fn peek_preceding_part_of_line(&self) -> &[u8];
    /// Return characters in preceding line without changing position.  EOL markers are stripped out.  
    fn peek_preceding_line(&self) -> &[u8];
    /// Return characters in next line without changing position.  EOL markers are stripped out.  
    fn peek_next_line(&self) -> &[u8];
    
}

impl Seek for PdfFileReader {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let last_index = self.data.len() as i64;
        let mut new_pos = match pos {
            SeekFrom::Current(offset) => self.cursor as i64 + offset,
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => last_index + offset
        };
        if new_pos <= 0 { new_pos = 0};
        if new_pos > last_index { new_pos = last_index };
        self.cursor = new_pos.try_into().unwrap(); // Bounds checked above
        Ok(self.cursor as u64)
    }

}

impl PdfFileReaderInterface for PdfFileReader {
    fn new(path: &str) -> Result<Self> {
        Ok(PdfFileReader{
            data: std::fs::read(path)?,
            cursor: 0,
            delimiters: PDF_DELIMITERS.iter().cloned().collect(),
            eol_markers: PDF_EOL_MARKERS.iter().cloned().collect(),
        })
    }
    fn get_n(&mut self, n: usize) -> &[u8] {
        let old_cursor = self.cursor;
        if old_cursor >= self.data.len() { return &[] };
        self.cursor = self.bound_n((self.cursor + n) as i64);
        println!("get_n: {} Slice from: {} to {}", n, old_cursor, self.cursor);
        &self.data[(old_cursor) .. (self.cursor)]
    }
    fn peek_ahead_n(&self, n: usize) -> &[u8] {
        if self.cursor >= self.data.len() { return &[] };
        let end_index = self.bound_n((self.cursor + n) as i64);
        println!("peek_ahead_n: {} Slice from: {} to {}", n, self.cursor, end_index);
        &self.data[self.cursor..end_index]
    }
    fn peek_behind_n(&self, n: usize) -> &[u8] {
        if self.cursor <= 0 { return &[] };
        let start_index = self.bound_n(self.cursor as i64 - n as i64);
        println!("peek_behind_n: {} Slice from: {} to {}", n, start_index, self.cursor);
        &self.data[start_index..self.cursor]
    }
    fn get_until_delimiter(&mut self) -> &[u8] {
        let start_index = self.cursor;
        let last_index = self.data.len();
        while self.cursor < last_index {
            if self.delimiters.contains(&self.data[self.cursor]) { break };
            self.cursor += 1;
        }
        &self.data[start_index..self.cursor]
    }
    fn get_current_word(&mut self) -> &[u8] {
        if self.cursor >= self.data.len()
            || self.delimiters.contains(&self.data[self.cursor]) {
                return &[]
        };
            
        println!("cursor at: {}", self.cursor);
        let mut start_index = self.cursor;
        let last_index = self.data.len();
        while self.cursor < last_index {
            if self.delimiters.contains(&self.data[self.cursor]) { break };
            self.cursor += 1;
        }
        loop {
            if self.delimiters.contains(&self.data[start_index]) { 
                start_index += 1;
                break };
            if start_index == 0 { break };
            start_index -= 1;
        }
        println!("Slice from {} to {}", start_index, self.cursor);
        &self.data[start_index..self.cursor]
    }

    fn get_next_word(&mut self) -> &[u8] {
        if self.cursor >= self.data.len() {
                return &[]
        };
        // Handle case where we are in a word already by delegation
        if !self.delimiters.contains(&self.data[self.cursor]) {
            return self.get_current_word()
        };
        let mut have_seen_word = false;
        let mut start_index = self.cursor;
        let last_index = self.data.len();
        while self.cursor < last_index {
            if !self.delimiters.contains(&self.data[self.cursor]) {
                if !have_seen_word {
                    start_index = self.cursor;
                    have_seen_word = true;
                };
            } else if have_seen_word {
                break
            };
            self.cursor += 1;
        }
        if !have_seen_word { return &[] };
        info!("Slice from {} to {}", start_index, self.cursor);
        &self.data[start_index..self.cursor]
    }

    fn get_in_charset(&mut self, valid_set: &HashSet<u8>) -> &[u8] {
        let start_index = self.cursor;
        let last_index = self.data.len();
        while self.cursor < last_index {
            if !valid_set.contains(&self.data[self.cursor]) { break };
            self.cursor += 1;
        }
        &self.data[start_index..self.cursor]
    }
    fn get_until_charset(&mut self, delimiter_set: &HashSet<u8>) -> &[u8] {
        let start_index = self.cursor;
        let last_index = self.data.len();
        while self.cursor < last_index {
            if delimiter_set.contains(&self.data[self.cursor]) { break };
            self.cursor += 1;
        }
        &self.data[start_index..self.cursor]
    }
    fn get_current_line(&mut self) -> &[u8] {
            if self.cursor >= self.data.len() {
                return &[]
        };
            
        info!("cursor at: {}", self.cursor);
        let mut start_index = self.cursor;
        let last_index = self.data.len();
        while self.cursor < last_index {
            if self.delimiters.contains(&self.data[self.cursor]) { break };
            self.cursor += 1;
        }
        loop {
            if self.delimiters.contains(&self.data[start_index]) { 
                start_index += 1;
                break };
            if start_index == 0 { break };
            start_index -= 1;
        }
        println!("Slice from {} to {}", start_index, self.cursor);
        &self.data[start_index..self.cursor]
    }
    fn get_rest_of_line(&mut self) -> &[u8]  {
        &self.data[..1]
    }
    fn peek_preceding_part_of_line(&self) -> &[u8]  {
        &self.data[..1]
    }
    fn peek_preceding_line(&self) -> &[u8]  {
        &self.data[..1]
    }
    fn peek_next_line(&self) -> &[u8] {
        &self.data[..1]
    }
}

impl PdfFileReader {
    fn bound_n(&self, n: i64) -> usize {
        let last_index = self.data.len() as i64;  // Allows cursor to hang over by 1
        if n < 0 { return 0 };
        if n > last_index { return last_index as usize };
        n as usize
    }

    pub fn position(&self) -> usize {
        self.cursor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn get_test_data() -> Vec<u8> {
        vec!(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14)
    }

    fn get_word_test() -> Vec<u8> {
        Vec::from(
            "\nAa..\rBb.. Cc..".to_string()
        )
    }

    fn get_line_test() -> Vec<u8> {
        Vec::from(
            "\nAa..\rBb.. Cc..\r\nDd..\r\rEe..".to_string()
        )
    }

    fn get_reader(data: &Vec<u8>) -> PdfFileReader {
        PdfFileReader{
            data: data.clone(),
            cursor: 0,
            delimiters: PDF_DELIMITERS.iter().cloned().collect(),
            eol_markers: PDF_EOL_MARKERS.iter().cloned().collect(),
        } 
    }

    #[test]
    fn test_seek() {
        let test_data = get_test_data();
        let mut reader = get_reader(&test_data);
        let data_len = test_data.len();
        assert_eq!(reader.position(), 0);
        for i in 0..(data_len + 1) {
            assert_eq!(reader.position(), i);
            reader.seek(SeekFrom::Current(1)).unwrap();
        }
        reader.seek(SeekFrom::Current(100)).unwrap();
        assert_eq!(reader.position(), data_len);
        for i in 0..(data_len + 1) {
            reader.seek(SeekFrom::Start(i as u64)).unwrap();
            assert_eq!(reader.position(), i);
            reader.seek(SeekFrom::Start(i as u64)).unwrap();
            assert_eq!(reader.position(), i);
        }
        reader.seek(SeekFrom::Start(data_len as u64 + 100)).unwrap();
        assert_eq!(reader.position(), data_len);
        for i in 0..(data_len as i64 + 1) {
            println!("{}", i);
            reader.seek(SeekFrom::End(-1 * i)).unwrap();
            assert_eq!(reader.position(), data_len - i as usize);
            reader.seek(SeekFrom::End(-1 * i)).unwrap();
            assert_eq!(reader.position(), data_len - i as usize);
        }
        reader.seek(SeekFrom::End(-1 * (data_len as i64 + 100))).unwrap();
        assert_eq!(reader.position(), 0);
    }

    #[test]
    fn test_get_n() {
        let test_data = get_test_data();
        let mut reader = get_reader(&test_data);
        assert_eq!(reader.get_n(14), &test_data[..14]);
        assert_eq!(reader.position(), 14);
        assert_eq!(reader.get_n(1), &test_data[14..]);
        assert_eq!(reader.position(), 15);

        reader.seek(SeekFrom::Start(0));
        assert_eq!(reader.get_n(0), &[]);
        assert_eq!(reader.position(), 0);
        assert_eq!(reader.get_n(100), &test_data[..]);
        assert_eq!(reader.position(), 15);
        assert_eq!(reader.get_n(100), &[]);
        assert_eq!(reader.position(), 15);

        reader.seek(SeekFrom::Start(0));
        assert_eq!(reader.get_n(7), &test_data[..7]);
        assert_eq!(reader.position(), 7);
        assert_eq!(reader.get_n(8), &test_data[7..]);
        assert_eq!(reader.position(), 15);
    }

    #[test]
    fn test_peek_ahead_n() {
        let test_data = get_test_data();
        let mut reader = get_reader(&test_data);
        let data_len = test_data.len();
        assert_eq!(reader.position(), 0);
        for i in 0..(data_len + 1) {
            for j in i..(data_len + 1) {
                reader.seek(SeekFrom::Start(i as u64)).unwrap();
                assert_eq!(reader.peek_ahead_n(j - i), &test_data[i..j]);
                assert_eq!(reader.position(), i);
            }
        }
        reader.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(reader.peek_ahead_n(100), &test_data[..]);
        assert_eq!(reader.position(), 0);
        assert_eq!(reader.peek_ahead_n(0), &[]);
        assert_eq!(reader.position(), 0);
    }

    #[test]
    fn test_peek_behind_n() {
        let test_data = get_test_data();
        let mut reader = get_reader(&test_data);
        assert_eq!(reader.peek_behind_n(0), &[]);
        assert_eq!(reader.position(), 0);
        assert_eq!(reader.peek_behind_n(100), &[]);
        assert_eq!(reader.position(), 0);
        
        reader.get_n(100);
        assert_eq!(reader.position(), 15);

        assert_eq!(reader.peek_behind_n(0), &[]);
        assert_eq!(reader.position(), 15);
        assert_eq!(reader.peek_behind_n(100), &test_data[..]);
        assert_eq!(reader.position(), 15);
        assert_eq!(reader.peek_behind_n(1), &test_data[14..]);
        assert_eq!(reader.position(), 15);
        assert_eq!(reader.peek_behind_n(2), &test_data[13..]);
        assert_eq!(reader.position(), 15);
        assert_eq!(reader.peek_behind_n(7), &test_data[8..]);
        assert_eq!(reader.position(), 15);
        assert_eq!(reader.peek_behind_n(8), &test_data[7..]);
        assert_eq!(reader.position(), 15);
    }

    #[test]
    fn test_get_until_charset() {
        let test_data = get_test_data();
        let mut reader = get_reader(&test_data);
        let mut delimiter = HashSet::new();

        delimiter.insert(test_data[0]);
        delimiter.insert(test_data[10]);
        assert!(!test_data.contains(&20));  // Intended to be a delimiter not in the data
        delimiter.insert(20);

        assert_eq!(reader.get_until_charset(&delimiter), &[]);
        assert_eq!(reader.position(), 0);
        assert_eq!(reader.get_until_charset(&delimiter), &[]);
        assert_eq!(reader.position(), 0);

        assert_eq!(reader.get_n(1), &test_data[0..1]);
        assert_eq!(reader.get_until_charset(&delimiter), &test_data[1..10]);
        assert_eq!(reader.position(), 10);
        assert_eq!(reader.get_until_charset(&delimiter), &[]);
        assert_eq!(reader.position(), 10);
        assert_eq!(reader.get_until_charset(&delimiter), &[]);
        assert_eq!(reader.position(), 10);

        assert_eq!(reader.get_n(1), &test_data[10..11]);
        assert_eq!(reader.get_until_charset(&delimiter), &test_data[11..]);
        assert_eq!(reader.position(), 15);
        assert_eq!(reader.get_until_charset(&delimiter), &[]);
        assert_eq!(reader.position(), 15);
        assert_eq!(reader.get_until_charset(&delimiter), &[]);
        assert_eq!(reader.position(), 15);
    }

    #[test]
    fn test_get_in_charset() {
        let test_data = get_test_data();
        let mut reader = get_reader(&test_data);
        let mut charset: HashSet<u8> = (0..100).into_iter().collect();
        charset.remove(&test_data[0]);
        charset.remove(&test_data[10]);
        assert_eq!(reader.get_in_charset(&charset), &[]);
        assert_eq!(reader.position(), 0);
        assert_eq!(reader.get_in_charset(&charset), &[]);
        assert_eq!(reader.position(), 0);
        assert_eq!(reader.get_n(1), &test_data[0..1]);
        assert_eq!(reader.get_in_charset(&charset), &test_data[1..10]);
        assert_eq!(reader.position(), 10);
        assert_eq!(reader.get_in_charset(&charset), &[]);
        assert_eq!(reader.position(), 10);
        assert_eq!(reader.get_in_charset(&charset), &[]);
        assert_eq!(reader.position(), 10);
        assert_eq!(reader.get_n(1), &test_data[10..11]);
        assert_eq!(reader.get_in_charset(&charset), &test_data[11..]);
        assert_eq!(reader.position(), 15);
        assert_eq!(reader.get_in_charset(&charset), &[]);
        assert_eq!(reader.position(), 15);
        assert_eq!(reader.get_in_charset(&charset), &[]);
        assert_eq!(reader.position(), 15);
    }

    #[test]
    fn test_get_until_delimiters() {
        let test_data = get_test_data();
        let mut reader = get_reader(&test_data);
        assert_eq!(reader.get_until_delimiter(), &[]);
        assert_eq!(reader.position(), 0);
        assert_eq!(reader.get_until_delimiter(), &[]);
        assert_eq!(reader.position(), 0);
        reader.seek(SeekFrom::Current(1)).unwrap();
        assert_eq!(reader.get_until_delimiter(), &test_data[1..(b'\t' as usize)]); // = 9
        assert_eq!(reader.position(), (b'\t' as usize));
        assert_eq!(reader.get_until_delimiter(), &[]);
        assert_eq!(reader.position(), (b'\t' as usize));
        reader.seek(SeekFrom::Current(-2)).unwrap();
        assert_eq!(reader.get_until_delimiter(), &test_data[(b'\t' as usize - 2)..(b'\t' as usize)]);
        assert_eq!(reader.position(), (b'\t' as usize));
        reader.seek(SeekFrom::Current(1)).unwrap();
        assert_eq!(reader.get_until_delimiter(), &test_data[(b'\t' as usize + 1)..(b'\n' as usize)]); // = 10
        assert_eq!(reader.position(), (b'\n' as usize));
        assert_eq!(reader.get_until_delimiter(), &[]);
        assert_eq!(reader.position(), (b'\n' as usize));
        reader.seek(SeekFrom::Current(1)).unwrap();
        assert_eq!(reader.get_until_delimiter(), &test_data[(b'\n' as usize + 1)..12]); // form feed
        assert_eq!(reader.position(), 12);
        assert_eq!(reader.get_until_delimiter(), &[]);
        assert_eq!(reader.position(), 12);
        reader.seek(SeekFrom::Current(1)).unwrap();
        assert_eq!(reader.get_until_delimiter(), &test_data[13..(b'\r' as usize)]); // 13
        assert_eq!(reader.position(), b'\r' as usize);
        assert_eq!(reader.get_until_delimiter(), &[]);
        assert_eq!(reader.position(), b'\r' as usize);
    }

    #[test]
    fn test_get_current_word() {
        let test_data = get_word_test();
        let first_word = Vec::from("Aa..".to_string());
        let second_word = Vec::from("Bb..".to_string());
        let third_word = Vec::from("Cc..".to_string());
        let mut reader = get_reader(&test_data);
        assert_eq!(reader.get_current_word(), &[]);
        assert_eq!(reader.position(), 0);

        reader.seek(SeekFrom::Current(1)).unwrap();
        assert_eq!(reader.get_current_word(), &first_word[..]);
        assert_eq!(reader.position(), 5);

        reader.seek(SeekFrom::Current(-1)).unwrap();
        assert_eq!(reader.get_current_word(), &first_word[..]);
        assert_eq!(reader.position(), 5);
        assert_eq!(reader.get_current_word(), &[]);
        assert_eq!(reader.position(), 5);

        reader.seek(SeekFrom::Current(1)).unwrap();
        assert_eq!(reader.get_current_word(), &second_word[..]);
        assert_eq!(reader.position(), 10);
        
        reader.seek(SeekFrom::Current(-1)).unwrap();
        assert_eq!(reader.get_current_word(), &second_word[..]);
        assert_eq!(reader.position(), 10);
        assert_eq!(reader.get_current_word(), &[]);
        assert_eq!(reader.position(), 10);

        reader.seek(SeekFrom::Current(1)).unwrap();
        assert_eq!(reader.get_current_word(), &third_word[..]);
        assert_eq!(reader.position(), 15);
        
        reader.seek(SeekFrom::Current(-1)).unwrap();
        assert_eq!(reader.get_current_word(), &third_word[..]);
        assert_eq!(reader.position(), 15);
        assert_eq!(reader.get_current_word(), &[]);
        assert_eq!(reader.position(), 15);
    }

    #[test]
    fn test_get_next_word() {
        let test_data = get_word_test();
        let first_word = Vec::from("Aa..".to_string());
        let second_word = Vec::from("Bb..".to_string());
        let third_word = Vec::from("Cc..".to_string());
        let mut reader = get_reader(&test_data);
        assert_eq!(reader.get_next_word(), &first_word[..]);
        assert_eq!(reader.position(), 5);

        reader.seek(SeekFrom::Current(-1)).unwrap();
        assert_eq!(reader.get_next_word(), &first_word[..]);
        assert_eq!(reader.position(), 5);
        assert_eq!(reader.get_next_word(), &second_word[..]);
        assert_eq!(reader.position(), 10);
        
        reader.seek(SeekFrom::Current(-1)).unwrap();
        assert_eq!(reader.get_next_word(), &second_word[..]);
        assert_eq!(reader.position(), 10);
        assert_eq!(reader.get_next_word(), &third_word[..]);
        assert_eq!(reader.position(), 15);
        
        reader.seek(SeekFrom::Current(-1)).unwrap();
        assert_eq!(reader.get_next_word(), &third_word[..]);
        assert_eq!(reader.position(), 15);
        assert_eq!(reader.get_next_word(), &[]);
        assert_eq!(reader.position(), 15);
    }

    fn iteration_test() {
        let reader = PdfFileReader::new("data/simple_pdf.pdf").unwrap();
        //let lines = reader.get_ref().lines();
        //println!("{}", lines.next_back());

    }
}