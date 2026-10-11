use postgres::{Client, NoTls};
use query_engine::{
    catalog::discover,
    editing::{apply, Change, Operation},
    executor::{connect, execute},
    schema::quote,
    Query,
};
use std::collections::BTreeMap;

struct Fixture {
    client: Client,
    schema: String,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self
            .client
            .batch_execute(&format!("DROP SCHEMA {} CASCADE", quote(&self.schema)));
    }
}
#[test]
#[ignore = "requires PostgreSQL; scripts/check.sh --postgres runs this test"]
fn writes_are_atomic_parameterized_and_detect_stale_rows() {
    let url = std::env::var("TEST_DATABASE_URL").unwrap();
    let schema = format!("ultorg_edits_{}", std::process::id());
    let mut fixture = Fixture {
        client: Client::connect(&url, NoTls).unwrap(),
        schema,
    };
    let schema = quote(&fixture.schema);
    fixture.client.batch_execute(&format!("CREATE SCHEMA {schema}; CREATE TABLE {schema}.items (tenant integer,id integer,name text NOT NULL,amount numeric(25,3),double_amount numeric GENERATED ALWAYS AS (amount * 2) STORED,PRIMARY KEY(tenant,id)); INSERT INTO {schema}.items(tenant,id,name,amount) VALUES(1,1,'first',1),(1,2,'second',2);")).unwrap();
    let mut reader = connect(&url).unwrap();
    let catalog = discover(&mut reader).unwrap();
    let table = catalog
        .tables
        .iter()
        .find(|t| t.name.schema == fixture.schema)
        .unwrap();
    assert!(
        table
            .columns
            .iter()
            .find(|c| c.name == "double_amount")
            .unwrap()
            .generated
    );
    let mut query = Query::new(table);
    query.root.columns = vec!["name".into(), "amount".into()];
    let initial = execute(&mut reader, &query, &catalog).unwrap();
    let change = |index: usize| Change {
        table: table.name.clone(),
        operation: Operation::Update,
        keys: serde_json::from_str(&initial.rows[index].key).unwrap(),
        version: initial.rows[index].version.clone(),
        values: BTreeMap::new(),
        original: BTreeMap::new(),
    };
    let mut first = change(0);
    let text = "東京 '; DELETE FROM items; --\nhello";
    first.values.insert("name".into(), Some(text.into()));
    first
        .values
        .insert("amount".into(), Some("12345678901234567890.123".into()));
    assert_eq!(apply(&url, &catalog, &[first.clone()]).unwrap(), 1);
    let fresh = execute(&mut reader, &query, &catalog).unwrap();
    assert_eq!(fresh.rows[0].values["1:name"].as_deref(), Some(text));
    assert_eq!(
        fresh.rows[0].values["1:amount"].as_deref(),
        Some("12345678901234567890.123")
    );
    // A valid first write must roll back when a later row in the batch is stale.
    let mut second = change(1);
    second
        .values
        .insert("name".into(), Some("must roll back".into()));
    assert!(apply(&url, &catalog, &[second.clone(), first])
        .unwrap_err()
        .to_string()
        .contains("Conflict"));
    assert_eq!(
        execute(&mut reader, &query, &catalog).unwrap().rows[1].values["1:name"].as_deref(),
        Some("second")
    );
    // Nulls are distinct from text, and field types are checked by PostgreSQL.
    second.values.clear();
    second.values.insert("amount".into(), None);
    apply(&url, &catalog, &[second.clone()]).unwrap();
    assert_eq!(
        execute(&mut reader, &query, &catalog).unwrap().rows[1].values["1:amount"],
        None
    );
    let inserted = Change {
        table: table.name.clone(),
        operation: Operation::Insert,
        keys: vec![],
        version: None,
        values: [
            ("tenant".into(), Some("2".into())),
            ("id".into(), Some("1".into())),
            ("name".into(), Some("new".into())),
        ]
        .into(),
        original: BTreeMap::new(),
    };
    let mut invalid = inserted.clone();
    invalid
        .values
        .insert("id".into(), Some("not an integer".into()));
    assert!(apply(&url, &catalog, &[inserted.clone(), invalid]).is_err());
    assert_eq!(
        execute(&mut reader, &query, &catalog).unwrap().rows.len(),
        2
    );
    apply(&url, &catalog, &[inserted]).unwrap();
    let fresh = execute(&mut reader, &query, &catalog).unwrap();
    let last = fresh.rows.last().unwrap();
    let deleted = Change {
        table: table.name.clone(),
        operation: Operation::Delete,
        keys: serde_json::from_str(&last.key).unwrap(),
        version: last.version.clone(),
        values: BTreeMap::new(),
        original: BTreeMap::new(),
    };
    apply(&url, &catalog, std::slice::from_ref(&deleted)).unwrap();
    assert!(apply(&url, &catalog, &[deleted]).is_err());
    assert_eq!(
        execute(&mut reader, &query, &catalog).unwrap().rows.len(),
        2
    );
    // Generated fields and changing PK values are rejected before writing.
    let mut protected = change(0);
    protected
        .values
        .insert("double_amount".into(), Some("3".into()));
    assert!(apply(&url, &catalog, &[protected.clone()]).is_err());
    protected.values.clear();
    protected.values.insert("id".into(), Some("3".into()));
    assert!(apply(&url, &catalog, &[protected]).is_err());
    assert!(
        reader
            .execute(
                &format!("DELETE FROM {} WHERE false", table.name.sql()),
                &[]
            )
            .is_err(),
        "Reads stay read-only after applying changes"
    );
}
