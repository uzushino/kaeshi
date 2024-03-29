use anyhow::anyhow;

use chrono::prelude::*;
use gluesql::Payload;
use sql_builder::esc;
use std::collections::BTreeMap;

use super::storage::MemoryStorage;
use futures_await_test::async_test;

#[derive(Clone)]
pub struct Glue {
    table_name: Option<String>,
    timestamp: Option<String>,
    columns: Vec<String>,
    storage: Option<MemoryStorage>,
}

impl Glue {
    pub fn new() -> Self {
        let storage = MemoryStorage::new();

        Glue {
            table_name: None,
            timestamp: None,
            columns: Vec::default(),
            storage: Some(storage),
        }
    }

    pub async fn create_table(
        &mut self,
        table_name: Option<String>,
        columns: Vec<String>,
        timestamp: Option<String>,
    ) -> anyhow::Result<Option<Payload>> {
        self.table_name = table_name;
        self.timestamp = timestamp;

        self.columns = columns.iter().map(|c| c.to_string()).collect();

        let s = self
            .columns
            .iter()
            .map(|s| format!(r#"{} TEXT"#, s.trim()))
            .collect::<Vec<_>>()
            .join(",");

        if let Some(timestamp) = &self.timestamp {
            self.execute(
                format!(
                    "CREATE TABLE {} ({}, {} timestamp);",
                    self.table_name(),
                    s,
                    timestamp
                )
                .as_str(),
            )
            .await
        } else {
            self.execute(format!("CREATE TABLE {} ({});", self.table_name(), s).as_str())
                .await
        }
    }

    fn table_name(&self) -> String {
        self.table_name
            .as_ref()
            .unwrap_or(&"kaeshi".to_string())
            .to_string()
    }

    fn sql_value(s: &String) -> String {
        format!("'{}'", esc(s))
    }

    pub async fn insert(
        &mut self,
        row: &BTreeMap<String, String>,
    ) -> anyhow::Result<Option<Payload>> {
        let c = self
            .columns
            .iter()
            .map(|c| {
                row.get(c)
                    .map(|c| c.as_str().replace("'", "\'").to_string())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();

        let sql = if self.timestamp.is_none() {
            format!(
                r#"INSERT INTO {} VALUES ({})"#,
                self.table_name().as_str(),
                c.iter().map(Self::sql_value).collect::<Vec<_>>().join(","),
            )
        } else {
            let local: DateTime<Local> = Local::now();

            format!(
                r#"INSERT INTO {} VALUES ({}, '{}')"#,
                self.table_name().as_str(),
                c.iter().map(Self::sql_value).collect::<Vec<_>>().join(","),
                local.to_rfc3339().as_str()
            )
        };

        self.execute(sql.as_str()).await
    }

    pub async fn execute(&mut self, sql: &str) -> anyhow::Result<Option<Payload>> {
        let query = gluesql::parse(sql).unwrap();
        let q = query.get(0);

        if let Some(q) = q {
            let storage = self.storage.take().unwrap();
            let q = gluesql::translate(q).unwrap();

            if let Ok((s, payload)) = gluesql::execute(storage.clone(), &q).await {
                self.storage = Some(s);
                return Ok(Some(payload));
            } else {
                self.storage = Some(storage);
            }
        }

        Err(anyhow!("Error: {}", sql))
    }
}

#[allow(unused_imports)]
mod test {
    use super::*;

    #[async_test]
    async fn it_select() {
        let mut glue = Glue::new();
        let _ = glue
            .create_table(Some("kaeshi".to_string()), vec!["id".to_string()], None)
            .await;
        let query = glue.execute("SELECT * FROM kaeshi").await;

        match query {
            Ok(Some(Payload::Select {
                labels: l, rows: _, ..
            })) => {
                assert_eq!(vec!["id"], l);
            }
            n => {
                println!("{:?}", n)
            }
        }
    }

    #[async_test]
    async fn it_select_with_timestamp() {
        let mut glue = Glue::new();
        let _ = glue
            .create_table(
                Some("kaeshi".to_string()),
                vec!["id".to_string()],
                Some(String::from("created_at")),
            )
            .await;
        let query = glue.execute("SELECT * FROM kaeshi").await;

        match query {
            Ok(Some(Payload::Select {
                labels: l, rows: _, ..
            })) => {
                assert_eq!(vec!["id", "created_at"], l);
            }
            n => {
                println!("{:?}", n)
            }
        }
    }
}
