# warn when an epic is closed with open steps

An epic can be closed while steps are open, silently.

- Where: `src/plan.rs` report_closed, docs/errors.md, docs/commands.md.
- Done when: closing an epic-tagged task with open relaters prints a warning on stderr, exit 0; follow-up relations on non-epic tasks stay quiet; tests cover it.
