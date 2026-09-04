# CLI protocol schema 1

The desktop sends one JSON request to `omapress rpc` on stdin and closes stdin. The command writes exactly one JSON response on stdout and exits. Diagnostics never share stdout outside this envelope. Requests are limited to 6 MiB and unknown schema/command/fields are rejected.

```json
{"schema":1,"command":"inspect","path":"/home/user/Publications/example","args":{}}
```

Success: `{"schema":1,"ok":true,"result":{...}}`.
Failure: `{"schema":1,"ok":false,"error":"actionable message"}` and nonzero exit.
Validation includes `result.valid`; the standalone CLI exits nonzero when it is false. Unknown deployment outcomes are successful protocol responses with `result.status="unknown"`, never a Published result.

| Command | Arguments | Result |
|---|---|---|
| init | name, author, base_url | Publication inspection |
| inspect | none | Configuration, articles, source_hash, diagnostics, deployment state |
| read | article | Article metadata/body/text, hash, optional recovery |
| validate | none | valid, diagnostics, source_hash |
| new | series, expected_source_hash | New draft path, inspection |
| save-document | article, meta, body, expected_source_hash | Inspection |
| save | article, text (full front matter), expected_source_hash | Inspection |
| render-document | meta, body | Rendered article and X export |
| render | text (full front matter) | Rendered article and X export |
| import-article | source (local filename), expected_source_hash | Imported article, inspection |
| import-media | source (local filename), expected_source_hash | Content-addressed media path, new source hash |
| delete | article, expected_source_hash | Inspection; only unpublished drafts |
| save-config | text (TOML), expected_source_hash | Inspection |
| build | expected_source_hash, output (default output) | Build manifest |
| feed-check | none | Verified output hashes and feeds |
| export-x | article | html, text, caption, warnings |
| github-status | none | Authenticated account, no token |
| repositories | none | Available repository list |
| setup | repository, create, expected_source_hash | Destination validation/setup outcome |
| publish-plan | expected_source_hash | Exact source/output/remote review |
| publish | expected_source_hash, expected_remote_head | published or unknown |
| recheck | none | published, unknown or not_published; never pushes |
| rollback | deployment_id, expected_source_hash, expected_remote_head | published or unknown |
| recovery-document | article, meta, body, expected_source_hash | Private recovery saved |
| recovery-save | article, text, expected_source_hash | Private recovery saved |
| recovery-clear | article | Recovery removed |

Recovery records bind the base source hash, but saving an unsaved recovery copy does not require the live source to match it: this preserves work after a conflicting external edit. Applying that recovery to source still requires the expected source hash and stops on conflict.

Preview is an explicit CLI child command rather than an RPC request. It prints one `preview_ready` event with a loopback URL, source hash and draft flag, then serves the immutable output until stopped. It has no authoring or publication endpoints.

Initialisation has no existing source to compare and requires a new or empty folder. Pure reads, validation and exports do not mutate article source. Every source mutation requires a reviewed hash. Private recovery deletion and deployment recheck affect local state only.
