# OmaPress name and upgrade

The app, launcher, package and documentation are named **OmaPress**. Commands are
`omapress`, `omapress-desktop` and `omapress-companion`. The repository remains
`tcballard/OmaPress`. This replaces the temporary Pressroom development name.

## Existing development installations

The Arch `omapress` package declares `conflicts` and `replaces` for `pressroom`.
Accept removal of the old package when pacman prompts. Publication directories,
articles and `.omapress` history are not removed. The new desktop copies missing
settings from the former Pressroom settings store once, retaining that store and
never overwriting existing OmaPress settings.

For a portable bundle, install into the same prefix. The old installer has no
ownership manifest: remove the old `pressroom`, `pressroom-desktop` and
`pressroom-companion` executables and old `pressroom.desktop` launcher only after
checking that they belong to your previous installation. New bundles use
`share/omapress`; old support files can remain until the upgrade is verified.

Re-run `<prefix>/share/omapress/install-companion.py --prefix <prefix>` and reload
the unpacked extension from `<prefix>/share/omapress/companion`. Registration
updates the existing native-host entry to the new wrapper. The extension key and
origin are unchanged. No browser registration is changed during package install.

## Workers

Before replacing a worker installation, stop its old timer and let an in-flight job
finish. Inspect uncertain results before scheduling anything again:

```sh
systemctl --user disable --now pressroom-worker.timer
systemctl --user status pressroom-worker.service
```

Upgrade both the desktop and SSH worker to OmaPress: remote calls now invoke
`omapress rpc`. Copy the new `omapress-worker.service` and `omapress-worker.timer`
templates from `share/omapress/worker` into your user systemd directory. Copy your
existing worker environment configuration to `~/.config/omapress/worker.env`,
retain mode 0600, and change `PRESSROOM_PUBLICATION` to `OMAPRESS_PUBLICATION`.
Keep the actual publication and credential paths unchanged. Check ExecStart against
where you installed the new CLI. Follow `scheduling.md` to enable the new timer;
never run both timers for the same publication. Remove the old copied units after
verification and run `systemctl --user daemon-reload`.

## Compatibility identifiers

These private identifiers deliberately remain stable so renaming cannot strand
stored credentials or prepared outboxes:

- Publication files and `.omapress` state retain their existing format.
- The Secret Service application attribute remains `pressroom`.
- The prepared browser outbox remains under `$XDG_STATE_HOME/pressroom`.
- The native-host protocol identifier remains `com.pressroom.companion`.
- `PRESSROOM_CLI`, `PRESSROOM_X_CREDENTIALS_FILE` and
  `PRESSROOM_SUBSTACK_GATEWAY_CREDENTIALS_FILE` are accepted as fallbacks;
  their `OMAPRESS_` equivalents take precedence.

Earlier handoff entries and input checksum files are historical evidence and retain
names as they were when recorded. They are not manifests of the renamed source.
