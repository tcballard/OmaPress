# OmaPress v0.0.2

Your theme, throughout your writing workspace.

OmaPress now reads the active Omarchy Quattro palette, bringing Familiar’s light background, dark text and blue accents into the app. The toolbar also gives the title and Publish button enough room inside rounded window corners.

- Follow the current Quattro theme, with support for the older theme location.
- Update colours when you switch themes and keep the last complete palette while theme files are replaced.
- Improve sidebar contrast for light themes and add space beneath the footer.
- Add automated checks for Familiar colours, live theme changes and toolbar spacing.

This is an early testing release. The original app launch was tested on Tom’s XPS; the corrected appearance still needs confirmation on that device. Automated checks cover the native build, theme behaviour, application launch and clipboard. Real provider publishing workflows remain separate acceptance work.

Download the Linux x86_64 bundle and SHA256SUMS, verify the checksum, then run `sh install.sh "$HOME/.local"` from the extracted directory. Close OmaPress before updating. Existing publications and settings are preserved. To roll back, reinstall the v0.0.1 bundle; this update makes no publication data-format changes.

The bundled binaries require Qt 6 and the dependencies listed in the README. This GitHub release does not itself update the Omarchy package repository.
