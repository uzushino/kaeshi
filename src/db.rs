use anyhow::anyhow;
use gluesql::Payload;
use chrono::prelude::*;
use std::collections::BTreeMap;
use sql_builder::esc;

use super::storage::Storage;

#[derive(Clone)]
pub struct Glue {
    table_name: Option<String>,
    columns: Vec<String>,
    storage: Option<Storage>
}

impl Glue {
    pub fn new() -> Self {
        let storage = Storage::new().unwrap();
        
        Glue {
            table_name: None,
            columns: Vec::default(),
            storage: Some(storage)
        }
    }

    pub fn create_table(&mut self, table_name: Option<String>, columns: Vec<&String>) -> anyhow::Result<Option<Payload>> {
        self.table_name = table_name;

        self.columns = columns
            .iter()
            .map(|c| c.to_string())
            .collect();

        let s = self.columns
            .iter()
            .map(|s| format!(r#""{}" TEXT"#, s))
            .collect::<Vec<_>>()
            .join(",");

        self.execute(format!("CREATE TABLE {} ({}, created_at TEXT);", self.table_name(), s).as_str())
    }
   
    fn table_name(&self) -> String {
        self.table_name.as_ref().unwrap_or(&"main".to_string()).to_string()
    }

    pub fn insert(&mut self, row: &BTreeMap<String, String>) -> anyhow::Result<Option<Payload>> {
        let local: DateTime<Local> = Local::now();

        let c = self.columns
            .iter()
            .map(|c| {
                let a = row.get(c).map(|c| c.to_string()).unwrap_or(String::default());
                format!(r#"{}"#, esc(a.as_str()).to_string())
            })
            .collect::<Vec<_>>();

        let sql = { 
            format!(r#"INSERT INTO {} VALUES ({}, "{}")"#, 
                self.table_name().as_str(), 
                c.join(","), 
                esc(local.to_rfc3339().as_str())
            )
        };

        self.execute(sql.as_str())
    }

    pub fn execute(&mut self, sql: &str) -> anyhow::Result<Option<Payload>> {
        log::debug!("sql=> {}", sql);

        let query = gluesql::parse(sql)?;
        let q = query.get(0);

        if let Some(q) = q {
            let storage = self.storage.take().unwrap();
            
            if let Ok((s, payload)) =  gluesql::execute(storage.clone(), &q) {
                self.storage = Some(s);
                return Ok(Some(payload));
            } else {
                self.storage = Some(storage);
            }
        }

        Err(anyhow!("Error: {}", sql))
    }
}

mod test {
    use super::*;

    #[test]
    fn it_select() {
        let mut glue = Glue::new();

        let query = glue.execute("SELECT * FROM main;");
        match query {
            Ok(Some(Payload::Select(v))) => {
                println!("{:?}", v);
            },
            n => { println!("{:?}", n) }
        }
    }
}
