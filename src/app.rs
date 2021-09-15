use std::io::{ BufRead };
use std::collections::{ HashSet };
use std::{collections::BTreeMap};
use std::option::Option;
use serde::Deserialize;
use nom::{
    IResult,
    branch::alt,
    bytes::streaming::take_until,
    bytes::complete::tag,
};
use log::error;
// use crossbeam_channel::{ self, unbounded, Sender, Receiver };
use tokio::sync::mpsc;
use async_recursion::async_recursion;

use super::parser;
use super::db;

#[derive(Debug, Deserialize, Clone)]
pub enum VarExpr {
   Regex(String),
   If(TokenExpr),
}

#[derive(Debug, Deserialize, Clone)]
pub struct TokenExpr {
    pub tag: String,
    vars: Option<BTreeMap<String, VarExpr>>,
}

pub type Token = TokenExpr;

pub type DB = Vec<BTreeMap<String, String>>;

impl TokenExpr {
    pub fn new_with_tag(tag: &String) -> TokenExpr {
        TokenExpr {
            tag: tag.clone(), 
            vars: None,
        }
    }

    pub async fn evaluate(&self, rx: &mut mpsc::UnboundedReceiver<InputToken>, syn: &parser::Syntax) -> (bool, DB) {
        let mut results = Vec::default();

        match rx.recv().await {
            Some(InputToken::Channel(mut text)) => {
                if let Ok((_, mut result)) = self.parse(rx, text.as_str(), false, syn).await {
                    results.append(&mut result);
                    
                    loop {
                        match rx.recv().await {
                            Some(InputToken::Channel(text)) => {
                                if let Ok((_, mut row)) = self.parse(rx, &text[..], false, syn).await {
                                    results.append(&mut row);
                                } else {
                                    break
                                }
                            },
                            Some(InputToken::Byte(b'\0')) => return (true, results),
                            _ => break
                        }
                    }
                }
            },
            Some(InputToken::Byte(b'\0')) => return (true, results),
            _ => {}
        }

        (false, results)
    }

