# Publishing queue

Development implementation: paste/import → review → publish now or schedule.
The editor's **Add article…** accepts plain text and Markdown copied from ChatGPT,
without requiring front matter. Add a title, summary, series and artwork; save and
mark the article Ready. Rich clipboard HTML is not converted automatically.

Open **Queue…**, choose the date/time and timezone, select Website and/or X, and
schedule the saved version. Europe/London follows UK clock changes. Ambiguous or
nonexistent times at the clock change are rejected. The RPC also accepts an RFC3339
`at` with an explicit offset instead of `local_time`.

## What a schedule authorizes

- X publishes the selected article using the worker's connected account.
- Website deploys **all Ready/Published articles in that publication together**.
  Keep later website articles in Draft. This release does not provide independent
  per-article website release dates. One outstanding website job is allowed.
- Substack can use the optional [Gateway integration](substack-gateway.md) to submit
  a reviewed schedule directly to Substack. The browser handoff remains available.
  No Substack job is sent to this website/X worker.
- The whole source hash is pinned. Any content, configuration, artwork or theme
  change on the executing host blocks pending jobs. Cancel and review again;
  the queue never quietly publishes a newly edited version.
- Jobs more than one hour late stop as blocked. Successful and uncertain attempts
  are never automatically retried. Each destination's outcome is retained.
- An interrupted running job becomes `needs_review`. Inspect its receipts and
  the destination; website recovery uses `pressroom recheck`, while X supports
  recording a published URL. Then use **Reconcile recorded results** to close the job against those receipts without resending. Failed, unconfirmed destinations remain stopped for operator review; this initial UI has no blind Retry.
- Jobs are processed sequentially, one per worker invocation. A timer checks roughly
  once per minute after the preceding invocation finishes; this is not second-exact delivery.

## Local and remote operation

The desktop **does not execute jobs itself**. Install the timer below on a computer
that will remain awake. A local timer cannot publish while its computer is off.
For laptop-off publishing, use a dedicated always-on Linux user and SSH worker.
No inbound application HTTP listener is installed; SSH uses existing host-key trust
and noninteractive authentication. Do not disable StrictHostKeyChecking.

1. Build the matching CLI (`cargo build --release --locked`) on the worker and
   install `target/release/pressroom` as `/usr/local/bin/pressroom`.
2. Create a private parent directory, e.g. `/srv/pressroom`, writable only by the
   worker user. The publication child is created by the app's first upload.
3. Configure and verify an SSH alias locally, e.g. `pressroom-worker`. Ensure
   `ssh pressroom-worker pressroom --version` succeeds without an interactive prompt.
4. In Queue, enable the SSH worker, enter that alias and the absolute publication
   path, then **Upload reviewed publication**. Initial transfer carries source and
   destination history, never credentials. The encoded transfer budget is 6 MiB;
   oversized artwork produces an explicit error before upload.
5. Subsequent uploads require a refreshed worker source hash and no pending or
   uncertain job. Existing worker history is authoritative and preserved. Linux
   atomic directory exchange replaces the complete tree without a partial source.
6. Connect credentials on the worker, install/start the timer, then schedule.
   Refresh Queue to see the worker's last invocation and recorded destination results.
   An accepted queue submission is not proof that the timer is installed/running.

Use one worker as the publication's publishing authority. Remote receipts remain
on that worker; they are not automatically merged into the laptop's direct Publish
history. Do not independently publish the same article from both locations. Remote
credentials are configured separately; check that they belong to the intended account.

## Worker credentials

Website publishing needs `git`, `gh` and the worker user's GitHub authorization,
with the publication's configured repository accessible. Run `gh auth login` as
that user. X needs `curl` and either Secret Service or an explicitly configured
`PRESSROOM_X_CREDENTIALS_FILE` containing the OAuth credential JSON:

```json
{"access_token":"…","refresh_token":"…","client_id":"…","user_id":"…","username":"…","expires_at":0}
```

Supply actual credentials from your authorized X OAuth flow; this is a format
example, not a working token. Use the expiry from that flow, not zero. The file
must be an absolute, owner-only regular file owned by the worker user. Keep its
parent directory private. Token refresh atomically updates this file. It is never
uploaded by the desktop or included in source. Secret Service remains the desktop
default. This release does not automate remote OAuth enrollment.

## Install the optional user timer

Run as the worker user, from the matching source checkout:

```sh
mkdir -p ~/.config/systemd/user ~/.config/pressroom
cp deploy/worker/pressroom-worker.service deploy/worker/pressroom-worker.timer ~/.config/systemd/user/
cp deploy/worker/worker.env.example ~/.config/pressroom/worker.env
chmod 600 ~/.config/pressroom/worker.env
# Edit worker.env to use your actual absolute publication and credential paths.
$EDITOR ~/.config/pressroom/worker.env
systemctl --user daemon-reload
systemctl --user enable --now pressroom-worker.timer
systemctl --user status pressroom-worker.timer
journalctl --user -u pressroom-worker.service
```

To keep a user service active after logout, a host administrator can enable linger
for the dedicated worker user (`loginctl enable-linger USER`). Nothing enables it
automatically. Start with a nonproduction publication and verify the credentials
and receipts before relying on unattended delivery.

Stop scheduling with `systemctl --user disable --now pressroom-worker.timer`.
Stop an in-flight worker with `systemctl --user stop pressroom-worker.service`;
its job will require review on restart. Remove the two unit files and reload the
user daemon to uninstall. Preserve the publication's `.omapress` history and all
articles. Uninstalling the desktop does not cancel jobs already accepted remotely;
cancel them in Queue or stop the remote timer first.

## Evidence boundary

Automated tests use real CLI processes and filesystem operations with SSH/X doubles.
They cover intake, queue reopening, duplicate IDs, cancellation, DST, corrupt/newer
state, interruption, uncertain provider outcomes, source changes, private credentials
and remote source replacement. These are not live provider/VPS acceptance tests.
The existing Arch recipe remains pinned to the earlier reviewed release candidate;
build this branch for these new features until a separate package revision is cut.
