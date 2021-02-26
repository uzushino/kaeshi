use async_trait::async_trait;

use im::{vector, HashMap, Vector};

use gluesql_core::{data, parser::ast::{ColumnDef, ColumnOption, ColumnOptionDef, Value as AstValue}};
use gluesql_core::{
    AlterTable, AlterTableError, MutResult, Result, Row, RowIter, Schema, Store, StoreMut, Value,
};

#[derive(Clone, Debug)]
pub struct DataKey {
    pub table_name: String,
    pub id: u64,
}

#[derive(Clone)]
pub struct MemoryStorage {
    schema_map: HashMap<String, Schema>,
    data_map: HashMap<String, Vec<(u64, Row)>>,
    id: u64,
}

impl MemoryStorage {
    pub fn new() -> Result<Self> {
        let schema_map = HashMap::new();
        
        Ok(Self {
            schema_map,
            data_map: HashMap::default(),
            id: 0,
        })
    }
}

#[async_trait(?Send)]
impl StoreMut<DataKey> for MemoryStorage {
    async fn insert_schema(self, schema: &Schema) -> MutResult<Self, ()> {
        let table_name = schema.table_name.to_string();
        let mut s= HashMap::default();

        s.insert(table_name, schema.clone());

        let storage = Self {
            schema_map: s,
            data_map: self.data_map,
            id: self.id,
        };

        Ok((storage, ()))
    }

    async fn delete_schema(self, table_name: &str) -> MutResult<Self, ()> {
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

    async fn insert_data(self, table_name: &str, rows: Vec<Row>) -> MutResult<Self, ()> {
        let Self {
            schema_map,
            mut data_map,
            id: self_id,
        } = self;

        for row in rows.iter() {
            let new_rows= match data_map.get_mut(table_name) {
                Some(rows) => {
                    let rows= match rows.into_iter().position(|(item_id, _)| *item_id == self_id) {
                        Some(index) => {
                            rows[index] = (self_id, row.clone());
                            rows
                        },
                        None => {
                            rows.push((self_id, row.clone()));
                            rows
                        }
                    };

                    rows.clone()
                }
                _ => vec![(self_id, row.clone())]
            };

            data_map.insert(table_name.to_string(), new_rows);
        }

        Ok((Self {
            schema_map,
            data_map: data_map,
            id: self_id,
        }, ()))
    }

    async fn delete_data(self, _key: Vec<DataKey>) -> MutResult<Self, ()> {
        Ok((self, ()))
    }

    async fn update_data(self, _rows: Vec<(DataKey, Row)>) -> MutResult<Self, ()> {
        Ok((self, ()))
    }
}

#[async_trait(?Send)]
impl Store<DataKey> for MemoryStorage {
    async fn fetch_schema(&self, table_name: &str) -> Result<Option<Schema>> {
        let schema = self
            .schema_map
            .get(table_name)
            .cloned();

        Ok(schema)
    }

    async fn scan_data(&self, table_name: &str) -> Result<RowIter<DataKey>> {
        let items = match self.data_map.get(table_name) {
            Some(items) => items
                .iter()
                .map(|(id, row)| {
                    let key = DataKey {
                        table_name: table_name.to_string(),
                        id: *id,
                    };

                    (key, row.clone())
                })
                .collect(),
            None => vec![],
        };

        Ok(Box::new(items.into_iter().map(Ok)))
    }
}

#[async_trait(?Send)]
impl AlterTable for MemoryStorage {
    async fn rename_schema(self, _table_name: &str, _new_table_name: &str) -> MutResult<Self, ()> {
        Ok((self, ()))
    }

    async fn rename_column(self, _table_name: &str, _old_column_name: &str, _new_column_name: &str) -> MutResult<Self, ()> {
        Ok((self, ()))
    }

    async fn add_column(self, _table_name: &str, _column_def: &ColumnDef) -> MutResult<Self, ()> {
        Ok((self, ()))
    }

    async fn drop_column(self, _table_name: &str, _column_name: &str, _if_exists: bool) -> MutResult<Self, ()> {
        Ok((self, ()))
    }
}
