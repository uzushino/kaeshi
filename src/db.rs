use std::collections::HashMap;
use gluesql::{Error, MutResult, Result, Row, RowIter, Schema, Store, StoreError, StoreMut, Value};

pub struct MemoryStorage {
    schema_map: HashMap<String, Schema>,
    data_map: HashMap<String, DB>,
    id: u64,
}

use crate::app::DB;

impl MemoryStorage {
    pub fn new(data: HashMap<String, DB>) -> Result<Self> {
        let mut schema_map = HashMap::new();
        let schema = Schema { 
            table_name: "public".to_owned(),
            column_defs: Vec::default(),
        };
        schema_map.insert("public".to_owned(), schema);

        Ok(Self {
            schema_map,
            data_map: data,
            id: 0,
        })
    }
}

#[derive(Clone, Debug)]
pub struct DataKey {
    pub table_name: String,
    pub id: u64,
}


impl StoreMut<DataKey> for MemoryStorage {
    fn generate_id(self, table_name: &str) -> MutResult<Self, DataKey> {
        let id = self.id + 1;
        let storage = Self {
            schema_map: self.schema_map,
            data_map: self.data_map,
            id,
        };

        let key = DataKey {
            table_name: table_name.to_string(),
            id,
        };

        Ok((storage, key))
    }

    fn insert_schema(self, schema: &Schema) -> MutResult<Self, ()> {
        Ok((self, ()))
    }

    fn delete_schema(self, table_name: &str) -> MutResult<Self, ()> {
        let Self {
            mut schema_map,
            mut data_map,
            id,
        } = self;

        data_map.remove(table_name);
        schema_map.remove(table_name);
        let storage = Self {
            schema_map,
            data_map,
            id,
        };

        Ok((storage, ()))
    }

    fn insert_data(self, key: &DataKey, row: Row) -> MutResult<Self, Row> {
        Ok((self, row))
    }

    fn delete_data(self, key: &DataKey) -> MutResult<Self, ()> {
        Ok((self, ()))
    }
}

impl Store<DataKey> for MemoryStorage {
    fn fetch_schema(&self, table_name: &str) -> Result<Schema> {
        let schema = self
            .schema_map
            .get(table_name)
            .ok_or(StoreError::SchemaNotFound)?
            .clone();

        Ok(schema)
    }

    fn scan_data(&self, table_name: &str) -> Result<RowIter<DataKey>> {
        let items = match self.data_map.get(table_name) {
            Some(items) => {
                let mut kv = Vec::default();

                for (id, db) in items.iter().enumerate() {
                    let key = DataKey {
                        table_name: table_name.to_string(),
                        id: id as u64,
                    };

                    let rows = db.iter().map(|(_k, v)| Value::Str(v.clone())).collect();

                    kv.push((key, Row(rows)));
                }

                kv
            }
            None => Vec::default(),
        };

        let items = items.into_iter().map(Ok);

        Ok(Box::new(items))
    }
}

mod test {
    use std::collections::HashMap;
    use super::*;

    #[test]
    fn it_select() {
        let mut db: HashMap<String, DB> = HashMap::default();
        let row: DB = Vec::default();
        db.insert("aaa".to_string(), row);

        let storage = MemoryStorage::new(db);
        let query = gluesql::parse("SELECT 1 FROM public").unwrap();

        match gluesql::execute(storage.unwrap(), &query[0]) {
            Ok((storage, payload)) => {
                println!("{:?}", payload);
            },
            Err((storage, error)) => {
                println!("{:?}", error);
            }
        }
    }
}
