use super::geometry;

use pest::{Parser};
use pest::iterators::Pair;
use data_string::DataString;


use geometry::{Transform, Point};
use super::TextBlock;
use crate::errors::*;
use ErrorKind::*;

#[derive(Parser)]
#[grammar = "pdf_doc/layout/postscript.pest"]
pub struct PSParser;

#[derive(Debug)]
pub struct CommandStream(Vec<Command>);

#[derive(Debug, Clone)]
enum DisplayObject {
    Text(TextBlock),
    Other
}

#[derive(Debug, Clone)]
enum Command {
    GraphicsState(GraphicsStateChange),
    PathBuild,
    PathDraw,
    Clip,
    TextState(TextStateChange),
    InterimTextWrite(usize, usize),
    TextWrite(Vec<u8>),
    Object
}

#[derive(Debug, Clone)]
enum GraphicsStateChange {
    Push,
    Pop,
    MultiplyCTM(Transform),
    ApplyParams(String),
    Other
}
#[derive(Debug, Clone)]
enum TextStateChange{
    NewCharSpacing(f32),
    NewWordSpacing(f32),
    NewHScaling(f32),
    NewTextLeading(f32),
    NewFont(String),
    NewFontSize(f32),
    NewRenderMode(u8),
    NewTextRise(f32),
    NewKnockout(bool),
    NewTM(Transform),
    TranslateTLM(f32, f32),
    TranslateTLMByCurrentLeading,
    AdvanceTLM(f32),
    ResetTM,
}

#[derive(Debug, Clone)]
pub struct PageState {
    ctm: Transform,
    tm: Transform,
    char_spacing: f32,
    word_spacing: f32,
    h_scaling: f32,
    text_leading: f32,
    font: Option<String>,
    font_size: Option<f32>,
    render_mode: u8,
    knockout: bool
}

impl PageState {
    pub fn new() -> Self {
        PageState {
            ctm: Transform::identity(),
            tm: Transform::identity(),
            char_spacing: 0.0,
            word_spacing: 0.0,
            h_scaling: 100.0,
            text_leading: 0.0,
            font: None,
            font_size: None,
            render_mode: 0,
            knockout: true
        }
    }

    pub fn update_ctm(&mut self, t: Transform) {
        self.ctm *= t;
        self.tm *= t;
    }

    pub fn update_tm(&mut self, t: Transform) {
        self.tm *= t;
    }
}

