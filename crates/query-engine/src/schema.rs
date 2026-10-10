use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableName {
    pub schema: String,
    pub name: String,
}
impl TableName {
    pub fn qualified(&self) -> String {
        format!("{}.{}", self.schema, self.name)
    }
    pub fn sql(&self) -> String {
        format!("{}.{}", quote(&self.schema), quote(&self.name))
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForeignKey {
    pub name: String,
    pub from: TableName,
    pub to: TableName,
    pub from_columns: Vec<String>,
    pub to_columns: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: TableName,
    pub columns: Vec<Column>,
    pub primary_keys: Vec<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    pub tables: Vec<TableInfo>,
    pub foreign_keys: Vec<ForeignKey>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relationship {
    pub constraint: String,
    pub target: TableName,
    pub parent_columns: Vec<String>,
    pub child_columns: Vec<String>,
}
impl Catalog {
    pub fn table(&self, name: &TableName) -> Option<&TableInfo> {
        self.tables.iter().find(|t| &t.name == name)
    }
    pub fn relationships(&self, name: &TableName) -> Vec<Relationship> {
        let mut out = vec![];
        for fk in &self.foreign_keys {
            if &fk.from == name {
                out.push(Relationship {
                    constraint: fk.name.clone(),
                    target: fk.to.clone(),
                    parent_columns: fk.from_columns.clone(),
                    child_columns: fk.to_columns.clone(),
                });
            }
            if &fk.to == name {
                out.push(Relationship {
                    constraint: fk.name.clone(),
                    target: fk.from.clone(),
                    parent_columns: fk.to_columns.clone(),
                    child_columns: fk.from_columns.clone(),
                });
            }
        }
        out
    }
}
pub fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
