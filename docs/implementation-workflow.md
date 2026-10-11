# Implementation workflow

Goal: open seeded PostgreSQL data, navigate Customers → Orders → Line Items inline,
change fields/relationships, filter/sort, undo/redo, and save/reopen views.

1. Repair keyboard dispatch and modal behavior; add interaction regression tests.
2. Build recursive result layout, path-based selection, scrolling, and layout tests.
3. Implement PostgreSQL catalog discovery and bounded/cancellable background reads.
4. Add serializable query trees, field/relationship picker, typed filters, scoped
   parent matching, sorting, undo/redo, and saved views.
5. Seed PostgreSQL with Compose; exercise the complete workflow against real data.
6. Run formatting, workspace tests, clippy, integration tests, and terminal smoke checks.

Use the existing working tree; preserve unrelated edits. No database writes from
the browser are part of this milestone. The formula milestone follows below.

Validation is repeated by `.github/workflows/ci.yml` and `scripts/check.sh`.

## Completed validation

- Keyboard-to-app regression tests cover commands, repeated actions, mode exits,
  history, views, counts, search, query edits, undo/redo, and persistence.
- Layout tests cover three levels, independent siblings, empty child collections,
  stable cell addresses, and rendering at narrow terminal sizes.
- Arrow conversion tests cover nulls, decimal precision, timestamp units, and Unicode.
- Live PostgreSQL tests cover foreign keys in both directions, composite keys,
  quoted identifiers, wide schemas, exact numeric values, scoped filters, safe
  parameters, limits, saved views, read-only access, and server-side cancellation.
- A PTY smoke run opened the example and exercised commands, nested values, filter,
  parent matching, undo, help, field picker, folding, and normal shutdown.

The first four milestones are implemented. The fixture database remains available
through Compose. Formula columns are covered by the next milestone below; subsequent release milestones follow.

## Formula milestone

Acceptance: calculate line-item amounts in the seeded customer view, aggregate
order totals, sort by total, and save/reopen with identical results. Include more
matching line items than the display limit and an independent sibling collection.

1. Add a bounded numeric expression parser and serialized formula definitions.
2. Compile formulas and correlated aggregates in PostgreSQL before display limits;
   validate types, missing dependencies and cycles before changing a view.
3. Integrate commands, field picker, visibility, filters/sorts, undo/redo and save/load.
4. Add a runnable totals example and document null, empty collection and division rules.
5. Verify parsing/rejections, app interactions and real PostgreSQL results, including
   exact decimals, filters, nested aggregates, limits, empty children and roundtrips.
6. Run the existing formatting, workspace tests, Clippy and PostgreSQL workflow.

Completed: parser/compiler, commands/picker, saved formulas, and
`examples/customer-totals.json`. `scripts/check.sh --postgres` passes: 53 tests
without a database, two doctests, three live PostgreSQL tests, formatting and strict
Clippy. Live tests verify totals with a one-row display limit, independent sibling
collections, nested aggregates, numeric sorting, exact decimals, hidden-field and
formula filters, parent matching, empty/all-null collections and division by zero.
A PTY smoke run created formulas through keyboard input, sorted totals, limited the
grid to one row, saved/reopened the view, undid an edit, inspected the picker and
help, and exited normally. Totals remained 1109.325 for the customer and 497.685
for the displayed order despite omitted orders and line items.

## Complete first-release workflow

Release scope is tracked in [roadmap.md](roadmap.md). Following the formula milestone:

1. Added per-record collection paging and load-more with explicit ranges, lazy folded
   collections, and formula totals independent of collection windows.
2. Added value/query/schema/SQL panels, scrollable help, Unicode command editing,
   formula/filter prompts, column ordering/widths, and loaded-result CSV/JSON exports.
3. Added atomic multi-view workspaces with selection, scroll, paging and per-view
   query history; startup restoration, duplicate/close/list views, catalog reconnect.
4. Added certificate-verified TLS with a disposable TLS fixture verifying trusted
   roots, rejection of untrusted certificates, and rejection of hostname mismatches.
5. Added opt-in staged insert/update/delete, visible pending-change markers, review,
   explicit apply/discard, transactional batches and optimistic row-version checks.
   Disposable-table tests verify parameterization, decimal precision, generated/key
   field protection, stale/deleted row conflicts and rollback of earlier batch writes.
6. Added a reproducible PTY acceptance script to CI. It exercises nested page controls,
   panels, export, tabs, staged CRUD/discard and workspace startup restoration. It
   creates/deletes one unique fixture customer and cleans up after failures.

Validation commands:

- `scripts/check.sh --postgres`: formatting, workspace tests, strict Clippy and live
  PostgreSQL read/write integration tests.
- `scripts/check-tls.sh`: isolated TLS server trust and hostname tests.
- `cargo build --locked` then Python with `dev/requirements-test.txt` installed running
  `scripts/smoke.py`: keyboard/terminal acceptance against the local Compose fixture.

The first release remains PostgreSQL-specific. Formulas support numeric arithmetic,
`sum` and `count`; exports cover loaded windows. Paging uses offsets, so concurrent
writes can move page boundaries. Editing requires primary keys and uses explicit
transactional apply; query undo does not undo database writes. These boundaries are
also described in README and are not claimed as complete Ultorg feature parity.
