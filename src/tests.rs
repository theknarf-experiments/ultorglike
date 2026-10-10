use crate::{app::App, ui};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use data_model::document::{Document, Record};
use query_engine::{Catalog, Column, ForeignKey, TableInfo, TableName};
use ratatui::{backend::TestBackend, Terminal};
use tui_input::Mode;
fn table(name: &str, cols: &[&str]) -> TableInfo {
    TableInfo {
        name: TableName {
            schema: "public".into(),
            name: name.into(),
        },
        columns: cols
            .iter()
            .map(|c| Column {
                name: c.to_string(),
                data_type: "text".into(),
                nullable: true,
            })
            .collect(),
        primary_keys: vec!["id".into()],
    }
}
fn app() -> App {
    let mut app = App::new(None, None);
    let customers = table("customers", &["id", "name"]);
    let orders = table("orders", &["id", "customer_id", "status"]);
    let items = table("line_items", &["id", "order_id", "product"]);
    app.catalog = Catalog {
        foreign_keys: vec![
            ForeignKey {
                name: "orders_customer".into(),
                from: orders.name.clone(),
                to: customers.name.clone(),
                from_columns: vec!["customer_id".into()],
                to_columns: vec!["id".into()],
            },
            ForeignKey {
                name: "items_order".into(),
                from: items.name.clone(),
                to: orders.name.clone(),
                from_columns: vec!["order_id".into()],
                to_columns: vec!["id".into()],
            },
        ],
        tables: vec![customers, orders, items],
    };
    app.handle_command("open customers");
    app
}
fn key(app: &mut App, code: KeyCode) {
    app.handle_event(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
}
fn type_keys(app: &mut App, s: &str) {
    for c in s.chars() {
        key(app, KeyCode::Char(c));
    }
}
fn command(app: &mut App, s: &str) {
    type_keys(app, ":");
    assert!(matches!(app.input.mode(), Mode::Command(_)));
    type_keys(app, s);
    key(app, KeyCode::Enter);
}
#[test]
fn command_help_escape_and_repeat_dispatch() {
    let mut app = app();
    for _ in 0..3 {
        command(&mut app, "help");
        assert_eq!(app.input.mode(), &Mode::Help);
        key(&mut app, KeyCode::Char('q'));
        assert!(!app.should_quit);
        assert_eq!(app.input.mode(), &Mode::Normal);
    }
    type_keys(&mut app, ":partial");
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.input.mode(), &Mode::Normal);
    type_keys(&mut app, "?");
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.input.mode(), &Mode::Normal);
    command(&mut app, "unknown");
    assert!(app.error);
    assert!(app.message.contains("Unknown"));
    command(&mut app, "q");
    assert!(app.should_quit);
}
#[test]
fn nested_query_editing_undo_and_hidden_dependencies() {
    let mut app = app();
    command(&mut app, "join orders customers");
    command(&mut app, "join line_items customers.orders");
    assert_eq!(
        app.query.as_ref().unwrap().root.children[0].children.len(),
        1
    );
    command(&mut app, "filter orders.status = Shipped");
    command(&mut app, "match orders");
    assert!(app.query.as_ref().unwrap().root.children[0].require_match);
    command(&mut app, "keep orders");
    assert!(!app.query.as_ref().unwrap().root.children[0].require_match);
    command(&mut app, "show orders.status");
    command(&mut app, "hide orders.status");
    let child = &app.query.as_ref().unwrap().root.children[0];
    assert_eq!(child.filters.len(), 1);
    assert!(!child.columns.contains(&"status".into()));
    command(&mut app, "undo");
    assert!(app.query.as_ref().unwrap().root.children[0]
        .columns
        .contains(&"status".into()));
    command(&mut app, "redo");
    assert!(!app.query.as_ref().unwrap().root.children[0]
        .columns
        .contains(&"status".into()));
    let prior = app.query.clone();
    command(&mut app, "show invalid");
    assert!(app.error);
    assert_eq!(app.query, prior);
}
#[test]
fn table_switch_picker_and_command_history() {
    let mut app = app();
    command(&mut app, "open orders");
    command(&mut app, "view 1");
    assert_eq!(app.query.as_ref().unwrap().root.table.name, "customers");
    type_keys(&mut app, "gt");
    assert_eq!(app.query.as_ref().unwrap().root.table.name, "orders");
    command(&mut app, "fields");
    assert!(app.picker.is_some());
    type_keys(&mut app, "/status");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.picker.as_ref().unwrap().entries.len(), 1);
    key(&mut app, KeyCode::Esc);
    type_keys(&mut app, ":");
    key(&mut app, KeyCode::Up);
    assert_eq!(app.input.command_buffer(), "fields");
    key(&mut app, KeyCode::Up);
    assert_eq!(app.input.command_buffer(), "view 1");
}
#[test]
fn navigation_counts_search_and_narrow_terminal() {
    let mut app = app();
    let rows = (0..60)
        .map(|i| Record {
            key: i.to_string(),
            values: [("1:name".into(), Some(format!("Customer {i}")))].into(),
            children: Default::default(),
        })
        .collect();
    app.document = Document {
        fields: app.query.as_ref().unwrap().root.fields(),
        rows,
        truncated: false,
        collections: vec![],
    };
    app.grid = data_model::document::Grid::build(&app.document);
    type_keys(&mut app, "10j");
    assert_eq!(app.y, 10);
    type_keys(&mut app, "gg");
    assert_eq!(app.y, 0);
    type_keys(&mut app, "/Customer 35");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.y, 35);
    for (w, h) in [(100, 30), (30, 10), (12, 6), (2, 2)] {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    }
}
#[test]
fn saves_and_reopens_queries_without_credentials() {
    let mut app = app();
    command(&mut app, "join orders customers");
    let path = std::env::temp_dir().join(format!("ultorglike-view-{}.json", std::process::id()));
    command(&mut app, &format!("save {}", path.display()));
    assert!(!app.error, "{}", app.message);
    let saved = app.query.clone();
    command(&mut app, "collapse orders");
    command(&mut app, &format!("load {}", path.display()));
    assert_eq!(app.query, saved);
    assert!(!std::fs::read_to_string(&path).unwrap().contains("password"));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn formula_commands_picker_undo_and_persistence() {
    let mut app = app();
    for table in &mut app.catalog.tables {
        for col in &mut table.columns {
            if col.name == "id" {
                col.data_type = "integer".into();
            }
        }
    }
    for text in [
        "join orders customers",
        "formula orders.amount = [id] * 1.25",
        "formula total = sum([orders.amount])",
        "sort_desc total",
        "filter total > 100",
        "hide orders.amount",
    ] {
        command(&mut app, text);
        assert!(!app.error, "{}: {}", text, app.message);
    }
    let saved = app.query.clone();
    command(&mut app, "formula orders.amount = [id] * 2");
    assert_ne!(app.query, saved);
    type_keys(&mut app, "u");
    assert_eq!(app.query, saved);
    command(&mut app, "redo");
    assert_ne!(app.query, saved);
    command(&mut app, "undo");
    for invalid in [
        "formula orders.amount = [missing]",
        "formula total = [total]",
        "unformula orders.amount",
        "formula name = 1",
    ] {
        command(&mut app, invalid);
        assert!(app.error, "{invalid}");
        assert_eq!(app.query, saved);
    }
    command(&mut app, "fields orders");
    type_keys(&mut app, "/amount");
    key(&mut app, KeyCode::Enter);
    let picker = app.picker.as_ref().unwrap();
    assert_eq!(picker.entries.len(), 1);
    assert!(picker.entries[0]
        .label
        .contains("[ ] ƒ amount = [id] * 1.25"));
    // Toggle hidden formula without removing its definition or downstream dependencies.
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Esc);
    assert!(app.query.as_ref().unwrap().root.children[0]
        .columns
        .contains(&"amount".into()));
    let path =
        std::env::temp_dir().join(format!("ultorglike-formulas-{}.json", std::process::id()));
    command(&mut app, &format!("save {}", path.display()));
    let saved = app.query.clone();
    command(&mut app, "unformula total");
    assert!(!app.error, "{}", app.message);
    assert!(app.query.as_ref().unwrap().root.sort.is_empty());
    assert!(app.query.as_ref().unwrap().root.filters.is_empty());
    command(&mut app, &format!("load {}", path.display()));
    assert_eq!(app.query, saved);
    std::fs::remove_file(path).unwrap();
    assert!(app.grid.headings.iter().any(|h| h.label.contains('ƒ')));
}
