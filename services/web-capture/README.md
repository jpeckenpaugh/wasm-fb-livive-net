# web-capture

Local capture service for the `web://` virtual folder in the v2 appliance.

```bash
npm install
npm start
# http://127.0.0.1:8787/health
```

Uses the system Chrome through `playwright-core`. Set `CHROME_PATH` if Chrome is
not in a standard location. Set `PORT` to change the listening port.

The service is intentionally local-only. See `docs/web-capture.md` for the
artifact contract, limits, and security notes.
