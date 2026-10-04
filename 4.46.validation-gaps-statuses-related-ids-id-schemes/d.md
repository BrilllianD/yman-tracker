## 2026-10-04T06:46:39Z — Bronnikov Aleksandr

From step 39: the title rule is `char::is_control`, which does not match U+2028/U+2029 (line/paragraph separators) or bidi override characters. Decide whether titles (and tags) should refuse those too.

