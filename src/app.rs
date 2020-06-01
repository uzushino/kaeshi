use std::cell::RefCell;
use std::collections::{ BTreeMap, HashMap };
use std::option::Option;
use serde::{Serialize, Deserialize};
use nom::{
    IResult,
    character::complete::{ anychar },
    sequence::{ preceded },
    bytes::streaming::{ take_until },
    bytes::complete::{
        tag,
    },
    multi::{ many0, many_till }
};

use crate::parser;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Token {
    #[serde(rename = "tag")]
    Tag(String),
    #[serde(rename = "many")]
    Many(Box<Token>),
    #[serde(rename = "skip")]
    Skip,
    #[serde(rename = "while")]
    While(Box<Token>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct App {
    pub templates: Vec<Token>,
    vars: Option<Vec<String>>,
    filters: Option<Vec<String>>,
}

pub fn make_combinator<'a>() -> impl Fn(Vec<parser::Node>, &'a str) -> IResult<&'a str, BTreeMap<String, String>> {
    move |tokens: Vec<parser::Node>, mut input: &'a str| {
        let mut h: BTreeMap<String, String> = BTreeMap::default();
        
        for (idx, token) in tokens.iter().enumerate() {

            match token {
                parser::Node::Lit(a, b, c) => {
                    let a: IResult<&str, &str> = tag(&format!("{}{}{}", a, b, c)[..])(input);
                    match a {
                        Ok((rest, _b)) => input = rest,
                        _ => {
                            let err = (input, nom::error::ErrorKind::Fix);
                            return Err(nom::Err::Error(err));
                        }
                    }
                },
            
                parser::Node::Expr(_, parser::Expr::Var(key)) => {
                    let next = tokens.get(idx + 1);

                    if let Some(parser::Node::Lit(a, b, c)) = next {
                        let result: IResult<&'a str, &'a str> = 
                            take_until(&format!("{}{}{}", a, b, c)[..])(input);
                        if let Ok((rest, hit))  = result {
                            h.insert(key.to_string(), hit.to_string());
                            input = rest;
                        }
                    } else {
                        let result: IResult<&'a str, &'a str> = take_until("\n")(input);
                        if let Ok((rest, capture))  = result {
                            h.insert(key.to_string(), capture.to_string());
                            input = rest;
                        }
                    }
                },
                /*
                parser::Node::Expr(_, parser::Expr::Filter("truncate", arguments)) => {
                    match (&arguments[0], &arguments[1]) {
                        (parser::Expr::Var(key), parser::Expr::NumLit(n)) => {
                            if let Ok(n) = n.parse() {
                                let next = tokens.get(idx + 1).map(|t| t.clone());
                                let (rest, hit) = get_expr_value(input, next).unwrap();
                                let mut s = String::from(hit);

                                s.truncate(n);
                                h.insert(key.to_string(), s);

                                input = rest;
                            }
                        }
                        _ => {}
                    }
                },
                */
                _ => {},
            }   
        };
        
        IResult::Ok((input, h))
    }
}

fn get_expr_value<'a>(input: &'a str, next: Option<parser::Node<'a>>) -> Option<(&'a str, &'a str)> {
    match next {
        Some(parser::Node::Lit(a, b, c)) => {
            let result: IResult<&'a str, &'a str> = 
                take_until(&format!("{}{}{}", a, b, c)[..])(input);

            result.ok()
        }
        None => {
            let result: IResult<&'a str, &'a str> = take_until("\n")(input);
            match result {
                Ok((rest, capture)) => Some((rest, capture)),
                Err(_) => Some((input, input))
            }
        },
        _ => None,
    }
}

#[allow(dead_code)]
pub fn slice_to_string(s: &[u8]) -> String {
    String::from_utf8(s.to_vec()).unwrap()
}

impl App {
    pub fn load_from_file<'a>(file: &'a str) -> Result<BTreeMap<String, App>, serde_yaml::Error> {
        let contents = std::fs::read_to_string(file).unwrap();
        serde_yaml::from_str(&contents)
    }
    
    pub fn load_from_str(content: impl ToString) -> Result<BTreeMap<String, App>, serde_yaml::Error> {
        serde_yaml::from_str(content.to_string().as_str())
    }

    pub fn build<'a>(templates: Vec<Token>) -> impl Fn(&'a str) -> IResult<&'a str, Vec<BTreeMap<String, String>>> {
        move |mut text: &str| {
            let syn = parser::Syntax::default();
            let mut results= Vec::default();
            let old = templates.clone();

            for (i, tok) in templates.iter().enumerate() {
                let parsed = match tok {
                    Token::Many(t) => {
                        let comb= Self::build(vec![*t.clone()]);
                        let (rest, result) = many0(comb)(text.trim()).unwrap();
                        
                        let a: Vec<BTreeMap<String, String>> = result
                            .iter()
                            .flatten()
                            .map(|s| s.clone())
                            .collect::<Vec<_>>();

                        Some((rest, a.clone()))
                    }
                    Token::Skip => {
                        let remain = &old[(i+1)..old.len()];
                        let acc = Self::build(remain.to_vec());
                        let r = 
                            many_till(anychar,  acc)(text);
                        match r {
                            Ok((_rest, (matches, _))) => {
                                Some((&text[matches.len()..], Vec::default()))
                            },
                            _ => None
                        }
                    }
                    Token::While(t) => {
                        let acc = Self::build(vec![*t.clone()]);
                        let r = many_till(anychar, acc)(text.trim());

                        match r {
                            Ok((rest, _b)) => Some((rest, Vec::default())),
                            _ => None
                        }
                    }
                    Token::Tag(ref tag) => {
                        let (_, tbl) = parser::parse_template(tag.as_bytes(), &syn).unwrap();
                        let comb = make_combinator();
                        let r = comb(tbl, text);
                        match r {
                            Ok((rest, value)) => {
                        
                                if value.is_empty() {
                                    Some((rest, Vec::default()))
                                } else {
                                    Some((rest, vec![value]))
                                }
                            },
                            _ => None
                        }
                    },
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
}

#[allow(unused_imports)]
mod test {
    use super::*;

    use nom::multi::many0;
    use crate::table;

    #[test]
    fn csv_parse() {
        const YML: &str = r#"
csv:
  templates:
    - 
      tag: "id,name,age,email\n"
    -
      skip:
    - 
      many: 
        tag: "hoge,{{i}},{{n}},{{a}},{{e}}"
    -
      skip:
    - 
      tag: "total,{{total}}"
"#;
        let app: BTreeMap<String, App> = App::load_from_str(YML).unwrap();
        let input = r#"
id,name,age,email
==
hoge,1,2,3,4
hoge,5,6,7,8
------
total,20
"#;
        let combinate = App::build(app["csv"].templates.clone());
        match combinate(input.trim()) {
            Ok((_rest, rows)) => {
                table::printstd(&rows);
            },
            Err(e) =>  { dbg!(e); }
        }
    }
}