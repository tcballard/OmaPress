# Substack browser companion

OmaPress prepares one saved version, including local PNG/JPEG artwork, in a
private local outbox. The companion fills the title, subtitle and body of a blank
Substack draft. It never clicks Publish or chooses an audience. Review the draft,
wait for image uploads and autosave, then choose your audience and email delivery
in Substack. Once published, select the matching article in the companion and
choose **Record this published page**. This is your confirmation, not independent
provider verification.

## Install once

Install the current OmaPress bundle, then register its native host:

```sh
python3 ~/.local/share/omapress/install-companion.py --prefix ~/.local
```

For the Arch package, use `/usr/share/omapress/install-companion.py --prefix /usr`.
Registration writes only your user's Chromium and Google Chrome host manifests;
it needs no root privileges. Open your browser's extensions manager, enable
Developer mode, and choose **Load unpacked** with
`~/.local/share/omapress/companion` (or `/usr/share/omapress/companion`). Pin the
OmaPress companion if desired. Chromium-family browsers using other profile
paths require a matching native-host registration. The stable extension ID is
`lpaeafalgclmnaglhijapdfcmnomkpnc`.

## Use

1. Save the reviewed article and mark it Ready in OmaPress.
2. Select **Substack draft** alongside Website and/or X in the publish panel, or
   use **Prepare Substack draft** by itself.
3. Open a blank editor at your publication's `*.substack.com/publish/…` address.
4. Open the companion, select the article, and choose **Fill this blank draft**.
5. Review the actual Substack result and publish there. On the public `/p/…` page,
   use **Record this published page** for the matching article.

It refuses to overwrite a populated editor. An unrecognized editor or a mismatch
after paste produces an error, never a published receipt. The editor itself may
change before an error is detected; inspect it before retrying. Filling supports
Substack-hosted editors; recording public URLs also supports custom domains.
Changing the source publication invalidates the prepared version. Prepare again
before using it. Once a published URL is recorded, preparation is blocked to
avoid duplicating the post; corrections belong in the existing Substack post.

## Boundaries and removal

The extension has `activeTab`, `scripting`, and `nativeMessaging` permissions. It
has no persistent host permissions, cookie access, account-password access,
background content scripts, or network service. Only its popup talks to the
restricted native host. The host exposes list/read/remove/confirm operations for
prepared entries, not the engine's general RPC interface. Chunked responses stay
below Chrome's 1 MiB native-message limit and are integrity checked before use.

The outbox lives in `$XDG_STATE_HOME/omapress` (default
`~/.local/state/omapress`) with private directory/file permissions. Only referenced
images enter exports. Remove entries using the companion when finished. Removing
a queue entry never removes an article or its publication receipt.

Unregister with `install-companion.py --prefix PREFIX --remove`, then remove the
extension in your browser. Uninstalling the application preserves your authored
articles and outbox; delete the outbox directory explicitly if no longer needed.

## Evidence and limitations

The editor selectors and paste interaction are informed by Automattic's public
[WordPress-to-Substack extension](https://github.com/a8cteam51/WPSubstackChromeExtension).
This implementation does not copy its code or call undocumented publishing APIs.
The native protocol follows [Chrome's specification](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging).

Native framing, allowlisted origin, private storage, integrity checks, stale
source refusal, explicit confirmation, and chunk transfer are covered by local
integration tests. Current authenticated Substack editor paste, image upload,
autosave, and final publication remain **unverified**. The available remote
browser could not reach the editor (HTTP 502). Live desktop acceptance is required
before promoting this experimental companion to a release.
