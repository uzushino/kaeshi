use std::collections::BTreeMap;
use std::option::Option;
use serde::Deserialize;
use nom::{
    IResult,
    branch::alt,
    bytes::streaming::take_until,
    bytes::complete::tag,
};
use log::{ debug, error };
// use crossbeam_channel::{ self, unbounded, Sender, Receiver };
use tokio::sync::mpsc;

use super::parser;
use super::table;
use super::db;
use std::panic::AssertUnwindSafe;
use std::io::{ BufRead };
use std::collections::{ HashSet };

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
    // Begin
    begin: Option<String>,
    // End
    end: Option<String>,

    vars: Option<BTreeMap<String, VarExpr>>,
}

pub type Token = TokenExpr;

pub type DB = Vec<BTreeMap<String, String>>;

impl TokenExpr {
    pub fn new_with_tag(tag: &String) -> TokenExpr {
        TokenExpr {
            tag: tag.clone(), 
            many: None, 
            count: None,
            begin: None,
            end: None,
            vars: None,
        }
    }

    pub async fn parse_count(&self, rx: &mut mpsc::UnboundedReceiver<InputToken>, syn: &parser::Syntax) -> (bool, DB) {
        let mut results = Vec::default();

        for _ in 1..self.count.unwrap_or(1) {
            match rx.recv().await {
                Some(InputToken::Channel(text)) => {
                    if let Ok((_, mut row)) = self.parse(&text[..], syn) {
                        results.append(&mut row);
                    }
                },
                Some(InputToken::Byte(b'\0')) => return (true, results),
                _ => break
            }
        }

        (false, results)
    }

    pub async fn parse_many(&self, mut rx: mpsc::UnboundedReceiver<InputToken>, syn: &parser::Syntax) -> (bool, DB) {
        let mut results = Vec::default();

        loop {
            match rx.recv().await {
                Some(InputToken::Channel(text)) => {
                    if let Ok((_, mut row)) = self.parse(&text[..], syn) {
                        results.append(&mut row);
                    } else {
                        break
                    }
                },
                Some(InputToken::Byte(b'\0')) => return (true, results),
                _ => break
            }
        }

        (false, results)
    }

    pub async fn evaluate(&self, rx: &mut mpsc::UnboundedReceiver<InputToken>, syn: &parser::Syntax) -> (bool, DB) {
        let mut results = Vec::default();

        match rx.recv().await {
            Some(InputToken::Channel(text)) => {
                if let Ok((_, mut result)) = self.parse(text.as_str(), syn) {
                    results.append(&mut result);

                    if self.count.is_some() {
                        self.parse_count(rx, syn).await;
                    } else if self.many.is_some() {
                        loop {
                            match rx.recv().await {
                                Some(InputToken::Channel(text)) => {
                                    if let Ok((_, mut row)) = self.parse(&text[..], syn) {
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
                }
            },
            Some(InputToken::Byte(b'\0')) => return (true, results),
            _ => {}
        }

        (false, results)
    }

    pub fn parse<'a>(&self, text: &'a str, syn: &parser::Syntax) -> IResult<&'a str, DB> {
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
       
       for (u, _ch) in input.chars().into_iter().enumerate() {
           let s = &input[u..];
           let t: IResult<&str, &str> = alt((tag("\n"), tag(&format!("{}{}{}", a, b, c)[..])))(s);

           if let Ok(_) = t {
               result = Ok((&input[u..input.len()], &input[0..u]));
               break
           } 
       }

       result
    } else {
        take_until("\n")(input)
    };

    result.map(|(a, b)| (a, b.to_string()))
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
                        _ => return Err(default_error(input))
                    }
                },
                parser::Node::Expr(_, parser::Expr::Filter("trim", vars)) => {
                    if let Some(parser::Expr::Var(t)) = vars.first() {
                        let next = tokens.get(idx + 1);
                        let result = token_expr(input, next);

                        if let Ok((rest, hit)) = result {
                            input = rest;
                            h.insert(t.to_string(), hit.trim().to_string());
                        } else {
                            input = "";
                        }
                    }
                },
                parser::Node::Expr(_, parser::Expr::Var(key)) => {
                    let next = tokens.get(idx + 1);
                    let result = token_expr(input, next);

                    if let Ok((rest, hit)) = result {
                        input = rest;
                        h.insert(key.to_string(), hit);
                    } else {
                        input = "";
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

    pub async fn execute_query(&self, query: String) -> anyhow::Result<Option<gluesql_core::Payload>>{
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

    pub async fn execute(&self, sql: &str) -> anyhow::Result<Option<gluesql_core::Payload>> {
        self.db.borrow_mut().execute(sql).await
    }
}
