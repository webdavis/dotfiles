# Finding policy boundaries

Captured from the same Bash source and private command described in [S025-S360](S025-S360.md), before its
Rust implementation. Every case returned 0 with empty standard error. Empty output means discard. The
adapter owns conversion of missing, null and false path fields into typed absence and of numeric counters
into the zero fact. The domain judges those facts.

| Input                                                                                   | Output                                                                                                           |
| --------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `{"name":"new_admin_user","columns":{"target_path":"/owned/.renameio-TempDir-a/file"}}` | \`\`                                                                                                             |
| `{"name":"es_launchd_writes","columns":{"path":"/owned/a\tb\nc"}}`                      | `{"q":"es_launchd_writes","act":"changed","cols":{"path":"/owned/a\tb\nc"},"ep":"/owned/a b c"}`                 |
| `{"name":"system_extensions_new","columns":{"bundle_path":"","path":"/fallback"}}`      | `{"q":"system_extensions_new","act":"changed","cols":{"bundle_path":"","path":"/fallback"},"ep":""}`             |
| `{"name":"system_extensions_new","columns":{"bundle_path":false,"path":"/fallback"}}`   | `{"q":"system_extensions_new","act":"changed","cols":{"bundle_path":false,"path":"/fallback"},"ep":"/fallback"}` |
| `{"name":"new_admin_user","counter":-1}`                                                | `{"q":"new_admin_user","act":"changed","cols":{},"ep":""}`                                                       |
| `{"name":"new_admin_user","counter":0}`                                                 | \`\`                                                                                                             |
| `{"name":"new_admin_user","counter":1}`                                                 | `{"q":"new_admin_user","act":"changed","cols":{},"ep":""}`                                                       |
| `{"name":"new_admin_user","counter":null}`                                              | `{"q":"new_admin_user","act":"changed","cols":{},"ep":""}`                                                       |
| `{"name":"new_admin_user","counter":false}`                                             | `{"q":"new_admin_user","act":"changed","cols":{},"ep":""}`                                                       |
| `{"name":"new_admin_user","counter":"0"}`                                               | `{"q":"new_admin_user","act":"changed","cols":{},"ep":""}`                                                       |
