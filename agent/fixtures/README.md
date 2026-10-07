`two-pages.pdf` is an original GeoD test fixture generated locally with lopdf 0.45.0 by the native PDF ingestion test. It contains two A4 pages with Helvetica text, “GeoD PDF check - page 1” and “GeoD PDF check - page 2”. It contains no downloaded data, credentials, scripts, links or forms.

The PDF attachment ID binds its original bytes, MIME type and two-page count. The SDK test checks those exact bytes against the native ingestion result; its upstream reply is controlled and is not proof of live model understanding.

`notes.docx`, `data.xlsx` and `slides.pptx` are original, owned Office test fixtures generated with `generate-office.py`. DOCX and XLSX use Python authoring libraries; PPTX is an authored minimal OOXML package with its slide order deliberately differing from file numbering. XLSX includes inline and shared strings, phonetic annotations and a stored formula. They contain no user documents or credentials.

`office-records.json` is the matching native ingestion and content-preview result. After deliberate fixture changes, regenerate the native records with the scoped `office_documents` test and its `GEOD_AGENT_OFFICE_FIXTURE` export under the repository's ignored verification directory. Tests consume the checked-in fixtures and records without depending on those Python authoring tools. Controlled SDK replies are transport evidence, not model reading evidence.
