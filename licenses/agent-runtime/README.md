# Owned Agent runtime notices

These unchanged upstream files belong to the fixed development runtime prepared
by `scripts/prepare-agent-runtime.mjs`. Preparation verifies their hashes and
copies them into the runtime's `licenses/` directory. It also records the actual
JavaScript bundle dependencies, copies their published copyright/license files,
and retains any emitted legal-comment file.

`sources.json` records official source URLs, exact versions, sizes and SHA-256.
The official Node 24.14.0 release checksum list was checked against the pinned
Windows x64 executable SHA-256. Its full combined LICENSE includes Node's own
terms and the bundled dependency terms; it must not be replaced with a short
MIT declaration.

The official Codex `rust-v0.159.2` tag resolves to commit
`ff6aec96948b70d94983af2641a6b67c94faeff5`. Its complete Apache-2.0 LICENSE and
unchanged NOTICE are retained. The NOTICE attributes Ratatui-derived code; the
0.30.2 license matches the release's Cargo.lock dependency. The release's
vendored WezTerm license is retained as well. Voice binaries and their LGPL
components, shell/code-mode helper executables and Linux bubblewrap are not
copied by this development preparation.

The published `@ai-sdk/provider-utils` 5.0.53 package omits its license file.
Its upstream copyright declaration is pinned to the package's official release
commit. The complete Apache-2.0 terms are supplied alongside it by the unchanged
Codex LICENSE; the generated inventory points Apache packages to that terms file.

This preserves the upstream runtime notices and the actual bundled npm
attributions. It does not establish a complete transitive Rust-binary license
audit, corresponding-source fulfillment, installed distribution acceptance or
permission to publish. Those remain separate release checks. The generated
inventory contains relative artifact names and official source URLs, never
workstation paths, credentials or model settings.
