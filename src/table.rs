use std::collections::{ HashMap, HashSet };
use prettytable::{ Table, Cell, Row };

pub fn printstd(rows: &Vec<HashMap<String, String>>) {
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

    table.printstd();
}