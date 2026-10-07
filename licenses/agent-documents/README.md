PDF parsing uses lopdf 0.45.0 (MIT), pinned in the native manifest and lockfile. Its original license is in `lopdf-LICENSE`. Source: https://github.com/J-F-Liu/lopdf/tree/v0.45.0 .

PDF preview uses pdfjs-dist 6.4.299 (Apache-2.0), pinned in the root package and lockfile. The build copies its original LICENSE with the local runtime resources to `pdfjs/notices/LICENSE`. Source: https://github.com/mozilla/pdf.js .

Office content preview uses quick-xml 0.42.0 (MIT) and zip 8.6.0 (MIT), both already pinned in the workspace lockfile and now explicit desktop dependencies. Original notices are `quick-xml-0.42.0-LICENSE-MIT.md` and `zip-8.6.0-LICENSE`. Source: https://github.com/tafia/quick-xml and https://github.com/zip-rs/zip2 .

These notices cover the newly integrated document libraries; they are not a full release or installer license audit.

Audio validation uses Symphonia 0.6.1 (MPL-2.0), pinned with only MP3, WAV/PCM, FLAC, OGG and Vorbis enabled. The same MPL license covers the enabled Symphonia core, common, metadata, codec and format crates. Its unchanged upstream notice is `symphonia-0.6.1-LICENSE`; corresponding source is https://github.com/pdeljanov/Symphonia/tree/v0.6.1 . No library source was modified. New helper dependencies include extended 0.1.0 (MIT), regex-lite 0.1.9 and lazy_static 1.5.1 (MIT OR Apache-2.0); their original notices are retained alongside it. ffmpeg only authors our synthetic test fixtures and is not bundled or invoked by the application.

The worktree also retains the pinned 0.6.1 ISO/MP4 and Matroska demuxers under the same MPL-2.0 notice. Audio/video integration is deferred by user decision; retaining code and notices does not claim completed integration or release acceptance.
