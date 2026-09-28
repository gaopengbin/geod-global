# shadcn/ui source attribution

Upstream: https://github.com/shadcn-ui/ui

Pinned commit: `98a1fe67b439324ddc857f47fbdce056600a4329`.

License: MIT. The complete upstream license is in `LICENSE.md`.
`provenance.json` records each unmodified snapshot, URL and SHA-256.

The Input, Textarea, Select, Badge and Table composition in
`prototype/src/ui/index.jsx` is adapted from the corresponding
`apps/v4/registry/new-york-v4/ui/*.tsx` files. TypeScript declarations are removed
for the application's JSX stack; theme classes are mapped to Beautiful UI's
tokens. Select uses the upstream Radix Trigger/Portal/Content/Item structure.
A visually hidden native form adapter preserves existing option children,
empty values, required validation and FormData without exposing the operating
system's visual menu. The original NativeSelect snapshot is retained as the
reference for that native form contract.

Accessible interactive behavior not supplied by Beautiful UI uses official
Radix primitives through exact npm dependencies: Dialog, Checkbox, Slider,
Switch, Progress, Collapsible, ToggleGroup, Toast, Select and Slot. Their package
versions and integrity hashes are in the repository's root `package-lock.json`;
package licenses are collected separately by the release packaging script.

All application-specific labels and state remain outside this shared layer.
