# Same-origin web bundle delivery

The account server may serve a built browser client from an explicitly
configured `CLUBSCAPE_WEB_ROOT`. Leaving it unset preserves account-only
behavior. This mechanism does not implement or approve game presentation.

The root must contain `clubscape-web.json`:

```json
{
  "schema_version": 1,
  "files": [
    {
      "url": "/",
      "path": "index.html",
      "sha256": "<SHA-256 of the built file>",
      "content_type": "text/html; charset=utf-8"
    }
  ]
}
```

The manifest is a deployment projection of built public files, not a second
source-asset authority. Each entry is loaded and hash-verified before startup.
Only these explicit same-origin routes are served; there is no directory
listing, arbitrary repository exposure, route-to-index fallback or private
configuration delivery. `/v1/*` and `/healthz` remain reserved. Dot paths,
traversal, encoded/ambiguous paths, symlinks and nonpublic file extensions fail
validation. Invalid configured content aborts startup instead of silently
falling back to an empty client.

Files are held as immutable shared bytes for the process lifetime. Limits are
2 MiB manifest, 20,000 entries, 64 MiB per file and 512 MiB total; content/assets
must remain region/content-streamed instead of one enormous public file.
Changing a built file requires a new manifest and server restart, not
unverified live replacement.

GET/HEAD and ETag revalidation are supported. Security headers restrict
resources/connections to the same origin, prohibit framing/object embeds,
enable cross-origin isolation and disable MIME sniffing. Scripts/styles
must be external built assets; this does not authorize unsafe inline script
or a browser sandbox bypass.

The real-listener integration test serves only a labeled infrastructure
fixture, verifies ETag/private-path/method behavior, and stops its service.
It is not evidence of an OSRS-faithful client, real UI signup or WebGPU
rendering. The eventual approved browser build must use this delivery path
and then run the separate M1 player-journey/presentation gates.
