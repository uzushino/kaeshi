use std::collections::HashMap;
use gluesql::{Error, MutResult, Result, Row, RowIter, Schema, Store, StoreError, StoreMut, Value};

pub struct Glue {
    storage: Option<MemoryStorage>
}

impl Glue {
    pub fn new() -> Self {
        let db = HashMap::default();
        let storage = MemoryStorage::new(db).unwrap();
        
        Glue {
            storage: Some(storage)
        }
    }

    pub fn execute(&mut self, sql: String) {
        let query = gluesql::parse("
        CREATE TABLE Test (id INTEGER, msg TEXT);
        SELECT * FROM Test;
        ").unwrap();

        for q in query {
            let s = self.storage.take().unwrap();
            
            if let Ok((s, payload)) = gluesql::execute(s, &q) {
                self.storage = Some(s);
                println!("{:?}", payload);
            }
        }
    }
}

pub struct MemoryStorage {
    schema_map: HashMap<String, Schema>,
    data_map: HashMap<String, (usize, DB)>,
    id: u64,
}

use crate::app::DB;

impl MemoryStorage {
    pub fn new(data: HashMap<String, (usize, DB)>) -> Result<Self> {
        let mut schema_map = HashMap::new();
        
        let schema = Schema { 
            table_name: "public".to_owned(),
            column_defs: vec![],
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
        let table_name = schema.table_name.to_string();
        let mut s= HashMap::default();
        s.insert(table_name, schema.clone());
        //let schema_map = self.schema_map.update(table_name, schema.clone());
        let storage = Self {
            schema_map: s,
            data_map: self.data_map,
            id: self.id,
        };

        Ok((storage, ()))
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
        let DataKey { table_name, id } = key;
        let table_name = table_name.to_string();
        let item = (*id, row.clone());
        let Self {
            schema_map,
            data_map,
            id: self_id,
        } = self;

        let storage = Self {
            schema_map,
            data_map,
            id: self_id,
        };

        Ok((storage, row))
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

                for (id, db) in items.0.iter().enumerate() {
                    let key = DataKey {
                        table_name: table_name.to_string(),
                        id: id as u64,
                    };

                    let rows = db.iter().map(|(_k, v)| Value::Str(v.clone())).collect();

                    kv.push((key, Row(rows)));
                }
                println!("aa: {:?}", kv);
                kv
            }
            None => Vec::default(),
        };

        let items = items.into_iter().map(Ok);

        Ok(Box::new(items))
    }
}

mod test {
    use std::collections::BTreeMap;
    use super::*;

    #[test]
    fn it_select() {
        //let mut db: HashMap<String, DB> = HashMap::default();
        
        //let mut data: BTreeMap<String, String> = BTreeMap::default();
        // data.insert("name".to_owned(), "hoge".to_owned());

        //let row: DB = vec![];
        //db.insert("aaa".to_string(), row);

        let mut glue = Glue::new();
        //let storage = MemoryStorage::new(db).unwrap();

        glue.execute("SELECT * FROM TEST;".into());
    }
}
