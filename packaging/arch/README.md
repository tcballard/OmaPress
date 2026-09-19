# Development snapshot package

This recipe packages source commit `6738df25df0ee11f0916c73bfc7de105010cb099`,
the complete publishing stack through PR #5. It is an immutable development
snapshot (`0.1.0rc1.r20260919`), not a stable upstream release.

The codeload archive was downloaded and its SHA-256 measured:
`5cc435bcc8714b806d9403975eaf142dfc765d7b3907fd9bd9ced26d2f345aa7`.
All 73 archived files matched Git blob hashes from that commit. Source, checksum,
version and architecture must be updated together for a subsequent snapshot.
Do not point this recipe at a moving branch or copy its checksum to another source.

On Arch, review the recipe, then run `makepkg --syncdeps --cleanbuild` as your
ordinary user. The recipe's check phase runs engine tests, mocked distribution and
OAuth/native-message tests, deployment/preview tests, and desktop-file validation.
It installs only package-owned files. The optional companion registration is an
explicit per-user action after installation; no pacman hook edits home directories.

The `arch-package` CI job builds unsigned in an Arch container as an unprivileged
user, compares generated `.SRCINFO`, runs namcap, inspects the archive, installs,
launches with Qt's offscreen backend, checks native-host registration/removal,
reinstalls, and removes the package. The job log is evidence only when it passes
for the exact PR commit. This is a clean container, not a clean-chroot certification
or a real Omarchy/Hyprland desktop acceptance run. Only x86_64 is declared.

The source's native UI and companion tests passed Ubuntu CI. Final publishing with
real X/Substack accounts, Omarchy launcher/theme/accessibility checks, an actual
upgrade from a previously released OmaPress package, and App Store submission
remain outside that CI evidence. No signing key or repository deployment is used.

Pacman removal preserves authored publications and private outbox state. Unregister
the optional browser host using the installed helper before removing the package;
remove the browser extension separately. See `docs/substack-companion.md`.

Namcap errors fail CI and its full report is attached to the unsigned package
artifact. Warnings for the optional Python registration helper and external
commands (`git`, `gh`, `curl`, `secret-tool`, `xdg-open`) require contextual review:
they are runtime integrations, not necessarily linked ELF dependencies. Qt Wayland
is needed on the actual desktop even though CI launches with the offscreen plugin.
The recipe disables empty debug-package generation because the Rust release
profile already strips its binaries.
