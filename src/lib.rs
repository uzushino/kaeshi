mod app;
mod db;
mod parser;
mod storage;
pub mod output;

use clap::arg_enum;

pub use app::{App, AppConfig, InputToken, TokenExpr, DB};

arg_enum! {
    #[derive(Debug)]
    pub enum OutputType {
        Table,
        Csv,
        JSON,
    }
}
