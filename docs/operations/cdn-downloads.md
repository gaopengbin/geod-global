# GeoD Global CDN downloads

Public distribution uses the same Tencent COS bucket and CDN hostname as GeoD Agent, with a separate product prefix:

- Bucket: `geod-downloads-1301384002`, Guangzhou.
- CDN: `https://download.laogao.xyz/`.
- Global prefix: `global-updates/windows-x86_64/<version>/`.
- Agent objects and update indexes are not modified.

The current publishing credential is encrypted outside the repository at `%USERPROFILE%/.codex/secrets/geod-cos-publisher.xml`. On October 20, 2026, the existing `geod-publisher` received the supplementary `GeoDGlobalReleasePublisher` policy (ID `290490336`) after explicit confirmation. Its original Agent policy was preserved. No new key was created.

The supplementary [Global upload policy](cos-publisher-global-policy.json) adds only Global version-file HEAD and multipart upload operations, with forbidden-overwrite protection. It adds no large-object download, object deletion, ACL, CDN administration or CAM permission.

The bucket remains private. A separate, explicitly confirmed bucket statement allows this account's Tencent CDN service principal to GetObject, HeadObject and OptionsObject only for `global-updates/windows-x86_64/0.2.0/*`. The original Agent CDN statement was preserved. A future version needs a reviewed CDN origin scope before its website links are published; uploading alone does not establish public delivery.

The four locally accepted 0.2.0 distribution files were uploaded through official COS SDK 1.9.44 with MD5 enabled. Object size, immutable cache header and caller-supplied SHA metadata matched. That metadata is not a server-calculated SHA-256 digest. The public checks verified both small files by full SHA-256, and the EXE/ZIP by HEAD plus their first and last 64 KiB (262,144 package bytes total). No complete package was re-downloaded from the CDN. This verifies delivery and byte samples, not a download-speed benchmark or a remote full-file SHA calculation.

The website now uses these versioned CDN files, with GitHub retained as the alternate download. See the [release distribution record](../releases/0.2.0-web-distribution.json). Current links are listed in [downloads.json](https://geod-global.laogao.xyz/downloads.json).

The package files remain the already accepted release bytes. This distribution change does not enable an updater or add a publisher signature.
