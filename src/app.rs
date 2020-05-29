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
    vars: Vec<String>,
    filters: Vec<String>,
}

pub fn make_combinator<'a>() -> impl Fn(Vec<parser::Node>, &'a str) -> IResult<&'a str, BTreeMap<String, String>> {
    move |tokens: Vec<parser::Node>, mut input: &'a str| {
        let mut h: BTreeMap<String, String> = BTreeMap::default();
        
        for (idx, token) in tokens.iter().enumerate() {

            match token {
                parser::Node::Lit(a, b, c) => {
                    let a: IResult<&str, &str> = tag(&format!("{}{}{}", a, b, c)[..])(input);
                    match a {
                        Ok((rest, _b)) => {
                            dbg!((input, &rest, &_b));
                            input = rest
                        },
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
                        dbg!((input, &result));
                        if let Ok((rest, hit))  = result {
                            h.insert(key.to_string(), hit.to_string());
                            input = rest;
                        }
                    } else {
                        dbg!(&input);
                        let result: IResult<&'a str, &'a str> = take_until("\n")(input);
                        dbg!(&result);
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

    pub fn build<'a>(templates: Vec<Token>) -> impl Fn(&'a str) -> IResult<&'a str, Vec<BTreeMap<String, String>>> {
        move |mut text: &str| {
            let syn = parser::Syntax::default();
            let mut results= Vec::default();
            let old = templates.clone();

            for (i, tok) in templates.iter().enumerate() {
                let t= tok.clone();
                let parsed = match t {
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
                        let r = many_till(anychar,  acc)(text);
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
                    Token::Tag(ref r) => {
                        let (_, tbl) = parser::parse_template(r.as_bytes(), &syn).unwrap();
                        let comb = make_combinator();
                        dbg!(text);
                        let aa = comb(tbl, text);
                        match aa {
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
                        let err = ("", nom::error::ErrorKind::Fix);
                        return Err(nom::Err::Error(err));
                    }
                }
            }

            dbg!((text, &results));

            Ok((text, results))
        }
    }
}
/*
#[allow(unused_imports)]
mod test {
    use super::*;
    use nom::multi::many0;

    #[test]
    fn csv_parse() {
        let app: BTreeMap<String, App> = App::load_from_file("sample.yml").unwrap();
        let csv = app["csv"].clone();
        let syn = parser::Syntax::default();
        
        let s = r#"id,name,age,email
1,abc,10,abc@example.com
2,def,20,def@example.com
        "#;

        let title = csv.templates[0].clone();
        let many = csv.templates[2].clone();
        let (_, tokens) = parser::parse_template(
            title, 
            &syn
        ).unwrap();

        let title_combinator = make_combinator(&tokens);

        match title_combinator(s) {
            Ok((rest, r)) => {
                let (_, tokens) = parser::parse_template(
                    many, 
                    &syn
                ).unwrap();

                let record_combinator = many0(|s: &str| make_combinator(&tokens)(s.trim()));
                match record_combinator(rest) {
                    Ok((rest, values)) => {
                        assert!(rest.is_empty());
                        assert_eq!(values.len(), 2);
                    },  
                    Err(_) => {}
                }
            },
            Err(_) => {}
        }
    }
}
*/