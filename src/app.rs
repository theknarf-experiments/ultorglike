use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use data_model::document::{Address, Document, Grid};
use query_engine::{
    executor::{Response, Worker},
    Catalog, Filter, Formula, Operator, Page, Query, Relationship, Sort,
};
use tui_input::{InputContext, Mode};

#[derive(Clone)]
pub enum Choice {
    Table(usize),
    Field(String),
    Relationship(Relationship),
}
#[derive(Clone)]
pub struct Entry {
    pub label: String,
    pub choice: Choice,
}
pub struct Picker {
    pub node: Option<u64>,
    pub text: String,
    pub searching: bool,
    pub selected: usize,
    pub entries: Vec<Entry>,
}

pub struct App {
    pub input: InputContext,
    pub should_quit: bool,
    pub catalog: Catalog,
    pub query: Option<Query>,
    pub document: Document,
    pub grid: Grid,
    pub column: usize,
    pub y: usize,
    pub scroll_x: usize,
    pub scroll_y: usize,
    pub viewport_width: usize,
    pub viewport_height: usize,
    pub picker: Option<Picker>,
    pub message: String,
    pub busy: bool,
    pub error: bool,
    pub search: String,
    pub count: String,
    pub views: Vec<Query>,
    pub active_view: usize,
    undo: Vec<Query>,
    redo: Vec<Query>,
    worker: Option<Worker>,
    initial_view: Option<String>,
}
impl App {
    pub fn new(url: Option<String>, initial_view: Option<String>) -> Self {
        let mut input = InputContext::new();
        for name in [
            "quit",
            "help",
            "tables",
            "open",
            "fields",
            "join",
            "formula",
            "unformula",
            "filter",
            "unfilter",
            "sort",
            "sort_desc",
            "unsort",
            "hide",
            "show",
            "match",
            "keep",
            "expand",
            "collapse",
            "undo",
            "redo",
            "save",
            "load",
            "nextpage",
            "prevpage",
            "more",
            "limit",
            "refresh",
            "view",
        ] {
            input.register_handler(name, |_| false);
        }
        let mut worker = url.map(Worker::new);
        if let Some(worker) = worker.as_mut() {
            worker.submit(None);
        }
        let busy = worker.is_some();
        Self {
            input,
            should_quit: false,
            catalog: Catalog::default(),
            query: None,
            document: Document::default(),
            grid: Grid::default(),
            column: 0,
            y: 0,
            scroll_x: 0,
            scroll_y: 0,
            viewport_width: 80,
            viewport_height: 20,
            picker: None,
            message: if busy {
                "Connecting to PostgreSQL…".into()
            } else {
                String::new()
            },
            busy,
            error: false,
            search: String::new(),
            count: String::new(),
            views: vec![],
            active_view: 0,
            undo: vec![],
            redo: vec![],
            worker,
            initial_view,
        }
    }
    pub fn poll(&mut self) {
        let response = self.worker.as_mut().and_then(Worker::poll);
        if let Some(response) = response {
            self.busy = false;
            match response {
                Err(error) => self.fail(format!(
                    "Query failed: {error}. Previous results remain visible; u to undo."
                )),
                Ok(Response::Catalog(catalog)) => {
                    self.catalog = catalog;
                    if let Some(path) = self.initial_view.take() {
                        self.handle_command(&format!("load {path}"));
                    } else if let Some(index) = self
                        .catalog
                        .tables
                        .iter()
                        .position(|t| t.name.name == "customers")
                        .or(if self.catalog.tables.is_empty() {
                            None
                        } else {
                            Some(0)
                        })
                    {
                        self.open_table(index);
                    } else {
                        self.message = "No visible tables. Press t to inspect the catalog.".into();
                    }
                }
                Ok(Response::Data(document)) => {
                    let address = self.address();
                    self.document = document;
                    self.rebuild(address);
                    self.error = false;
                    self.message = if self.document.truncated {
                        format!(
                            "Partial result: {} rows per page; ]p next, [p previous, + more",
                            self.query.as_ref().map_or(100, |q| q.limit)
                        )
                    } else {
                        format!("{} root rows", self.document.rows.len())
                    };
                }
            }
        }
    }
    pub fn address(&self) -> Option<Address> {
        self.grid
            .cell_at(self.column, self.y)
            .map(|c| c.address.clone())
    }
    pub fn selected_field(&self) -> Option<&str> {
        self.grid.leaves.get(self.column).map(String::as_str)
    }
    pub fn node_id(&self) -> u64 {
        self.selected_field()
            .and_then(|f| f.split(':').next()?.parse().ok())
            .unwrap_or(1)
    }
    pub fn collection(&self, id: u64) -> Option<&data_model::document::Collection> {
        let depth = self.query.as_ref()?.root.path(id)?.len() - 1;
        let address = self.address().unwrap_or_default();
        if address.rows.len() < depth {
            return None;
        }
        self.document
            .collections
            .iter()
            .find(|c| c.node == id && c.parents == address.rows[..depth])
    }
    pub fn range(&self) -> String {
        self.collection(self.node_id())
            .map(|c| {
                format!(
                    "rows {}–{}{}",
                    if c.loaded == 0 { 0 } else { c.offset + 1 },
                    c.offset + c.loaded,
                    if c.more { " +" } else { "" }
                )
            })
            .unwrap_or_default()
    }
    fn page(&mut self, action: &str, id: u64) -> std::result::Result<(), String> {
        if self.busy {
            return Err("Wait for the current query or cancel with Esc".into());
        }
        let collection = self
            .collection(id)
            .ok_or("Expand and select the collection first")?
            .clone();
        if action != "prevpage" && !collection.more {
            return Err("No more rows in this collection".into());
        }
        let mut query = self.query.clone().ok_or("Open a table first")?;
        let (offset, limit) = query.window(id, &collection.parents);
        let (offset, limit) = match action {
            "nextpage" => (offset + collection.loaded, query.limit),
            "prevpage" => (offset.saturating_sub(query.limit), query.limit),
            _ => (offset, (limit + query.limit).min(5000)),
        };
        if action == "more" && limit == 5000 && collection.loaded == 5000 {
            return Err("Use nextpage to browse past 5000 loaded rows".into());
        }
        query
            .pages
            .retain(|p| p.node != id || p.parents != collection.parents);
        query.pages.push(Page {
            node: id,
            parents: collection.parents,
            offset,
            limit,
        });
        query.validate(&self.catalog).map_err(|e| e.to_string())?;
        self.query = Some(query);
        self.error = false;
        self.refresh();
        Ok(())
    }
    pub fn location(&self) -> String {
        let Some(query) = &self.query else {
            return String::new();
        };
        let mut path = query.root.path(self.node_id()).unwrap_or_default();
        if let Some((_, column)) = self.selected_field().and_then(|f| f.split_once(':')) {
            path.push(column.into());
        }
        path.join(" → ")
    }
    fn fail(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.error = true;
    }
    fn rebuild(&mut self, address: Option<Address>) {
        self.grid = Grid::build(&self.document);
        if let Some(address) = address {
            if let Some(cell) = self.grid.cells.iter().find(|c| c.address == address) {
                self.column = cell.column;
                self.y = cell.y;
            } else if let Some(col) = self.grid.leaves.iter().position(|f| f == &address.field) {
                self.column = col;
            }
        }
        self.clamp();
    }
    pub fn clamp(&mut self) {
        self.column = self.column.min(self.grid.leaves.len().saturating_sub(1));
        self.y = self.y.min(self.grid.height.saturating_sub(1));
        if self.y < self.scroll_y {
            self.scroll_y = self.y;
        }
        if self.y >= self.scroll_y + self.viewport_height.max(1) {
            self.scroll_y = self.y + 1 - self.viewport_height.max(1);
        }
        self.scroll_y = self
            .scroll_y
            .min(self.grid.height.saturating_sub(self.viewport_height));
        let x = self.grid.x(self.column);
        let width = self
            .grid
            .widths
            .get(self.column)
            .copied()
            .unwrap_or(1)
            .min(self.viewport_width);
        if x < self.scroll_x {
            self.scroll_x = x;
        }
        if x + width > self.scroll_x + self.viewport_width {
            self.scroll_x = x + width - self.viewport_width;
        }
        self.scroll_x = self
            .scroll_x
            .min(self.grid.width().saturating_sub(self.viewport_width));
    }
    fn refresh(&mut self) {
        if let Some(query) = &self.query {
            let address = self.address();
            self.document.fields = query.root.fields();
            if let Some(worker) = &mut self.worker {
                worker.submit(Some((query.clone(), self.catalog.clone())));
                self.busy = true;
            }
            self.rebuild(address);
        } else if let Some(worker) = &mut self.worker {
            worker.submit(None);
            self.busy = true;
        }
    }
    fn change(
        &mut self,
        modify: impl FnOnce(&mut Query, &Catalog) -> std::result::Result<(), String>,
    ) {
        let Some(old) = self.query.clone() else {
            self.fail("Open a table first (t)");
            return;
        };
        let mut next = old.clone();
        next.pages.clear();
        let result = modify(&mut next, &self.catalog)
            .and_then(|_| next.validate(&self.catalog).map_err(|e| e.to_string()));
        match result {
            Err(e) => self.fail(e),
            Ok(()) => {
                if next != old {
                    self.undo.push(old);
                    self.redo.clear();
                    self.query = Some(next);
                    self.error = false;
                    self.refresh();
                }
            }
        }
    }
    fn open_table(&mut self, index: usize) {
        let Some(table) = self.catalog.tables.get(index) else {
            return;
        };
        let query = Query::new(table);
        if let Some(old) = self.query.take() {
            if let Some(slot) = self.views.get_mut(self.active_view) {
                *slot = old;
            }
        }
        self.views.push(query.clone());
        self.active_view = self.views.len() - 1;
        self.query = Some(query);
        self.undo.clear();
        self.redo.clear();
        self.document = Document::default();
        self.column = 0;
        self.y = 0;
        self.scroll_y = 0;
        self.scroll_x = 0;
        self.close_popup();
        self.refresh();
    }
    fn switch_view(&mut self, index: usize) {
        if index >= self.views.len() {
            self.fail("No such view");
            return;
        }
        if let Some(query) = self.query.take() {
            self.views[self.active_view] = query;
        }
        self.active_view = index;
        self.query = Some(self.views[index].clone());
        self.undo.clear();
        self.redo.clear();
        self.document = Document::default();
        self.column = 0;
        self.y = 0;
        self.refresh();
    }
    fn close_popup(&mut self) {
        self.picker = None;
        self.input.set_mode(Mode::Normal);
    }
    fn show_picker(&mut self, node: Option<u64>) {
        self.input.set_mode(Mode::Other("picker".into()));
        self.picker = Some(Picker {
            node,
            text: String::new(),
            searching: false,
            selected: 0,
            entries: vec![],
        });
        self.update_picker();
    }
    fn update_picker(&mut self) {
        let Some(picker) = &mut self.picker else {
            return;
        };
        let mut entries = vec![];
        if let Some(id) = picker.node {
            if let Some(node) = self.query.as_ref().and_then(|q| q.root.find(id)) {
                if let Some(table) = self.catalog.table(&node.table) {
                    for col in &table.columns {
                        let mark = if node.columns.contains(&col.name) {
                            "x"
                        } else {
                            " "
                        };
                        let filter = node
                            .filters
                            .iter()
                            .find(|f| f.column == col.name)
                            .map(|f| format!("  [filter {:?} {}]", f.op, f.value))
                            .unwrap_or_default();
                        let sort = node
                            .sort
                            .iter()
                            .find(|s| s.column == col.name)
                            .map(|s| {
                                if s.descending {
                                    " [sort ↓]"
                                } else {
                                    " [sort ↑]"
                                }
                            })
                            .unwrap_or("");
                        entries.push(Entry {
                            label: format!(
                                "[{mark}] {} : {}{filter}{sort}",
                                col.name, col.data_type
                            ),
                            choice: Choice::Field(col.name.clone()),
                        });
                    }
                }
                for formula in &node.formulas {
                    let mark = if node.columns.contains(&formula.name) {
                        "x"
                    } else {
                        " "
                    };
                    let filter = node
                        .filters
                        .iter()
                        .find(|f| f.column == formula.name)
                        .map(|f| format!(" [filter {:?} {}]", f.op, f.value))
                        .unwrap_or_default();
                    let sort = node
                        .sort
                        .iter()
                        .find(|s| s.column == formula.name)
                        .map(|s| {
                            if s.descending {
                                " [sort ↓]"
                            } else {
                                " [sort ↑]"
                            }
                        })
                        .unwrap_or("");
                    entries.push(Entry {
                        label: format!(
                            "[{mark}] ƒ {} = {}{filter}{sort}",
                            formula.name, formula.expression
                        ),
                        choice: Choice::Field(formula.name.clone()),
                    });
                }
                for rel in self.catalog.relationships(&node.table) {
                    let exists = node.children.iter().any(|c| c.join.as_ref() == Some(&rel));
                    entries.push(Entry {
                        label: format!(
                            "[{}] → {} via {} ({} = {})",
                            if exists { "x" } else { " " },
                            rel.target.qualified(),
                            rel.constraint,
                            rel.parent_columns.join(","),
                            rel.child_columns.join(",")
                        ),
                        choice: Choice::Relationship(rel),
                    });
                }
            }
        } else {
            entries = self
                .catalog
                .tables
                .iter()
                .enumerate()
                .map(|(i, t)| Entry {
                    label: t.name.qualified(),
                    choice: Choice::Table(i),
                })
                .collect();
        }
        let text = picker.text.to_lowercase();
        picker.entries = entries
            .into_iter()
            .filter(|e| e.label.to_lowercase().contains(&text))
            .collect();
        picker.selected = picker.selected.min(picker.entries.len().saturating_sub(1));
    }
    fn picker_key(&mut self, key: KeyEvent) {
        let Some(picker) = self.picker.as_mut() else {
            return;
        };
        if picker.searching {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => picker.searching = false,
                KeyCode::Backspace => {
                    picker.text.pop();
                }
                KeyCode::Char(c) => {
                    picker.text.push(c);
                    picker.selected = 0;
                }
                _ => {}
            }
            self.update_picker();
            return;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.close_popup(),
            KeyCode::Char('/') => picker.searching = true,
            KeyCode::Char('j') | KeyCode::Down => {
                picker.selected = (picker.selected + 1).min(picker.entries.len().saturating_sub(1))
            }
            KeyCode::Char('k') | KeyCode::Up => picker.selected = picker.selected.saturating_sub(1),
            KeyCode::Tab | KeyCode::Right => {
                let id = picker.node;
                let choice = picker
                    .entries
                    .get(picker.selected)
                    .map(|e| e.choice.clone());
                if let (Some(id), Some(Choice::Relationship(rel))) = (id, choice) {
                    let child = self
                        .query
                        .as_ref()
                        .and_then(|q| q.root.find(id))
                        .and_then(|n| n.children.iter().find(|n| n.join.as_ref() == Some(&rel)))
                        .map(|n| n.id);
                    if let Some(child) = child {
                        self.show_picker(Some(child));
                    } else {
                        self.fail("Add the relationship with Enter first");
                    }
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                let id = picker.node;
                let choice = picker
                    .entries
                    .get(picker.selected)
                    .map(|e| e.choice.clone());
                match choice {
                    Some(Choice::Table(index)) => self.open_table(index),
                    Some(Choice::Field(column)) => {
                        self.change(|q, _| {
                            let node = q.root.find_mut(id.unwrap()).unwrap();
                            if node.columns.contains(&column) {
                                node.columns.retain(|c| c != &column);
                            } else {
                                node.columns.push(column);
                            }
                            Ok(())
                        });
                        self.update_picker();
                    }
                    Some(Choice::Relationship(rel)) => {
                        self.change(|q, c| {
                            let node = q.root.find_mut(id.unwrap()).unwrap();
                            if let Some(i) = node
                                .children
                                .iter()
                                .position(|n| n.join.as_ref() == Some(&rel))
                            {
                                node.children.remove(i);
                            } else {
                                q.add_relationship(id.unwrap(), rel, c)
                                    .map_err(|e| e.to_string())?;
                            }
                            Ok(())
                        });
                        self.update_picker();
                    }
                    None => {}
                }
            }
            _ => {}
        }
    }
    pub fn handle_event(&mut self, event: Event) {
        let Event::Key(key) = event else {
            return;
        };
        if key.kind == KeyEventKind::Release {
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            if self.input.mode() != &Mode::Normal {
                self.close_popup();
            } else {
                self.should_quit = true;
            }
            return;
        }
        if self.input.mode() == &Mode::Help {
            if matches!(
                key.code,
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Char('H')
            ) {
                self.close_popup();
            }
            return;
        }
        if self.picker.is_some() {
            self.picker_key(key);
            return;
        }
        if self.input.mode() == &Mode::Other("search".into()) {
            match key.code {
                KeyCode::Esc => self.input.set_mode(Mode::Normal),
                KeyCode::Enter => {
                    self.input.set_mode(Mode::Normal);
                    self.find(false);
                }
                KeyCode::Backspace => {
                    self.search.pop();
                }
                KeyCode::Char(c) => self.search.push(c),
                _ => {}
            }
            return;
        }
        if self.input.mode() == &Mode::Normal {
            if let KeyCode::Char(c @ '0'..='9') = key.code {
                if c != '0' || !self.count.is_empty() {
                    if self.count.len() < 4 {
                        self.count.push(c);
                    }
                    return;
                }
            }
            if key.code == KeyCode::Esc {
                self.count.clear();
            }
        }
        if let Some(command) = self.input.handle_event(&Event::Key(key)) {
            self.handle_command(&command);
        }
    }
    fn movement(&mut self, action: &str, count: usize) {
        for _ in 0..count {
            match action {
                "pan_down" => {
                    self.y = self
                        .grid
                        .cell_at(self.column, self.y)
                        .map_or(self.y + 1, |c| c.y + c.height);
                }
                "pan_up" => {
                    self.y = self
                        .grid
                        .cell_at(self.column, self.y)
                        .map_or(self.y, |c| c.y)
                        .saturating_sub(1);
                }
                "pan_left" => self.column = self.column.saturating_sub(1),
                "pan_right" => self.column += 1,
                "page_down" => self.y += self.viewport_height / 2,
                "page_up" => self.y = self.y.saturating_sub(self.viewport_height / 2),
                _ => {}
            }
            self.clamp();
        }
    }
    fn find(&mut self, backwards: bool) {
        if self.search.is_empty() {
            return;
        }
        let mut cells: Vec<_> = self.grid.cells.iter().collect();
        cells.sort_by_key(|c| (c.y, c.column));
        let start = cells
            .iter()
            .position(|c| c.column == self.column && c.y <= self.y && self.y < c.y + c.height)
            .unwrap_or(0);
        for step in 1..=cells.len() {
            let i = if backwards {
                (start + cells.len() - step) % cells.len()
            } else {
                (start + step) % cells.len()
            };
            let c = cells[i];
            if c.text.to_lowercase().contains(&self.search.to_lowercase()) {
                self.column = c.column;
                self.y = c.y;
                self.clamp();
                self.message = format!("/{}", self.search);
                return;
            }
        }
        self.message = format!("No match for {} in loaded rows", self.search);
    }
    fn field_target(&self, text: Option<&str>) -> std::result::Result<(u64, String), String> {
        let q = self.query.as_ref().ok_or("Open a table first")?;
        if let Some(text) = text {
            if let Some((id, col)) = text.split_once(':') {
                return Ok((id.parse().map_err(|_| "Invalid field ID")?, col.into()));
            }
            if let Some((path, col)) = text.rsplit_once('.') {
                return Ok((q.resolve(path).ok_or("Unknown relation path")?, col.into()));
            }
            return Ok((self.node_id(), text.into()));
        }
        let field = self.selected_field().ok_or("Select a field")?;
        let (id, col) = field.split_once(':').ok_or("Select a primitive field")?;
        Ok((id.parse().unwrap(), col.into()))
    }
    fn relation_target(&self, text: Option<&str>) -> std::result::Result<u64, String> {
        match text {
            Some(path) => self
                .query
                .as_ref()
                .and_then(|q| q.resolve(path))
                .ok_or_else(|| "Unknown relation path".into()),
            None => Ok(self.node_id()),
        }
    }
    pub fn handle_command(&mut self, command: &str) {
        if let Err(error) = self.command(command) {
            self.fail(error);
        }
    }
    fn command(&mut self, command: &str) -> std::result::Result<(), String> {
        let mut words = command.trim().splitn(2, char::is_whitespace);
        let action = words.next().unwrap_or("");
        let arg = words.next().unwrap_or("").trim();
        let optional = if arg.is_empty() { None } else { Some(arg) };
        let count = self.count.parse::<usize>().unwrap_or(1);
        self.count.clear();
        match action {
            "q" | "quit" => self.should_quit = true,
            "help" | "?" => self.input.set_mode(Mode::Help),
            "escape" => {
                if self.busy {
                    if let Some(worker) = &mut self.worker {
                        worker.cancel();
                    }
                    self.busy = false;
                    self.message = "Query cancelled; :refresh to retry".into();
                } else {
                    self.close_popup();
                }
            }
            "pan_up" | "pan_down" | "pan_left" | "pan_right" | "page_down" | "page_up" => {
                self.movement(action, count)
            }
            "jump_to_first_row" => {
                self.y = 0;
                self.clamp();
            }
            "jump_to_last_row" => {
                self.y = self.grid.height.saturating_sub(1);
                self.clamp();
            }
            "first_column" => {
                self.column = 0;
                self.clamp();
            }
            "last_column" => {
                self.column = self.grid.leaves.len().saturating_sub(1);
                self.clamp();
            }
            "search" => {
                self.search.clear();
                self.input.set_mode(Mode::Other("search".into()));
            }
            "search_next" => self.find(false),
            "search_previous" => self.find(true),
            "tables" => self.show_picker(None),
            "fields" => {
                let id = self.relation_target(optional)?;
                self.show_picker(Some(id));
            }
            "open" => {
                let index = self
                    .catalog
                    .tables
                    .iter()
                    .position(|t| t.name.qualified() == arg || t.name.name == arg)
                    .ok_or("Table not found; use t to browse")?;
                self.open_table(index);
            }
            "next_view" => {
                if !self.views.is_empty() {
                    self.switch_view((self.active_view + 1) % self.views.len());
                }
            }
            "previous_view" => {
                if !self.views.is_empty() {
                    self.switch_view((self.active_view + self.views.len() - 1) % self.views.len());
                }
            }
            "view" => {
                let index = arg.parse::<usize>().map_err(|_| "Usage: view 1")?;
                self.switch_view(index.checked_sub(1).ok_or("View numbers start at 1")?);
            }
            "join" => {
                let mut args = arg.split_whitespace();
                let target = args
                    .next()
                    .ok_or("Usage: join target_table [parent.path]")?;
                let parent = self.relation_target(args.next())?;
                let node = self
                    .query
                    .as_ref()
                    .and_then(|q| q.root.find(parent))
                    .ok_or("Unknown relation")?;
                let rels: Vec<_> = self
                    .catalog
                    .relationships(&node.table)
                    .into_iter()
                    .filter(|r| {
                        r.target.name == target
                            || r.target.qualified() == target
                            || r.constraint == target
                    })
                    .collect();
                if rels.len() != 1 {
                    return Err(
                        "Relationship ambiguous or missing; use the field picker (f)".into(),
                    );
                }
                let rel = rels[0].clone();
                self.change(|q, c| {
                    q.add_relationship(parent, rel, c)
                        .map_err(|e| e.to_string())?;
                    Ok(())
                });
            }
            "expand" | "collapse" | "toggle_relation" => {
                let id = self.relation_target(optional)?;
                if id == 1 {
                    self.show_picker(Some(id));
                } else {
                    self.change(|q, _| {
                        let node = q.root.find_mut(id).ok_or("Unknown relation")?;
                        node.expanded = match action {
                            "expand" => true,
                            "collapse" => false,
                            _ => !node.expanded,
                        };
                        Ok(())
                    });
                    if action == "collapse" {
                        if let Some(col) =
                            self.grid.leaves.iter().position(|f| f == &id.to_string())
                        {
                            self.column = col;
                            self.clamp();
                        }
                    }
                }
            }
            "formula" => {
                let (target, expression) = arg
                    .split_once('=')
                    .ok_or("Usage: formula orders.total = sum([line_items.amount]) + [freight]")?;
                let (id, name) = self.field_target(Some(target.trim()))?;
                let expression = expression.trim().to_string();
                self.change(|q, _| {
                    let node = q.root.find_mut(id).ok_or("Unknown relation")?;
                    if let Some(formula) = node.formulas.iter_mut().find(|f| f.name == name) {
                        formula.expression = expression;
                    } else {
                        node.formulas.push(Formula {
                            name: name.clone(),
                            expression,
                        });
                    }
                    if !node.columns.contains(&name) {
                        node.columns.push(name);
                    }
                    Ok(())
                });
            }
            "unformula" => {
                let (id, name) = self.field_target(optional)?;
                self.change(|q, _| {
                    let node = q.root.find_mut(id).ok_or("Unknown relation")?;
                    if !node.formulas.iter().any(|f| f.name == name) {
                        return Err("Select a formula to remove".into());
                    }
                    node.formulas.retain(|f| f.name != name);
                    node.columns.retain(|c| c != &name);
                    node.filters.retain(|f| f.column != name);
                    node.sort.retain(|s| s.column != name);
                    Ok(())
                });
            }
            "hide" | "show" => {
                let (id, column) = self.field_target(optional)?;
                self.change(|q, _| {
                    let node = q.root.find_mut(id).ok_or("Unknown relation")?;
                    if action == "hide" {
                        node.columns.retain(|c| c != &column);
                    } else if !node.columns.contains(&column) {
                        node.columns.push(column);
                    }
                    Ok(())
                });
            }
            "sort" | "sort_desc" | "unsort" => {
                let (id, column) = self.field_target(optional)?;
                self.change(|q, _| {
                    let node = q.root.find_mut(id).ok_or("Unknown relation")?;
                    node.sort.retain(|s| s.column != column);
                    if action != "unsort" {
                        node.sort.insert(
                            0,
                            Sort {
                                column,
                                descending: action == "sort_desc",
                            },
                        );
                    }
                    Ok(())
                });
            }
            "filter_prompt" => {
                let (id, column) = self.field_target(None)?;
                let path = self
                    .query
                    .as_ref()
                    .and_then(|q| q.root.path(id))
                    .unwrap_or_default()
                    .join(".");
                self.input.enter_command_mode();
                for ch in format!("filter {path}.{column} = ").chars() {
                    self.input.push_command_char(ch);
                }
            }
            "filter" => {
                let mut args = arg.splitn(3, char::is_whitespace);
                let field = args
                    .next()
                    .filter(|s| !s.is_empty())
                    .ok_or("Usage: filter orders.status = Shipped")?;
                let op = Operator::parse(args.next().ok_or("Missing filter operator")?)
                    .map_err(|e| e.to_string())?;
                let value = args.next().unwrap_or("").trim().to_string();
                let (id, column) = self.field_target(Some(field))?;
                self.change(|q, _| {
                    let node = q.root.find_mut(id).ok_or("Unknown relation")?;
                    node.filters.retain(|f| f.column != column);
                    node.filters.push(Filter { column, op, value });
                    Ok(())
                });
            }
            "unfilter" => {
                let (id, column) = self.field_target(optional)?;
                self.change(|q, _| {
                    q.root
                        .find_mut(id)
                        .ok_or("Unknown relation")?
                        .filters
                        .retain(|f| f.column != column);
                    Ok(())
                });
            }
            "match" | "keep" => {
                let id = self.relation_target(optional)?;
                if id == 1 {
                    return Err("Select a child relation".into());
                }
                // Match propagates to the root; keep clears that relation's parent restriction.
                fn mark(node: &mut query_engine::Node, id: u64) -> bool {
                    if node.id == id {
                        node.require_match = true;
                        return true;
                    }
                    for c in &mut node.children {
                        if mark(c, id) {
                            c.require_match = true;
                            return true;
                        }
                    }
                    false
                }
                self.change(|q, _| {
                    if action == "match" {
                        mark(&mut q.root, id);
                        q.root.require_match = false;
                    } else {
                        q.root.find_mut(id).ok_or("Unknown relation")?.require_match = false;
                    }
                    Ok(())
                });
            }
            "undo" => {
                if let Some(previous) = self.undo.pop() {
                    if let Some(current) = self.query.replace(previous) {
                        self.redo.push(current);
                    }
                    self.refresh();
                }
            }
            "redo" => {
                if let Some(next) = self.redo.pop() {
                    if let Some(current) = self.query.replace(next) {
                        self.undo.push(current);
                    }
                    self.refresh();
                }
            }
            "nextpage" | "prevpage" | "more" => {
                let id = self.relation_target(optional)?;
                self.page(action, id)?;
            }
            "limit" => {
                let n = arg
                    .parse::<usize>()
                    .map_err(|_| "Usage: limit 200 (1–500 rows per relation)")?;
                self.change(|q, _| {
                    q.limit = n;
                    Ok(())
                });
            }
            "refresh" => self.refresh(),
            "save" => {
                let path = std::path::Path::new(optional.unwrap_or("views/default.json"));
                let query = self.query.as_ref().ok_or("Open a table first")?;
                if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let temporary = path.with_extension("json.tmp");
                std::fs::write(
                    &temporary,
                    serde_json::to_vec_pretty(query).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                std::fs::rename(temporary, path).map_err(|e| e.to_string())?;
                self.message = format!("Saved {}", path.display());
                self.error = false;
            }
            "load" => {
                let path = optional.unwrap_or("views/default.json");
                if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 1_000_000 {
                    return Err("View file is too large".into());
                }
                let query: Query =
                    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?;
                query.validate(&self.catalog).map_err(|e| e.to_string())?;
                if let Some(old) = self.query.take() {
                    self.views[self.active_view] = old;
                }
                self.views.push(query.clone());
                self.active_view = self.views.len() - 1;
                self.query = Some(query);
                self.undo.clear();
                self.redo.clear();
                self.document = Document::default();
                self.refresh();
            }
            "" => {}
            _ => return Err(format!("Unknown command: {action}. Press ? for help.")),
        }
        Ok(())
    }
}
