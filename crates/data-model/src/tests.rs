use super::cursor::{Cursor, Direction, Position};
use super::nested::{CellValue, NestedRelation, NestedRow, PrimitiveValue, Value};
use super::view::{ColumnMeta, Filter, FilterOperator, HierarchicalView};
use arrow::datatypes::DataType;

// ============================================================
// Cursor tests
// ============================================================

#[test]
fn test_cursor_new() {
    let cursor = Cursor::new();
    assert_eq!(cursor.row, 0);
    assert_eq!(cursor.col, 0);
    assert!(cursor.parent.is_none());
}

#[test]
fn test_cursor_nested() {
    let cursor = Cursor::nested(5, "orders".to_string());
    assert!(cursor.is_nested());
    assert_eq!(cursor.parent.as_ref().unwrap().parent_row, 5);
    assert_eq!(cursor.parent.as_ref().unwrap().expanded_column, "orders");
}

#[test]
fn test_cursor_collapse() {
    let cursor = Cursor::nested(3, "children".to_string());
    let (row, col) = cursor.collapse();
    assert_eq!(row, 3);
    assert_eq!(col, 0);
}

#[test]
fn test_cursor_expand() {
    let mut cursor = Cursor::new();
    cursor.expand(10, "orders".to_string());
    assert!(cursor.is_nested());
    assert_eq!(cursor.row, 0);
    assert_eq!(cursor.col, 0);
    assert_eq!(cursor.parent.as_ref().unwrap().expanded_column, "orders");
}

#[test]
fn test_cursor_clear_nested() {
    let mut cursor = Cursor::nested(5, "orders".to_string());
    cursor.clear_nested();
    assert!(!cursor.is_nested());
    assert!(cursor.parent.is_none());
}

// ============================================================
// Value tests
// ============================================================

#[test]
fn test_primitive_value_to_string() {
    let pv = PrimitiveValue::new(CellValue::Int32(42), DataType::Int32);
    assert_eq!(pv.value.to_display_string(), "42");
}

#[test]
fn test_primitive_value_float_rounding() {
    let pv = PrimitiveValue::new(CellValue::Float64(1.23456), DataType::Float64);
    let s = pv.value.to_display_string();
    assert_eq!(s, "1.23456");
}

#[test]
fn test_primitive_value_truncated_string() {
    let long_string = "a".repeat(100);
    let pv = PrimitiveValue::new(CellValue::Utf8(long_string), DataType::Utf8);
    let s = pv.value.to_display_string();
    assert!(s.len() < 100);
    assert!(s.ends_with("..."));
}

#[test]
fn test_nested_relation_len() {
    let nr = NestedRelation::new(
        vec!["id".to_string()],
        vec![DataType::Int32],
        vec![
            vec![CellValue::Int32(1)],
            vec![CellValue::Int32(2)],
            vec![CellValue::Int32(3)],
        ],
    );
    assert_eq!(nr.len(), 3);
    assert!(!nr.is_empty());
}

#[test]
fn test_nested_relation_get() {
    let nr = NestedRelation::new(
        vec!["a".to_string(), "b".to_string()],
        vec![DataType::Int32, DataType::Utf8],
        vec![
            vec![CellValue::Int32(1), CellValue::Utf8("x".to_string())],
            vec![CellValue::Int32(2), CellValue::Utf8("y".to_string())],
        ],
    );
    assert_eq!(nr.get(0, 0), Some(&CellValue::Int32(1)));
    assert_eq!(nr.get(0, 1), Some(&CellValue::Utf8("x".to_string())));
    assert_eq!(nr.get(2, 0), None); // Out of bounds
}

#[test]
fn test_nested_relation_to_text() {
    let nr = NestedRelation::new(
        vec!["Name".to_string(), "Age".to_string()],
        vec![DataType::Utf8, DataType::Int32],
        vec![
            vec![CellValue::Utf8("Alice".to_string()), CellValue::Int32(30)],
            vec![CellValue::Utf8("Bob".to_string()), CellValue::Int32(25)],
        ],
    );
    let text = nr.to_text();
    assert!(text.contains("Name"));
    assert!(text.contains("Age"));
    assert!(text.contains("Alice"));
    assert!(text.contains("Bob"));
}

