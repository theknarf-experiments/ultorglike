use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct ViewState {
    address: Option<Address>,
    column: usize,
    y: usize,
    scroll_x: usize,
    scroll_y: usize,
    pub undo: Vec<Query>,
    pub redo: Vec<Query>,
}
#[derive(Serialize, Deserialize)]
struct SavedView {
    query: Query,
    pages: Vec<Page>,
    state: ViewState,
}
#[derive(Serialize, Deserialize)]
struct Session {
    version: u32,
    active: usize,
    views: Vec<SavedView>,
}
impl App {
    fn view_state(&self) -> ViewState {
        ViewState {
            address: self.address(),
            column: self.column,
            y: self.y,
            scroll_x: self.scroll_x,
            scroll_y: self.scroll_y,
            undo: self.undo.clone(),
            redo: self.redo.clone(),
        }
    }
    pub(super) fn stash_view(&mut self) {
        if let Some(q) = &self.query {
            if let Some(slot) = self.views.get_mut(self.active_view) {
                *slot = q.clone();
            }
            self.view_states.insert(
                self.active_view,
                self.pending_position
                    .clone()
                    .unwrap_or_else(|| self.view_state()),
            );
        }
        self.pending_position = None;
    }
    pub(super) fn restore_position(&mut self) {
        if let Some(state) = self.pending_position.take() {
            self.column = state.column;
            self.y = state.y;
            self.scroll_x = state.scroll_x;
            self.scroll_y = state.scroll_y;
            self.rebuild(state.address);
        }
    }
    pub(super) fn session_command(&mut self, action: &str, arg: &str) -> Result<bool, String> {
        match action {
            "session_save" => {
                if self.busy {
                    return Err("Wait for the current query before saving the workspace".into());
                }
                if arg.is_empty() {
                    return Err("Usage: session_save workspaces/project.json".into());
                }
                self.stash_view();
                let session = Session {
                    version: 1,
                    active: self.active_view,
                    views: self
                        .views
                        .iter()
                        .enumerate()
                        .map(|(i, q)| SavedView {
                            query: q.clone(),
                            pages: q.pages.clone(),
                            state: self.view_states.get(&i).cloned().unwrap_or_default(),
                        })
                        .collect(),
                };
                if session.views.is_empty() {
                    return Err("Open a view first".into());
                }
                let bytes = serde_json::to_vec_pretty(&session).map_err(|e| e.to_string())?;
                if bytes.len() > 10_000_000 {
                    return Err("Workspace exceeds 10 MB; close unused views before saving".into());
                }
                workbench::atomic_write(std::path::Path::new(arg), &bytes)?;
                self.message = format!("Saved workspace {arg}");
                self.error = false;
            }
            "session_load" => {
                if arg.is_empty() {
                    return Err("Usage: session_load workspaces/project.json".into());
                }
                if std::fs::metadata(arg).map_err(|e| e.to_string())?.len() > 10_000_000 {
                    return Err("Workspace exceeds 10 MB".into());
                }
                let mut session: Session =
                    serde_json::from_slice(&std::fs::read(arg).map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?;
                if session.version != 1
                    || session.views.is_empty()
                    || session.views.len() > 100
                    || session.active >= session.views.len()
                {
                    return Err("Invalid workspace version or views".into());
                }
                for view in &mut session.views {
                    view.query.pages = std::mem::take(&mut view.pages);
                    view.query
                        .validate(&self.catalog)
                        .map_err(|e| e.to_string())?;
                    if view.state.undo.len() > 100 || view.state.redo.len() > 100 {
                        return Err("Workspace history exceeds 100 changes".into());
                    }
                    for q in view.state.undo.iter().chain(&view.state.redo) {
                        q.validate(&self.catalog).map_err(|e| e.to_string())?;
                    }
                    // Coordinates are untrusted; bounds are refined after rows load.
                    if [
                        view.state.column,
                        view.state.y,
                        view.state.scroll_x,
                        view.state.scroll_y,
                    ]
                    .iter()
                    .any(|n| *n > 10_000_000)
                    {
                        return Err("Invalid workspace position".into());
                    }
                }
                self.views = session.views.iter().map(|v| v.query.clone()).collect();
                self.view_states = session
                    .views
                    .into_iter()
                    .enumerate()
                    .map(|(i, v)| (i, v.state))
                    .collect();
                self.active_view = session.active;
                self.query = None;
                self.close_popup();
                self.switch_view(self.active_view);
                self.error = false;
            }
            "duplicate" => {
                if self.views.len() >= 100 {
                    return Err("Close a view before opening more (maximum 100)".into());
                }
                let q = self.query.clone().ok_or("Open a view first")?;
                self.stash_view();
                self.views.push(q);
                self.switch_view(self.views.len() - 1);
            }
            "close" => {
                if self.views.is_empty() {
                    return Err("No open view".into());
                }
                self.stash_view();
                let removed = self.active_view;
                self.views.remove(removed);
                self.view_states = std::mem::take(&mut self.view_states)
                    .into_iter()
                    .filter(|(i, _)| *i != removed)
                    .map(|(i, s)| (if i > removed { i - 1 } else { i }, s))
                    .collect();
                self.query = None;
                self.document = Document::default();
                self.grid = Grid::default();
                if let Some(worker) = &mut self.worker {
                    worker.cancel();
                }
                self.busy = false;
                if self.views.is_empty() {
                    self.active_view = 0;
                    self.undo.clear();
                    self.redo.clear();
                    self.show_picker(None);
                } else {
                    self.switch_view(removed.min(self.views.len() - 1));
                }
            }
            "views" => {
                let text = self
                    .views
                    .iter()
                    .enumerate()
                    .map(|(i, q)| {
                        format!(
                            "{} {} {}",
                            if i == self.active_view { ">" } else { " " },
                            i + 1,
                            q.root.table.qualified()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                self.open_panel("Views · :view N to switch", text);
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
