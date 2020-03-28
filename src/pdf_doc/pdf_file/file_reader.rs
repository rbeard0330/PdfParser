use std::io::{Seek, SeekFrom};
use std::collections::HashSet;
use std::convert::TryInto;
use std::ops::{Index, Range, RangeTo, RangeFrom, RangeFull};

use crate::errors::*;

const PDF_EOL_MARKERS: [u8; 2] = [b'\n', b'\r'];
const PDF_DELIMITERS: [u8; 17] = [
    b' ', b'\n', b'\r', b'\\', b'\t', b'<', b'>', b'(', b')', b'[', b']', b'{', b'}', b'/', b'%', 0, 12
];



pub struct PdfFileReader {
    data: Vec<u8>,
    cursor: usize,
    delimiters: HashSet<u8>,
    eol_markers: HashSet<u8>,
}


pub trait PdfFileReaderInterface: Index<Range<usize>> + Sized {
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

impl Index<usize> for PdfFileReader {
    type Output = u8;

    fn index(&self, ix: usize) -> &Self::Output {
        &self.data[ix]
    }
}

impl Index<Range<usize>> for PdfFileReader {
    type Output = [u8];

    fn index(&self, ix: Range<usize>) -> &Self::Output {
        &self.data[ix]
    }
}

impl Index<RangeTo<usize>> for PdfFileReader {
    type Output = [u8];

    fn index(&self, ix: RangeTo<usize>) -> &Self::Output {
        &self.data[ix]
    }
}

impl Index<RangeFrom<usize>> for PdfFileReader {
    type Output = [u8];

    fn index(&self, ix: RangeFrom<usize>) -> &Self::Output {
        &self.data[ix]
    }
}

impl Index<RangeFull> for PdfFileReader {
    type Output = [u8];

    fn index(&self, ix: RangeFull) -> &Self::Output {
        &self.data[ix]
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
        if old_cursor >= self.len() { return &[] };
        self.cursor = self.bound_n((self.cursor + n) as i64);
        println!("get_n: {} Slice from: {} to {}", n, old_cursor, self.cursor);
        &self[(old_cursor) .. (self.cursor)]
    }
    fn peek_ahead_n(&self, n: usize) -> &[u8] {
        if self.cursor >= self.len() { return &[] };
        let end_index = self.bound_n((self.cursor + n) as i64);
        println!("peek_ahead_n: {} Slice from: {} to {}", n, self.cursor, end_index);
        &self[self.cursor..end_index]
    }
    fn peek_behind_n(&self, n: usize) -> &[u8] {
        if self.cursor <= 0 { return &[] };
        let start_index = self.bound_n(self.cursor as i64 - n as i64);
        println!("peek_behind_n: {} Slice from: {} to {}", n, start_index, self.cursor);
        &self[start_index..self.cursor]
    }
    fn get_until_delimiter(&mut self) -> &[u8] {
        let start_index = self.cursor;
        while self.cursor < self.len() {
            if self.is_on_delimiter() { break };
            self.cursor += 1;
        }
        &self[start_index..self.cursor]
    }
    fn get_current_word(&mut self) -> &[u8] {
        if self.cursor >= self.len()
            || self.is_on_delimiter() {
                return &[]
        };
            
        println!("cursor at: {}", self.cursor);
        let mut start_index = self.cursor;
        while self.cursor < self.len() {
            if self.is_on_delimiter() { break };
            self.cursor += 1;
        }
        loop {
            if self.delimiters.contains(&self[start_index]) { 
                start_index += 1;
                break };
            if start_index == 0 { break };
            start_index -= 1;
        }
        println!("get_current_word: Slice from {} to {}", start_index, self.cursor);
        &self[start_index..self.cursor]
    }

