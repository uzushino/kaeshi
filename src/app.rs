use std::collections::BTreeMap;
use std::option::Option;
use serde::{Serialize, Deserialize};
use nom::{
    IResult,
    branch::alt,
    bytes::streaming::take_until,
    bytes::complete::tag,
    character::complete::alphanumeric1,
    multi::many1,
    sequence::terminated,
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
            if input.is_empty() {
                break
            }

            match token {
                parser::Node::Lit(a, b, c) => {
                    let a: IResult<&str, &str> = tag(&format!("{}{}{}", a, b, c)[..])(input);
                    match a {
                        Ok((rest, _b)) => input = rest,
                        _ => {
                            let err = (input, nom::error::ErrorKind::Eof);
                            return Err(nom::Err::Error(err));
                        }
                    }
                },
                parser::Node::Expr(_, parser::Expr::Var(key)) => {
                    let next = tokens.get(idx + 1);
                    
                    if let Some(parser::Node::Lit(a, b, c)) = next {
                        let result: IResult<&str, &str> = terminated(
                            alphanumeric1,
                            alt((tag(""), tag(&format!("{}{}{}", a, b, c)[..])))
                        )(input);

                        if let Ok((rest, hit)) = result {
                            h.insert(key.to_string(), hit.to_string());
                            input = rest;
                        } else {
                            let err = (input, nom::error::ErrorKind::ParseTo);
                            return Err(nom::Err::Error(err));
                        }
                    } else {
                        let result: IResult<&'a str, &'a str> = 
                            take_until("\n")(input);

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
                if text.is_empty() {
                    let err = (text, nom::error::ErrorKind::NonEmpty);
                    return Err(nom::Err::Error(err));
                }

                let parsed = match tok {
                    Token::Many(t) => {
                        let comb = Self::build(vec![*t.clone()]);
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
                            
                            if let Ok(_) = acc(s) {
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
}

#[allow(unused_imports)]
mod test {
    use super::*;
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
        tag: "{{i}},{{n}},{{a}},{{e}}\n"
    -
      skip:
    -
      tag: "total: {{t}}"
"#;
        let app: BTreeMap<String, App> = App::load_from_str(YML).unwrap();
        let input = r#"
id,name,age,email
==
1,2,3,4
5,6,7,8
==
total: 20
"#;
        let combinate = App::build(app["csv"].templates.clone());
        match combinate(input.trim_start()) {
            Ok((_rest, rows)) => table::printstd(&rows),
            Err(_) =>  assert!(false)
        }
    }
}