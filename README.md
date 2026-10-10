# ultorglike

A Rust terminal database browser inspired by Ultorg and Sieuferd. Explore related
records in one nested grid, navigate with Vim motions, and build queries by selecting
fields and relationships. PostgreSQL performs joins, filtering, and sorting.

## Run the seeded workflow

Requires stable Rust and Docker Compose.

```sh
docker compose up -d --wait
cargo run -- --view examples/customers.json
```

Compose starts PostgreSQL 17 on `localhost:55432` and automatically imports
[dev/seed.sql](dev/seed.sql) when its data volume is first initialized. It contains
125 customers, orders, line items, products, and contacts. There are customers with
no orders, orders with no items, Unicode names, nullable columns, exact decimal
prices, and enough rows to exercise scrolling. Data persists across restarts.
Initialization scripts do not rerun on an existing volume.

The example view shows Customers → Orders → Line Items → Products inline. To build
it yourself, start `cargo run`, then use `f` to select the Orders relationship,
`Tab` to enter its field picker, and repeat for Line Items and Products. Use `/`
inside the picker to search. Enter toggles the selected field/relationship; Escape
returns to the grid. Removing a relationship removes its query subtree; `u` restores it.
`zc` instead folds a relationship while retaining its filters and sorting.

Try the following commands in the app:

```text
:filter customers.orders.status = Shipped
:match customers.orders
:sort_desc customers.orders.freight
:show customers.country
:save views/shipped.json
```

Filtering a child collection preserves its parents by default. `:match orders`
requires matching children and propagates that requirement to the root. `:keep orders`
restores parents without matches. Filtering on a hidden field still applies; header
indicators and the field picker expose those dependencies. `:unfilter orders.status`
removes a filter. Filters replace the existing condition on the same field; conditions
on different fields combine with AND.

Use `u` / `Ctrl-r` to undo/redo query changes. Reopen a saved query with
`:load views/shipped.json` or `cargo run -- --view views/shipped.json`.
Saved files contain the query and presentation settings, never connection credentials.

## Connect to another database

```sh
DATABASE_URL=postgresql://user:password@localhost/database cargo run
# Alternatively: cargo run -- --database-url postgresql://user:password@localhost/database
```

The initial backend supports PostgreSQL connections without TLS. It discovers
schemas, tables, primary keys, and foreign keys (including composite keys), and
allows traversing foreign keys in either direction. Connections use read-only
transactions by default. Database editing and formula evaluation are not implemented.
The unused DataFusion/DuckDB stubs have been replaced by the working PostgreSQL path.

`POSTGRES_PORT=55433 docker compose up -d --wait` overrides the local port; set
`DATABASE_URL` to match when launching the app.

## Keyboard controls

| Key | Action |
| --- | --- |
| `h j k l` / arrows | Move between visible cells |
| `10j`, `5k`, etc. | Repeat a movement |
| `gg` / `G` | First / last loaded row |
| `0` / `$` | First / last visible column |
| `Ctrl-d` / `Ctrl-u` | Half-page down / up |
| `Enter` / `zo`, `zc`, `za` | Expand, collapse, toggle the selected relation |
| `/`, `n`, `N` | Search loaded values; next / previous match |
| `t` | Searchable table picker; opening creates a view |
| `f` | Fields and relationships for the selected relation |
| `F` | Start a filter command for the selected field |
| `s` / `S` | Sort selected field ascending / descending |
| `x` | Hide selected field |
| `u` / `Ctrl-r` | Undo / redo query changes |
| `gt` / `gT`, `:view 2` | Switch open views |
| `:` | Command input; Up/Down history, Tab completion |
| `?` / `H` | Help |
| `Esc` | Cancel input, close popup, or cancel running query |
| `q` | Close help/picker, or quit from browsing |

The picker uses `j/k` or arrows to select, `/` to edit its search, Enter/Space to
toggle, and Tab/Right to enter an already-added relationship. Escape first leaves
search editing, then closes the picker. Use `:fields customers` to return to a parent.

Commands accept relation paths (`customers.orders` or `orders`) and field paths
(`orders.freight`). `:help` lists them. Available comparisons are `=`, `!=`, `>`,
`>=`, `<`, `<=`, `contains`, `null`, and `notnull`. Values containing spaces are
entered literally after the operator. `contains` is case-insensitive and treats
wildcard characters literally. PostgreSQL validates values against column types.
`:sort`/`:sort_desc` make the field the primary sort term, retaining previous terms
as tie-breakers; `:unsort field` removes a term.

## Query execution and layout

- A serializable query tree is independent of results and cursor state. Schema paths
  have stable IDs; selected rows use primary-key identities across refreshes.
- A single SQL statement builds independent nested collections, avoiding Cartesian
  multiplication and per-row network queries. Filter values are parameters and SQL
  identifiers are quoted.
- A background worker opens a connection per request. Superseded requests are
  cancelled and stale responses discarded. A ten-second statement timeout bounds
  expensive queries; errors leave the previous result visible and the query editable.
- Results default to 100 rows **per relation instance**. Truncation is shown in the
  status bar. `:limit 200` increases the limit (maximum 500). Search applies to loaded
  rows only; filters and sorts apply in PostgreSQL before limiting.
- The grid renders recursive grouped headers, spanning parent cells, empty child
  placeholders, persistent horizontal/vertical scrolling, and Unicode values.
  `NULL` denotes a scalar null, `∅` an empty relationship, and `…` a pending field.
- Maximum query depth is eight, with at most 24 table instances. Tables without
  primary keys have weaker identity guarantees. Session tabs/history are in memory;
  saving preserves query settings, not cursor/scroll position or undo history.

## Development workflow

```sh
./scripts/check.sh             # format, workspace tests, strict clippy
./scripts/check.sh --postgres  # also start Compose and run real database integration
```

The same workflow runs in [GitHub Actions](.github/workflows/ci.yml). The database
integration test is explicitly ignored by ordinary `cargo test`, so unit tests work
without Docker. It can also run against an existing seeded fixture:

```sh
TEST_DATABASE_URL=postgresql://ultorg:ultorg@localhost:55432/ultorg \
  cargo test -p query-engine --test postgres -- --ignored --test-threads=1
```

[Implementation workflow](docs/implementation-workflow.md) records this milestone.

| Component | Responsibility |
| --- | --- |
| `src/app.rs` | Modal interactions, query editing, history, saved views |
| `src/ui.rs` | Ratatui rendering and terminal clipping |
| `crates/data-model/src/document.rs` | Recursive result document and layout |
| `crates/data-model/src/from_arrow.rs` | Retained Arrow conversion adapter |
| `crates/query-engine` | PostgreSQL catalog, query model, compiler, worker |
| `crates/tui-input` | Reusable key sequence and command input handling |
| `crates/tui-widgets` | Reusable terminal widgets |

Research references are in `ref/`: Related Worksheets (2011), Automatic Layout of
Structured Hierarchical Reports (2013), and Sieuferd (2016).
