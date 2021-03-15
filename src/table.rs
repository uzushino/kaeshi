use std::{collections::{ HashSet, BTreeMap }};
use prettytable::{ Table, Cell, Row };
use std::io::Write;

pub fn printjson<W>(mut writer: W, rows: &Vec<BTreeMap<String, String>>) -> anyhow::Result<(), std::io::Error> where W: Write {
    let j = serde_json::to_string(&rows).unwrap();
    writer.write_all(j.as_bytes())
}

pub fn printstd<W>(mut writer: W, rows: &Vec<BTreeMap<String, String>>) -> anyhow::Result<usize> 
   where W: Write  {
    let titles: HashSet<String> = rows.iter().fold(HashSet::<String>::default(), |acc, row| {
        let ks: HashSet<String> =
            row.keys().cloned().collect();

        acc.union(&ks)
            .cloned()
            .collect::<HashSet<String>>() 
    });

    let mut table = Table::new();
    table.set_titles(titles.iter().collect());

    for row in rows {
        let record = titles.iter().fold(Vec::default(), |mut acc, title| {
            let a = match row.get(title) {
                Some(title) => title.as_ref(),
                _ => "",
            };
            acc.push(Cell::new(a));
            acc
        });

        table.add_row(Row::new(record));
    }

    table
        .print(&mut writer)
        .map_err(|_| anyhow::anyhow!("Cannot write output"))
}

pub fn printstd_noheader<W>(mut writer: W, rows: &Vec<Vec<String>>) -> anyhow::Result<usize> where W: Write  { 
    let mut table = Table::new();
    for row in rows {
        table.add_row(Row::new(row.iter().map(|c| Cell::new(c)).collect()));
    }

    table
        .print(&mut writer)
        .map_err(|_| anyhow::anyhow!("Cannot write output"))
}