// ============================================================
// HierarchicalView tests
// ============================================================

#[test]
fn test_hierarchical_view_new() {
    let view = HierarchicalView::new(
        "Test Table",
        vec!["id".to_string(), "name".to_string()],
        vec![DataType::Int32, DataType::Utf8],
    );
    assert_eq!(view.title, "Test Table");
    assert_eq!(view.columns.len(), 2);
    assert!(view.rows.is_empty());
    assert!(!view.is_nested);
}

#[test]
fn test_hierarchical_view_nested() {
    let view = HierarchicalView::nested(
        "Parent",
        0,
        "children",
        vec!["child_id".to_string()],
        vec![DataType::Int32],
    );
    assert!(view.is_nested);
    assert!(view.cursor.is_nested());
}

#[test]
fn test_hierarchical_view_add_row() {
    let mut view = HierarchicalView::new("Table", vec!["col".to_string()], vec![DataType::Utf8]);
    view.add_row(NestedRow::new(
        vec!["col".to_string()],
        vec![Value::Primitive(PrimitiveValue::new(
            CellValue::Utf8("test".to_string()),
            DataType::Utf8,
        ))],
    ));
    assert_eq!(view.rows.len(), 1);
}

#[test]
fn test_hierarchical_view_max_row() {
    let view = HierarchicalView::new("Empty", vec!["x".to_string()], vec![DataType::Int32]);
    assert_eq!(view.max_row(), 0); // Empty view

    let mut view = HierarchicalView::new("One Row", vec!["x".to_string()], vec![DataType::Int32]);
    view.add_row(NestedRow::new(
        vec!["x".to_string()],
        vec![Value::Primitive(PrimitiveValue::new(
            CellValue::Null,
            DataType::Int32,
        ))],
    ));
    assert_eq!(view.max_row(), 0);
}

#[test]
fn test_hierarchical_view_current_row() {
    let mut view = HierarchicalView::new("Table", vec!["col".to_string()], vec![DataType::Utf8]);
    view.add_row(NestedRow::new(
        vec!["col".to_string()],
        vec![Value::Primitive(PrimitiveValue::new(
            CellValue::Utf8("row1".to_string()),
            DataType::Utf8,
        ))],
    ));
    view.cursor.row = 0;
    assert!(view.current_row().is_some());
}

#[test]
fn test_hierarchical_view_current_value() {
    let mut view = HierarchicalView::new("Table", vec!["col".to_string()], vec![DataType::Utf8]);
    let val = Value::Primitive(PrimitiveValue::new(
        CellValue::Utf8("hello".to_string()),
        DataType::Utf8,
    ));
    view.add_row(NestedRow::new(vec!["col".to_string()], vec![val.clone()]));
    view.cursor.row = 0;
    view.cursor.col = 0;
    assert!(view.current_value().is_some());
    assert_eq!(view.current_value().unwrap(), &val);
}

#[test]
fn test_hierarchical_view_find_column_index() {
    let view = HierarchicalView::new(
        "Table",
        vec!["id".to_string(), "name".to_string(), "email".to_string()],
        vec![DataType::Int32, DataType::Utf8, DataType::Utf8],
    );
    assert_eq!(view.find_column_index("name"), Some(1));
    assert_eq!(view.find_column_index("nonexistent"), None);
}

#[test]
fn test_hierarchical_view_is_nested_relation() {
    let mut view = HierarchicalView::new(
        "Table",
        vec!["id".to_string(), "orders".to_string()],
        vec![DataType::Int32, DataType::Utf8],
    );

    // Add a row with a nested relation in the second column
    view.add_row(NestedRow::new(
        vec!["id".to_string(), "orders".to_string()],
        vec![
            Value::Primitive(PrimitiveValue::new(CellValue::Int32(1), DataType::Int32)),
            Value::Relation(NestedRelation::new(
                vec!["order_id".to_string()],
                vec![DataType::Int32],
                vec![],
            )),
        ],
    ));

    assert!(!view.is_nested_relation(0, 0));
    assert!(view.is_nested_relation(0, 1));
}

