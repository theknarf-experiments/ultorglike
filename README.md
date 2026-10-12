# ultorglike

A Rust terminal database workbench inspired by Ultorg and Sieuferd. Explore related
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

## Formulas and nested totals

Open the ready-made example:

```sh
cargo run -- --view examples/customer-totals.json
```

It computes discounted line-item amounts, order totals including freight, item
counts, and each customer's total spend. Orders and customers sort by their totals.
To build it from `examples/customers.json`, enter:

```text
:formula orders.line_items.amount = [quantity] * [unit_price] * (1 - [discount])
:formula orders.total = sum([line_items.amount]) + [freight]
:formula orders.item_count = count([line_items])
:formula customers.total_spend = sum([orders.total])
:sort_desc orders.total
:sort_desc customers.total_spend
:save views/customer-totals.json
```

`:formula path.name = expression` creates or replaces a formula and shows its column.
Formula names use letters, digits and underscores, starting with a letter or underscore.
Headers mark formulas with `ƒ`; the field picker (`f`) shows their expressions and
can toggle visibility. Use `:hide orders.total` to hide a formula while keeping its
calculation, or `:unformula orders.total` to remove it and its filters/sorts. Removing
a formula or relationship still needed by another formula is rejected; hide/fold it
instead, or remove the dependent formula first. Query changes support undo/redo.

Expressions support decimal literals, parentheses, unary `+`/`-`, and `+ - * /`.
`[field]` refers to a numeric database column or formula in the same relation,
including hidden fields. `sum([child.field])` sums a numeric column or formula on an
attached immediate child relationship. `count([child])` counts rows;
`count([child.field])` counts non-null values of any type. Chain formulas across
levels, as in the customer total above. References use the relationship labels
shown in the view; field names with spaces work inside brackets.

Calculations use PostgreSQL `numeric` arithmetic and preserve decimal precision.
Null inputs propagate through arithmetic; division by zero returns `NULL`.
Sums ignore null values and return zero for empty/all-null collections; counts
return zero for empty collections. Aggregates include **all matching child rows**,
regardless of `:limit`, visibility, or folding. Child filters and `:match` restrictions
apply before aggregation. Independent sibling collections never multiply totals.
Filters and sorts also work on formulas, e.g. `:filter orders.total > 400`.

Formulas are a bounded expression language, not arbitrary SQL: numeric database
fields, arithmetic, `sum`, and `count` are supported. Text/date functions, explicit
casts, and direct references across multiple relationship levels are not supported.
Definitions are saved with the query; existing views without formulas still load.

## Connect to another database

```sh
DATABASE_URL=postgresql://user:password@localhost/database cargo run
# Alternatively: cargo run -- --database-url postgresql://user:password@localhost/database
```

