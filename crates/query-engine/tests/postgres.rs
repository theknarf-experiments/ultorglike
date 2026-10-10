//! Run explicitly with TEST_DATABASE_URL against the Compose fixture.
use query_engine::{
    catalog::discover,
    executor::{connect, execute},
    Filter, Operator, Query, Sort,
};

#[test]
#[ignore = "requires seeded PostgreSQL; scripts/check.sh runs this test"]
fn seeded_workflow() {
    let url = std::env::var("TEST_DATABASE_URL").expect("Set TEST_DATABASE_URL");
    let mut client = connect(&url).unwrap();
    let catalog = discover(&mut client).unwrap();
    assert_eq!(catalog.tables.len(), 5);
    let customer = catalog
        .tables
        .iter()
        .find(|t| t.name.name == "customers")
        .unwrap();
    assert_eq!(customer.primary_keys, ["id"]);
    assert_eq!(catalog.foreign_keys.len(), 4);
    let mut query = Query::new(customer);
    query.limit = 200;
    query.root.columns = vec!["company_name".into(), "notes".into(), "created_at".into()];
    let orders_rel = catalog
        .relationships(&customer.name)
        .into_iter()
        .find(|r| r.target.name == "orders")
        .unwrap();
    let orders = query
        .add_relationship(1, orders_rel.clone(), &catalog)
        .unwrap();
    let orders_table = query.root.find(orders).unwrap().table.clone();
    let items_rel = catalog
        .relationships(&orders_table)
        .into_iter()
        .find(|r| r.target.name == "line_items")
        .unwrap();
    let items = query.add_relationship(orders, items_rel, &catalog).unwrap();
    query.root.find_mut(items).unwrap().columns = vec!["quantity".into(), "unit_price".into()];
    let contacts = catalog
        .relationships(&customer.name)
        .into_iter()
        .find(|r| r.target.name == "contacts")
        .unwrap();
    let contacts = query.add_relationship(1, contacts, &catalog).unwrap();
    let data = execute(&mut client, &query, &catalog).unwrap();
    assert_eq!(data.rows.len(), 125);
    let familia = data
        .rows
        .iter()
        .find(|r| r.values["1:company_name"].as_deref() == Some("Familia Arquibaldo"))
        .unwrap();
    assert_eq!(familia.children[&orders.to_string()].len(), 3);
    assert_eq!(familia.children[&contacts.to_string()].len(), 2);
    assert_eq!(familia.values["1:notes"], None);
    assert!(familia.values["1:created_at"]
        .as_ref()
        .unwrap()
        .starts_with("2024-01-01"));
    let first = &familia.children[&orders.to_string()][0];
    assert_eq!(first.children[&items.to_string()].len(), 3);
    assert_eq!(
        first.children[&items.to_string()][0].values[&format!("{items}:unit_price")].as_deref(),
        Some("9.65")
    );
    let empty = data
        .rows
        .iter()
        .find(|r| r.values["1:company_name"].as_deref() == Some("No Orders Yet"))
        .unwrap();
    assert!(empty.children[&orders.to_string()].is_empty());
    assert!(data
        .rows
        .iter()
        .any(|r| r.values["1:company_name"].as_deref() == Some("東京商店")));
    let grid = data_model::document::Grid::build(&data);
    assert_eq!(grid.header_height, 3);
    // 3 + 2 + 1 line items for Familia; two contacts must not multiply these.
    assert_eq!(
        grid.cells
            .iter()
            .find(|c| c.text == "Familia Arquibaldo")
            .unwrap()
            .height,
        6
    );

    // Child-only filtering preserves all parents, including those with no children.
    query.root.find_mut(orders).unwrap().filters.push(Filter {
        column: "status".into(),
        op: Operator::Eq,
        value: "Shipped".into(),
    });
    assert_eq!(
        execute(&mut client, &query, &catalog).unwrap().rows.len(),
        125
    );
    query.root.find_mut(orders).unwrap().require_match = true;
    let matched = execute(&mut client, &query, &catalog).unwrap();
    assert_eq!(matched.rows.len(), 3);
    query.root.find_mut(orders).unwrap().sort.push(Sort {
        column: "order_date".into(),
        descending: true,
    });
    assert_eq!(
        execute(&mut client, &query, &catalog).unwrap().rows.len(),
        3
    );

    // Parameterization: quotes and SQL-looking text are ordinary values.
    query.root.filters.push(Filter {
        column: "company_name".into(),
        op: Operator::Eq,
        value: "x' OR true --".into(),
    });
    assert!(execute(&mut client, &query, &catalog)
        .unwrap()
        .rows
        .is_empty());
    query.root.filters.clear();
    query.root.find_mut(orders).unwrap().require_match = false;
    query.root.find_mut(orders).unwrap().filters.clear();
    query.root.find_mut(orders).unwrap().filters.push(Filter {
        column: "freight".into(),
        op: Operator::Gt,
        value: "100".into(),
    });
    query.root.find_mut(orders).unwrap().require_match = true;
    assert_eq!(
        execute(&mut client, &query, &catalog).unwrap().rows.len(),
        2
    );
    query.root.find_mut(orders).unwrap().filters[0].value = "invalid numeric".into();
    assert!(execute(&mut client, &query, &catalog).is_err());
    query.root.find_mut(orders).unwrap().filters.clear();
    query.root.find_mut(orders).unwrap().require_match = false;
    query.limit = 2;
    let limited = execute(&mut client, &query, &catalog).unwrap();
    assert_eq!(limited.rows.len(), 2);
    assert!(limited.truncated);
    let saved = serde_json::to_string(&query).unwrap();
    let reopened: Query = serde_json::from_str(&saved).unwrap();
    reopened.validate(&catalog).unwrap();
    assert_eq!(
        execute(&mut client, &reopened, &catalog).unwrap().rows,
        limited.rows
    );
    // The browser connection cannot modify source data.
    assert!(client
        .execute("DELETE FROM contacts WHERE false", &[])
        .is_err());
}

