use query_engine::{
    editing::{apply, Change, Operation},
    embedded::Connection,
    Backend, Catalog, Filter, Formula, Operator, Page, Query, Sort,
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    path: PathBuf,
    url: String,
}
impl Fixture {
    fn new(backend: Backend) -> Self {
        let path = std::env::temp_dir().join(format!(
            "ultorg-{}-{}-{}.db",
            backend.label(),
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let sql = include_str!("../../../dev/embedded-seed.sql");
        match backend {
            Backend::SQLite => rusqlite::Connection::open(&path)
                .unwrap()
                .execute_batch(sql)
                .unwrap(),
            Backend::DuckDB => duckdb::Connection::open(&path)
                .unwrap()
                .execute_batch(sql)
                .unwrap(),
            _ => unreachable!(),
        }
        let url = format!(
            "{}:{}",
            if backend == Backend::SQLite {
                "sqlite"
            } else {
                "duckdb"
            },
            path.display()
        );
        Self { path, url }
    }
    fn open(&self) -> Connection {
        Connection::open(&self.url, false).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
fn named(catalog: &Catalog, name: &str) -> Query {
    Query::new(catalog.tables.iter().find(|t| t.name.name == name).unwrap())
}
fn attach(query: &mut Query, catalog: &Catalog, parent: u64, name: &str) -> u64 {
    let relationship = catalog
        .relationships(&query.root.find(parent).unwrap().table)
        .into_iter()
        .find(|r| r.target.name == name)
        .unwrap();
    query
        .add_relationship(parent, relationship, catalog)
        .unwrap()
}
fn run_workflow(backend: Backend) {
    let fixture = Fixture::new(backend);
    let connection = fixture.open();
    let catalog = connection.discover().unwrap();
    assert_eq!(catalog.backend, backend);
    assert_eq!(catalog.foreign_keys.len(), 3);
    let mut query = named(&catalog, "customers");
    query.root.columns = vec!["name".into(), "notes".into()];
    query.limit = 1;
    let orders = attach(&mut query, &catalog, 1, "orders");
    let items = attach(&mut query, &catalog, orders, "items");
    let contacts = attach(&mut query, &catalog, 1, "contacts");
    let item = query.root.find_mut(items).unwrap();
    item.formulas.push(Formula {
        name: "amount".into(),
        expression: "[quantity] * [price]".into(),
    });
    item.columns = vec!["amount".into()];
    let order = query.root.find_mut(orders).unwrap();
    order.formulas.push(Formula {
        name: "total".into(),
        expression: "sum([items.amount])".into(),
    });
    order.columns = vec!["total".into()];
    query.root.formulas.push(Formula {
        name: "spend".into(),
        expression: "sum([orders.total])".into(),
    });
    query.root.formulas.push(Formula {
        name: "count".into(),
        expression: "count([orders])".into(),
    });
    query.root.formulas.push(Formula {
        name: "ratio".into(),
        expression: "1 / 2".into(),
    });
    query.root.formulas.push(Formula {
        name: "zero".into(),
        expression: "1 / 0".into(),
    });
    query
        .root
        .columns
        .extend(["spend", "count", "ratio", "zero"].map(str::to_string));
    let data = connection.execute(&query, &catalog).unwrap();
    assert_eq!(data.rows.len(), 1);
    let row = &data.rows[0];
    assert_eq!(row.values["1:name"].as_deref(), Some("東京商店"));
    assert_eq!(row.values["1:notes"], None);
    assert_eq!(row.values["1:zero"], None);
    assert_eq!(
        row.values["1:spend"]
            .as_ref()
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        37.625
    );
    assert_eq!(
        row.values["1:ratio"]
            .as_ref()
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        0.5
    );
    assert_eq!(
        row.values["1:count"]
            .as_ref()
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        2.0
    );
    assert_eq!(row.children[&orders.to_string()].len(), 1);
    assert_eq!(row.children[&contacts.to_string()].len(), 1);
    assert!(data.truncated);
    let first_order = row.children[&orders.to_string()][0].key.clone();
    query.pages.push(Page {
        node: orders,
        parents: vec![row.key.clone()],
        offset: 1,
        limit: 1,
    });
    let next = connection.execute(&query, &catalog).unwrap();
    assert_ne!(
        next.rows[0].children[&orders.to_string()][0].key,
        first_order
    );
    assert_eq!(next.rows[0].values["1:spend"], row.values["1:spend"]);
    query.pages.clear();
    query.limit = 100;
    query.root.find_mut(orders).unwrap().expanded = false;
    let folded = connection.execute(&query, &catalog).unwrap();
    assert!(!folded.rows[0].children.contains_key(&orders.to_string()));
    assert_eq!(folded.rows[0].values["1:spend"], row.values["1:spend"]);
    assert_eq!(
        folded.rows[2].values["1:spend"]
            .as_ref()
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        0.0
    );
    query.root.find_mut(orders).unwrap().filters.push(Filter {
        column: "status".into(),
        op: Operator::Eq,
        value: "Shipped".into(),
    });
    query.root.find_mut(orders).unwrap().require_match = true;
    assert_eq!(connection.execute(&query, &catalog).unwrap().rows.len(), 1);
    query.root.find_mut(orders).unwrap().require_match = false;
    query.root.find_mut(orders).unwrap().filters.clear();
    query.root.sort.push(Sort {
        column: "spend".into(),
        descending: true,
    });
    query.root.filters.push(Filter {
        column: "spend".into(),
        op: Operator::Gt,
        value: "10".into(),
    });
    assert_eq!(connection.execute(&query, &catalog).unwrap().rows.len(), 1);
    query.root.filters.clear();
    for text in ["x' OR true --", "%", "_", "東京"] {
        query.root.filters = vec![Filter {
            column: "name".into(),
            op: Operator::Contains,
            value: text.into(),
        }];
        assert_eq!(
            connection.execute(&query, &catalog).unwrap().rows.len(),
            if text == "東京" { 1 } else { 0 }
        );
    }
    let reverse = catalog
        .relationships(&query.root.find(items).unwrap().table)
        .into_iter()
        .find(|r| r.target.name == "orders")
        .unwrap();
    assert_eq!(reverse.parent_columns, ["customer_id", "order_id"]);
    assert_eq!(reverse.child_columns, ["customer_id", "id"]);
    let view = named(&catalog, "customer_names");
    assert!(catalog
        .table(&view.root.table)
        .unwrap()
        .primary_keys
        .is_empty());
    assert_eq!(connection.execute(&view, &catalog).unwrap().rows.len(), 3);
    let quoted = named(&catalog, "odd \"table");
    assert_eq!(
        connection.execute(&quoted, &catalog).unwrap().rows[0].values["1:odd column"].as_deref(),
        Some("hello")
    );
    assert!(
        connection.batch("DELETE FROM customers").is_err(),
        "browsing connection must be read-only"
    );
    let roundtrip: Query = serde_json::from_str(&serde_json::to_string(&query).unwrap()).unwrap();
    roundtrip.validate(&catalog).unwrap();
}
fn run_edits(backend: Backend) {
    let fixture = Fixture::new(backend);
    let catalog = fixture.open().discover().unwrap();
    let mut query = named(&catalog, "customers");
    query.root.columns = vec!["id".into(), "name".into(), "notes".into()];
    let data = fixture.open().execute(&query, &catalog).unwrap();
    let record = &data.rows[2];
    let update = Change {
        table: query.root.table.clone(),
        operation: Operation::Update,
        keys: serde_json::from_str(&record.key).unwrap(),
        version: record.version.clone(),
        values: BTreeMap::from([("name".into(), Some("edited ' safely".into()))]),
        original: BTreeMap::new(),
    };
    assert_eq!(
        apply(&fixture.url, &catalog, std::slice::from_ref(&update)).unwrap(),
        1
    );
    let current = fixture.open().execute(&query, &catalog).unwrap();
    assert_eq!(
        current.rows[2].values["1:name"].as_deref(),
        Some("edited ' safely")
    );
    assert!(apply(&fixture.url, &catalog, std::slice::from_ref(&update))
        .unwrap_err()
        .to_string()
        .contains("Conflict"));
    let insert = Change {
        operation: Operation::Insert,
        keys: vec![],
        version: None,
        values: BTreeMap::from([
            ("id".into(), Some("50".into())),
            ("name".into(), Some("new".into())),
        ]),
        ..update.clone()
    };
    assert!(apply(&fixture.url, &catalog, &[insert.clone(), update]).is_err());
    assert_eq!(
        fixture.open().execute(&query, &catalog).unwrap().rows.len(),
        3,
        "conflict must roll back earlier insert"
    );
    apply(&fixture.url, &catalog, &[insert]).unwrap();
    let data = fixture.open().execute(&query, &catalog).unwrap();
    let record = &data.rows[3];
    let delete = Change {
        table: query.root.table.clone(),
        operation: Operation::Delete,
        keys: serde_json::from_str(&record.key).unwrap(),
        version: record.version.clone(),
        values: BTreeMap::new(),
        original: BTreeMap::new(),
    };
    apply(&fixture.url, &catalog, &[delete]).unwrap();
    assert_eq!(
        fixture.open().execute(&query, &catalog).unwrap().rows.len(),
        3
    );
    // A hidden field modification must also make the full-row snapshot stale.
    let record = &data.rows[2];
    let change = Change {
        table: query.root.table.clone(),
        operation: Operation::Update,
        keys: serde_json::from_str(&record.key).unwrap(),
        version: record.version.clone(),
        values: BTreeMap::from([("name".into(), Some("overwrite".into()))]),
        original: BTreeMap::new(),
    };
    Connection::open(&fixture.url, true)
        .unwrap()
        .batch("UPDATE customers SET notes='external' WHERE id=3")
        .unwrap();
    assert!(apply(&fixture.url, &catalog, &[change]).is_err());
}
#[test]
fn sqlite_workflow() {
    run_workflow(Backend::SQLite);
}
#[test]
fn duckdb_workflow() {
    run_workflow(Backend::DuckDB);
}
#[test]
fn sqlite_edits() {
    run_edits(Backend::SQLite);
}
#[test]
fn duckdb_edits() {
    run_edits(Backend::DuckDB);
}
#[test]
fn missing_files_are_not_created() {
    for prefix in ["sqlite:", "duckdb:"] {
        assert!(Connection::open(&format!("{prefix}/missing/ultorg-file.db"), false).is_err());
    }
}

fn run_worker(backend: Backend) {
    use query_engine::executor::{Response, Worker};
    use std::time::{Duration, Instant};
    let fixture = Fixture::new(backend);
    {
        let connection = Connection::open(&fixture.url, true).unwrap();
        connection.batch(if backend==Backend::SQLite {
            "CREATE VIEW slow AS WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<1000000000) SELECT sum(i % 97) AS total FROM n"
        } else {
            "CREATE VIEW slow AS SELECT sum(i % 97) AS total FROM range(10000000000) n(i)"
        }).unwrap();
    }
    let catalog = fixture.open().discover().unwrap();
    let mut worker = Worker::new(fixture.url.clone());
    worker.submit(Some((named(&catalog, "slow"), catalog.clone())));
    std::thread::sleep(Duration::from_millis(200));
    worker.submit(Some((named(&catalog, "customers"), catalog.clone())));
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(response) = worker.poll() {
            match response.unwrap() {
                Response::Data(data) => assert_eq!(data.rows.len(), 3),
                _ => panic!("expected data"),
            };
            break;
        }
        assert!(
            Instant::now() < deadline,
            "cancelled SQL must release the embedded worker"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let table = named(&catalog, "customers").root.table;
    let change = Change {
        table,
        operation: Operation::Insert,
        keys: vec![],
        version: None,
        values: BTreeMap::from([
            ("id".into(), Some("77".into())),
            ("name".into(), Some("worker edit".into())),
        ]),
        original: BTreeMap::new(),
    };
    assert_eq!(
        worker
            .apply_changes(catalog.clone(), vec![change])
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap(),
        1
    );
    worker.submit(None);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(response) = worker.poll() {
            match response.unwrap() {
                Response::Catalog(c) => assert_eq!(c, catalog),
                _ => panic!("expected catalog"),
            };
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn sqlite_worker_cancels_and_writes() {
    run_worker(Backend::SQLite);
}
#[test]
fn duckdb_worker_cancels_and_writes() {
    run_worker(Backend::DuckDB);
}

fn run_types(backend: Backend) {
    let fixture = Fixture::new(backend);
    {
        let connection = Connection::open(&fixture.url, true).unwrap();
        connection.batch("CREATE TABLE typed (id INTEGER PRIMARY KEY, amount DECIMAL(25,3), data BLOB, day DATE, calculated INTEGER GENERATED ALWAYS AS (id * 2));").unwrap();
        connection.batch(if backend==Backend::SQLite {
            "INSERT INTO typed(id,amount,data,day) VALUES(1,12345678901234567890.123,unhex('00FF31'),'2026-10-12')"
        } else {
            "INSERT INTO typed(id,amount,data,day) VALUES(1,12345678901234567890.123,from_hex('00FF31'),'2026-10-12')"
        }).unwrap();
    }
    let catalog = fixture.open().discover().unwrap();
    let mut query = named(&catalog, "typed");
    query.root.columns = vec![
        "amount".into(),
        "data".into(),
        "day".into(),
        "calculated".into(),
    ];
    query.root.filters.push(Filter {
        column: "day".into(),
        op: Operator::Eq,
        value: "2026-10-12".into(),
    });
    let data = fixture.open().execute(&query, &catalog).unwrap();
    let row = &data.rows[0];
    assert_eq!(row.values["1:data"].as_deref(), Some("00FF31"));
    assert_eq!(row.values["1:day"].as_deref(), Some("2026-10-12"));
    assert_eq!(row.values["1:calculated"].as_deref(), Some("2"));
    if backend == Backend::DuckDB {
        assert_eq!(
            row.values["1:amount"].as_deref(),
            Some("12345678901234567890.123")
        );
    }
    let mut change = Change {
        table: query.root.table.clone(),
        operation: Operation::Update,
        keys: serde_json::from_str(&row.key).unwrap(),
        version: row.version.clone(),
        values: BTreeMap::from([("calculated".into(), Some("5".into()))]),
        original: BTreeMap::new(),
    };
    assert!(apply(&fixture.url, &catalog, &[change.clone()]).is_err());
    change.values = BTreeMap::from([("data".into(), Some("GG".into()))]);
    assert!(apply(&fixture.url, &catalog, &[change.clone()]).is_err());
    change.values = BTreeMap::from([("data".into(), Some("CAFE00".into()))]);
    apply(&fixture.url, &catalog, &[change]).unwrap();
    assert_eq!(
        fixture.open().execute(&query, &catalog).unwrap().rows[0].values["1:data"].as_deref(),
        Some("CAFE00")
    );
    if backend == Backend::SQLite {
        Connection::open(&fixture.url,true).unwrap().batch("CREATE TABLE dynamic (id INTEGER PRIMARY KEY, value); INSERT INTO dynamic VALUES(1,1)").unwrap();
        let catalog = fixture.open().discover().unwrap();
        let query = named(&catalog, "dynamic");
        let data = fixture.open().execute(&query, &catalog).unwrap();
        let row = &data.rows[0];
        Connection::open(&fixture.url, true)
            .unwrap()
            .batch("UPDATE dynamic SET value='1'")
            .unwrap();
        let change = Change {
            table: query.root.table.clone(),
            operation: Operation::Update,
            keys: serde_json::from_str(&row.key).unwrap(),
            version: row.version.clone(),
            values: BTreeMap::from([("value".into(), Some("2".into()))]),
            original: BTreeMap::new(),
        };
        assert!(apply(&fixture.url, &catalog, &[change])
            .unwrap_err()
            .to_string()
            .contains("Conflict"));
    }
}
#[test]
fn sqlite_types_and_generated_columns() {
    run_types(Backend::SQLite);
}
#[test]
fn duckdb_types_and_generated_columns() {
    run_types(Backend::DuckDB);
}

#[test]
fn sqlite_implicit_foreign_keys_and_dynamic_storage_classes() {
    let fixture = Fixture::new(Backend::SQLite);
    Connection::open(&fixture.url,true).unwrap().batch("CREATE TABLE parent (ID INTEGER PRIMARY KEY); CREATE TABLE child (ID INTEGER PRIMARY KEY, ParentId REFERENCES PARENT); INSERT INTO parent VALUES(1); INSERT INTO child VALUES(1,1)").unwrap();
    let connection = fixture.open();
    let catalog = connection.discover().unwrap();
    let mut query = named(&catalog, "parent");
    let child = attach(&mut query, &catalog, 1, "child");
    assert_eq!(
        connection.execute(&query, &catalog).unwrap().rows[0].children[&child.to_string()].len(),
        1
    );
    let explanation = query_engine::query::explain(&query, &catalog).unwrap();
    assert!(explanation.contains("Child collection: child"));
    assert!(explanation.contains("?1"));
}

#[test]
fn sqlite_edit_versions_preserve_real_precision() {
    let fixture = Fixture::new(Backend::SQLite);
    Connection::open(&fixture.url,true).unwrap().batch("CREATE TABLE precise (id INTEGER PRIMARY KEY, value REAL, name TEXT); INSERT INTO precise VALUES(1,1.0,'before')").unwrap();
    let catalog = fixture.open().discover().unwrap();
    let query = named(&catalog, "precise");
    let data = fixture.open().execute(&query, &catalog).unwrap();
    let row = &data.rows[0];
    Connection::open(&fixture.url, true)
        .unwrap()
        .batch("UPDATE precise SET value=1.0000000000000002")
        .unwrap();
    let change = Change {
        table: query.root.table.clone(),
        operation: Operation::Update,
        keys: serde_json::from_str(&row.key).unwrap(),
        version: row.version.clone(),
        values: BTreeMap::from([("name".into(), Some("after".into()))]),
        original: BTreeMap::new(),
    };
    assert!(apply(&fixture.url, &catalog, &[change])
        .unwrap_err()
        .to_string()
        .contains("Conflict"));
}
