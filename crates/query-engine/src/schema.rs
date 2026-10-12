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
    #[serde(default)]
    pub generated: bool,
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
    #[serde(default)]
    pub backend: Backend,
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

/// SQL dialect and embedded-file connection selection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Backend {
    #[default]
    Postgres,
    SQLite,
    DuckDB,
}
impl Backend {
    pub fn from_url(url: &str) -> Self {
        if url.starts_with("sqlite:") {
            Self::SQLite
        } else if url.starts_with("duckdb:") {
            Self::DuckDB
        } else {
            Self::Postgres
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Postgres => "PostgreSQL",
            Self::SQLite => "SQLite",
            Self::DuckDB => "DuckDB",
        }
    }
    pub(crate) fn parameter(self, index: usize) -> String {
        match self {
            Self::SQLite => format!("?{index}"),
            Self::DuckDB => format!("${index}"),
            Self::Postgres => format!("${index}::text"),
        }
    }
    pub(crate) fn numeric(self, expression: &str) -> String {
        match self {
            Self::Postgres => format!("({expression})::numeric"),
            Self::SQLite => format!("CAST({expression} AS REAL)"),
            Self::DuckDB => format!("({expression})"),
        }
    }
    pub(crate) fn cast(self, expression: &str, data_type: &str) -> String {
        if data_type.eq_ignore_ascii_case("blob") {
            match self {
                Self::SQLite => format!("unhex({expression})"),
                Self::DuckDB => format!("from_hex({expression})"),
                Self::Postgres => format!("CAST({expression} AS {data_type})"),
            }
        } else if self == Self::SQLite {
            // Let column affinity (including STRICT tables) apply SQLite's own
            // rules. CAST('invalid' AS INTEGER) would silently replace text with 0.
            expression.to_string()
        } else {
            format!("CAST({expression} AS {data_type})")
        }
    }

    pub(crate) fn text(self, expression: &str, data_type: &str) -> String {
        match self {
            Self::SQLite => format!("CASE WHEN typeof({expression})='blob' THEN hex({expression}) ELSE CAST({expression} AS TEXT) END"),
            Self::DuckDB if data_type.eq_ignore_ascii_case("blob") => format!("hex({expression})"),
            _ => format!("CAST({expression} AS TEXT)"),
        }
    }
    pub(crate) fn is_numeric(self, data_type: &str) -> bool {
        let dt = data_type.to_ascii_lowercase();
        if self == Self::Postgres {
            matches!(
                dt.as_str(),
                "smallint" | "integer" | "bigint" | "real" | "double precision" | "numeric"
            ) || dt.starts_with("numeric(")
        } else if self == Self::DuckDB {
            matches!(
                dt.as_str(),
                "tinyint"
                    | "smallint"
                    | "integer"
                    | "bigint"
                    | "hugeint"
                    | "utinyint"
                    | "usmallint"
                    | "uinteger"
                    | "ubigint"
                    | "uhugeint"
                    | "bignum"
                    | "float"
                    | "double"
            ) || dt.starts_with("decimal(")
        } else {
            dt.contains("int")
                || dt.contains("real")
                || dt.contains("float")
                || dt.contains("double")
                || dt.starts_with("numeric")
                || dt.starts_with("decimal")
        }
    }
}
