use anyhow::anyhow;
use gluesql::Payload;
use chrono::prelude::*;

use super::storage::Storage;

#[derive(Clone)]
pub struct Glue {
    storage: Option<Storage>
}

impl Glue {
    pub fn new() -> Self {
        let storage = Storage::new().unwrap();
        
        Glue {
            storage: Some(storage)
        }
    }

    pub fn create_table(&mut self, columns: Vec<&String>) -> anyhow::Result<Option<Payload>> {
        let s: Vec<String> = columns
            .iter()
            .map(|s| format!("{} TEXT", s))
            .collect();
        let s = s.join(",");
        self.execute(format!("CREATE TABLE TextStore ({}, created_at TEXT);", s).as_str())
    }
    
    pub fn insert(&mut self, msg: &str) -> anyhow::Result<Option<Payload>> {
        let local: DateTime<Local> = Local::now();
        self.execute(
            format!(r#"INSERT INTO TextStore VALUES ("{}", "{}")"#, msg, local.to_rfc3339()).as_str())
    }

    pub fn execute(&mut self, sql: &str) -> anyhow::Result<Option<Payload>> {
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

        glue.insert("Hoge");

        let query = glue.execute("SELECT * FROM TextStore;");
        match query {
            Ok(Some(Payload::Select(v))) => {
                println!("{:?}", v);
            },
            n => { println!("{:?}", n) }
        }
    }
}
