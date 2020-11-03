use std::collections::HashMap;
use gluesql::{MutResult, Result, Row, RowIter, Schema, Store, StoreError, StoreMut, AlterTable};
use sqlparser::ast::{ColumnDef};

#[derive(Clone, Debug)]
pub struct DataKey {
    pub table_name: String,
    pub id: u64,
}

#[derive(Clone)]
pub struct Storage {
    schema_map: HashMap<String, Schema>,
    data_map: HashMap<String, Vec<(u64, Row)>>,
    id: u64,
}

impl Storage {
    pub fn new() -> Result<Self> {
        let schema_map = HashMap::new();
        
        Ok(Self {
            schema_map,
            data_map: HashMap::default(),
            id: 0,
        })
    }
}

impl StoreMut<DataKey> for Storage {
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

    fn insert_data(self, key: &DataKey, row: Row) -> MutResult<Self, ()> {
        let DataKey { table_name, id } = key;
        let (id, _row)= (*id, row.clone());

        let Self {
            schema_map,
            mut data_map,
            id: self_id,
        } = self;

        let new_rows= match data_map.get_mut(table_name) {
            Some(rows) => {
                let rows= match rows.iter().position(|(item_id, _)| *item_id == id) {
                    Some(index) => {
                        rows[index] = (id, _row);
                        rows
                    },
                    None => {
                        rows.push((id, _row));
                        rows
                    }
                };

                rows.clone()
            }
            _ => vec![(id, _row)]
        };

        let mut data_map = HashMap::new();
        data_map.insert(table_name.clone(), new_rows);

        let storage = Self {
            schema_map,
            data_map: data_map.clone(),
            id: self_id,
        };

        Ok((storage, ()))
    }

    fn delete_data(self, _key: &DataKey) -> MutResult<Self, ()> {
        Ok((self, ()))
    }
}

impl Store<DataKey> for Storage {
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

        let items = items.into_iter().map(Ok);

        Ok(Box::new(items))
    }
}

impl AlterTable for Storage {
    fn rename_schema(self, table_name: &str, new_table_name: &str) -> MutResult<Self, ()> {
        Ok((self, ()))
    }

    fn rename_column(
        self,
        table_name: &str,
        old_column_name: &str,
        new_column_name: &str,
    ) -> MutResult<Self, ()> {
        Ok((self, ()))
    }

    fn add_column(self, table_name: &str, column_def: &ColumnDef) -> MutResult<Self, ()> {
        Ok((self, ()))
    }

    fn drop_column(
        self,
        table_name: &str,
        column_name: &str,
        if_exists: bool,
    ) -> MutResult<Self, ()> {
        Ok((self, ()))
    }
}
