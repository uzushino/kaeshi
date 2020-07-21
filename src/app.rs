use std::collections::BTreeMap;
use std::option::Option;
use serde::Deserialize;
use nom::{
    IResult,
    branch::alt,
    bytes::streaming::take_until,
    bytes::complete::tag,
    multi::many1,
};
//use std::sync::mpsc::{ self, Sender };
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::process::{Command, Stdio};
use std::thread::{self, JoinHandle};
use log::{ debug, error };

use crossbeam_channel::{ self, unbounded, bounded, Sender, Receiver };

use tempra::parser;
use tempra::table;

#[derive(Debug, Deserialize, Clone)]
pub struct TokenExpr {
    tag: String,

    // Many
    many: Option<bool>,

    // Count
    count: Option<usize>,
}

pub type Token = TokenExpr;

impl TokenExpr {
    pub fn evaluate(&self, rx: &Receiver<InputToken>, syn: &parser::Syntax) -> Vec<BTreeMap<String, String>> {
        let mut results = Vec::default();

        match rx.recv() {
            Ok(InputToken::Channel(text)) => {
                if let Ok((_, mut result)) = self.parse(text.as_str(), syn) {
                    results.append(&mut result);

                    if self.count.is_some() {
                        for _ in 1..self.count.unwrap_or(1) {
                            match rx.recv() {
                                Ok(InputToken::Channel(text)) => {
                                    if let Ok((_, mut row)) = self.parse(&text[..], syn) {
                                        results.append(&mut row);
                                    }
                                },
                                _ => break
                            }
                        }
                    } else if self.many.is_some() {
                        loop {
                            match rx.recv() {
                                Ok(InputToken::Channel(text)) => {
                                    if let Ok((_, mut row)) = self.parse(&text[..], syn) {
                                        results.append(&mut row);
                                    } else {
                                        break
                                    }
                                },
                                _ => break
                            }
                        }
                    }
                }
            },
            _ => {}
        }

        results
    }

    pub fn parse<'a>(&self, text: &'a str, syn: &parser::Syntax) -> IResult<&'a str, Vec<BTreeMap<String, String>>> {
        let (_, tokens) = parser::parse_template(self.tag.as_bytes(), &syn).unwrap();

        make_combinator()(tokens, text)
            .map(|(rest, value)| {
                if value.is_empty() {
                    (rest, Vec::default())
                } else {
                    (rest, vec![value])
                }
            })
    }
}

#[derive(Debug, Deserialize, Clone)]
pub enum Output {
    Table,
    Json
}

#[derive(Debug, Deserialize, Clone)]
pub enum InputToken {
    Byte(u8),
    Channel(String),
    EOF,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub templates: Vec<Token>,
    output: Option<Output>,
    vars: Option<Vec<String>>,
    filters: Option<Vec<String>>,
}

pub fn make_combinator<'a>() -> impl Fn(Vec<parser::Node>, &'a str) -> IResult<&'a str, BTreeMap<String, String>> {
    move |tokens: Vec<parser::Node>, mut input: &'a str| {
        let mut h: BTreeMap<String, String> = BTreeMap::default();

        for (idx, token) in tokens.iter().enumerate() {
            if input.is_empty() {
                break
            }
            
            match token {
                parser::Node::Lit(a, b, c) => {
                    let a: IResult<&str, &str> = tag(&format!("{}{}{}", a, b, c)[..])(input);

                    match a {
                        Ok((rest, _b)) => input = rest,
                        _ => {
                            let err = nom::error::make_error(input, nom::error::ErrorKind::Eof);
                            return Err(nom::Err::Error(err));
                        }
                    }
                },
                parser::Node::Expr(_, parser::Expr::Filter("trim", vars)) => {
                    if let Some(parser::Expr::Var(t)) = vars.first() {
                        if let Some(v) = h.get_mut(t.clone()) {
                            *v = v.trim().to_string();
                        }
                    }
                },
                parser::Node::Expr(_, parser::Expr::Var(key)) => {
                    let next = tokens.get(idx + 1);
                    
                    if let Some(parser::Node::Lit(a, b, c)) = next {
                        let err = (input, nom::error::ErrorKind::TakeUntil);
                        let mut result: IResult<&str, &str> = Err(nom::Err::Error(err));
                        
                        for (u, _ch) in input.chars().into_iter().enumerate() {
                            let s = &input[u..];
                            let t: IResult<&str, &str> = alt((tag("\n"), tag(&format!("{}{}{}", a, b, c)[..])))(s);
                            if let Ok(_) = t {
                                result = Ok((&input[u..input.len()], &input[0..u]));
                                break
                            } 
                        }

                        if let Ok((rest, hit)) = result {
                            h.insert(key.to_string(), hit.to_string());
                            input = rest;
                        } else {
                            let err = (input, nom::error::ErrorKind::ParseTo);
                            return Err(nom::Err::Error(err));
                        }
                    } else {
                        let result: IResult<&'a str, &'a str> = take_until("\n")(input);
                        if let Ok((rest, capture))  = result {
                            h.insert(key.to_string(), capture.to_string());
                            input = rest;
                        } else {
                            h.insert(key.to_string(), input.to_string());
                            input = "";
                        }
                    }
                },
                _ => {},
            }   
        };

        IResult::Ok((input, h))
    }
}