    pub async fn parse<'a>(&self, rx: &mut mpsc::UnboundedReceiver<InputToken>, text: &'a str, without_block: bool, syn: &parser::Syntax) -> IResult<String, DB> {
        let (_, tokens) = if without_block {
            parser::parse_template(self.tag.as_bytes(), &syn).unwrap()
        } else {
            parser::parse_template(self.tag.as_bytes(), &syn).unwrap()
        };
       
        log::debug!("first: {}", text);
        log::debug!("tokens: {:?}", tokens);

        Self::parse_token(rx, &text.to_string(), &tokens).await
    }

    fn merge<'a>(first_context: &BTreeMap<String, String>, second_context: &BTreeMap<String, String>) -> BTreeMap<String, String> {
        let mut new_context = BTreeMap::new();

        for (key, value) in first_context.iter() {
            new_context.insert(key.clone(), value.clone());
        }

        for (key, value) in second_context.iter() {
            new_context.insert(key.clone(), value.clone());
        }

        new_context
    }

    async fn read_line(rx: &mut mpsc::UnboundedReceiver<InputToken>) -> String {
        match rx.recv().await {
            Some(InputToken::Channel(line)) => line,
            _ => String::default() 
        }
    }

    #[async_recursion]
    async fn parse_token<'a>(rx: &mut mpsc::UnboundedReceiver<InputToken>, input: &String, tokens: &Vec<parser::Node<'a>>) -> IResult<String, DB>{
        let mut input = input.to_string();
        let mut h: BTreeMap<String, String> = BTreeMap::default();

        for (idx, token) in tokens.iter().enumerate() {
            if input.is_empty() {
                input = Self::read_line(rx).await;
            }

            match token {
                parser::Node::Lit(a, b, c) => {
                    let a: IResult<&str, &str> = tag(&format!("{}{}{}", a, b, c)[..])(input.as_str());
                    match a {
                        Ok((rest, _b)) => input = rest.to_string(),
                        _ => return Err(default_error(input.as_str()).map(|(s, k)| (s.to_string(), k)))
                    }
                },
                parser::Node::Expr(_, parser::Expr::Filter("skip", _)) => {
                    let next = tokens.get(idx + 1);
                    let result = token_expr(input.as_str(), next);
                    
                    if let Ok((rest, _)) = result {
                        input = rest.to_string();
                    } else {
                        input = String::default();
                    }

                },
                parser::Node::Expr(_, parser::Expr::Var(key)) => {
                    let next = tokens.get(idx + 1);
                    let result = token_expr(input.as_str(), next);

                    if let Ok((rest, hit)) = result {
                        input = rest.to_string();
                        h.insert(key.to_string(), hit);
                    } else {
                        input = String::default();
                    }
                },
                parser::Node::Cond(exprs, _) => {
                    for (_ws, expr, ns) in exprs.iter() {
                        match expr {
                            Some(parser::Expr::BinOp(op, left, right)) => {
                                if Self::bin_op(&mut h, op, left, right) {
                                    if let Ok((_, h2)) = Self::parse_token(rx, &input, ns).await {
                                        for m in h2.iter() {
                                            for (k, v) in m.iter() {
                                                h.insert(k.to_string(), v.to_owned());
                                            }
                                        }
                                    } 
                                }
                            },
                            _ => {}
                        }
                    }
                },
                parser::Node::Loop(_, _, parser::Expr::Range("..", Some(s), Some(e)), nodes, _) => {
                    let s: u32 = Self::get_variable(&mut h, s).unwrap_or_default();
                    let e: u32 = Self::get_variable(&mut h, e).unwrap_or_default();
                    
                    for n in s..e {
                        if let Ok((_, h2)) = Self::parse_token(rx, &input, nodes).await {
                            for m in h2.iter() {
                                for (k, v) in m.iter() {
                                    h.insert(format!("i{}_{}", n, k), v.to_owned());
                                }
                            }
                        }
                        
                        input = Self::read_line(rx).await
                    }
                }

                _ => {},
            }
        };
        
        if h.is_empty() {
            IResult::Ok((String::default(), Vec::default()))
        } else {
            IResult::Ok((String::default(), vec![h]))
        }

    }

    fn bin_op(h: &mut BTreeMap<String, String>, op: &str, left: &parser::Expr, right: &parser::Expr) -> bool {
        match op {
            "==" =>Self::get_variable::<String>(&h, left) == Self::get_variable::<String>(&h, right),
            "!=" =>Self::get_variable::<String>(&h, left) != Self::get_variable::<String>(&h, right),
            ">" => Self::get_variable::<f64>(&h, left) > Self::get_variable::<f64>(&h, right),
            "<" => Self::get_variable::<f64>(&h, left) < Self::get_variable::<f64>(&h, right),
            ">=" => Self::get_variable::<f64>(&h, left) >= Self::get_variable::<f64>(&h, right),
            "<=" => Self::get_variable::<f64>(&h, left) <= Self::get_variable::<f64>(&h, right),
            _ => false
        }
    }
    
    fn get_variable<F>(h: &BTreeMap<String, String>, expr: &parser::Expr) -> Option<F> 
        where F: std::str::FromStr + PartialOrd, F::Err: std::fmt::Debug {
        let a = match expr {
            parser::Expr::Var(n) => h.get(*n).map(String::to_string),
            parser::Expr::NumLit(num) => Some(num.to_string()),
            parser::Expr::StrLit(s) => Some(s.to_string()),
            _ => None
        };

        a.map(|v| v.parse::<F>().unwrap())
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

fn make_error(input: &str, kind: nom::error::ErrorKind) -> nom::Err<(&str, nom::error::ErrorKind)> {
    let err = (input, kind);
    nom::Err::Error(err)
}

fn default_error(input: &str) -> nom::Err<(&str, nom::error::ErrorKind)> {
    make_error(input, nom::error::ErrorKind::Eof)
}

fn token_expr<'a>(input: &'a str, token: Option<&parser::Node>) -> IResult<&'a str, String> {
   let result: IResult<&str, &str> = if let Some(parser::Node::Lit(a, b, c)) = token {
        let mut result: IResult<&str, &str> = Err(default_error(input));
        let mut idx = 0usize; 
        for ch in input.chars().into_iter() {
            idx += ch.len_utf8();

            let s = &input[idx..];
            let t: IResult<&str, &str> = alt((tag("\n"), tag(&format!("{}{}{}", a, b, c)[..])))(s);

            if let Ok(_) = t {
                result = Ok((&input[idx..input.len()], &input[0..idx]));
                break
            } 
        }

        result
    } else {
        take_until("\n")(input)
    };

    result.map(|(a, b)| (a, b.to_string()))
}

