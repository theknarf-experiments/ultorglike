//! Nested relational data model.
//!
//! From Bakke's research, data is represented as a hierarchy where:
//! - **Primitive values** are scalars (strings, numbers, dates)
//! - **Relation values** are lists of tuples (nested tables)
//! - **Tuples** have labels (field names) and values (primitive or relation)

use arrow::datatypes::DataType;

/// A single cell value in the nested relational model.
///
/// This corresponds to the "terminology" diagram from the SIEUFERD paper (Figure 3):
/// - Primitive values (numbers, strings, dates)
/// - Relation values (nested tables containing tuples)
/// - Labels (field names, rendered in bold in the UI)
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A primitive scalar value
    Primitive(PrimitiveValue),
    /// A nested relation (list of tuples) — represents a 1-to-N relationship
    Relation(NestedRelation),
}

impl Value {
    /// Get the nested relation if this is a relation value.
    pub fn get_relation(&self) -> Option<&NestedRelation> {
        match self {
            Value::Relation(nr) => Some(nr),
            _ => None,
        }
    }

    /// Get the primitive value if this is a primitive value.
    pub fn get_primitive(&self) -> Option<&PrimitiveValue> {
        match self {
            Value::Primitive(pv) => Some(pv),
            _ => None,
        }
    }

    /// Returns true if this is a nested relation.
    pub fn is_relation(&self) -> bool {
        matches!(self, Value::Relation(_))
    }

    /// Convert this value to a display string.
    pub fn to_display_string(&self) -> String {
        match self {
            Value::Primitive(pv) => pv.value.to_display_string(),
            Value::Relation(nr) => {
                if nr.is_empty() {
                    "[no data]".to_string()
                } else {
                    format!("[{} items]", nr.len())
                }
            }
        }
    }
}

/// A primitive (scalar) cell value.
#[derive(Debug, Clone, PartialEq)]
pub struct PrimitiveValue {
    /// The underlying data
    pub value: CellValue,
    /// The Arrow data type
    pub data_type: DataType,
}

impl PrimitiveValue {
    pub fn new(value: CellValue, data_type: DataType) -> Self {
        Self { value, data_type }
    }
}

/// The actual content of a primitive cell.
#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    Null,
    Boolean(bool),
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    UInt8(u8),
    UInt16(u16),
    UInt32(u32),
    UInt64(u64),
    Float32(f32),
    Float64(f64),
    Utf8(String),
    LargeUtf8(String),
    Binary(Vec<u8>),
    Date(i32),                      // days since Unix epoch
    Time(i64),                      // nanoseconds since midnight
    Timestamp(i64, Option<String>), // nanoseconds since epoch, optional timezone
}

impl CellValue {
    /// Render this value as a human-readable string for display in the TUI.
    pub fn to_display_string(&self) -> String {
        match self {
            CellValue::Null => "".to_string(),
            CellValue::Boolean(b) => format!("{}", b),
            CellValue::Int8(v) => format!("{}", v),
            CellValue::Int16(v) => format!("{}", v),
            CellValue::Int32(v) => format!("{}", v),
            CellValue::Int64(v) => format!("{}", v),
            CellValue::UInt8(v) => format!("{}", v),
            CellValue::UInt16(v) => format!("{}", v),
            CellValue::UInt32(v) => format!("{}", v),
            CellValue::UInt64(v) => format!("{}", v),
            CellValue::Float32(v) => {
                if v.is_nan() || v.is_infinite() {
                    v.to_string()
                } else {
                    format!("{:.4}", v)
                        .trim_end_matches('0')
                        .trim_end_matches('.')
                        .to_string()
                }
            }
            CellValue::Float64(v) => {
                if v.is_nan() || v.is_infinite() {
                    v.to_string()
                } else {
                    format!("{:.6}", v)
                        .trim_end_matches('0')
                        .trim_end_matches('.')
                        .to_string()
                }
            }
            CellValue::Utf8(s) => {
                if s.len() > 80 {
                    format!("{}...", s.chars().take(77).collect::<String>())
                } else {
                    s.clone()
                }
            }
            CellValue::LargeUtf8(s) => {
                if s.len() > 80 {
                    format!("{}...", s.chars().take(77).collect::<String>())
                } else {
                    s.clone()
                }
            }
            CellValue::Binary(bytes) => format!("<{} bytes>", bytes.len()),
            CellValue::Date(days) => {
                let epoch = chrono::NaiveDate::from_yo_opt(1970, 1).unwrap();
                let date = epoch + chrono::Duration::days(*days as i64);
                format!("{}", date)
            }
            CellValue::Time(nsecs) => {
                let time = chrono::NaiveTime::from_num_seconds_from_midnight_opt(
                    (nsecs / 1_000_000_000) as u32,
                    (nsecs % 1_000_000_000) as u32,
                )
                .unwrap_or_default();
                format!("{}", time)
            }
            CellValue::Timestamp(nsecs, tz) => {
                let ts = chrono::DateTime::from_timestamp_nanos(*nsecs);
                if let Some(ref tz_str) = tz {
                    format!("{} {}", ts.format("%Y-%m-%d %H:%M:%S"), tz_str)
                } else {
                    format!("{}", ts.format("%Y-%m-%d %H:%M:%S"))
                }
            }
        }
    }
}

