//! Explicit, atomic data writes. Reads remain read-only; writes require this API.
use crate::{executor::connect, schema::quote, Catalog, Error, Result, TableName};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    Insert,
    Update,
    Delete,
}
#[derive(Clone, Debug)]
pub struct Change {
    pub table: TableName,
    pub operation: Operation,
    pub keys: Vec<Option<String>>,
    pub version: Option<String>,
    pub values: BTreeMap<String, Option<String>>,
    pub original: BTreeMap<String, Option<String>>,
}

pub fn apply(url: &str, catalog: &Catalog, changes: &[Change]) -> Result<usize> {
    if changes.is_empty() || changes.len() > 100 {
        return Err(Error::Invalid("Stage between 1 and 100 row changes".into()));
    }
    let mut client = connect(url)?;
    let mut transaction = client.build_transaction().read_only(false).start()?;
    transaction.batch_execute("SET LOCAL lock_timeout='2s'")?;
    let mut seen = vec![];
    for change in changes {
        let table = catalog
            .table(&change.table)
            .ok_or_else(|| Error::Invalid("Edited table is no longer available".into()))?;
        if table.primary_keys.is_empty() {
            return Err(Error::Invalid(
                "Editing requires a table with a primary key".into(),
            ));
        }
        if change.operation != Operation::Insert {
            if change.keys.len() != table.primary_keys.len()
                || change.keys.iter().any(Option::is_none)
                || change.version.is_none()
            {
                return Err(Error::Invalid(
                    "Edit is missing its row identity/version; refresh the view".into(),
                ));
            }
            let identity = (change.table.clone(), change.keys.clone());
            if seen.contains(&identity) {
                return Err(Error::Invalid(
                    "Duplicate staged row; combine its field updates".into(),
                ));
            }
            seen.push(identity);
        }
        let mut parameters: Vec<Option<String>> = vec![];
        let mut fields = vec![];
        let mut values = vec![];
        for (name, value) in &change.values {
            let col = table
                .columns
                .iter()
                .find(|c| &c.name == name)
                .ok_or_else(|| Error::Invalid(format!("Unknown edited column {name}")))?;
            if col.generated
                || (change.operation == Operation::Update && table.primary_keys.contains(name))
            {
                return Err(Error::Invalid(format!(
                    "Cannot edit generated/primary-key column {name}"
                )));
            }
            if value.is_none() && !col.nullable {
                return Err(Error::Invalid(format!("{name} does not allow NULL")));
            }
            parameters.push(value.clone());
            fields.push(quote(name));
            values.push(format!("(${}::text)::{}", parameters.len(), col.data_type));
        }
        let mut conditions = vec![];
        if change.operation != Operation::Insert {
            for (name, value) in table.primary_keys.iter().zip(&change.keys) {
                let col = table.columns.iter().find(|c| &c.name == name).unwrap();
                parameters.push(value.clone());
                conditions.push(format!(
                    "{} = (${}::text)::{}",
                    quote(name),
                    parameters.len(),
                    col.data_type
                ));
            }
            parameters.push(change.version.clone());
            conditions.push(format!("xmin::text = ${}::text", parameters.len()));
        }
        let sql = match change.operation {
            Operation::Insert if fields.is_empty() => {
                format!("INSERT INTO {} DEFAULT VALUES", table.name.sql())
            }
            Operation::Insert => format!(
                "INSERT INTO {} ({}) VALUES ({})",
                table.name.sql(),
                fields.join(","),
                values.join(",")
            ),
            Operation::Delete => format!(
                "DELETE FROM {} WHERE {}",
                table.name.sql(),
                conditions.join(" AND ")
            ),
            Operation::Update => {
                if fields.is_empty() {
                    return Err(Error::Invalid("Update contains no changed fields".into()));
                }
                let assignments = fields
                    .iter()
                    .zip(&values)
                    .map(|(f, v)| format!("{f}={v}"))
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "UPDATE {} SET {assignments} WHERE {}",
                    table.name.sql(),
                    conditions.join(" AND ")
                )
            }
        };
        let params: Vec<&(dyn postgres::types::ToSql + Sync)> = parameters
            .iter()
            .map(|p| p as &(dyn postgres::types::ToSql + Sync))
            .collect();
        let affected = transaction.execute(&sql, &params)?;
        if affected != 1 {
            return Err(Error::Invalid(format!("Conflict on {}: row changed or disappeared since it was loaded. Entire batch rolled back; discard and refresh.", table.name.qualified())));
        }
    }
    transaction.commit()?;
    Ok(changes.len())
}
