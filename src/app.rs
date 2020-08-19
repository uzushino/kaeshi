use std::collections::BTreeMap;
use std::option::Option;
use serde::Deserialize;
use nom::{
    IResult,
    branch::alt,
    bytes::streaming::take_until,
    bytes::complete::tag,
};
//use std::sync::mpsc::{ self, Sender };
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::process::{Command, Stdio};
use std::thread::{self, JoinHandle};

use crossbeam_channel::{ self, unbounded, bounded, Sender, Receiver };

use tempra::parser;
use tempra::table;

#[derive(Debug, Deserialize, Clone)]
pub enum VarExpr {
   Regex(String),
   If(TokenExpr),
}

#[derive(Debug, Deserialize, Clone)]
pub struct TokenExpr {
    pub tag: String,
    // Many
    pub many: Option<bool>,
    // Count
    count: Option<usize>,

    vars: BTreeMap<String, VarExpr>
}

pub type Token = TokenExpr;

impl TokenExpr {
    pub fn new_with_tag(tag: &String) -> TokenExpr {
        TokenExpr {
            tag: tag.clone(), 
            many: None, 
            count: None, 
            vars: BTreeMap::new()
        }
    }

    pub fn evaluate(&self, rx: &Receiver<InputToken>, syn: &parser::Syntax) -> (bool, Vec<BTreeMap<String, String>>) {
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
                                Ok(InputToken::Byte(b'\0')) => return (true, results),
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
                                Ok(InputToken::Byte(b'\0')) => return (true, results),
                                _ => break
                            }
                        }
                    }
                }
            },
            Ok(InputToken::Byte(b'\0')) => return (true, results),
            _ => {}
        }

        (false, results)
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

#[derive(Debug, Deserialize, Clone, Default)]
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
                parser::Node::Expr(_, parser::Expr::Filter("skip", _)) => {},
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

    pub handler: Option<JoinHandle<()>>,

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

            'main: loop {
                let (is_break, mut row) = first.evaluate(&rx, &syn);
                rows.append(&mut row);

                if !rest.is_empty() {
                    for template in rest {
                        let (is_break, mut row) = template.evaluate(&rx, &syn);
                        rows.append(&mut row);

                        if is_break {
                            break 'main;
                        }
                    }
                }
               
                if is_break {
                    break;
                } 
            }
                
            let _ = table::printstd(&mut writer, &rows);
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
}
