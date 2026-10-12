//! Create a disposable embedded database without installing either database CLI.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 || !matches!(args[0].as_str(), "sqlite" | "duckdb") {
        return Err("Usage: cargo run -p query-engine --example seed -- sqlite|duckdb FILE".into());
    }
    if std::path::Path::new(&args[1]).exists() {
        return Err("Refusing to seed an existing file; choose a new path".into());
    }
    let sql = include_str!("../../../dev/embedded-seed.sql");
    match args[0].as_str() {
        "sqlite" => rusqlite::Connection::open(&args[1])?.execute_batch(sql)?,
        "duckdb" => duckdb::Connection::open(&args[1])?.execute_batch(sql)?,
        _ => unreachable!(),
    }
    println!("Created {} fixture: {}", args[0], args[1]);
    Ok(())
}
