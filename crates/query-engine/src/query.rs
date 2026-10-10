use crate::{schema::*, Error, Result};
use data_model::document::Field;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Query {
    pub version: u32,
    pub root: Node,
    pub limit: usize,
    pub next_id: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub id: u64,
    pub table: TableName,
    pub label: String,
    pub columns: Vec<String>,
    pub children: Vec<Node>,
    pub join: Option<Relationship>,
    pub expanded: bool,
    /// A semijoin: exclude a parent if this child has no matching rows.
    pub require_match: bool,
    pub filters: Vec<Filter>,
    pub sort: Vec<Sort>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sort {
    pub column: String,
    pub descending: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filter {
    pub column: String,
    pub op: Operator,
    pub value: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operator {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    Contains,
    IsNull,
    NotNull,
}
impl Operator {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "=" | "eq" => Ok(Self::Eq),
            "!=" | "ne" => Ok(Self::Ne),
            ">" => Ok(Self::Gt),
            ">=" => Ok(Self::Ge),
            "<" => Ok(Self::Lt),
            "<=" => Ok(Self::Le),
            "contains" => Ok(Self::Contains),
            "null" => Ok(Self::IsNull),
            "notnull" => Ok(Self::NotNull),
            _ => Err(Error::Invalid(
                "Use =, !=, >, >=, <, <=, contains, null, or notnull".into(),
            )),
        }
    }
}
impl Node {
    fn new(id: u64, table: &TableInfo) -> Self {
        let mut columns: Vec<_> = table
            .columns
            .iter()
            .filter(|c| {
                c.name.contains("name") || c.name.contains("title") || c.name.contains("date")
            })
            .map(|c| c.name.clone())
            .take(3)
            .collect();
        if columns.is_empty() {
            columns = table
                .columns
                .iter()
                .take(3)
                .map(|c| c.name.clone())
                .collect();
        }
        Self {
            id,
            table: table.name.clone(),
            label: table.name.name.clone(),
            columns,
            children: vec![],
            join: None,
            expanded: true,
            require_match: false,
            filters: vec![],
            sort: vec![],
        }
    }
    pub fn find(&self, id: u64) -> Option<&Self> {
        if self.id == id {
            Some(self)
        } else {
            self.children.iter().find_map(|n| n.find(id))
        }
    }
    pub fn find_mut(&mut self, id: u64) -> Option<&mut Self> {
        if self.id == id {
            Some(self)
        } else {
            self.children.iter_mut().find_map(|n| n.find_mut(id))
        }
    }
    pub fn path(&self, id: u64) -> Option<Vec<String>> {
        if self.id == id {
            return Some(vec![self.label.clone()]);
        }
        self.children
            .iter()
            .find_map(|c| c.path(id))
            .map(|mut path| {
                path.insert(0, self.label.clone());
                path
            })
    }
    pub fn fields(&self) -> Vec<Field> {
        let mut fields: Vec<_> = self
            .columns
            .iter()
            .map(|name| {
                let mut label = name.clone();
                if self.filters.iter().any(|f| &f.column == name) {
                    label.push_str(" [f]");
                }
                if let Some(sort) = self.sort.iter().find(|s| &s.column == name) {
                    label.push_str(if sort.descending { " ↓" } else { " ↑" });
                }
                Field {
                    id: format!("{}:{}", self.id, name),
                    label,
                    children: vec![],
                    relation: false,
                    expanded: false,
                }
            })
            .collect();
        for child in &self.children {
            let mut label = format!("{} {}", if child.expanded { "▾" } else { "▸" }, child.label);
            if child.require_match {
                label.push_str(" [match]");
            }
            if !child.filters.is_empty() {
                label.push_str(" [f]");
            }
            if child
                .filters
                .iter()
                .any(|f| !child.columns.contains(&f.column))
                || child
                    .sort
                    .iter()
                    .any(|s| !child.columns.contains(&s.column))
            {
                label.push_str(" [hidden]");
            }
            fields.push(Field {
                id: child.id.to_string(),
                label,
                children: child.fields(),
                relation: true,
                expanded: child.expanded,
            });
        }
        fields
    }
}
impl Query {
    pub fn new(table: &TableInfo) -> Self {
        Self {
            version: 1,
            root: Node::new(1, table),
            limit: 100,
            next_id: 2,
        }
    }
    pub fn add_relationship(
        &mut self,
        parent: u64,
        rel: Relationship,
        catalog: &Catalog,
    ) -> Result<u64> {
        let table = catalog
            .table(&rel.target)
            .ok_or_else(|| Error::Invalid("Related table is unavailable".into()))?;
        let mut child = Node::new(self.next_id, table);
        let id = child.id;
        child.join = Some(rel);
        let node = self
            .root
            .find_mut(parent)
            .ok_or_else(|| Error::Invalid("Unknown parent".into()))?;
        let same = node
            .children
            .iter()
            .filter(|c| c.table == child.table)
            .count();
        if same > 0 {
            child.label = format!("{}_{}", child.label, same + 1);
        }
        node.children.push(child);
        self.next_id += 1;
        Ok(id)
    }
    pub fn resolve(&self, path: &str) -> Option<u64> {
        if let Ok(id) = path.parse::<u64>() {
            return self.root.find(id).map(|n| n.id);
        }
        let mut parts = path.split('.');
        let mut node = &self.root;
        let first = parts.next()?;
        if first != node.label {
            node = node.children.iter().find(|c| c.label == first)?;
        }
        for part in parts {
            node = node.children.iter().find(|c| c.label == part)?;
        }
        Some(node.id)
    }
    pub fn validate(&self, catalog: &Catalog) -> Result<()> {
        if self.version != 1 || self.limit == 0 || self.limit > 500 {
            return Err(Error::Invalid(
                "Unsupported view version or limit (must be 1–500)".into(),
            ));
        }
        fn check(
            node: &Node,
            catalog: &Catalog,
            parent: Option<&Node>,
            depth: usize,
            ids: &mut Vec<u64>,
        ) -> Result<()> {
            if depth > 8 || ids.len() >= 24 || ids.contains(&node.id) {
                return Err(Error::Invalid(
                    "Query exceeds 8 levels / 24 relations, or contains duplicate IDs".into(),
                ));
            }
            ids.push(node.id);
            let table = catalog.table(&node.table).ok_or_else(|| {
                Error::Invalid(format!("Table {} not found", node.table.qualified()))
            })?;
            for col in node
                .columns
                .iter()
                .chain(node.filters.iter().map(|f| &f.column))
                .chain(node.sort.iter().map(|s| &s.column))
            {
                if !table.columns.iter().any(|c| &c.name == col) {
                    return Err(Error::Invalid(format!("Unknown column {col}")));
                }
            }
            if node.columns.is_empty() && node.children.is_empty() {
                return Err(Error::Invalid("Keep at least one visible field".into()));
            }
            if let Some(parent) = parent {
                let rel = node
                    .join
                    .as_ref()
                    .ok_or_else(|| Error::Invalid("Missing join".into()))?;
                if rel.target != node.table || !catalog.relationships(&parent.table).contains(rel) {
                    return Err(Error::Invalid(
                        "Join does not match a discovered foreign key".into(),
                    ));
                }
            }
            for child in &node.children {
                check(child, catalog, Some(node), depth + 1, ids)?;
            }
            Ok(())
        }
        let mut ids = vec![];
        check(&self.root, catalog, None, 0, &mut ids)?;
        if ids.iter().any(|id| *id >= self.next_id) {
            return Err(Error::Invalid("Invalid next field ID".into()));
        }
        Ok(())
    }
}

pub struct Compiled {
    pub sql: String,
    pub params: Vec<String>,
}
pub fn compile(query: &Query, catalog: &Catalog) -> Result<Compiled> {
    query.validate(catalog)?;
    let mut compiler = Compiler {
        catalog,
        params: vec![],
        limit: query.limit,
    };
    let sql = compiler.select(&query.root, None)?;
    Ok(Compiled {
        sql,
        params: compiler.params,
    })
}
struct Compiler<'a> {
    catalog: &'a Catalog,
    params: Vec<String>,
    limit: usize,
}
impl Compiler<'_> {
    fn conditions(&mut self, node: &Node, parent: Option<u64>) -> Result<String> {
        let alias = format!("t{}", node.id);
        let mut terms = vec![];
        if let Some(parent) = parent {
            let join = node
                .join
                .as_ref()
                .ok_or_else(|| Error::Invalid("Missing join".into()))?;
            for (p, c) in join.parent_columns.iter().zip(&join.child_columns) {
                terms.push(format!("{alias}.{} = t{parent}.{}", quote(c), quote(p)));
            }
        }
        let table = self.catalog.table(&node.table).unwrap();
        for filter in &node.filters {
            let col = format!("{alias}.{}", quote(&filter.column));
            if filter.op == Operator::IsNull {
                terms.push(format!("{col} IS NULL"));
                continue;
            }
            if filter.op == Operator::NotNull {
                terms.push(format!("{col} IS NOT NULL"));
                continue;
            }
            self.params.push(filter.value.clone());
            let param = format!("${}::text", self.params.len());
            if filter.op == Operator::Contains {
                terms.push(format!("strpos(lower({col}::text), lower({param})) > 0"));
                continue;
            }
            let dt = &table
                .columns
                .iter()
                .find(|c| c.name == filter.column)
                .unwrap()
                .data_type;
            let op = match filter.op {
                Operator::Eq => "=",
                Operator::Ne => "<>",
                Operator::Gt => ">",
                Operator::Ge => ">=",
                Operator::Lt => "<",
                Operator::Le => "<=",
                _ => unreachable!(),
            };
            terms.push(format!("{col} {op} ({param})::{dt}"));
        }
        for child in &node.children {
            if child.require_match {
                let conditions = self.conditions(child, Some(node.id))?;
                terms.push(format!(
                    "EXISTS (SELECT 1 FROM {} t{} WHERE {conditions})",
                    child.table.sql(),
                    child.id
                ));
            }
        }
        Ok(if terms.is_empty() {
            "TRUE".into()
        } else {
            terms.join(" AND ")
        })
    }
    fn select(&mut self, node: &Node, parent: Option<u64>) -> Result<String> {
        let alias = format!("t{}", node.id);
        let table = self.catalog.table(&node.table).unwrap();
        let keys = if table.primary_keys.is_empty() {
            table.columns.iter().map(|c| c.name.clone()).collect()
        } else {
            table.primary_keys.clone()
        };
        let key = keys
            .iter()
            .map(|c| format!("{alias}.{}::text", quote(c)))
            .collect::<Vec<_>>()
            .join(",");
        // text conversion preserves NUMERIC precision and PostgreSQL timestamp units.
        let values = node
            .columns
            .iter()
            .map(|c| {
                format!(
                    "'{}', {alias}.{}::text",
                    format!("{}:{c}", node.id).replace('\'', "''"),
                    quote(c)
                )
            })
            .collect::<Vec<_>>();
        // PostgreSQL limits function calls to 100 arguments (50 key/value pairs).
        let values = if values.is_empty() {
            "'{}'::jsonb".into()
        } else {
            values
                .chunks(50)
                .map(|chunk| format!("jsonb_build_object({})", chunk.join(",")))
                .collect::<Vec<_>>()
                .join(" || ")
        };
        let mut children = vec![];
        for child in &node.children {
            let sql = self.select(child, Some(node.id))?;
            children.push(format!("'{}', COALESCE((SELECT jsonb_agg(s.doc ORDER BY s.ord) FROM ({sql}) s), '[]'::jsonb)",child.id));
        }
        let conditions = self.conditions(node, parent)?;
        let mut sort = node
            .sort
            .iter()
            .map(|s| {
                format!(
                    "{alias}.{} {} NULLS LAST",
                    quote(&s.column),
                    if s.descending { "DESC" } else { "ASC" }
                )
            })
            .collect::<Vec<_>>();
        for key in &keys {
            if !node.sort.iter().any(|s| &s.column == key) {
                let cast = if table.primary_keys.is_empty() {
                    "::text"
                } else {
                    ""
                };
                sort.push(format!("{alias}.{}{cast} ASC NULLS LAST", quote(key)));
            }
        }
        let order = if sort.is_empty() {
            "1".into()
        } else {
            sort.join(",")
        };
        Ok(format!("SELECT jsonb_build_object('key',to_jsonb(ARRAY[{key}]::text[])::text,'values',({values}),'children',jsonb_build_object({})) AS doc, row_number() OVER (ORDER BY {order}) AS ord FROM {} {alias} WHERE {conditions} ORDER BY {order} LIMIT {}",children.join(","),node.table.sql(),self.limit+1))
    }
}
