use std::collections::HashMap;
use prettytable::{ Table };

pub fn printstd(rows: &Vec<HashMap<String, String>>) {
    dbg!(rows) ;
    
    let table = Table::new();
    table.printstd();
}