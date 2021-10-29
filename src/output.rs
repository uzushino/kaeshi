use std::collections::{BTreeMap, HashSet};
use std::io::Write;

use prettytable::{Cell, Row, Table};
use crate::OutputType;

pub fn print<W>(writer: W, rows: &Vec<BTreeMap<String, String>>, output_type: OutputType) -> anyhow::Result<usize>
where W: Write {
    match output_type {
        OutputType::Table => printstd(writer, rows),
        OutputType::JSON => printjson(writer, rows),
        _ => Ok(0usize)
    }
}

pub fn printjson<W>(
    mut writer: W,
    rows: &Vec<BTreeMap<String, String>>,
) -> anyhow::Result<usize, anyhow::Error>
where
    W: Write,
{
    let j = serde_json::to_string(&rows).unwrap();

    writer.write_all(j.as_bytes())
        .and_then(|_| Ok(1usize))
        .map_err(|_| anyhow::anyhow!("Cannot write output"))
}

pub fn printstd<W>(mut writer: W, rows: &Vec<BTreeMap<String, String>>) -> anyhow::Result<usize>
where
    W: Write,
{
    let titles: HashSet<String> = rows.iter().fold(HashSet::<String>::default(), |acc, row| {
        let ks: HashSet<String> = row.keys().cloned().collect();

        acc.union(&ks).cloned().collect::<HashSet<String>>()
    });

    let mut table = Table::new();
    table.set_titles(titles.iter().collect());

    for row in rows {
        let record = titles.iter().fold(Vec::default(), |mut acc, title| {
            acc.push(Cell::new(
                row.get(title).map(|s| s.as_ref()).unwrap_or_default(),
            ));
            acc
        });

        table.add_row(Row::new(record));
    }

    table
        .print(&mut writer)
        .map_err(|_| anyhow::anyhow!("Cannot write output"))
}

pub fn printstd_noheader<W>(mut writer: W, rows: &Vec<Vec<String>>) -> anyhow::Result<usize>
where
    W: Write,
{
    let mut table = Table::new();

    for row in rows {
        table.add_row(Row::new(row.iter().map(|c| Cell::new(c)).collect()));
    }

    table
        .print(&mut writer)
        .map_err(|_| anyhow::anyhow!("Cannot write output"))
}
