use query_engine::{query::compile, Catalog, Column, Formula, Query, TableInfo, TableName};

fn fixture() -> (Query, Catalog) {
    let table = TableInfo {
        name: TableName {
            schema: "public".into(),
            name: "items".into(),
        },
        columns: [
            ("id", "integer"),
            ("quantity", "integer"),
            ("price", "numeric(25,3)"),
            ("name", "text"),
        ]
        .into_iter()
        .map(|(name, data_type)| Column {
            name: name.into(),
            data_type: data_type.into(),
            nullable: true,
            generated: false,
        })
        .collect(),
        primary_keys: vec!["id".into()],
    };
    (
        Query::new(&table),
        Catalog {
            tables: vec![table],
            foreign_keys: vec![],
        },
    )
}
fn formula(name: &str, expression: &str) -> Formula {
    Formula {
        name: name.into(),
        expression: expression.into(),
    }
}
#[test]
fn validates_hidden_dependencies_and_rejects_cycles_types_and_sql() {
    let (mut q, catalog) = fixture();
    q.root.formulas = vec![
        formula("amount", "[quantity] * [price]"),
        formula("taxed", "[amount] * 1.25"),
    ];
    q.root.columns = vec!["taxed".into()];
    q.validate(&catalog).unwrap();
    compile(&q, &catalog).unwrap();
    for expression in [
        "[missing]",
        "[name] * 2",
        "[taxed]",
        "sum([missing.price])",
        "1; DELETE FROM items",
        "[quantity] +",
    ] {
        let mut bad = q.clone();
        bad.root.formulas[0].expression = expression.into();
        assert!(bad.validate(&catalog).is_err(), "{expression}");
    }
    let mut bad = q.clone();
    bad.root.formulas[0].name = "price".into();
    assert!(bad.validate(&catalog).is_err());
    bad.root.formulas[0].name = "bad;name".into();
    assert!(bad.validate(&catalog).is_err());
    bad.root.formulas[0].name = "taxed".into();
    assert!(bad.validate(&catalog).is_err());
    q.root.formulas.remove(0);
    assert!(
        q.validate(&catalog).is_err(),
        "Removing a hidden dependency must fail"
    );
}
#[test]
fn bounded_dependency_expansion_and_backward_compatible_views() {
    let (mut q, catalog) = fixture();
    let legacy = serde_json::to_string(&q).unwrap();
    assert!(!legacy.contains("formulas"));
    let restored: Query = serde_json::from_str(&legacy).unwrap();
    restored.validate(&catalog).unwrap();
    q.root.formulas.push(formula("f0", "1"));
    for i in 1..20 {
        q.root.formulas.push(formula(
            &format!("f{i}"),
            &format!("[f{}] + [f{}]", i - 1, i - 1),
        ));
    }
    let error = q.validate(&catalog).unwrap_err().to_string();
    assert!(error.contains("expansion is too large"), "{error}");
}
