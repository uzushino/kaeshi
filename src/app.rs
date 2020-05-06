use std::cell::RefCell;
use std::collections::{ BTreeMap, HashMap };
use serde::{Serialize, Deserialize};
use serde_yaml::{ self, Error };
use crate::parser;
use nom::{
    IResult,
    bytes::streaming::{ take_until },
    bytes::complete::{
        tag,
    },
};
use nom::multi::many0;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct App {
    templates: Vec<HashMap<String, String>>,
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

pub fn slice_to_string(s: &[u8]) -> String {
    String::from_utf8(s.to_vec()).unwrap()
}

impl App {
    pub fn load_from_file<'a>(file: &'a str) -> BTreeMap<String, App> {
        let contents = std::fs::read_to_string(file).unwrap();
        serde_yaml::from_str(&contents).unwrap()
    }

    pub fn table<'a>(&self, text: &'a str) -> Vec<BTreeMap<String, String>>{
        let mut tables: Vec<BTreeMap<String, String>> = vec![];
        let syn = parser::Syntax::default();
        let body = RefCell::new(text.to_owned());

        for template in self.templates.clone() {
            for (k, rule) in template.clone().into_iter() {
                let (_, result) =
                    parser::parse_template(rule.as_bytes(), &syn).unwrap();
                let s = body.borrow().clone();  

                match k.as_str() {
                    "many" => {
                        let comb = make_combinator(&result);
                        let comb = many0(comb);
                        match comb(s.as_str()) {
                            Ok((rest, mut results)) => {
                                tables.append(&mut results);
                                body.replace(rest.to_string());
                            },
                            _ => {}
                        };
                    },
                    "tag" => {
                        let comb = make_combinator(&result);
                        match comb(s.as_str()) {
                            Ok((rest, result)) => {
                                tables.push(result);
                                body.replace(rest.to_string());
                            },
                            _ => {}
                        };
                    },
                    _ => {}
                };
            }
        }

        tables
    }
}


mod test {
    use super::*;
    use nom::multi::many0;
   
    #[test]
    fn csv_parse() {
        let app: BTreeMap<String, App> = App::load_from_file("sample.yml");
        let csv = app["csv"].clone();
        let syn = parser::Syntax::default();
        
        let s = r#"id,name,age,email
1,abc,10,abc@example.com
2,def,20,def@example.com
        "#;

        let title: String = csv.templates[0]["tag"].clone();
        let many: String = csv.templates[1]["many"].clone();
        let (_, tokens) = parser::parse_template(
            title.as_bytes(), 
            &syn
        ).unwrap();

        let title_combinator = make_combinator(&tokens);

        match title_combinator(s) {
            Ok((rest, _)) => {
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