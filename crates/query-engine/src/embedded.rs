//! File-backed engines. Each request uses one read-only connection and snapshot;
//! child collections are queried independently, avoiding sibling Cartesian products.
use crate::{
    query::flat_compile, schema::quote, Backend, Catalog, Column, Error, ForeignKey, Node, Query,
    Result, TableInfo, TableName,
};
use data_model::document::{Collection, Document, Record};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

type Row = Vec<Option<String>>;
pub enum Connection {
    SQLite(rusqlite::Connection),
    DuckDB(duckdb::Connection),
}
impl Connection {
    pub fn open(url: &str, writable: bool) -> Result<Self> {
        let backend = Backend::from_url(url);
        let path = url.split_once(':').map(|(_, path)| path).unwrap_or("");
        if !Path::new(path).is_file() {
            return Err(Error::Invalid(format!(
                "Database file does not exist: {path}"
            )));
        }
        match backend {
            Backend::SQLite => {
                let flags = if writable {
                    rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
                } else {
                    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                };
                let conn = rusqlite::Connection::open_with_flags(path, flags)?;
                conn.busy_timeout(Duration::from_secs(2))?;
                conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF;")?;
                Ok(Self::SQLite(conn))
            }
            Backend::DuckDB => {
                let config = duckdb::Config::default()
                    .access_mode(if writable {
                        duckdb::AccessMode::ReadWrite
                    } else {
                        duckdb::AccessMode::ReadOnly
                    })?
                    .enable_external_access(false)?
                    .enable_autoload_extension(false)?;
                Ok(Self::DuckDB(duckdb::Connection::open_with_flags(
                    path, config,
                )?))
            }
            Backend::Postgres => Err(Error::Invalid("Expected sqlite:PATH or duckdb:PATH".into())),
        }
    }
    pub(crate) fn interrupt(&self) -> Box<dyn Fn() + Send> {
        match self {
            Self::SQLite(conn) => {
                let handle = conn.get_interrupt_handle();
                Box::new(move || handle.interrupt())
            }
            Self::DuckDB(conn) => {
                let handle = conn.interrupt_handle();
                Box::new(move || handle.interrupt())
            }
        }
    }
    pub fn batch(&self, sql: &str) -> Result<()> {
        match self {
            Self::SQLite(conn) => conn.execute_batch(sql)?,
            Self::DuckDB(conn) => conn.execute_batch(sql)?,
        }
        Ok(())
    }
    fn rows(&self, sql: &str, params: &[Option<String>]) -> Result<Vec<Row>> {
        let mut out = vec![];
        match self {
            Self::SQLite(conn) => {
                let mut statement = conn.prepare(sql)?;
                let count = statement.column_count();
                let mut rows = statement.query(rusqlite::params_from_iter(params))?;
                while let Some(row) = rows.next()? {
                    out.push(
                        (0..count)
                            .map(|i| row.get(i))
                            .collect::<std::result::Result<_, _>>()?,
                    );
                }
            }
            Self::DuckDB(conn) => {
                let mut statement = conn.prepare(sql)?;
                let mut rows = statement.query(duckdb::params_from_iter(params))?;
                while let Some(row) = rows.next()? {
                    out.push(
                        (0..row.as_ref().column_count())
                            .map(|i| row.get(i))
                            .collect::<std::result::Result<_, _>>()?,
                    );
                }
            }
        }
        Ok(out)
    }
    fn write(&self, sql: &str, params: &[Option<String>]) -> Result<usize> {
        Ok(match self {
            Self::SQLite(conn) => conn.execute(sql, rusqlite::params_from_iter(params))?,
            Self::DuckDB(conn) => conn.execute(sql, duckdb::params_from_iter(params))?,
        })
    }
    pub fn discover(&self) -> Result<Catalog> {
        match self {
            Self::SQLite(_) => self.sqlite_catalog(),
            Self::DuckDB(_) => self.duckdb_catalog(),
        }
    }
    fn sqlite_catalog(&self) -> Result<Catalog> {
        let mut catalog = Catalog {
            backend: Backend::SQLite,
            ..Default::default()
        };
        let tables = self.rows("SELECT name, type FROM sqlite_schema WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%' ORDER BY name", &[])?;
        for row in tables {
            let name = TableName {
                schema: "main".into(),
                name: value(&row, 0),
            };
            let info = self.rows("SELECT name, type, CAST(\"notnull\" AS TEXT), CAST(pk AS TEXT), CAST(hidden AS TEXT) FROM pragma_table_xinfo(?1) WHERE hidden <> 1 ORDER BY cid", &[Some(name.name.clone())])?;
            let mut keys = vec![];
            let columns = info
                .into_iter()
                .map(|row| {
                    let pk: usize = value(&row, 3).parse().unwrap_or(0);
                    if pk > 0 {
                        keys.push((pk, value(&row, 0)));
                    }
                    Column {
                        name: value(&row, 0),
                        data_type: value(&row, 1),
                        nullable: value(&row, 2) == "0" && pk == 0,
                        generated: value(&row, 4) != "0",
                    }
                })
                .collect();
            keys.sort();
            catalog.tables.push(TableInfo {
                name,
                columns,
                primary_keys: if value(&row, 1) == "view" {
                    vec![]
                } else {
                    keys.into_iter().map(|(_, name)| name).collect()
                },
            });
        }
        for table in &catalog.tables {
            let rows = self.rows("SELECT CAST(id AS TEXT), CAST(seq AS TEXT), \"table\", \"from\", \"to\" FROM pragma_foreign_key_list(?1) ORDER BY id, seq", &[Some(table.name.name.clone())])?;
            let mut foreign_keys: BTreeMap<String, ForeignKey> = BTreeMap::new();
            for row in rows {
                let id = value(&row, 0);
                let Some(target_table) = catalog
                    .tables
                    .iter()
                    .find(|t| t.name.name.eq_ignore_ascii_case(&value(&row, 2)))
                else {
                    continue;
                };
                let to = target_table.name.clone();
                let seq: usize = value(&row, 1).parse().unwrap_or(0);
                let target = row[4]
                    .as_ref()
                    .and_then(|name| {
                        target_table
                            .columns
                            .iter()
                            .find(|c| c.name.eq_ignore_ascii_case(name))
                            .map(|c| c.name.clone())
                    })
                    .or_else(|| {
                        row[4]
                            .is_none()
                            .then(|| target_table.primary_keys.get(seq).cloned())
                            .flatten()
                    })
                    .unwrap_or_default();
                let fk = foreign_keys
                    .entry(id.clone())
                    .or_insert_with(|| ForeignKey {
                        name: format!("{}_fk_{id}", table.name.name),
                        from: table.name.clone(),
                        to,
                        from_columns: vec![],
                        to_columns: vec![],
                    });
                let from = table
                    .columns
                    .iter()
                    .find(|c| c.name.eq_ignore_ascii_case(&value(&row, 3)))
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| value(&row, 3));
                fk.from_columns.push(from);
                fk.to_columns.push(target);
            }
            catalog
                .foreign_keys
                .extend(foreign_keys.into_values().filter(|f| {
                    catalog.tables.iter().any(|t| t.name == f.to)
                        && f.to_columns.iter().all(|c| !c.is_empty())
                }));
        }
        Ok(catalog)
    }
    fn duckdb_catalog(&self) -> Result<Catalog> {
        let mut catalog = Catalog {
            backend: Backend::DuckDB,
            ..Default::default()
        };
        for row in self.rows("SELECT table_schema, table_name FROM information_schema.tables WHERE table_catalog=current_database() ORDER BY 1,2", &[])? {
            let name = TableName {schema:value(&row,0),name:value(&row,1)};
            let params = [Some(name.schema.clone()),Some(name.name.clone())];
            let columns = self.rows("SELECT column_name, data_type, is_nullable, COALESCE(column_default,'') FROM information_schema.columns WHERE table_catalog=current_database() AND table_schema=$1 AND table_name=$2 ORDER BY ordinal_position", &params)?.into_iter().map(|row| Column {name:value(&row,0),data_type:value(&row,1),nullable:value(&row,2)=="YES",generated:false}).collect();
            let primary_keys = self.rows("SELECT unnest(constraint_column_names) FROM duckdb_constraints() WHERE database_name=current_database() AND schema_name=$1 AND table_name=$2 AND constraint_type='PRIMARY KEY'", &params)?.into_iter().map(|r|value(&r,0)).collect();
            catalog.tables.push(TableInfo{name,columns,primary_keys});
        }
        // Lists are unnested together, preserving the order of composite keys.
        let rows = self.rows("SELECT constraint_name, schema_name, table_name, referenced_table, unnest(constraint_column_names), unnest(referenced_column_names) FROM duckdb_constraints() WHERE database_name=current_database() AND constraint_type='FOREIGN KEY' ORDER BY schema_name,table_name,constraint_index", &[])?;
        for row in rows {
            let name = value(&row, 0);
            let from = TableName {
                schema: value(&row, 1),
                name: value(&row, 2),
            };
            if let Some(fk) = catalog
                .foreign_keys
                .iter_mut()
                .find(|f| f.name == name && f.from == from)
            {
                fk.from_columns.push(value(&row, 4));
                fk.to_columns.push(value(&row, 5));
            } else {
                catalog.foreign_keys.push(ForeignKey {
                    name,
                    from,
                    to: TableName {
                        schema: value(&row, 1),
                        name: value(&row, 3),
                    },
                    from_columns: vec![value(&row, 4)],
                    to_columns: vec![value(&row, 5)],
                });
            }
        }
        // DuckDB's catalog does not distinguish generated expressions from defaults.
        // Its binder rejects generated-column writes when applying the transaction.
        Ok(catalog)
    }
    pub fn execute(&self, query: &Query, catalog: &Catalog) -> Result<Document> {
        self.execute_cancellable(query, catalog, &Arc::new(AtomicBool::new(false)))
    }
    pub(crate) fn execute_cancellable(
        &self,
        query: &Query,
        catalog: &Catalog,
        cancelled: &Arc<AtomicBool>,
    ) -> Result<Document> {
        query.validate(catalog)?;
        self.batch("BEGIN TRANSACTION")?;
        let result = (|| {
            let mut execution = Execution {
                connection: self,
                query,
                catalog,
                collections: vec![],
                cancelled,
                deadline: Instant::now() + Duration::from_secs(10),
            };
            let rows = execution.collection(&query.root, &[], &[])?;
            Ok(Document {
                fields: query.root.fields(),
                rows,
                truncated: execution.collections.iter().any(|c| c.more || c.offset > 0),
                collections: execution.collections,
            })
        })();
        self.batch("ROLLBACK")?;
        result
    }
}
fn value(row: &Row, i: usize) -> String {
    row[i].clone().unwrap_or_default()
}
struct Execution<'a> {
    connection: &'a Connection,
    query: &'a Query,
    catalog: &'a Catalog,
    collections: Vec<Collection>,
    cancelled: &'a AtomicBool,
    deadline: Instant,
}
impl Execution<'_> {
    fn collection(
        &mut self,
        node: &Node,
        parents: &[String],
        joins: &[(Option<String>, String)],
    ) -> Result<Vec<Record>> {
        if self.cancelled.load(Ordering::SeqCst) || Instant::now() > self.deadline {
            return Err(Error::Invalid("Query cancelled or timed out".into()));
        }
        let compiled = flat_compile(self.query, self.catalog, node, parents, joins)?;
        let table = self.catalog.table(&node.table).unwrap();
        let mut rows = self.connection.rows(
            &compiled.sql,
            &compiled.params.into_iter().map(Some).collect::<Vec<_>>(),
        )?;
        let (offset, limit) = self.query.window(node.id, parents);
        let more = rows.len() > limit;
        rows.truncate(limit);
        self.collections.push(Collection {
            node: node.id,
            parents: parents.to_vec(),
            offset,
            loaded: rows.len(),
            more,
        });
        let mut records = vec![];
        for row in rows {
            let base: BTreeMap<_, _> = table
                .columns
                .iter()
                .map(|c| c.name.clone())
                .zip(row.iter().cloned())
                .collect();
            let keys: Vec<_> = if table.primary_keys.is_empty() {
                row[..table.columns.len()].to_vec()
            } else {
                table.primary_keys.iter().map(|k| base[k].clone()).collect()
            };
            let key = serde_json::to_string(&keys)?;
            let version = if table.primary_keys.is_empty() {
                None
            } else {
                Some(serde_json::to_string(
                    &row[..snapshot_fields(table, self.catalog.backend, "").len()],
                )?)
            };
            let snapshot_len = snapshot_fields(table, self.catalog.backend, "").len();
            let values = node
                .columns
                .iter()
                .enumerate()
                .map(|(i, name)| (format!("{}:{name}", node.id), row[snapshot_len + i].clone()))
                .collect();
            let mut children = BTreeMap::new();
            let mut child_parents = parents.to_vec();
            child_parents.push(key.clone());
            for child in node.children.iter().filter(|n| n.expanded) {
                let join = child.join.as_ref().unwrap();
                let joins = join
                    .parent_columns
                    .iter()
                    .map(|c| {
                        let storage = if self.catalog.backend == Backend::SQLite {
                            let index = table
                                .columns
                                .iter()
                                .position(|column| &column.name == c)
                                .unwrap();
                            row[table.columns.len() + index].clone().unwrap_or_default()
                        } else {
                            String::new()
                        };
                        (base[c].clone(), storage)
                    })
                    .collect::<Vec<_>>();
                children.insert(
                    child.id.to_string(),
                    self.collection(child, &child_parents, &joins)?,
                );
            }
            records.push(Record {
                key,
                version,
                values,
                children,
            });
        }
        Ok(records)
    }
}

