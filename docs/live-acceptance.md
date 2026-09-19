# Development package acceptance

Use a disposable publication and destinations you control. Record package revision,
source commit, host/Omarchy version, account identity, UTC time and receipt URLs.
Never put credentials in a report. Do not use real subscribers for a test send.

## Install

Download `pressroom-arch-unsigned` from the successful packaging PR workflow,
unzip it, and run `sha256sum -c SHA256SUMS` in the extracted directory. Install the
non-debug `.pkg.tar.zst` with `sudo pacman -U ./pressroom-*.pkg.tar.zst` on an
up-to-date Arch/Omarchy machine. This is an unsigned development build; checksums
check file integrity, not publisher identity. CI artifacts require GitHub access.

Launch Pressroom from the application launcher. Confirm one window, readable theme,
clipboard paste, file dialogs, preview and restart persistence on real Wayland.
Add an article from ChatGPT, save, reopen, inspect artwork and mark it Ready.

## Destination checks

1. Website: use a test repository, review the full Ready/Published set, publish,
   then open the recorded site URL. Check article, image, feed and canonical URL.
2. X: connect your test account, verify the displayed identity, review the draft,
   explicitly publish one test article and inspect its recorded URL and image.
   If the result is uncertain, inspect X and reconcile; do not blindly resend.
3. Substack: follow `substack-gateway.md`, connect your own gateway and test
   publication, prepare one draft and inspect it in Substack. Verify text and images.
   Schedule a reviewed test draft with an appropriate audience, confirm its time
   in Substack, cancel it and confirm cancellation there. Separately, deliberately
   schedule one test publication, close Pressroom and stop the gateway, and verify
   Substack publishes it at the accepted time. Record the resulting URL.
4. Browser fallback: register the companion, transfer a draft to a blank Substack
   editor, inspect it, and confirm the final published URL after an explicit send.

## Laptop-off worker check

Follow `scheduling.md` on a dedicated always-on host. Verify the CLI version, SSH
identity, credentials, copied service's absolute executable path and timer heartbeat.
Upload the reviewed test publication and schedule Website/X. Shut down the laptop
before the due time. Verify publication and per-destination receipts on the worker;
restart the laptop and refresh Queue. Remote receipts remain worker-owned.

Cancel an additional queued test job and verify it never publishes. Edit source
before another test job and verify it blocks instead of publishing changed content.
Stop the timer after testing. Substack schedules must be cancelled separately in
Substack/API; removing Pressroom does not cancel remote work.

## Release gate

CI validates provider doubles, native offscreen dialogs, package build and installation.
It does not establish real Wayland behavior, X entitlements, a live gateway session,
Substack delivery or an operational VPS. Record each live result as passed, failed
or not run. Keep stable release and App Store submission pending until these results
have been reviewed. No live-account tests have been completed in this packaging pass.
