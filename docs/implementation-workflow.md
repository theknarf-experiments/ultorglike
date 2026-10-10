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
the browser are part of this milestone. Formula evaluation is a later milestone.

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
through Compose. Formula columns and database editing remain future milestones.