/// Apply a batch atomically. Embedded engines compare the complete loaded row
/// inside the write transaction, including fields hidden in the current view.
pub fn apply(url: &str, catalog: &Catalog, changes: &[crate::editing::Change]) -> Result<usize> {
    use crate::editing::Operation;
    if changes.is_empty() || changes.len() > 100 {
        return Err(Error::Invalid("Stage between 1 and 100 row changes".into()));
    }
    let connection = Connection::open(url, true)?;
    connection.batch(if catalog.backend == Backend::SQLite {
        "BEGIN IMMEDIATE"
    } else {
        "BEGIN TRANSACTION"
    })?;
    let result = (|| {
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
            let mut conditions = vec![];
            let mut identity = vec![];
            if change.operation != Operation::Insert {
                if change.keys.len() != table.primary_keys.len()
                    || change.keys.iter().any(Option::is_none)
                    || change.version.is_none()
                {
                    return Err(Error::Invalid(
                        "Edit is missing its row identity/version; refresh the view".into(),
                    ));
                }
                let key = (change.table.clone(), change.keys.clone());
                if seen.contains(&key) {
                    return Err(Error::Invalid(
                        "Duplicate staged row; combine its field updates".into(),
                    ));
                }
                seen.push(key);
                for (name, value) in table.primary_keys.iter().zip(&change.keys) {
                    let column = table.columns.iter().find(|c| &c.name == name).unwrap();
                    identity.push(value.clone());
                    conditions.push(format!(
                        "{} = {}",
                        quote(name),
                        catalog.backend.cast(
                            &catalog.backend.parameter(identity.len()),
                            &column.data_type
                        )
                    ));
                }
                let fields = snapshot_fields(table, catalog.backend, "").join(",");
                let current = connection.rows(
                    &format!(
                        "SELECT {fields} FROM {} WHERE {}",
                        table.name.sql(),
                        conditions.join(" AND ")
                    ),
                    &identity,
                )?;
                if current.len() != 1 || Some(serde_json::to_string(&current[0])?) != change.version
                {
                    return Err(Error::Invalid(format!("Conflict on {}: row changed or disappeared since it was loaded. Entire batch rolled back; discard and refresh.",table.name.qualified())));
                }
            }
            let mut params = identity;
            let mut fields = vec![];
            let mut values = vec![];
            for (name, value) in &change.values {
                let column = table
                    .columns
                    .iter()
                    .find(|c| &c.name == name)
                    .ok_or_else(|| Error::Invalid(format!("Unknown edited column {name}")))?;
                if column.generated
                    || (change.operation == Operation::Update && table.primary_keys.contains(name))
                {
                    return Err(Error::Invalid(format!(
                        "Cannot edit generated/primary-key column {name}"
                    )));
                }
                if value.is_none() && !column.nullable {
                    return Err(Error::Invalid(format!("{name} does not allow NULL")));
                }
                if column.data_type.eq_ignore_ascii_case("blob")
                    && value.as_ref().is_some_and(|s| {
                        s.len() % 2 != 0 || !s.bytes().all(|b| b.is_ascii_hexdigit())
                    })
                {
                    return Err(Error::Invalid(format!(
                        "{name} requires an even-length hexadecimal BLOB"
                    )));
                }
                params.push(value.clone());
                fields.push(quote(name));
                values.push(
                    catalog
                        .backend
                        .cast(&catalog.backend.parameter(params.len()), &column.data_type),
                );
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
            if connection.write(&sql, &params)? != 1 {
                return Err(Error::Invalid(
                    "Conflict: expected exactly one changed row; entire batch rolled back".into(),
                ));
            }
        }
        connection.batch("COMMIT")?;
        Ok(changes.len())
    })();
    if result.is_err() {
        let _ = connection.batch("ROLLBACK");
    }
    result
}

// Include SQLite storage classes: INTEGER 1 and TEXT '1' (or BLOB '31')
// must not share an optimistic-edit version in a dynamically typed database.
pub(crate) fn snapshot_fields(table: &TableInfo, backend: Backend, prefix: &str) -> Vec<String> {
    let mut fields = table
        .columns
        .iter()
        .map(|c| {
            let column = format!("{prefix}{}",quote(&c.name));
            let text = backend.text(&column,&c.data_type);
            if backend==Backend::SQLite {
                // SQLite's ordinary REAL-to-TEXT cast rounds to 15 significant
                // digits. Preserve enough digits for identity and conflict checks.
                format!("CASE WHEN typeof({column})='real' THEN printf('%!.26g',{column}) ELSE {text} END")
            } else {text}
        })
        .collect::<Vec<_>>();
    if backend == Backend::SQLite {
        fields.extend(
            table
                .columns
                .iter()
                .map(|c| format!("typeof({prefix}{})", quote(&c.name))),
        );
    }
    fields
}