#[allow(dead_code)]
pub fn slice_to_string(s: &[u8]) -> String {
    String::from_utf8(s.to_vec()).unwrap()
}

pub struct App<'a> {
    tx: Sender<InputToken>,

    handler: Option<JoinHandle<()>>,

    config: &'a AppConfig,
}

impl<'a> App<'a> {
    pub fn new_with_config(config: &AppConfig) -> anyhow::Result<App> {
        let (tx, rx): (Sender<InputToken>, Receiver<InputToken>) = unbounded();
        let templates = config.templates.clone();

        let handler = thread::spawn(move || {
            let mut writer = BufWriter::new(io::stdout());
            let first = templates.first().unwrap();
            let rest = &templates[1..];
            let syn = parser::Syntax::default();
            let mut rows: Vec<BTreeMap<String, String>> = Vec::default();

            loop {
                let mut row = first.evaluate(&rx, &syn);
                rows.append(&mut row);

                for template in rest {
                    let mut row= template.evaluate(&rx, &syn);
                    rows.append(&mut row);
                }

                let _ = table::printstd(&mut writer, &rows);
            }
        });

        Ok(App {
            tx,
            config,
            handler: Some(handler),
        })
    }

    pub fn send_byte(&self, b: u8) -> anyhow::Result<()> {
        self.tx.send(InputToken::Byte(b))?;
        
        Ok(())
    }
    
    pub fn send_string(&self, txt: String) -> anyhow::Result<()> {
        self.tx.send(InputToken::Channel(txt.clone()))?;
        
        Ok(())
    }

    /*
    pub fn build<'b>(templates: Vec<Token>) -> impl Fn(&'b str) -> IResult<&'b str, Vec<BTreeMap<String, String>>> {
        move |mut text: &str| {
            let syn = parser::Syntax::default();
            let mut results= Vec::default();
            let old = templates.clone();
            
            for (i, tok) in templates.iter().enumerate() {
                if text.is_empty() {
                    let err = (text, nom::error::ErrorKind::NonEmpty);
                    return Err(nom::Err::Error(err));
                }

                let parsed = match tok.keys() {
                    Token::Many(t) => {
                        let ts = t.iter().map(|tmpl| *tmpl.clone()).collect();
                        let comb = Self::build(ts);

                        many1(comb)(text)
                            .map(|(rest, result)| {
                                let a  = result
                                    .iter()
                                    .flatten()
                                    .map(|s| s.clone())
                                    .collect::<Vec<_>>();
                                (rest, a)
                            })
                            .ok()
                    }
                    Token::Skip => {
                        let remain = &old[(i+1)..];
                        let acc = Self::build(remain.to_vec());
                        let mut result = None;

                        for (u, _ch) in text.chars().into_iter().enumerate() {
                            let s = &text[u..];
                            
                            if acc(s).is_ok() {
                                result = Some((s, Vec::default()));
                                break
                            } 
                        }
                        
                        result
                    }
                    Token::Tag(ref tag) => {
                        let (_, tokens) = parser::parse_template(tag.as_bytes(), &syn).unwrap();

                        make_combinator()(tokens, text)
                            .map(|(rest, value)| {
                                if value.is_empty() {
                                    (rest, Vec::default())
                                } else {
                                    (rest, vec![value])
                                }
                            })
                            .ok()
                   },
                   _ => None
                };
               
                match parsed {
                    Some((rest, mut tables)) => {
                        if !tables.is_empty() {
                            results.append(&mut tables);
                        }

                        text = rest;
                    }
                    _ => {
                        let err = (text, nom::error::ErrorKind::Fix);
                        return Err(nom::Err::Error(err));
                    }
                }
            }

            Ok((text, results))
        }
    }
    */
}

#[allow(unused_imports)]
mod test {
    use super::*;
    use tempra::table;

    #[test]
    fn test_many() {
        const YML: &str = r#"
templates:
  - 
    tag: "id,name,age,email\n"
  -
    tag: "{{i}},{{n}},{{a}},{{e}}\n"
    count: 5
"#;
    }
}