#[test]
fn test_hierarchical_view_navigate_up() {
    let mut view = HierarchicalView::new("Table", vec!["col".to_string()], vec![DataType::Utf8]);
    view.cursor.row = 5;
    let pos = view.navigate(Direction::Up);
    assert_eq!(pos, Position::flat(4, 0));
}

#[test]
fn test_hierarchical_view_navigate_down() {
    let mut view = HierarchicalView::new("Table", vec!["col".to_string()], vec![DataType::Utf8]);
    // Add multiple rows
    for _ in 0..5 {
        view.add_row(NestedRow::new(
            vec!["col".to_string()],
            vec![Value::Primitive(PrimitiveValue::new(
                CellValue::Null,
                DataType::Utf8,
            ))],
        ));
    }
    let pos = view.navigate(Direction::Down);
    assert_eq!(pos, Position::flat(1, 0));
}

#[test]
fn test_hierarchical_view_navigate_right() {
    let mut view = HierarchicalView::new(
        "Table",
        vec!["a".to_string(), "b".to_string()],
        vec![DataType::Utf8, DataType::Utf8],
    );
    let pos = view.navigate(Direction::Right);
    assert_eq!(pos, Position::flat(0, 1));
}

#[test]
fn test_hierarchical_view_navigate_left_from_first() {
    let mut view = HierarchicalView::new(
        "Table",
        vec!["a".to_string(), "b".to_string()],
        vec![DataType::Utf8, DataType::Utf8],
    );
    // At first column, navigate left should keep us at column 0
    let pos = view.navigate(Direction::Left);
    assert_eq!(pos, Position::flat(0, 0));
}

#[test]
fn test_hierarchical_view_navigate_left_from_nested() {
    let mut view = HierarchicalView::new(
        "Table",
        vec!["a".to_string(), "b".to_string(), "children".to_string()],
        vec![DataType::Utf8, DataType::Utf8, DataType::Utf8],
    );

    // Set up a nested context
    view.cursor = Cursor::nested(2, "children".to_string());

    // Navigate left should collapse back to parent
    let pos = view.navigate(Direction::Left);
    assert!(!pos.is_nested());
    assert!(view.cursor.parent.is_none());
}

#[test]
fn test_hierarchical_view_jump_to_first() {
    let mut view = HierarchicalView::new("Table", vec!["col".to_string()], vec![DataType::Utf8]);
    view.cursor.row = 100;
    let pos = view.jump_to_first();
    assert_eq!(pos, Position::flat(0, 0));
    assert_eq!(view.cursor.row, 0);
}

#[test]
fn test_hierarchical_view_jump_to_last() {
    let mut view = HierarchicalView::new("Table", vec!["col".to_string()], vec![DataType::Utf8]);
    view.cursor.row = 0;
    let pos = view.jump_to_last();
    assert_eq!(pos.row(), 0); // Still 0 for empty view
}

#[test]
fn test_hierarchical_view_text_display() {
    let mut view = HierarchicalView::new(
        "Test",
        vec!["id".to_string(), "name".to_string()],
        vec![DataType::Int32, DataType::Utf8],
    );
    view.add_row(NestedRow::new(
        vec!["id".to_string(), "name".to_string()],
        vec![
            Value::Primitive(PrimitiveValue::new(CellValue::Int32(1), DataType::Int32)),
            Value::Primitive(PrimitiveValue::new(
                CellValue::Utf8("Alice".to_string()),
                DataType::Utf8,
            )),
        ],
    ));

    let text = view.to_text_display();
    assert!(text.contains("Test"));
    assert!(text.contains("Alice"));
}

// ============================================================
// Filter tests
// ============================================================

#[test]
fn test_filter_creation() {
    let filter = Filter {
        column_name: "name".to_string(),
        operator: FilterOperator::Contains,
        value: "test".to_string(),
    };
    assert_eq!(filter.column_name, "name");
}

// ============================================================
// ColumnMeta tests
// ============================================================

#[test]
fn test_column_meta_nested() {
    let meta = ColumnMeta {
        name: "orders".to_string(),
        data_type: DataType::Utf8,
        is_nested: true,
        child_columns: vec!["id".to_string(), "amount".to_string()],
    };
    assert!(meta.is_nested);
    assert_eq!(meta.child_columns.len(), 2);
}
