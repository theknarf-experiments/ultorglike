//! Recursive results and a terminal-independent layout with stable cell addresses.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub key: String,
    pub values: BTreeMap<String, Option<String>>,
    pub children: BTreeMap<String, Vec<Record>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub id: String,
    pub label: String,
    pub children: Vec<Field>,
    pub relation: bool,
    pub expanded: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Document {
    pub fields: Vec<Field>,
    pub rows: Vec<Record>,
    pub truncated: bool,
    pub collections: Vec<Collection>,
}

/// Loaded window of one collection, identified by its ancestor row identities.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collection {
    pub node: u64,
    pub parents: Vec<String>,
    pub offset: usize,
    pub loaded: usize,
    pub more: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Address {
    /// Stable primary-key identities of all ancestor rows, including this row.
    pub rows: Vec<String>,
    pub field: String,
}

#[derive(Clone, Debug)]
pub struct Cell {
    pub address: Address,
    pub column: usize,
    pub y: usize,
    pub height: usize,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct Heading {
    pub field: String,
    pub label: String,
    pub depth: usize,
    pub first_column: usize,
    pub columns: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Grid {
    pub headings: Vec<Heading>,
    pub cells: Vec<Cell>,
    pub widths: Vec<usize>,
    pub leaves: Vec<String>,
    pub height: usize,
    pub header_height: usize,
}

impl Grid {
    pub fn build(document: &Document) -> Self {
        let mut grid = Self::default();
        grid.schema(&document.fields, 0);
        grid.height = grid.records(&document.rows, &document.fields, 0, 0, &[]);
        for (i, id) in grid.leaves.iter().enumerate() {
            let label = grid
                .headings
                .iter()
                .find(|h| &h.field == id)
                .map(|h| h.label.as_str())
                .unwrap_or("");
            let width = grid
                .cells
                .iter()
                .filter(|c| c.column == i)
                .take(100)
                .map(|c| c.text.width())
                .max()
                .unwrap_or(0);
            grid.widths.push(width.max(label.width()).clamp(8, 32) + 2);
        }
        grid
    }

    fn schema(&mut self, fields: &[Field], depth: usize) {
        self.header_height = self.header_height.max(depth + 1);
        for field in fields {
            let start = self.leaves.len();
            if field.relation && field.expanded && !field.children.is_empty() {
                self.schema(&field.children, depth + 1);
            } else {
                self.leaves.push(field.id.clone());
            }
            self.headings.push(Heading {
                field: field.id.clone(),
                label: field.label.clone(),
                depth,
                first_column: start,
                columns: self.leaves.len() - start,
            });
        }
    }

    fn field_width(field: &Field) -> usize {
        if field.relation && field.expanded && !field.children.is_empty() {
            field.children.iter().map(Self::field_width).sum()
        } else {
            1
        }
    }

    fn records(
        &mut self,
        rows: &[Record],
        fields: &[Field],
        x: usize,
        y: usize,
        ancestors: &[String],
    ) -> usize {
        let mut offset = y;
        for row in rows {
            let mut path = ancestors.to_vec();
            path.push(row.key.clone());
            let mut column = x;
            let mut height = 1;
            let start_cell = self.cells.len();
            let mut scalar_cells = vec![];
            for field in fields {
                if field.relation && field.expanded && !field.children.is_empty() {
                    let children = row
                        .children
                        .get(&field.id)
                        .map(Vec::as_slice)
                        .unwrap_or(&[]);
                    if children.is_empty() {
                        self.empty_cells(&field.children, column, offset, &path);
                    } else {
                        height = height.max(self.records(
                            children,
                            &field.children,
                            column,
                            offset,
                            &path,
                        ));
                    }
                } else {
                    let text = if field.relation {
                        row.children
                            .get(&field.id)
                            .map(|rows| format!("[{} rows]", rows.len()))
                            .unwrap_or_else(|| "[folded]".into())
                    } else {
                        match row.values.get(&field.id) {
                            Some(Some(value)) => value.clone(),
                            Some(None) => "NULL".into(),
                            None => "…".into(),
                        }
                    };
                    scalar_cells.push(self.cells.len());
                    self.cells.push(Cell {
                        address: Address {
                            rows: path.clone(),
                            field: field.id.clone(),
                        },
                        column,
                        y: offset,
                        height: 1,
                        text,
                    });
                }
                column += Self::field_width(field);
            }
            for index in scalar_cells {
                self.cells[index].height = height;
            }
            // Extend empty sibling slots to the containing record's height.
            for cell in &mut self.cells[start_cell..] {
                if cell.address.rows == path && cell.text == "∅" {
                    cell.height = height;
                }
            }
            offset += height;
        }
        offset - y
    }

    fn empty_cells(&mut self, fields: &[Field], mut column: usize, y: usize, path: &[String]) {
        for field in fields {
            if field.relation && field.expanded && !field.children.is_empty() {
                self.empty_cells(&field.children, column, y, path);
            } else {
                self.cells.push(Cell {
                    address: Address {
                        rows: path.to_vec(),
                        field: field.id.clone(),
                    },
                    column,
                    y,
                    height: 1,
                    text: "∅".into(),
                });
            }
            column += Self::field_width(field);
        }
    }

    pub fn cell_at(&self, column: usize, y: usize) -> Option<&Cell> {
        self.cells
            .iter()
            .find(|c| c.column == column && c.y <= y && y < c.y + c.height)
    }

    pub fn x(&self, column: usize) -> usize {
        self.widths.iter().take(column).sum()
    }
    pub fn width(&self) -> usize {
        self.widths.iter().sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scalar(id: &str) -> Field {
        Field {
            id: id.into(),
            label: id.into(),
            children: vec![],
            relation: false,
            expanded: false,
        }
    }
    fn relation(id: &str, children: Vec<Field>) -> Field {
        Field {
            id: id.into(),
            label: id.into(),
            children,
            relation: true,
            expanded: true,
        }
    }
    fn row(key: &str, children: &[(&str, Vec<Record>)]) -> Record {
        Record {
            key: key.into(),
            children: children
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
            ..Default::default()
        }
    }
    #[test]
    fn nested_siblings_use_max_height_not_cartesian_product() {
        let doc = Document {
            fields: vec![
                scalar("name"),
                relation(
                    "orders",
                    vec![scalar("date"), relation("items", vec![scalar("product")])],
                ),
                relation("contacts", vec![scalar("phone")]),
            ],
            rows: vec![row(
                "customer",
                &[
                    (
                        "orders",
                        vec![row(
                            "order",
                            &[("items", vec![row("a", &[]), row("b", &[]), row("c", &[])])],
                        )],
                    ),
                    ("contacts", vec![row("d", &[]), row("e", &[])]),
                ],
            )],
            truncated: false,
            collections: vec![],
        };
        let grid = Grid::build(&doc);
        assert_eq!(grid.height, 3);
        assert_eq!(grid.header_height, 3);
        assert_eq!(grid.cell_at(0, 2).unwrap().height, 3);
        assert_eq!(
            grid.cell_at(2, 1).unwrap().address.rows,
            vec!["customer", "order", "b"]
        );
        assert_eq!(
            grid.cells
                .iter()
                .filter(|c| c.address.field == "phone")
                .count(),
            2
        );
    }
    #[test]
    fn empty_relations_keep_schema_and_parent_visible() {
        let doc = Document {
            fields: vec![scalar("name"), relation("orders", vec![scalar("date")])],
            rows: vec![row("a", &[])],
            truncated: false,
            collections: vec![],
        };
        let grid = Grid::build(&doc);
        assert_eq!(grid.height, 1);
        assert_eq!(grid.cell_at(1, 0).unwrap().text, "∅");
        let empty = Grid::build(&Document {
            rows: vec![],
            ..doc
        });
        assert_eq!(empty.leaves.len(), 2);
    }
}
