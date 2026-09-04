# Security model

OmaPress is a single-user local application. It protects against unsafe Markdown, accidental path traversal, stale reviews, draft leakage and uncertain deployment outcomes. It does not isolate malicious software already running as the same operating-system user.

- Raw Markdown HTML is escaped; link schemes and image paths are validated. Unknown schemes fail public preflight.
- Media is copied into content-addressed paths after type checks; SVG and executable files are not imported as article images.
- Public output is constructed from selected articles and their references. Hidden files, editorial sources and local state are never recursively copied into a site.
- Source reads and writes reject symlink traversal. Atomic writes and process locks coordinate OmaPress instances. External programs are outside those locks; source-hash guards detect changes before state-changing actions.
- Templates cannot invoke commands or read arbitrary files and run with an evaluation budget. Custom templates are local author-controlled presentation code; review them as part of the source snapshot.
- Preview binds only to loopback, uses an unguessable URL prefix, checks Host and method, and has no editing API or directory listing.
- Git/gh/curl commands receive argument arrays, not shell-interpolated user input. Git hooks and interactive credential prompts are disabled. Subprocess groups are torn down while the unreaped child still pins the group ID.
- Deployment uses an exact remote-head lease and durable pending journal. Recheck cannot push. Unknown or corrupt state fails closed.

Report defects privately to the repository owner before disclosing a reproducible draft-leakage or credential-exposure issue. No telemetry or analytics is collected by the desktop.
