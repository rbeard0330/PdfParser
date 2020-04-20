extern crate nalgebra as na;
pub mod postscript;
mod geometry;

use crate::errors::*;

use std::fmt;

use geometry::{Rect};
use postscript::CommandStream;


#[derive(Clone, Copy, Debug)]
struct Letter {
    b_box: Rect,
    letter: char
}

impl fmt::Display for Letter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.letter)
    }
}

#[derive(Clone, Debug)]
pub struct TextBlock {
    b_box: Rect,
    text: Vec<Letter>
}

impl fmt::Display for TextBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for letter in &self.text { write!(f, "{}", letter)? };
        Ok(())
    }
}

pub fn layout_from_contents(contents: Vec<u8>) -> Result<Vec<TextBlock>> {

        Ok(Vec::new())

}