    fn get_next_word(&mut self) -> &[u8] {
        if self.cursor >= self.len() {
                return &[]
        };
        // Handle case where we are in a word already by delegation
        if !self.is_on_delimiter() {
            return self.get_current_word()
        };
        let mut have_seen_word = false;
        let mut start_index = self.cursor;
        let last_index = self.data.len();
        while self.cursor < last_index {
            if !self.is_on_delimiter() {
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
        info!("get_next_word: Slice from {} to {}", start_index, self.cursor);
        &self[start_index..self.cursor]
    }

    fn get_in_charset(&mut self, valid_set: &HashSet<u8>) -> &[u8] {
        let start_index = self.cursor;
        while self.cursor < self.len() {
            if !valid_set.contains(&self[self.cursor]) { break };
            self.cursor += 1;
        }
        &self[start_index..self.cursor]
    }
    fn get_until_charset(&mut self, delimiter_set: &HashSet<u8>) -> &[u8] {
        let start_index = self.cursor;
        while self.cursor < self.len() {
            if delimiter_set.contains(&self[self.cursor]) { break };
            self.cursor += 1;
        }
        &self[start_index..self.cursor]
    }
    fn get_current_line(&mut self) -> &[u8] {
        if self.cursor >= self.len() {
            return &[]
        };
        let (start_index, end_index) = self.get_line_bounds_around_index(self.cursor);
        if end_index == self.len() {self.cursor = end_index; } else {
            self.cursor = self.get_index_after_line_break(end_index);
        };   
        println!("get_current_line: Slice from {} to {}, cursor at {}", start_index, end_index, self.cursor);
        &self[start_index..end_index]
    }

    fn get_rest_of_line(&mut self) -> &[u8]  {
        if self.cursor >= self.len() {
            return &[]
        };
        let (_start_index, end_index) = self.get_line_bounds_around_index(self.cursor);
        let mut start_index = self.cursor;
        self.cursor = end_index;
        if end_index == self.len() {self.cursor = end_index; } else {
            self.cursor = self.get_index_after_line_break(end_index);
        };
        if start_index > end_index { start_index = end_index; };
        println!("get_rest_of_line: Slice from {} to {}", start_index, end_index);
        &self[start_index..end_index]
    }
    fn peek_preceding_part_of_line(&self) -> &[u8]  {
        let mut end_index = self.cursor;
        if end_index >= self.len() {
            debug_assert!(end_index == self.len());
            end_index -= 1;
        };
        let (start_index, line_end) = self.get_line_bounds_around_index(end_index);
        if end_index > line_end { end_index = line_end; };
        //capture last character if not eol
        if self.cursor == self.len() && !self.eol_at(self.cursor - 1) { end_index += 1 };
        println!("peek_preceding_part_of_line: Slice from {} to {}", start_index, end_index);
        &self[start_index..end_index]
    }
    fn peek_preceding_line(&self) -> &[u8]  {
        if self.cursor < 2 { return &[] };
        let (start_index, end_index) = match self.len() - self.cursor {
            0 => {
                self.get_line_bounds_around_index(self.cursor - 1)
            },
            _ => {
                let (line_start, _line_end) = self.get_line_bounds_around_index(self.cursor);
                if line_start == 0 { return &[] };
                self.get_line_bounds_around_index(line_start - 1)
            }
        };
        println!("peek_next_line: Slice from {} to {}", start_index, end_index);
        &self.data[start_index..end_index]
    }
    fn peek_next_line(&self) -> &[u8] {
        if self.cursor >= self.len() { return &[] };
        let (_line_start, line_end) = self.get_line_bounds_around_index(self.cursor);
        if line_end >= self.len() { return &[] };
        let next_line_start = self.get_index_after_line_break(line_end);
        let (start_index, end_index) = self.get_line_bounds_around_index(next_line_start);
        debug_assert_eq!(next_line_start, start_index);
        println!("peek_next_line: Slice from {} to {}", start_index, end_index);
        &self.data[start_index..end_index]
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

    fn is_on_delimiter(&self) -> bool {
        self.delimiters.contains(&self.data[self.cursor])
    }

    fn is_on_eol(&self) -> bool {
        if self.cursor >= self.len() { return true }
        self.eol_markers.contains(&self.data[self.cursor])
    }

    fn eol_at(&self, index: usize) -> bool {
        self.eol_markers.contains(&self.data[index])
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }
    
    fn get_line_bounds_around_index(&self, index: usize) -> (usize, usize) {
        // if on line break, step back to body of line
        let mut end_index = index;
        if self.eol_at(index) {
            if index == 0 { return (0, 0) }; // null line
            if self[index] == b'\n'
                && index > 0
                && self[index - 1] == b'\r' {
                    end_index -= 2;
            } else { end_index -= 1 };
            if self.eol_at(end_index) {return (end_index + 1, end_index + 1)}; // null line
        }
        let mut start_index = end_index; // = index as adjusted for eol issues above

        while end_index < self.len() {
            if self.eol_at(end_index) {
                break
            };
            end_index += 1;
        }

        loop {
            if self.eol_at(start_index) { 
                start_index += 1;
                break
            };
            if start_index == 0 { break };
            start_index -= 1;
        }

        (start_index, end_index)
    }

    fn get_index_after_line_break(&self, index: usize) -> usize {
        if !self.eol_at(index) || index >= self.len() { return index };
        // Increment by 2 in instance where current value is \r and next is \n; else increment by 1
        if self[index] == b'\r' && index + 1 < self.len() && self[index + 1] == b'\n' { return index + 2};
        return index + 1
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

        reader.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(reader.get_n(0), &[]);
        assert_eq!(reader.position(), 0);
        assert_eq!(reader.get_n(100), &test_data[..]);
        assert_eq!(reader.position(), 15);
        assert_eq!(reader.get_n(100), &[]);
        assert_eq!(reader.position(), 15);

        reader.seek(SeekFrom::Start(0)).unwrap();
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

    fn get_line_test() -> Vec<u8> {
        Vec::from(
            "\nAa..\rBb.. Cc..\r\nDd..\r\rEe..".to_string()
        )
    }

    #[test]
    fn test_get_current_line() {
        let test_data = get_line_test();

        let first_line = Vec::from("Aa.."); // starts at 1, ends at 5
        let second_line = Vec::from("Bb.. Cc.."); // starts at 6, ends at 16
        let third_line = Vec::from("Dd.."); // starts at 17, ends at 21
        let fourth_line = Vec::from("Ee.."); // starts at 24, ends at 26

        let mut reader = get_reader(&test_data);

        for ix in 0..test_data.len() + 1 {
            reader.seek(SeekFrom::Start(ix as u64)).unwrap();
            let (target_slice, target_ix) = match ix {
                0 => (&first_line[0..0], 1),
                1 ..= 5 => (&first_line[..], 6),
                6 ..= 16 => (&second_line[..], 17),
                17 ..= 21 => (&third_line[..], 22),
                22 => (&first_line[0..0], 23),
                23 ..= 26 => (&fourth_line[..], 27),
                _ => (&first_line[0..0], 27)
            };
            assert_eq!(reader.get_current_line(), target_slice);
            println!("{}", reader.position());
            assert_eq!(reader.position(), target_ix);
        }
    }

    #[test]
    fn test_get_rest_of_line() {
        let test_data = get_line_test();

        let first_line = Vec::from("Aa.."); // starts at 1, ends at 5
        let second_line = Vec::from("Bb.. Cc.."); // starts at 6, ends at 16
        let third_line = Vec::from("Dd.."); // starts at 17, ends at 21
        let fourth_line = Vec::from("Ee.."); // starts at 24, ends at 26

        let mut reader = get_reader(&test_data);

        for ix in 0..test_data.len() + 1 {
            reader.seek(SeekFrom::Start(ix as u64)).unwrap();
            let (target_slice, target_ix) = match ix {
                0 => (&first_line[0..0], 1),
                1 ..= 5 => (&first_line[(ix - 1)..], 6),
                6 ..= 16 => {
                    let mut start_index = ix - 6;
                    // Need to truncate because 2 eol characters are not in the return slice
                    if start_index > 9 {start_index = 9}; 
                    (&second_line[start_index..], 17)
                },
                17 ..= 21 => (&third_line[(ix - 17)..], 22),
                22 => (&first_line[0..0], 23),
                23 ..= 26 => (&fourth_line[(ix - 23)..], 27),
                _ => (&first_line[0..0], 27)
            };
            println!("testing index: {}", ix);
            assert_eq!(reader.get_rest_of_line(), target_slice);
            assert_eq!(reader.position(), target_ix);
        }
    }

    #[test]
    fn test_peek_preceding_part_of_line() {
        let test_data = get_line_test();

        let first_line = Vec::from("Aa.."); // starts at 1, ends at 5
        let second_line = Vec::from("Bb.. Cc.."); // starts at 6, ends at 16
        let third_line = Vec::from("Dd.."); // starts at 17, ends at 21
        let fourth_line = Vec::from("Ee.."); // starts at 24, ends at 26

        let mut reader = get_reader(&test_data);

        for ix in 0..test_data.len() + 1 {
            reader.seek(SeekFrom::Start(ix as u64)).unwrap();
            println!("{}", ix);
            let target_slice = match ix {
                1 ..= 5 => &first_line[..(ix - 1)],
                6 ..= 16 => {
                    let mut end_index = ix - 6;
                    // Need to truncate because 2 eol characters are not in the return slice
                    if end_index > 9 {end_index = 9}; 
                    &second_line[..end_index]
                },
                17 ..= 21 => &third_line[..(ix - 17)],
                23 ..= 28 => &fourth_line[..(ix - 23)],
                _ => &[]
            };
            assert_eq!(reader.position(), ix);
            assert_eq!(reader.peek_preceding_part_of_line(), target_slice);
            // Should be idempotent
            assert_eq!(reader.position(), ix);
            assert_eq!(reader.peek_preceding_part_of_line(), target_slice);
        }
    }

    #[test]
    fn test_peek_next_line() {
        let test_data = get_line_test();

        let first_line = Vec::from("Aa.."); // starts at 1, ends at 5
        let second_line = Vec::from("Bb.. Cc.."); // starts at 6, ends at 16
        let third_line = Vec::from("Dd.."); // starts at 17, ends at 21
        let fourth_line = Vec::from("Ee.."); // starts at 24, ends at 26

        let mut reader = get_reader(&test_data);

        for ix in 0..test_data.len() + 1 {
            reader.seek(SeekFrom::Start(ix as u64)).unwrap();
            let target_slice = match ix {
                0 => &first_line[..],
                1 ..= 5 => &second_line[..],
                6 ..= 16 => &third_line[..],
                22 => &fourth_line[..],
                _ => &[]
            };
            assert_eq!(reader.position(), ix);
            assert_eq!(reader.peek_next_line(), target_slice);
            // should be idempotent
            assert_eq!(reader.position(), ix);
            assert_eq!(reader.peek_next_line(), target_slice);
        }
    }

    #[test]
    fn test_peek_preceding_line() {
        let test_data = get_line_test();

        let first_line = Vec::from("Aa.."); // starts at 1, ends at 5
        let second_line = Vec::from("Bb.. Cc.."); // starts at 6, ends at 16
        let third_line = Vec::from("Dd.."); // starts at 17, ends at 21
        let fourth_line = Vec::from("Ee.."); // starts at 24, ends at 26

        let mut reader = get_reader(&test_data);

        for ix in 0..test_data.len() + 1 {
            reader.seek(SeekFrom::Start(ix as u64)).unwrap();
            let target_slice = match ix {
                0 ..= 5 => &[],
                6 ..= 16 => &first_line[..],
                17 ..= 21 => &second_line[..],
                22 => &third_line[..],
                27 => &fourth_line[..],
                _ => &[]
            };
            assert_eq!(reader.position(), ix);
            assert_eq!(reader.peek_preceding_line(), target_slice);
            // should be idempotent
            assert_eq!(reader.position(), ix);
            assert_eq!(reader.peek_preceding_line(), target_slice);
        }
    }
}