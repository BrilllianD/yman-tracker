## 2026-10-20T20:07:01Z — Bronnikov Aleksandr

repro: REPRODUCED: Ay6 (cfg-diff3-continue) --continue rc 0 committed t.md with diff3 line '||||||| 66a9ddb' (merge.conflictStyle=diff3 from user config); control with '=======' -> rc 3 'error: still unmerged'; Ay6 (cfg-gpgsign) user commit.gpgSign=true + gpg.program=false: add rc 1 'gpg failed to sign the data'; state set rc 1 same; control before the setting: add rc 0; Ay6 (cfg-log-output) color.diff=always: yman log stdout carries ANSI codes (^[[33m73d1d26^[[m) vs plain '73d1d26'; log.decorate=full adds '(HEAD)'; log.showSignature adds 1 line '6739a6b fakegpg: verify called'; Ay6 (cfg-merge) merge.ff=only: sync rc 1 'fatal: Not possible to fast-forward, aborting.'; merge.verifySignatures=true: sync rc 1 'fatal: Commit 32b98dc does not have a GPG signature.'; control: sync rc 0; Ay6 (cfg-untracked-no) status.showUntrackedFiles=no: status --json dirty 0 with untracked 5.2.hand-made/ (1 without it); sync then snapshotted 0, origin has 0 file(s) of it; control: snapshotted 1, pushed; Ay7 detach 1 '[ab].png' also removed f/a.png (commit lists D 5.1.fix-login/f/[ab].png and D 5.1.fix-login/f/a.png); log 1 with GIT_LITERAL_PATHSPECS=1: 0 line(s) vs plain 4

## 2026-10-21T15:17:28Z — Bronnikov Aleksandr

decision 2026-10-21 (owner): honour commit.gpgSign from the user's config; yman does not pin commit.gpgSign or push.gpgSign. A failed signature must fail cleanly, which is #88's job (unstage on failed commit). gpgSign removed from this step's Done when.

