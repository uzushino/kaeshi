use std::cell::RefCell;
use std::collections::{ BTreeMap, HashMap };
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
pub struct App {
    pub templates: Vec<HashMap<String, String>>,
    vars: Vec<String>,
    filters: Vec<String>,
}

pub fn make_combinator<'a>(tokens: &'a Vec<parser::Node>) -> impl Fn(&'a str) -> IResult<&'a str, BTreeMap<String, String>> {
    move |mut input: &str| {
        let mut h: BTreeMap<String, String> = BTreeMap::default();
        
        for (idx, token) in tokens.iter().enumerate() {
            match token {
                parser::Node::Lit(a, b, c) => {
                    let (rest, _) = tag(&format!("{}{}{}", a, b, c)[..])(input)?;
                    input = rest;
                },
                parser::Node::Expr(_, parser::Expr::Var(key)) => {
                    let next = tokens.get(idx + 1);
                    if let Some((rest, hit)) = get_expr_value(input, next) {
                        h.insert(key.to_string(), hit.to_string());
                        input = rest;
                    }
                },
                parser::Node::Expr(_, parser::Expr::Filter("truncate", arguments)) => {
                    match (&arguments[0], &arguments[1]) {
                        (parser::Expr::Var(key), parser::Expr::NumLit(n)) => {
                            if let Ok(n) = n.parse() {
                                let next = tokens.get(idx + 1);
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
                _ => {},
            }   
        };
        
        IResult::Ok((input, h))
    }
}

fn get_expr_value<'a>(input: &'a str, next: Option<&'a parser::Node<'a>>) -> Option<(&'a str, &'a str)> {
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
                Err(_) => Some(("", input))
            }
        },
        _ => None,
    }
}

#[allow(dead_code)]
pub fn slice_to_string(s: &[u8]) -> String {
    String::from_utf8(s.to_vec()).unwrap()
}

use std::option::Option;

impl App {
    pub fn load_from_file<'a>(file: &'a str) -> Result<BTreeMap<String, App>, serde_yaml::Error> {
        let contents = std::fs::read_to_string(file).unwrap();
        serde_yaml::from_str(&contents)
    }

    pub fn combinator<'a>(templates: Vec<HashMap<String, String>>) -> impl Fn(&'a str) -> IResult<&'a str, Vec<BTreeMap<String, String>>> {
        move |text: &str| {
            let syn = parser::Syntax::default();
            let mut results= Vec::default();
            let body = RefCell::new(text.to_owned());
            let old = templates.clone();
            for (i, template) in templates.iter().enumerate() {
                for (k, rule) in template.clone().into_iter() {
                    let (_, result) =
                        parser::parse_template(rule.as_bytes(), &syn).unwrap();
                    let s = body.borrow().clone();  

                    let (rest, mut tables) = match k.as_str() {
                        "many" => {
                            let comb = make_combinator(&result);
                            let (rest, result) = many0(comb)(s.trim()).unwrap();

                            dbg!((&rest, &result));

                            (rest, result)
                        }
                        "skip" => {
                            let remain = &old[(i+1)..old.len()];
                            let acc = Self::combinator(remain.to_vec());
                            let (rest, _b) = many_till(anychar, preceded(tag("\n"), acc))(s.trim()).unwrap();

                            dbg!((&rest, &_b));

                            (rest, Vec::default())
                        }
                        "tag" => {
                            let comb = make_combinator(&result);
                            let (rest, value) = comb(s.as_str()).unwrap();
                            
                            if value.is_empty() {
                                (rest, Vec::default())
                            } else {
                                (rest, vec![value])
                            }
                        },
                        _ => { (s.as_str(), Vec::default())}
                    };

                    if !tables.is_empty() {
                        results.append(&mut tables);
                    }

                    body.replace(rest.to_string());
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

    #[test]
    fn csv_parse() {
        let app: BTreeMap<String, App> = App::load_from_file("sample.yml").unwrap();
        let csv = app["csv"].clone();
        let syn = parser::Syntax::default();
        
        let s = r#"id,name,age,email
1,abc,10,abc@example.com
2,def,20,def@example.com
        "#;

        let title: String = csv.templates[0]["tag"].clone();
        let many: String = csv.templates[2]["many"].clone();
        let (_, tokens) = parser::parse_template(
            title.as_bytes(), 
            &syn
        ).unwrap();

        let title_combinator = make_combinator(&tokens);

        match title_combinator(s) {
            Ok((rest, r)) => {
                dbg!(r);
                let (_, tokens) = parser::parse_template(
                    many.as_bytes(), 
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