pub fn command_stream_from_contents(contents: Vec<u8>) -> Result<CommandStream> {
    let mut command_vec = Vec::new();
    let mut content_string = DataString::from_vec(contents);
    let mut parse_result = PSParser::parse(Rule::block, content_string.as_ref())
        .chain_err(|| ParsingError("Error parsing page contents!".to_string()))?
        .flatten()
        .into_iter()
        .peekable();
    use Rule::*;
    let active_rules = vec!(
        q, Q, cm, gs,
        Tc, Tw, Tz, TL, Tf, Tr, Ts,
        Td, TD, Tm, Tstar,
        Tj, Tj_newline, Tj_scaled, TJ,
        text_block
    );
    use Command::*;
    use GraphicsStateChange::*;
    use TextStateChange::*;
    while let Some(pair) = parse_result.next() {
        if !active_rules.contains(&pair.as_rule()) { continue };
        
        let command = match pair.as_rule() {
            q => vec!(GraphicsState(Push)),
            Q => vec!(GraphicsState(Pop)),
            cm => {
                let mut args = Vec::new();
                for _ in 0..6 {
                    args.push(
                        parse_result.next().unwrap().as_str().parse().unwrap()
                    );
                };
                vec!(GraphicsState(
                    MultiplyCTM(geometry::transform_from_vec(args))
                ))
            },
            gs => {
                let dict_name = parse_result.next().unwrap().as_str().to_string();
                vec!(GraphicsState(ApplyParams(dict_name)))
            },
            Tc => {
                let argument = parse_result.next().unwrap().as_str().parse().unwrap();
                vec!(TextState(NewCharSpacing(argument)))
            },
            Tw => {
                let argument = parse_result.next().unwrap().as_str().parse().unwrap();
                vec!(TextState(NewWordSpacing(argument)))
            },
            Tz => {
                let argument = parse_result.next().unwrap().as_str().parse().unwrap();
                vec!(TextState(NewHScaling(argument)))
            },
            TL => {
                let argument = parse_result.next().unwrap().as_str().parse().unwrap();
                vec!(TextState(NewTextLeading(argument)))
            },
            Tf => {
                let font_name = parse_result.next().unwrap().as_str().to_string();
                let font_size = parse_result.next().unwrap().as_str().parse().unwrap();
                vec!(
                    TextState(NewFont(font_name)),
                    TextState(NewFontSize(font_size))
                )
            },
            Tr => {
                let argument = parse_result.next().unwrap().as_str().parse().unwrap();
                vec!(TextState(NewRenderMode(argument)))
            },
            Ts => {
                let argument = parse_result.next().unwrap().as_str().parse().unwrap();
                vec!(TextState(NewTextRise(argument)))
            },
            Td => {
                let mut args = Vec::new();
                for _ in 0..2 {
                    args.push(
                        parse_result.next().unwrap().as_str().parse().unwrap()
                    );
                };
                vec!(TextState(TranslateTLM(args[0], args[1])))
            },
            TD => {
                let mut args = Vec::new();
                for _ in 0..2 {
                    args.push(
                        parse_result.next().unwrap().as_str().parse().unwrap()
                    );
                };
                vec!(
                    TextState(NewTextLeading(-1.0 * args[1])),
                    TextState(TranslateTLM(args[0], args[1]))
                )
            },
            Tm => {
                let mut args = Vec::new();
                for _ in 0..6 {
                    args.push(
                        parse_result.next().unwrap().as_str().parse().unwrap()
                    );
                };
                vec!(TextState(
                    NewTM(geometry::transform_from_vec(args))
                ))
            },
            Tstar => {
                vec!(TextState(TranslateTLMByCurrentLeading))
            },
            Tj => {
                let text_to_show = parse_result.next().unwrap();
                let (start, end)  = span_from_pair(text_to_show);
                vec!(InterimTextWrite(start, end))
            },
            Tj_newline => {
                let text_to_show = parse_result.next().unwrap();
                let (start, end)  = span_from_pair(text_to_show);
                vec!(
                    TextState(TranslateTLMByCurrentLeading),
                    InterimTextWrite(start, end)
                )
            },
            Tj_scaled => {
                let mut args = Vec::new();
                for _ in 0..2 {
                    args.push(
                        parse_result.next().unwrap().as_str().parse().unwrap()
                    );
                };
                let text_to_show = parse_result.next().unwrap();
                let (start, end) = span_from_pair(text_to_show);
                vec!(
                    TextState(NewWordSpacing(args[0])),
                    TextState(NewCharSpacing(args[0])),
                    InterimTextWrite(start, end)
                )
            },
            TJ => {
                let mut commands = Vec::new();
                loop {
                    match parse_result.peek() {
                        Some(pair) => {
                            match pair.as_rule() {
                                number => {
                                    let arg = parse_result.next().unwrap().as_str().parse().unwrap();
                                    commands.push(TextState(AdvanceTLM(arg)));
                                },
                                string => {
                                    let text_to_show = parse_result.next().unwrap();
                                    let (start, end) = span_from_pair(text_to_show);
                                    commands.push(InterimTextWrite(start, end));
                                },
                                _ => break
                            }
                        },
                        None => break
                    };
                }
                commands
            },
            text_block => vec!(TextState(ResetTM)),
            rule @ _  => { 
                if active_rules.contains(&rule) { panic!(format!("{:?} not implemented", rule)) };
                unreachable!()
            }    
        };
        command_vec.extend(command);
    };
    let data = content_string.take_data().unwrap();
    for command in command_vec.iter_mut() {
        if let &mut InterimTextWrite(start, end) = command {
            let mut new_vec = Vec::new();
            new_vec.extend_from_slice(&data[start..end]);
            *command = TextWrite(new_vec);

        }
    }
    Ok(CommandStream(command_vec))
}

#[derive(Debug, Clone)]
enum TextStateChange{
    NewCharSpacing(f32),
    NewWordSpacing(f32),
    NewHScaling(f32),
    NewTextLeading(f32),
    NewFont(String),
    NewFontSize(f32),
    NewRenderMode(u8),
    NewTextRise(f32),
    NewKnockout(bool),
    NewTM(Transform),
    TranslateTLM(f32, f32),
    TranslateTLMByCurrentLeading,
    AdvanceTLM(f32),
    ResetTM,
}

