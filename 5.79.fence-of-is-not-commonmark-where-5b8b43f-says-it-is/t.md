# fence_of is not CommonMark where 5b8b43f says it is

Two deviations, both now hard refusals because of the new `unclosed code fence` error. (a) A backtick fence whose info string contains a backtick is not a fence in CommonMark; here it opens one, so a `# A` heading, then a line of three backticks followed by `x` and three more backticks and the word `inline`, then `# B`, fails with `line 2: unclosed code fence` although it parsed before this range. (b) A line indented four or more spaces is indented code in CommonMark but opens a fence here; `docs/commands.md` documents (b) as intended, so only (a) is a real deviation.

- Where: `src/sections.rs` (`fence_of`), `docs/commands.md`, unit tests.
- Done when: a backtick run followed by an info string containing a backtick is body text, and the doc states that the indentation rule is a deliberate simplification.
