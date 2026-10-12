use super::*;
use data_model::document::Record;
use query_engine::Node;
use std::path::Path;

pub struct Panel {
    pub title: String,
    pub text: String,
    pub x: usize,
    pub y: usize,
}

pub(super) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // Same-directory rename makes the destination atomic; create_new avoids clobbering
    // another process's temporary file or following a pre-existing symlink.
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result.map_err(|e| e.to_string())
}

pub(super) fn record_at<'a>(
    node: &Node,
    rows: &'a [Record],
    id: u64,
    keys: &[String],
) -> Option<&'a Record> {
    let row = rows.iter().find(|r| Some(&r.key) == keys.first())?;
    if node.id == id {
        return (keys.len() == 1).then_some(row);
    }
    for child in &node.children {
        if child.find(id).is_some() {
            return record_at(
                child,
                row.children.get(&child.id.to_string())?,
                id,
                keys.get(1..)?,
            );
        }
    }
    None
}

impl App {
    pub(super) fn open_panel(&mut self, title: &str, text: String) {
        self.close_popup();
        self.panel = Some(Panel {
            title: title.into(),
            text,
            x: 0,
            y: 0,
        });
        self.input.set_mode(Mode::Other("panel".into()));
    }
    pub(super) fn panel_key(&mut self, key: KeyEvent) {
        let panel = self.panel.as_mut().unwrap();
        let last = panel.text.lines().count().saturating_sub(1);
        let width = panel
            .text
            .lines()
            .map(unicode_width::UnicodeWidthStr::width)
            .max()
            .unwrap_or(0);
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.close_popup(),
            KeyCode::Char('j') | KeyCode::Down => panel.y = (panel.y + 1).min(last),
            KeyCode::Char('k') | KeyCode::Up => panel.y = panel.y.saturating_sub(1),
            KeyCode::Char('h') | KeyCode::Left => panel.x = panel.x.saturating_sub(4),
            KeyCode::Char('l') | KeyCode::Right => panel.x = (panel.x + 4).min(width),
            KeyCode::Char('g') | KeyCode::Home => panel.y = 0,
            KeyCode::Char('G') | KeyCode::End => panel.y = last,
            KeyCode::PageDown => panel.y = (panel.y + 10).min(last),
            KeyCode::PageUp => panel.y = panel.y.saturating_sub(10),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                panel.y = (panel.y + 10).min(last)
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                panel.y = panel.y.saturating_sub(10)
            }
            _ => {}
        }
    }
    fn prompt(&mut self, text: &str) {
        self.input.enter_command_mode();
        for ch in text.chars() {
            self.input.push_command_char(ch);
        }
    }
    pub(super) fn workbench_command(&mut self, action: &str, arg: &str) -> Result<bool, String> {
        let optional = (!arg.is_empty()).then_some(arg);
        match action {
            "inspect" => {
                let (id, column) = self.field_target(optional)?;
                let q = self.query.as_ref().ok_or("Open a table first")?;
                let node = q.root.find(id).ok_or("Unknown relation")?;
                let address = self.address().ok_or("Select a value")?;
                let row = record_at(&q.root, &self.document.rows, id, &address.rows)
                    .ok_or("No record at this cell")?;
                let field = format!("{id}:{column}");
                let value = row.values.get(&field).ok_or("Field is not loaded")?;
                let kind = if node.formulas.iter().any(|f| f.name == column) {
                    "numeric formula"
                } else {
                    self.catalog
                        .table(&node.table)
                        .and_then(|t| t.columns.iter().find(|c| c.name == column))
                        .map(|c| c.data_type.as_str())
                        .unwrap_or("unknown")
                };
                let expression = node
                    .formulas
                    .iter()
                    .find(|f| f.name == column)
                    .map(|f| format!("\nFormula: {}", f.expression))
                    .unwrap_or_default();
                let text = format!(
                    "{}.{}\nType: {kind}{expression}\nRow: {}\n\n{}",
                    q.root.path(id).unwrap().join("."),
                    column,
                    row.key,
                    value.as_deref().unwrap_or("NULL")
                );
                self.open_panel("Value", text);
            }
            "formula_prompt" => {
                let id = self.node_id();
                let q = self.query.as_ref().ok_or("Open a table first")?;
                let node = q.root.find(id).ok_or("Unknown relation")?;
                let selected = self
                    .selected_field()
                    .and_then(|f| f.split_once(':'))
                    .map(|(_, c)| c);
                let formula = node
                    .formulas
                    .iter()
                    .find(|f| Some(f.name.as_str()) == selected);
                let (name, expr) = formula
                    .map(|f| (f.name.as_str(), f.expression.clone()))
                    .unwrap_or(("calculated", String::new()));
                let path = q.root.path(id).unwrap().join(".");
                self.prompt(&format!("formula {path}.{name} = {expr}"));
            }
            "filter_prompt" => {
                let (id, column) = self.field_target(None)?;
                let q = self.query.as_ref().unwrap();
                let path = q.root.path(id).unwrap().join(".");
                let filter = q
                    .root
                    .find(id)
                    .unwrap()
                    .filters
                    .iter()
                    .find(|f| f.column == column);
                let (op, value) = filter
                    .map(|f| (operator(&f.op), f.value.as_str()))
                    .unwrap_or(("=", ""));
                self.prompt(&format!("filter {path}.{column} {op} {value}"));
            }
            "width" | "wider" | "narrower" => {
                let (target, width) = if action == "width" {
                    let (target, number) = arg
                        .rsplit_once(' ')
                        .map(|(t, n)| (Some(t), n))
                        .unwrap_or((None, arg));
                    (
                        target,
                        number
                            .parse::<usize>()
                            .map_err(|_| "Usage: width [field] 24")?,
                    )
                } else {
                    let current = self.grid.widths.get(self.column).copied().unwrap_or(10);
                    (
                        None,
                        if action == "wider" {
                            (current + 2).min(160)
                        } else {
                            current.saturating_sub(2).max(4)
                        },
                    )
                };
                let (id, column) = self.field_target(target)?;
                let node = self
                    .query
                    .as_ref()
                    .and_then(|q| q.root.find(id))
                    .ok_or("Unknown relation")?;
                if !node.columns.contains(&column) {
                    return Err("Show a field before resizing it".into());
                }
                self.change(|q, _| {
                    q.widths.insert(format!("{id}:{column}"), width);
                    Ok(())
                });
            }
            "move" => {
                let (target, index) = arg
                    .rsplit_once(' ')
                    .ok_or("Usage: move field 1 (position within relation)")?;
                let index = index
                    .parse::<usize>()
                    .map_err(|_| "Position must be a number starting at 1")?;
                let (id, column) = self.field_target(Some(target))?;
                self.change(|q, _| {
                    let node = q.root.find_mut(id).ok_or("Unknown relation")?;
                    let from = node
                        .columns
                        .iter()
                        .position(|c| c == &column)
                        .ok_or("Show the field before moving it")?;
                    if index == 0 || index > node.columns.len() {
                        return Err("Position is outside visible fields".into());
                    }
                    let name = node.columns.remove(from);
                    node.columns.insert(index - 1, name);
                    Ok(())
                });
            }
            "query" => {
                let q = self.query.as_ref().ok_or("Open a table first")?;
                let mut text = String::new();
                fn describe(node: &Node, depth: usize, text: &mut String) {
                    let indent = "  ".repeat(depth);
                    text.push_str(&format!(
                        "{indent}{} ({}){}{}\n{indent}  fields: {}\n",
                        node.label,
                        node.table.qualified(),
                        if node.require_match {
                            " [require matching rows]"
                        } else {
                            ""
                        },
                        if !node.expanded { " [folded]" } else { "" },
                        node.columns.join(", ")
                    ));
                    for f in &node.filters {
                        text.push_str(&format!(
                            "{indent}  filter {} {} {}{}\n",
                            f.column,
                            operator(&f.op),
                            f.value,
                            if node.columns.contains(&f.column) {
                                ""
                            } else {
                                " [hidden]"
                            }
                        ));
                    }
                    for s in &node.sort {
                        text.push_str(&format!(
                            "{indent}  sort {} {}{}\n",
                            s.column,
                            if s.descending {
                                "descending"
                            } else {
                                "ascending"
                            },
                            if node.columns.contains(&s.column) {
                                ""
                            } else {
                                " [hidden]"
                            }
                        ));
                    }
                    for f in &node.formulas {
                        text.push_str(&format!(
                            "{indent}  formula {} = {}\n",
                            f.name, f.expression
                        ));
                    }
                    for child in &node.children {
                        describe(child, depth + 1, text);
                    }
                }
                describe(&q.root, 0, &mut text);
                text.push_str(&format!("\nPage size: {}\nAggregates include all matching rows.\n\nLoaded collections:\n", q.limit));
                for c in &self.document.collections {
                    text.push_str(&format!(
                        "{} parents {:?}: offset {}, {} loaded{}\n",
                        q.root.path(c.node).unwrap_or_default().join("."),
                        c.parents,
                        c.offset,
                        c.loaded,
                        if c.more { ", more available" } else { "" }
                    ));
                }
                self.open_panel("Query", text);
            }
            "sql" => {
                let text = query_engine::query::explain(
                    self.query.as_ref().ok_or("Open a table first")?,
                    &self.catalog,
                )
                .map_err(|e| e.to_string())?;
                self.open_panel("Generated SQL", text);
            }
            "schema" => {
                let id = self.relation_target(optional)?;
                let node = self
                    .query
                    .as_ref()
                    .and_then(|q| q.root.find(id))
                    .ok_or("Open a table first")?;
                let table = self.catalog.table(&node.table).ok_or("Table unavailable")?;
                let mut text = format!(
                    "{}\nPrimary key: {}\n\n",
                    table.name.qualified(),
                    table.primary_keys.join(", ")
                );
                for c in &table.columns {
                    text.push_str(&format!(
                        "{}: {} {}\n",
                        c.name,
                        c.data_type,
                        if c.nullable { "NULL" } else { "NOT NULL" }
                    ));
                }
                text.push_str("\nRelationships:\n");
                for r in self.catalog.relationships(&node.table) {
                    text.push_str(&format!(
                        "{}: ({}) → {} ({})\n",
                        r.constraint,
                        r.parent_columns.join(", "),
                        r.target.qualified(),
                        r.child_columns.join(", ")
                    ));
                }
                self.open_panel("Schema", text);
            }
            "export" => {
                if self.busy || self.error {
                    return Err("Wait for a successful query before exporting".into());
                }
                let (format, path) = arg
                    .split_once(' ')
                    .ok_or("Usage: export json|csv filename")?;
                let bytes = self.export(format)?;
                atomic_write(Path::new(path.trim()), &bytes)?;
                self.message = format!(
                    "Exported loaded results to {}{}",
                    path.trim(),
                    if self.document.truncated {
                        " (partial result)"
                    } else {
                        ""
                    }
                );
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
    pub(super) fn export(&self, format: &str) -> Result<Vec<u8>, String> {
        let q = self.query.as_ref().ok_or("Open a table first")?;
        match format {
            "json" => {
                fn rows(node: &Node, records: &[Record]) -> Vec<serde_json::Value> {
                    records
                        .iter()
                        .map(|r| {
                            let mut fields = serde_json::Map::new();
                            for c in &node.columns {
                                fields.insert(
                                    c.clone(),
                                    serde_json::json!(r
                                        .values
                                        .get(&format!("{}:{c}", node.id))
                                        .cloned()
                                        .flatten()),
                                );
                            }
                            let mut children = serde_json::Map::new();
                            for child in &node.children {
                                if let Some(records) = r.children.get(&child.id.to_string()) {
                                    children.insert(
                                        child.label.clone(),
                                        serde_json::json!(rows(child, records)),
                                    );
                                }
                            }
                            // Separate fields/children avoids collisions between column and relationship names.
                            serde_json::json!({"key":r.key,"fields":fields,"children":children})
                        })
                        .collect()
                }
                serde_json::to_vec_pretty(&serde_json::json!({"scope":"loaded", "partial":self.document.truncated,"collections":self.document.collections,"query":q,"rows":rows(&q.root,&self.document.rows)})).map_err(|e| e.to_string())
            }
            "csv" => {
                fn cell(value: &str) -> String {
                    format!("\"{}\"", value.replace('"', "\"\""))
                }
                let headers = self
                    .grid
                    .leaves
                    .iter()
                    .map(|f| {
                        let (id, name) = f.split_once(':').unwrap_or((f, ""));
                        let mut path = id
                            .parse()
                            .ok()
                            .and_then(|id| q.root.path(id))
                            .unwrap_or_default();
                        if !name.is_empty() {
                            path.push(name.into());
                        }
                        cell(&path.join("."))
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                let mut output = format!("{headers}\r\n");
                for y in 0..self.grid.height {
                    let values = (0..self.grid.leaves.len())
                        .map(|column| {
                            let Some(c) = self.grid.cell_at(column, y) else {
                                return cell("");
                            };
                            let Some((id, _)) = c.address.field.split_once(':') else {
                                return cell("[folded]");
                            };
                            let value = id
                                .parse()
                                .ok()
                                .and_then(|id| {
                                    record_at(&q.root, &self.document.rows, id, &c.address.rows)
                                })
                                .and_then(|r| r.values.get(&c.address.field));
                            cell(match value {
                                Some(Some(value)) => value,
                                Some(None) => "\\N",
                                None => "",
                            })
                        })
                        .collect::<Vec<_>>()
                        .join(",");
                    output.push_str(&values);
                    output.push_str("\r\n");
                }
                Ok(output.into_bytes())
            }
            _ => Err("Export format must be json or csv".into()),
        }
    }
}
fn operator(op: &Operator) -> &'static str {
    match op {
        Operator::Eq => "=",
        Operator::Ne => "!=",
        Operator::Gt => ">",
        Operator::Ge => ">=",
        Operator::Lt => "<",
        Operator::Le => "<=",
        Operator::Contains => "contains",
        Operator::IsNull => "null",
        Operator::NotNull => "notnull",
    }
}