#[allow(dead_code)]
pub fn slice_to_string(s: &[u8]) -> String {
    String::from_utf8(s.to_vec()).unwrap()
}

pub struct App {
    tx: mpsc::UnboundedSender<InputToken>,
    // pub handler: Option<JoinHandle<()>>,
    config: AppConfig,
    pub db: std::cell::RefCell<db::Glue>,
}

impl App {
    pub async fn new_with_config(tx: mpsc::UnboundedSender<InputToken>, config: AppConfig) -> anyhow::Result<App> {
        let db= db::Glue::new();
        
        Ok(App {
            tx,
            config,
            db: std::cell::RefCell::new(db),
            //handler: Some(handler),
        })
    }

    pub async fn execute_query(&self, query: String) -> anyhow::Result<Option<gluesql::Payload>>{
        self.db.borrow_mut().execute(query.as_str()).await
    }

    pub fn send_byte(&self, b: u8) -> anyhow::Result<()> {
        self.tx.send(InputToken::Byte(b))?;
        
        Ok(())
    }
    
    pub fn send_string(&self, txt: String) -> anyhow::Result<()> {
        self.tx.send(InputToken::Channel(txt.clone()))?;
        
        Ok(())
    }

    pub async fn parse_handler(&self, rx: &mut mpsc::UnboundedReceiver<InputToken>, templates: Vec<TokenExpr>) -> anyhow::Result<()> {
        let syn = parser::Syntax::default();
        let mut rows: Vec<BTreeMap<String, String>> = Vec::default();

        'main: loop {
            for template in templates.iter() {
                let (is_break, mut row) = template.evaluate(rx, &syn).await;
                rows.append(&mut row);
                
                if is_break {
                    break 'main
                }
            }
        }

        let titles = rows.iter().fold(HashSet::<String>::default(), |acc, row| {
            let ks: HashSet<String> =
                row.keys().cloned().collect();

            acc.union(&ks)
                .cloned()
                .collect::<HashSet<String>>() 
        }).into_iter().collect::<Vec<_>>();
        
        self.db.borrow_mut().create_table(None, titles).await?;
        
        for row in rows.iter() {
            self.db.borrow_mut().insert(row).await?;
        }

        Ok(())
    }

    pub async fn input_handler(&self) -> anyhow::Result<()> { 
        let stdin = std::io::stdin();

        loop {
            let mut buf = Vec::with_capacity(1024usize);

            match stdin.lock().read_until(b'\n', &mut buf) {
                Ok(n) => {
                    let line = String::from_utf8_lossy(&buf).to_string();
                    if n == 0 {
                        self.send_byte(b'\0')?;
                        break;
                    }
                    
                    self.send_string(line.to_string())?;
                }
                Err(e) => {
                    error!("{}", e.to_string());
                    break;
                },
            }
        }

        Ok(())
    }

    pub async fn execute(&self, sql: &str) -> anyhow::Result<Option<gluesql::Payload>> {
        self.db.borrow_mut().execute(sql).await
    }
}
