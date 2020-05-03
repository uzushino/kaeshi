use std::collections::HashMap;
use std::collections::BTreeMap;

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use nom::{
    IResult,
    bytes::streaming::{ take_until },
    bytes::complete::{
        tag,
    },
};

use serde::{Serialize, Deserialize};
use serde_yaml::{ self, Error };
mod parser;
mod table;

#[derive(Debug, Serialize, Deserialize)]
struct App {
    templates: Vec<String>,
    vars: Vec<String>,
    filters: Vec<String>,
}

fn main() {
    let contents = std::fs::read_to_string("sample.yml").unwrap();
    let app: HashMap<String, App> = serde_yaml::from_str(&contents).unwrap();
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    let template = r#"
    abc
    a{{ b0 }}c
    a {{ b1 }} c
    "#;

    let thandle = thread::spawn(move || {
        let syn = parser::Syntax::default();
        let mut rows = Vec::default();
        let templates: Vec<&str> = template.trim().split("\n").collect();

        while running.load(Ordering::Relaxed) {
            let mut lines: Vec<String> = Vec::default();
            for _ in templates.clone() {
                let mut input = String::new();
                let _ = io::stdin().read_line(&mut input);
                lines.push(input.trim().to_string());
            }

            let tokens = parser::parse(template.trim(), &syn);
            match parsec(&tokens, lines.join("\n").trim()) {
                Ok((_rest, rebind)) => rows.push(rebind),
                Err(_) => {}
            };
            
            table::printstd(&rows);
        }
    });

    ctrlc::set_handler(move || {
        r.store(false, Ordering::Relaxed);
    })
    .expect("Error setting Ctrl-C handler");

    let _ = thandle.join();
}

fn parsec<'a>(tokens: &'a Vec<parser::Node>, mut input: &'a str) -> IResult<&'a str, HashMap<String, String>> {
    let mut h: HashMap<String, String> = HashMap::default();

    for (idx, token) in tokens.iter().enumerate() {
        match token {
            parser::Node::Lit(a, b, c) => {
                let (rest, _) = tag(&format!("{}{}{}", a, b, c)[..])(input)?;
                input = rest;
            },
            parser::Node::Expr(_, parser::Expr::Var(key)) => {
                let next = tokens.get(idx + 1);
                let (rest, hit) = get_expr_value(input, next).unwrap();
                h.insert(key.to_string(), hit.to_string());
                input = rest;
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

fn get_expr_value<'a>(input: &'a str, next: Option<&'a parser::Node<'a>>) -> Option<(&'a str, &'a str)> {
    match next {
        Some(parser::Node::Lit(a, b, c)) => {
            let result: IResult<&'a str, &'a str> = 
                take_until(&format!("{}{}{}", a, b, c)[..])(input);
            result.ok()
        }
        None => Some((input, input)),
        _ => None,
    }
}
