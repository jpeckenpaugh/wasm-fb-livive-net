# `web://` Captured Web Snapshots

`Files → web://<site>/` is a read-only virtual folder containing a captured web
page. The appliance never runs a browser, HTML parser, or network stack for this
feature. A small host-side capture service loads the page in an incognito
Chromium context and returns ordinary artifacts; the host stages text and
RGBA8888 pixels through the existing mount ABI.

```text
Files
  ~/
    wallpapers/
    samples/
    gh://
      jpeckenpaugh/
        mines-sm/
        mines-livive-net/
        gm-livive-net/
    web://
      livive.net/
      mines.livive.net/
      mines-sm.livive.net/
      gm.livive.net/
        # every site folder contains:
        index.html       raw HTTP response body
        rendered.html    post-load serialized DOM
        page.png         viewport screenshot
        text.txt         extracted visible text
        headers.txt      status, final URL, selected headers
        console.txt      console/page errors
        network.json     resource requests
        capture.json     capture manifest
```

`rendered.html` is source code. The guest shows it with the HTML syntax
highlighter. The visual page is `page.png`; pin it with `Enter` or
`Files → Set as Wallpaper` to see it full-screen on the Desktop.

Select any web artifact and use **`[ ↗ Live ]`**, `Files → Open in Browser`,
or `O` to open the live site in a new browser tab. The live tab uses the
browser's normal profile, not the incognito capture context. The same button
appears as **`[ ↗ GitHub ]`** inside `gh://` and opens the repo or file on
GitHub.

## Run it locally

The core appliance is static. The capture service is optional.

```bash
# terminal 1: capture service (uses installed Google Chrome)
make web-capture

# terminal 2: appliance
make serve
# open http://localhost:8080/v2/
```

The default `web/v2/sites.json` points at
`http://127.0.0.1:8787/api/captures`. Change that file for a different capture
service or sites.

For the Docker build:

```bash
docker compose up --build serve    # appliance at http://localhost:8080/v2/
# run the capture service separately if you want web:// snapshots
```

## Configuration

`web/v2/sites.json`:

```json
{
  "captureEndpoint": "http://127.0.0.1:8787/api/captures",
  "sites": [
    {
      "id": "livive.net",
      "label": "livive.net",
      "url": "https://livive.net/",
      "maxAgeMinutes": 15
    },
    {
      "id": "mines.livive.net",
      "label": "mines.livive.net",
      "url": "https://mines.livive.net/",
      "maxAgeMinutes": 15
    },
    {
      "id": "mines-sm.livive.net",
      "label": "mines-sm.livive.net",
      "url": "https://mines-sm.livive.net/",
      "maxAgeMinutes": 15
    },
    {
      "id": "gm.livive.net",
      "label": "gm.livive.net",
      "url": "https://gm.livive.net/",
      "maxAgeMinutes": 15
    }
  ]
}
```

- `id` becomes the virtual path segment: `web/<id>/...`.
- `label` is the display name in the Files row.
- `maxAgeMinutes` controls reuse of the in-page capture cache.
- If `captureEndpoint` is missing, the `web://` row is hidden and the feature is
  disabled.

`F5` or `Files → Refresh Web` re-captures all configured sites. The Files pane
shows `capturing`, `ready`, `stale`, or `error` in the folder summary.

## Capture service

`services/web-capture` is a local Node + `playwright-core` service. It uses the
system Chrome (`CHROME_PATH` overrides discovery).

API:

```text
POST   /api/captures                 { url, site, viewport, options }
GET    /api/captures/:id             state + manifest + context + artifact URLs
GET    /api/captures/:id/artifacts/:name
DELETE /api/captures/:id
GET    /health
```

Each capture uses a fresh browser context with downloads, persisted cookies, and
credentials disabled. Screenshots are viewport-only at the requested size,
animations are disabled, and scrollbars are hidden.

## Limits

- Raw source and rendered DOM are capped by the host at 768 KiB; the Context
  pane reports `BYTES` and `TRUNCATED`.
- The screenshot is staged at its native dimensions (at most the guest
  framebuffer).
- `index.html` (service side) 1 MiB, `rendered.html` 2 MiB, `text.txt` 256 KiB,
  total bundle 16 MiB, capture timeout 15 s.
- `MAX_MOUNT_FILES` is 128 and `MAX_NAME_LEN` is 64 to fit the vendored
  repositories and web artifacts.

## Security

This is a local developer tool. Do not expose the capture service as an open
screenshot API.

- Bind it to localhost or put it behind authenticated same-origin `/api/`.
- Private/loopback hostnames are rejected, but subresource egress is **not** yet
  filtered at the network layer. For a public deployment, run Chromium in a
  sandboxed container/VM behind an egress proxy that blocks private and metadata
  ranges.
- Incognito isolates cookies, not the network or browser sandbox.
- Headers redact `set-cookie`, `cookie`, `authorization`, and API keys;
  `network.json` redacts query strings.
- Captured HTML/DOM is untrusted text and is never executed by the guest.

## Known limitations

- The guest has no HTML/CSS renderer; `rendered.html` is source.
- `rendered.html` does not include shadow DOM internals, canvas pixels, iframe
  inner DOM, form values, or JavaScript heap state.
- Cookie banners, paywalls, and bot challenges are captured as a first-time
  visitor sees them. The capture service does not attempt evasion.
- The default manifest exposes four configured sites; `web://` nests them under
the virtual root.
- Full-file text is staged up to 768 KiB; larger artifacts show a truncated
  window and `TRUNCATED=true` in Context.
