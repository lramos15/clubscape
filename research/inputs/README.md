# Pinned source inputs

`osrs-240/master-index.dat` and `map-index.dat` are original JS5 source
containers from OpenRS2 cache 2695. Exact URLs, sizes and independently
established SHA-256 values are in `research/first-slice-sources.json`.

`just reference-fetch` retrieves only these allowlisted inputs, verifies them
before storing, and never silently replaces changed bytes. `just
reference-verify` works offline after retrieval.

These inputs originate from Jagex and are used under the owner's project-source
authorization in `prompt.md`. They are not RuneLite-licensed source code.
Preserve source identity and any supplied notices with subsequent imports.
No decoded content, source-rendered scene, approved capture pack, complete
inventory or ClubScape presentation is established by these two indexes.
