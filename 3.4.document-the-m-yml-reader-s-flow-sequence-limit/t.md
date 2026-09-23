# Document the `m.yml` reader's flow-sequence limit

The flow-sequence branch splits on a bare `,`, so a hand-written
`tags: ['a,b']` fails with `unterminated single-quoted string`. The writer
never emits flow form, so this is a reader limit, not a bug to fix.

- Where: `docs/storage.md` §6
- Done when: the limit is listed next to the other accepted-but-not-written
  forms.