The PostgreSQL backend supports certificate-verified TLS. It discovers
schemas, tables, primary keys, and foreign keys (including composite keys), and
allows traversing foreign keys in either direction. Connections use read-only
transactions by default. Database writes are available only with `--editable`; numeric formulas and aggregates execute in PostgreSQL.
Use `sslmode=require` in the connection URL to require encryption. TLS verifies the
server certificate and hostname using the system trust store; set `PGSSLROOTCERT`
to a PEM CA certificate for a private server. The default PostgreSQL `prefer` mode
also permits servers without TLS, including the local Compose fixture. Use
`:reconnect` to retry a connection and refresh the catalog without losing the view.
TLS uses [postgres-native-tls](https://docs.rs/postgres-native-tls/latest/postgres_native_tls/).

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
| `]p` / `[p`, `+` | Next / previous collection page; load more |
| `Enter` / `zo`, `zc`, `za` | Expand, collapse, toggle the selected relation |
| `/`, `n`, `N` | Search loaded values; next / previous match |
| `t` | Searchable table picker; opening creates a view |
| `f` | Fields and relationships for the selected relation |
| `F` | Start a filter command for the selected field |
| `s` / `S` | Sort selected field ascending / descending |
| `x` | Hide selected field |
| `K`, `=` | Full-value inspector / formula editor |
| `]w` / `[w` | Widen / narrow selected column |
| `i`, `dd` (with `--editable`) | Stage cell edit / row deletion |
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
- Results default to 100 rows **per relation instance**. The selected collection's
  range appears above the grid (`+` means more rows). `]p` / `[p` page through that
  collection; `+` loads another page into it (up to 5000 loaded rows). `:nextpage
  customers` pages the root even when a child is selected. `:prevpage` and `:more`
  also accept relation paths. `:limit 200` sets page size (maximum 500). Each
  parent's child collections page independently, with stable key ordering.
- Folded collections load when expanded; formulas still aggregate their full matching
  rows. Query edits reset collection windows. Reusable saved views open at page one.
  Search applies to loaded values; filters and sorts run in PostgreSQL before paging.
  Concurrent database changes may shift offset-based page boundaries.
- The grid renders recursive grouped headers, spanning parent cells, empty child
  placeholders, persistent horizontal/vertical scrolling, and Unicode values.
  `NULL` denotes a scalar null, `∅` an empty relationship, and `…` a pending field.
- Maximum query depth is eight, with at most 24 table instances. Tables without
  primary keys have weaker identity guarantees. Reusable view files save query settings;
  workspace files also preserve tabs, cursor/scroll, paging and undo history.

## Inspect, arrange, and export

`K` opens the full selected value, including multiline content and its type.
`:query` shows the query tree, filters, sorts, formulas, hidden dependencies and loaded
ranges. `:sql` shows generated SQL and separately bound parameters; `:schema` shows
columns, primary keys and relationships. All panels and help scroll with `hjkl`,
`g`/`G`, PageUp/PageDown, or Ctrl-u/Ctrl-d; Escape/q closes them.

`F` starts an editable filter command using the current condition; `=` starts a new
formula or edits the selected formula. Command input supports Left/Right, Home/End,
Delete/Backspace, Ctrl-u (delete before cursor) and Ctrl-k (delete after cursor),
including Unicode and horizontal scrolling. Up/Down recalls command history.

`:move orders.total 1` reorders a visible field within its relation. `:width
orders.total 24` sets its width in terminal cells; `]w`/`[w` widen/narrow the selected
column. Layout changes support undo and are stored with saved views.

`:export json results.json` exports the loaded nested result, with named fields,
children, row keys, collection ranges, query and an explicit partial-result flag.
`:export csv results.csv` exports the loaded grid with full-path headers, repeated
spanning parent values and standard CSV quoting. CSV uses `\N` for SQL NULL, empty
cells for absent children, and `[folded]` for unloaded collections. Siblings appear
side by side as in the grid; this is a presentation export, not a relational join.
Exports include the currently loaded windows only. JSON preserves explicit nulls
and is preferable when distinguishing null markers from literal text matters.

## Staged database editing

Start `cargo run -- --editable --view examples/customers.json` to enable editing.
The default remains read-only. `i` opens the selected cell's value in an editable
command; `:edit VALUE` or `:set orders.status VALUE` stages a new value. JSON strings
allow escaped newlines and significant whitespace: `:edit "a\nb"`. Use `:edit null`
for SQL NULL and `:edit "null"` for literal text. `dd` / `:delete` stages the selected
record for deletion. Updated cells show `*`; deleted rows show `×`.

`:insert {"id":"200","company_name":"New customer","country":"Norway"}` stages an
insert into the selected relation's table. Values must be JSON strings or null;
omitted columns use database defaults. Supply required foreign keys explicitly.
Use `:schema` to inspect the table first. Inserts are listed in the changes panel
and appear in the grid after applying, if they match the current query/window.

`:changes` or `:apply` opens a review of pending writes. Close the panel, then use
`:apply yes` to commit the batch in one transaction. `:discard`, `:discard last`, or
`:discard N` abandons staged changes. Up to 100 row changes can be staged; multiple
field updates on the same record are combined. A type, constraint, permission or
stale-row conflict rolls back the entire batch. Changed/deleted rows are detected
using the primary key and PostgreSQL row version loaded by the browser. Discard
and refresh a conflicting row before restaging. Connection errors during commit
can leave the outcome uncertain; inspect the database before retrying.

Editing requires a primary key. Formula columns, generated columns, and primary-key
updates are read-only. PostgreSQL permissions and foreign-key rules still apply,
including delete cascades. Reads stay read-only, even in editable mode. Query undo
(`u`) does not undo applied database writes. Pending writes are not saved in views
or workspaces; apply/discard them before exporting, saving/loading a workspace or
quitting. `:quit!` exits and discards pending local changes.

## Workspaces

`:duplicate` copies a view, `:close` closes it, and `:views` lists open views.
`gt`/`gT` or `:view N` switches views and restores the cursor/scroll position and
undo history. `:session_save workspaces/project.json` saves all open views,
layout, page positions and up to 100 query changes per view. Reopen with
`:session_load workspaces/project.json` or:

```sh
cargo run -- --session workspaces/project.json
```

Workspace files contain queries and row identities, never connection credentials
or cached result data. Supply the connection separately. They are validated against
the current catalog before replacing the open workspace. Files are written atomically.
Reusable `:save` view files still open at page one; workspace files retain page positions.

## Development workflow

```sh
./scripts/check.sh             # format, workspace tests, strict clippy
./scripts/check.sh --postgres  # also start Compose and run real database integration
./scripts/check-tls.sh         # isolated TLS fixture; trusted/untrusted/hostname checks
```

The same workflow runs in [GitHub Actions](.github/workflows/ci.yml). A keyboard/PTY
acceptance test also exercises the local Compose fixture (including a temporary
customer inserted, updated and deleted through the app):

```sh
python3 -m venv .venv
.venv/bin/pip install -r dev/requirements-test.txt
cargo build --locked
.venv/bin/python scripts/smoke.py
```
 The database
integration test is explicitly ignored by ordinary `cargo test`, so unit tests work
without Docker. It can also run against an existing seeded fixture:

```sh
TEST_DATABASE_URL=postgresql://ultorg:ultorg@localhost:55432/ultorg \
  cargo test -p query-engine --test postgres -- --ignored --test-threads=1
```

| Component | Responsibility |
| --- | --- |
| `src/app.rs`, `src/app/` | Modal interactions, query editing, workspaces, exports, staged writes |
| `src/ui.rs` | Ratatui rendering and terminal clipping |
| `crates/data-model/src/document.rs` | Recursive result document and layout |
| `crates/data-model/src/from_arrow.rs` | Retained Arrow conversion adapter |
| `crates/query-engine` | PostgreSQL catalog, query model, compiler, worker |
| `crates/tui-input` | Reusable key sequence and command input handling |
| `crates/tui-widgets` | Reusable terminal widgets |

Research references are in `ref/`: Related Worksheets (2011), Automatic Layout of
Structured Hierarchical Reports (2013), and Sieuferd (2016).
