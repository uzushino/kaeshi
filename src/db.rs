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

    pub fn create_table(&mut self) -> anyhow::Result<Option<Payload>> {
        self.execute("CREATE TABLE JsonStore (msg TEXT, created_at TEXT);")
    }
    
    pub fn insert(&mut self, msg: &str) -> anyhow::Result<Option<Payload>> {
        let local: DateTime<Local> = Local::now();

        log::debug!("{:?}", msg);

        self.execute(
            format!(r#"INSERT INTO JsonStore VALUES ("{}", "{}")"#, msg, local.to_rfc3339()).as_str())
    }

    pub fn execute(&mut self, sql: &str) -> anyhow::Result<Option<Payload>> {
        let query = gluesql::parse(sql)?;

        let q = query.get(0);
        if let Some(q) = q {
            let strage = self.storage.take().unwrap();
            
            if let Ok((s, payload)) =  gluesql::execute(strage.clone(), &q) {
                self.storage = Some(s);
                return Ok(Some(payload));
            } else {
                self.storage = Some(strage);
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

        glue.create_table();
        glue.insert("Hoge");

        let query = glue.execute("SELECT * FROM JsonStore;");
        match query {
            Ok(Some(Payload::Select(v))) => {
                println!("{:?}", v);
            },
            n => { println!("{:?}", n) }
        }
    }
}