/// A nested relation — a list of tuples representing a 1-to-N relationship.
///
/// This is the core structure for hierarchical views. Each `NestedRelation`
/// represents a child table (e.g., "Orders" for a given "Customer").
#[derive(Debug, Clone, PartialEq)]
pub struct NestedRelation {
    /// The schema/column names of this relation
    pub columns: Vec<String>,
    /// The Arrow data types
    pub data_types: Vec<DataType>,
    /// The rows — each row is a list of cell values
    pub rows: Vec<Vec<Value>>,
}

impl NestedRelation {
    /// Create a new nested relation with the given schema and rows.
    pub fn new(columns: Vec<String>, data_types: Vec<DataType>, rows: Vec<Vec<CellValue>>) -> Self {
        let rows = rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .enumerate()
                    .map(|(index, value)| {
                        Value::Primitive(PrimitiveValue::new(
                            value,
                            data_types.get(index).cloned().unwrap_or(DataType::Null),
                        ))
                    })
                    .collect()
            })
            .collect();
        Self {
            columns,
            data_types,
            rows,
        }
    }

    /// Construct arbitrary-depth rows, including relation-valued children.
    pub fn from_rows(
        columns: Vec<String>,
        data_types: Vec<DataType>,
        rows: Vec<Vec<Value>>,
    ) -> Self {
        Self {
            columns,
            data_types,
            rows,
        }
    }

    /// Number of rows in this relation.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether this relation has no rows.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Get the value at a specific row and column.
    pub fn get(&self, row: usize, col: usize) -> Option<&CellValue> {
        self.rows
            .get(row)?
            .get(col)?
            .get_primitive()
            .map(|p| &p.value)
    }

    /// Render this relation as a text representation for debugging.
    pub fn to_text(&self) -> String {
        let col_widths: Vec<usize> = self
            .columns
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let name_len = name.len();
                let data_len = self
                    .rows
                    .iter()
                    .map(|r| r.get(i).map(|v| v.to_display_string().len()).unwrap_or(0))
                    .max()
                    .unwrap_or(0);
                name_len.max(data_len).max(4)
            })
            .collect();

        let mut output = String::new();

        // Header
        for (i, col) in self.columns.iter().enumerate() {
            output.push_str(&format!("{:<width$}", col, width = col_widths[i]));
        }
        output.push('\n');

        // Separator
        output.push_str(
            &self
                .columns
                .iter()
                .map(|_| "-".repeat(col_widths[0]))
                .collect::<Vec<_>>()
                .join(" "),
        );
        output.push('\n');

        // Rows
        for row in &self.rows {
            for (i, val) in row.iter().enumerate() {
                output.push_str(&format!(
                    "{:<width$}",
                    val.to_display_string(),
                    width = col_widths[i]
                ));
            }
            output.push('\n');
        }

        output
    }
}

/// A row in a hierarchical view.
///
/// Each row contains a mix of primitive values (for display) and nested relations
/// (for expansion into child views via `l`/right-arrow).
#[derive(Debug, Clone, PartialEq)]
pub struct NestedRow {
    /// Column names (the schema)
    pub columns: Vec<String>,
    /// Cell values — each is either primitive or a nested relation
    pub values: Vec<Value>,
}

impl NestedRow {
    /// Create a new row.
    pub fn new(columns: Vec<String>, values: Vec<Value>) -> Self {
        Self { columns, values }
    }

    /// Get the primitive value at a column index.
    pub fn get_primitive(&self, col: usize) -> Option<&PrimitiveValue> {
        match &self.values[col] {
            Value::Primitive(pv) => Some(pv),
            _ => None,
        }
    }

    /// Get the nested relation at a column index.
    pub fn get_relation(&self, col: usize) -> Option<&NestedRelation> {
        match &self.values[col] {
            Value::Relation(nr) => Some(nr),
            _ => None,
        }
    }

    /// Number of columns in this row.
    pub fn len(&self) -> usize {
        self.columns.len()
    }

    /// Whether this row has no columns.
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }
}