#[test]
#[ignore = "requires seeded PostgreSQL; scripts/check.sh runs this test"]
fn composite_keys_quoted_identifiers_and_cancellation() {
    use postgres::{Client, NoTls};
    use query_engine::executor::{Response, Worker};
    use std::time::{Duration, Instant};
    let url = std::env::var("TEST_DATABASE_URL").expect("Set TEST_DATABASE_URL");
    struct Fixture {
        client: Client,
        schema: String,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = self.client.batch_execute(&format!(
                "DROP SCHEMA {} CASCADE",
                query_engine::schema::quote(&self.schema)
            ));
        }
    }
    let schema = format!("ultorg_test_{}", std::process::id());
    let mut fixture = Fixture {
        client: Client::connect(&url, NoTls).unwrap(),
        schema: schema.clone(),
    };
    let schema_sql = query_engine::schema::quote(&schema);
    fixture.client.batch_execute(&format!(r#"
        CREATE SCHEMA {schema_sql};
        CREATE TABLE {schema_sql}.parents (tenant integer,id integer,name text,PRIMARY KEY(tenant,id));
        CREATE TABLE {schema_sql}.children (id integer PRIMARY KEY,tenant integer,parent_id integer,"unit price" numeric(25,3), FOREIGN KEY(tenant,parent_id) REFERENCES {schema_sql}.parents(tenant,id));
        INSERT INTO {schema_sql}.parents VALUES (1,1,'A'),(2,1,'B');
        INSERT INTO {schema_sql}.children VALUES (1,1,1,12345678901234567890.123),(2,2,1,2.500);
        CREATE VIEW {schema_sql}.slow AS SELECT 1 AS id, pg_sleep(5)::text AS wait;
    "#)).unwrap();
    let wide_columns = (0..60)
        .map(|i| format!("field_{i} integer DEFAULT {i}"))
        .collect::<Vec<_>>()
        .join(",");
    fixture.client.batch_execute(&format!("CREATE TABLE {schema_sql}.wide (id integer PRIMARY KEY DEFAULT 1, {wide_columns}); INSERT INTO {schema_sql}.wide DEFAULT VALUES")).unwrap();
    let mut client = connect(&url).unwrap();
    let catalog = discover(&mut client).unwrap();
    let wide = catalog
        .tables
        .iter()
        .find(|t| t.name.schema == schema && t.name.name == "wide")
        .unwrap();
    let mut wide_query = Query::new(wide);
    wide_query.root.columns = wide.columns.iter().map(|c| c.name.clone()).collect();
    let wide_data = execute(&mut client, &wide_query, &catalog).unwrap();
    assert_eq!(
        wide_data.rows[0].values["1:field_59"].as_deref(),
        Some("59")
    );
    let parent = catalog
        .tables
        .iter()
        .find(|t| t.name.schema == schema && t.name.name == "parents")
        .unwrap();
    assert_eq!(parent.primary_keys, vec!["tenant", "id"]);
    let mut q = Query::new(parent);
    let rel = catalog.relationships(&parent.name).pop().unwrap();
    assert_eq!(rel.parent_columns.len(), 2);
    let id = q.add_relationship(1, rel, &catalog).unwrap();
    q.root.find_mut(id).unwrap().columns = vec!["unit price".into()];
    let result = execute(&mut client, &q, &catalog).unwrap();
    assert_eq!(result.rows.len(), 2);
    assert_ne!(result.rows[0].key, result.rows[1].key);
    assert_eq!(result.rows[0].children[&id.to_string()].len(), 1);
    assert_eq!(
        result.rows[0].children[&id.to_string()][0].values[&format!("{id}:unit price")].as_deref(),
        Some("12345678901234567890.123")
    );
    // New requests remain responsive and actually cancel the previous server query.
    let slow = catalog
        .tables
        .iter()
        .find(|t| t.name.schema == schema && t.name.name == "slow")
        .unwrap();
    let mut worker = Worker::new(url);
    worker.submit(Some((Query::new(slow), catalog.clone())));
    let pattern = format!("%{schema_sql}.\"slow\"%");
    let start = Instant::now();
    let pid = loop {
        let rows=fixture.client.query("SELECT pid FROM pg_stat_activity WHERE state='active' AND pid<>pg_backend_pid() AND query LIKE $1",&[&pattern]).unwrap();
        if let Some(row) = rows.first() {
            break row.get::<_, i32>(0);
        }
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "slow query never started"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    let start = Instant::now();
    worker.submit(Some((q, catalog)));
    loop {
        if let Some(response) = worker.poll() {
            match response.unwrap() {
                Response::Data(doc) => {
                    assert_eq!(doc.rows.len(), 2);
                    break;
                }
                _ => panic!("unexpected catalog"),
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "new query was blocked"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    loop {
        let running: bool = fixture
            .client
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1 AND state='active')",
                &[&pid],
            )
            .unwrap()
            .get(0);
        if !running {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "old SQL was not cancelled"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
