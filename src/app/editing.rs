use super::*;
use query_engine::editing::{Change, Operation};

impl App {
    fn editable_row(&self, id: u64) -> Result<Change, String> {
        let q = self.query.as_ref().ok_or("Open a table first")?;
        let node = q.root.find(id).ok_or("Unknown relation")?;
        let table = self.catalog.table(&node.table).ok_or("Unknown table")?;
        if table.primary_keys.is_empty() {
            return Err("Editing requires a table with a primary key".into());
        }
        let depth = q.root.path(id).unwrap().len();
        let address = self.address().ok_or("Select a record")?;
        let keys = address
            .rows
            .get(..depth)
            .ok_or("No record in this collection")?;
        let row = workbench::record_at(&q.root, &self.document.rows, id, keys)
            .ok_or("No loaded record at this cell")?;
        if row.version.is_none() {
            return Err("Refresh to load the row version before editing".into());
        }
        Ok(Change {
            table: table.name.clone(),
            operation: Operation::Update,
            keys: serde_json::from_str(&row.key).map_err(|_| "Row identity is not editable")?,
            version: row.version.clone(),
            values: Default::default(),
            original: Default::default(),
        })
    }
    pub(super) fn preview_edits(&mut self) {
        let Some(q) = &self.query else { return };
        for cell in &mut self.grid.cells {
            let Some((id, column)) = cell.address.field.split_once(':') else {
                continue;
            };
            let Some(node) = id.parse().ok().and_then(|id| q.root.find(id)) else {
                continue;
            };
            if q.root
                .path(node.id)
                .is_none_or(|path| path.len() != cell.address.rows.len())
            {
                continue;
            }
            let keys: Vec<Option<String>> = cell
                .address
                .rows
                .last()
                .and_then(|key| serde_json::from_str(key).ok())
                .unwrap_or_default();
            if let Some(change) = self.edits.iter().find(|e| {
                e.table == node.table && e.keys == keys && e.operation != Operation::Insert
            }) {
                if change.operation == Operation::Delete {
                    cell.text = format!("× {}", cell.text);
                } else if let Some(value) = change.values.get(column) {
                    cell.text = format!("* {}", value.as_deref().unwrap_or("NULL"));
                }
            }
        }
    }
    pub(super) fn poll_edits(&mut self) {
        use std::sync::mpsc::TryRecvError;
        let result = match self.applying.as_ref().map(|rx| rx.try_recv()) {
            Some(Ok(result)) => Some(result),
            Some(Err(TryRecvError::Disconnected)) => Some(Err(
                "Write worker disconnected; refresh and check the database before retrying".into(),
            )),
            _ => None,
        };
        if let Some(result) = result {
            self.applying = None;
            self.busy = false;
            match result {
                Ok(count) => {
                    self.edits.clear();
                    self.error = false;
                    self.message = format!("Applied {count} row changes");
                    self.refresh();
                }
                Err(error) => self.fail(format!("Apply failed: {error}. Staged changes retained. Check the database before retrying after connection errors.")),
            }
        }
    }
    pub(super) fn edit_command(&mut self, action: &str, arg: &str) -> Result<bool, String> {
        if self.applying.is_some() {
            return Err("Applying transaction; wait for its result".into());
        }
        if matches!(
            action,
            "q" | "quit" | "session_load" | "session_save" | "export"
        ) && !self.edits.is_empty()
        {
            return Err(
                "Staged changes remain: :changes to review, :apply to apply, or :discard".into(),
            );
        }
        if action == "quit!" {
            self.should_quit = true;
            return Ok(true);
        }
        if !matches!(
            action,
            "edit" | "edit_prompt" | "set" | "insert" | "delete" | "changes" | "apply" | "discard"
        ) {
            return Ok(false);
        }
        if !self.editable {
            return Err("Read-only mode. Restart with --editable to stage database changes".into());
        }
        if self.busy {
            return Err("Wait for the current query before editing".into());
        }
        if self.error && matches!(action, "edit_prompt" | "edit" | "set" | "delete") {
            return Err("Refresh successfully before staging more edits".into());
        }
        match action {
            "edit_prompt" => {
                let (id, column) = self.field_target(None)?;
                let change = self.editable_row(id)?;
                let q = self.query.as_ref().unwrap();
                let row = workbench::record_at(
                    &q.root,
                    &self.document.rows,
                    id,
                    &self.address().unwrap().rows,
                )
                .unwrap();
                let value = self
                    .edits
                    .iter()
                    .find(|e| e.table == change.table && e.keys == change.keys)
                    .and_then(|e| e.values.get(&column))
                    .or_else(|| row.values.get(&format!("{id}:{column}")))
                    .ok_or("Select a loaded field")?;
                let value = serde_json::to_string(value).map_err(|e| e.to_string())?;
                self.input.enter_command_mode();
                for ch in format!("edit {value}").chars() {
                    self.input.push_command_char(ch);
                }
            }
            "edit" | "set" => {
                if self.error {
                    return Err("Refresh successfully before staging more edits".into());
                }
                let (target, text) = if action == "set" {
                    let (field, value) = arg.split_once(' ').ok_or("Usage: set field value")?;
                    (Some(field), value.trim())
                } else {
                    (None, arg)
                };
                if text.is_empty() {
                    return Err(
                        "Use edit \"\" for empty text, edit null for NULL, or edit VALUE".into(),
                    );
                }
                let value: Option<String> = if text == "null" {
                    None
                } else if text.starts_with('"') {
                    Some(serde_json::from_str(text).map_err(|_| "Invalid JSON string")?)
                } else {
                    Some(text.into())
                };
                let (id, column) = self.field_target(target)?;
                let mut change = self.editable_row(id)?;
                let table = self.catalog.table(&change.table).unwrap();
                let col = table
                    .columns
                    .iter()
                    .find(|c| c.name == column)
                    .ok_or("Formula columns cannot be edited")?;
                if col.generated || table.primary_keys.contains(&column) {
                    return Err("Generated columns and primary keys cannot be edited".into());
                }
                if value.is_none() && !col.nullable {
                    return Err("This field does not allow NULL".into());
                }
                let q = self.query.as_ref().unwrap();
                let depth = q.root.path(id).unwrap().len();
                let address = self.address().unwrap();
                let row =
                    workbench::record_at(&q.root, &self.document.rows, id, &address.rows[..depth])
                        .unwrap();
                let original = row
                    .values
                    .get(&format!("{id}:{column}"))
                    .cloned()
                    .ok_or("Show the field before editing it")?;
                let index = self.edits.iter().position(|e| {
                    e.table == change.table
                        && e.keys == change.keys
                        && e.operation != Operation::Insert
                });
                if let Some(index) = index {
                    let existing = &mut self.edits[index];
                    if existing.version != change.version {
                        return Err(
                            "This row was refreshed after staging; discard its old changes first"
                                .into(),
                        );
                    }
                    if existing.operation == Operation::Delete {
                        return Err("Row is staged for deletion; discard that change first".into());
                    }
                    if value == original {
                        existing.values.remove(&column);
                        existing.original.remove(&column);
                    } else {
                        existing.values.insert(column.clone(), value);
                        existing.original.insert(column, original);
                    }
                    if existing.values.is_empty() {
                        self.edits.remove(index);
                    }
                } else if value != original {
                    if self.edits.len() >= 100 {
                        return Err("Apply/discard the current batch (maximum 100 rows)".into());
                    }
                    change.values.insert(column.clone(), value);
                    change.original.insert(column, original);
                    self.edits.push(change);
                }
                self.rebuild(self.address());
            }
            "delete" => {
                let mut change = self.editable_row(self.node_id())?;
                change.operation = Operation::Delete;
                if let Some(index) = self.edits.iter().position(|e| {
                    e.table == change.table
                        && e.keys == change.keys
                        && e.operation != Operation::Insert
                }) {
                    self.edits[index] = change;
                } else {
                    if self.edits.len() >= 100 {
                        return Err("Batch is full (100 rows)".into());
                    }
                    self.edits.push(change);
                }
                self.rebuild(self.address());
            }
            "insert" => {
                if self.edits.len() >= 100 {
                    return Err("Batch is full (100 rows)".into());
                }
                let node = self
                    .query
                    .as_ref()
                    .and_then(|q| q.root.find(self.node_id()))
                    .ok_or("Open a table first")?;
                let table = self.catalog.table(&node.table).unwrap();
                if table.primary_keys.is_empty() {
                    return Err("Editing requires a table with a primary key".into());
                }
                let values: std::collections::BTreeMap<String, Option<String>> = serde_json::from_str(arg).map_err(|_| "Usage: insert {\"column\":\"value\",\"nullable_column\":null}; values must be strings/null")?;
                for (name, value) in &values {
                    let column = table
                        .columns
                        .iter()
                        .find(|c| &c.name == name)
                        .ok_or("Unknown insert column")?;
                    if column.generated {
                        return Err(format!("{name} is generated; omit it"));
                    }
                    if value.is_none() && !column.nullable {
                        return Err(format!("{name} does not allow NULL"));
                    }
                }
                self.edits.push(Change {
                    table: table.name.clone(),
                    operation: Operation::Insert,
                    keys: vec![],
                    version: None,
                    values,
                    original: Default::default(),
                });
            }
            "discard" => {
                if arg.is_empty() || arg == "all" {
                    self.edits.clear();
                } else {
                    let index = if arg == "last" {
                        self.edits.len()
                    } else {
                        arg.parse::<usize>()
                            .map_err(|_| "Usage: discard [all|last|N]")?
                    };
                    if index == 0 || index > self.edits.len() {
                        return Err("No such staged change".into());
                    }
                    self.edits.remove(index - 1);
                }
                self.error = false;
                self.rebuild(self.address());
            }
            "changes" | "apply" => {
                if self.edits.is_empty() {
                    return Err("No staged changes".into());
                }
                if action == "apply" && arg == "yes" {
                    let worker = self.worker.as_mut().ok_or("No database connection")?;
                    worker.cancel();
                    self.applying =
                        Some(worker.apply_changes(self.catalog.clone(), self.edits.clone()));
                    self.busy = true;
                    self.message = "Applying transaction…".into();
                    self.close_popup();
                    return Ok(true);
                }
                let mut text = String::from("Staged database changes (not yet written)\n\n");
                for (index, change) in self.edits.iter().enumerate() {
                    text.push_str(&format!(
                        "{}. {:?} {} key {:?}\n",
                        index + 1,
                        change.operation,
                        change.table.qualified(),
                        change.keys
                    ));
                    for (field, value) in &change.values {
                        text.push_str(&format!(
                            "   {field}: {:?} → {:?}\n",
                            change.original.get(field),
                            value
                        ));
                    }
                }
                text.push_str("\nAll changes apply in one transaction. Changed/deleted rows cause a conflict.\nDeletes follow the database's foreign-key rules, including cascades.\nClose with q, then :apply yes to write, or :discard [all|last|N].\nQuery undo (u) does not undo database writes.\n");
                self.open_panel("Review database changes", text);
            }
            _ => unreachable!(),
        }
        self.message = format!(
            "{} staged row changes · :changes review · :apply · :discard",
            self.edits.len()
        );
        Ok(true)
    }
}