fn parse_command_stream(commands: CommandStream) -> Vec<TextBlock> {
    let mut state = PageState::new();
    let mut state_stack = Vec::new();
    let mut text_vec = Vec::new();
    for command in commands.0 {
        match command {
            Command::GraphicsState(change) => {
                use GraphicsStateChange::*;
                match change {
                    Push => {
                        let new_state = state.clone();
                        state_stack.push(state);
                        state = new_state;
                    },
                    Pop => state = state_stack.pop().unwrap(),
                    MultiplyCTM(t) => state.update_ctm(t),
                    ApplyParams(String) => {},
                    Other => {}
                };
            },
            Command::TextState(change) => {
                use TextStateChange::*;
                match change {
                    NewCharSpacing(val) => state.char_spacing = val,
                    NewWordSpacing(val) => state.word_spacing = val,
                    NewHScaling(val) => state.h_scaling = val,
                    NewTextLeading(val) => state.text_leading = val,
                    NewFont(font_name) => state.font = Some(font_name),
                    NewFontSize(val) => state.font_size = Some(val),
                    NewRenderMode(val) => state.render_mode = val,
                    NewKnockout(val) => state.knockout = val,
                    NewTM(t) => state.tm = t,
                    TranslateTLM(x, y) => {},
                    TranslateTLMByCurrentLeading => {},
                    AdvanceTLM(val) => {},
                    ResetTM => state.tm = Transform::identity()
                };
            },
            Command::TextWrite(v) => {

            },
            _ => {}
        };
        
    }

    text_vec

}

fn span_from_pair<R: pest::RuleType>(pair: Pair<R>) -> (usize, usize) {
    let active_rule = pair.as_rule();
    use pest::Token;
    let tokens: Vec<_> = pair
        .tokens()
        .filter(|token| match token {
            &Token::Start{ rule, ..} | &Token::End { rule, .. } => rule == active_rule})
        .collect();
    
    let start_ix = match tokens[0] {
        Token::Start { ref pos, .. } => pos.pos(),
        Token::End {..} => panic!("Expected start token in span_from_pair!")
    };
    let end_ix = match tokens[1] {
        Token::End { ref pos, .. } => pos.pos(),
        Token::Start {..} => panic!("Expected end token in span_from_pair!")
    };
    (start_ix, end_ix)
}




#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf_doc::PdfDoc;
    use std::fs;
    
    #[test]
    fn simple_parse() {
        let data = "/PlacedGraphic /MC0 BDC
        EMC";
        let result = PSParser::parse(Rule::block, data);
        if result.is_err() {
            println!("result: {}", result.unwrap_err());
        };
    }
    #[test]
    fn parse_test_file() {
        let data = fs::read_to_string("data/test.txt").expect("Bad file");
        let _result = PSParser::parse(Rule::text_block, &data).expect("Failed parse!");
        //println!("{:#?}", result);
        let commands = command_stream_from_contents(data.into_bytes()).unwrap();
        println!("Commands: {:#?}", commands);
        
    }
    #[test]
    fn real_parse() {
        let doc = PdfDoc::create_pdf_from_file("data/f1120.pdf").unwrap();
        for page_num in 1..doc.page_count() {
            let page = doc.page_tree.get_page(page_num).unwrap();
            
            let contents = match page.contents_as_binary() {
                Some(contents) => contents,
                None => vec!()
            };
            let mut content_string = DataString::from_vec(contents);
            let result = PSParser::parse(Rule::block, content_string.as_ref());
            if result.is_err() { 
                println!("Page {}: {:#?}", page_num, result);
                println!("{}", result.unwrap_err());
                let borrow = content_string.take_data().unwrap();
                // for index in 200..300 {
                //     println!("{}: {}", borrow[index] as char, borrow[index]);
                // }
                let letter_vec: Vec<char> = borrow.iter().map(|r| *r as char).collect();
                for index in 4500..5500 {
                    print!("{}", letter_vec[index]);
                }
                panic!();
                //println!("{}", content_string);
            };
            let data = content_string.take_data().unwrap();
            let commands = command_stream_from_contents(data);
            if commands.is_err() {
                println!("{:?}", commands);
                panic!();

            }
        }

    }
}