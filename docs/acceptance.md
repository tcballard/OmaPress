# Version 1 acceptance

Record exact commit, application version, Omarchy version, Qt version, hardware, date and results. Do not turn an automated pass into a real-machine claim.

## Automated release gates

- [ ] Rust formatting, Clippy and full workspace tests.
- [ ] Deterministic sample build, RSS/Atom/JSON feed validation and output hash verification.
- [ ] Real Git with controlled transport: successful publication, correction, rollback, stale review, remote movement, dirty worktree, failed push and unknown public verification.
- [ ] Native Qt build, application load and MIME clipboard smoke.
- [ ] Installable release bundle, checksums and combined executable size below 10 MB.
- [ ] Sanitised 1,000-article benchmark recorded separately from hardware targets.

## Tom's Omarchy XPS acceptance

- [ ] Open/create a publication; normal resize and window close behaviour.
- [ ] Keyboard-only new, select, edit, undo/redo, find, save and publish review.
- [ ] Screen reader names, focus order, contrast and text scaling.
- [ ] Live Omarchy theme switch and minimum-size layout.
- [ ] Edit/preview offline; no request at launch or while typing.
- [ ] Unsaved close guard, recovery after a forced app exit, reopen and stale-save conflict.
- [ ] Missing title, malformed source URL and missing image caught in checks.
- [ ] Preview the exact generated site and subscribe to combined/daily feeds in the native Omarchy RSS reader.
- [ ] Copy article HTML/plain text and caption separately; paste into current X Articles, inspect headings/lists/links/code and manually upload images.
- [ ] Correction keeps one feed item; rollback restores the previous public site while preserving source.
- [ ] Measure cold start to editable list <500 ms, incremental preview p95 <150 ms, full 1,000-text-article build <2 s, responsive typing and installed size.

## First real publication

- [ ] Confirm domain spelling and ownership (tentatively weekinomarchy.com).
- [ ] Create private source repository and separate public output repository.
- [ ] Import Tom's approved X editions and replace provisional branding.
- [ ] Confirm canonical domain/HTTPS and run one reviewed live publish.
- [ ] Verify live public site and feeds from outside the authoring machine.

Until the actual desktop and publication items are complete, distribute as a release candidate and identify those remaining acceptance gates explicitly.
