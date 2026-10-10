# Complete PostgreSQL TUI — first release

The release target is an Ultorg-inspired, Vim-operated relational workbench for
PostgreSQL. Each milestone is implemented, tested and committed before continuing.
This targets a usable end-to-end application rather than every feature of Ultorg.

- [x] Nested relational queries, field/relationship selection, typed filters/sorts,
  formula columns/aggregates, undo/redo, saved views, seeded development database.
- [x] Browse beyond the first result window: independent per-record collection
  pagination, next/previous/load-more, explicit range indicators, lazy folded children.
- [ ] Direct manipulation: contextual filter/formula entry, field ordering and widths,
  full-value inspector, scrolling help, schema/query/SQL inspection.
- [ ] Export results: nested JSON and rectangular CSV with explicit loaded-result scope.
- [ ] Saved workspaces: multiple views, selection/scroll restoration, close/duplicate
  views, atomic session save/load and startup option.
- [ ] Connection support: certificate-verified TLS, reconnect/catalog refresh,
  useful connection errors and documented configuration.
- [ ] Database editing scope pending user preference: staged changes with review,
  explicit apply/discard, transactions and stale-row protection if included.
- [ ] Final keyboard/terminal acceptance workflow, real PostgreSQL regression tests,
  documentation, clean commits and no known blockers in the above workflows.

Pagination is a presentation setting and never limits formula aggregates. Field
selection, filters and relation matching remain inspectable. No saved view or
workspace contains connection credentials. Database writes, if implemented, require
an explicit apply action in the product; read-only remains the default connection mode.
