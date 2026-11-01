/* Minimal local web-capture service for the v2 appliance.
 *
 * POST /api/captures            { url, viewport, options }
 * GET  /api/captures/:id        status + manifest + artifact URLs + context
 * GET  /api/captures/:id/artifacts/:name
 * DELETE /api/captures/:id      cancel/cleanup
 *
 * Uses the system Chrome through playwright-core. This is a developer tool:
 * keep it bound to localhost or put it behind an authenticated proxy.
 */

import http from 'node:http';
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright-core';

const PORT = Number(process.env.PORT || 8787);
const ROOT = path.dirname(fileURLToPath(import.meta.url));
const CAPTURE_DIR = path.join(ROOT, '.captures');
const MAX_TEXT = 256 * 1024;
const MAX_HTML = 1024 * 1024;
const MAX_RENDERED = 2 * 1024 * 1024;

const CHROME_CANDIDATES = [
  process.env.CHROME_PATH,
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome',
  '/usr/bin/google-chrome-stable',
  '/usr/bin/chromium',
].filter(Boolean);

const captures = new Map();
let browserPromise = null;
let activeJobs = 0;
const jobQueue = [];

async function findChrome() {
  for (const candidate of CHROME_CANDIDATES) {
    try {
      await fs.access(candidate);
      return candidate;
    } catch {
      /* keep looking */
    }
  }
  throw new Error('Chrome not found; set CHROME_PATH');
}

async function getBrowser() {
  if (!browserPromise) {
    browserPromise = (async () => {
      const executablePath = await findChrome();
      return chromium.launch({
        executablePath,
        headless: true,
        args: [
          '--disable-gpu',
          '--disable-dev-shm-usage',
          '--disable-extensions',
          '--hide-scrollbars',
          '--no-first-run',
        ],
      });
    })();
  }
  return browserPromise;
}

function enqueue(task) {
  return new Promise((resolve, reject) => {
    jobQueue.push({ task, resolve, reject });
    pump();
  });
}

function pump() {
  while (activeJobs < 2 && jobQueue.length) {
    const job = jobQueue.shift();
    activeJobs += 1;
    job.task()
      .then(job.resolve, job.reject)
      .finally(() => {
        activeJobs -= 1;
        pump();
      });
  }
}

function send(res, status, body, type = 'application/json') {
  const payload = type === 'application/json' ? JSON.stringify(body) : body;
  res.writeHead(status, {
    'content-type': type,
    'cache-control': 'no-store',
    'access-control-allow-origin': '*',
    'access-control-allow-methods': 'GET,POST,DELETE,OPTIONS',
    'access-control-allow-headers': 'content-type,authorization',
  });
  res.end(payload);
}

function redactUrl(value) {
  try {
    const url = new URL(value);
    if (url.search) url.search = '?redacted';
    return url.href;
  } catch {
    return String(value);
  }
}

function oneLine(value, max = 600) {
  return String(value == null ? '' : value)
    .replace(/[\r\n]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
    .slice(0, max);
}

function isBlockedHost(hostname) {
  const host = hostname.toLowerCase().replace(/\.$/, '');
  if (host === 'localhost' || host.endsWith('.localhost') || host.endsWith('.local')) return true;
  if (host === 'metadata.google.internal') return true;
  if (host === '::1' || host === '[::1]') return true;
  if (/^127\./.test(host) || /^10\./.test(host) || /^169\.254\./.test(host)) return true;
  if (/^192\.168\./.test(host)) return true;
  if (/^172\.(1[6-9]|2\d|3[0-1])\./.test(host)) return true;
  return false;
}

function validateUrl(raw) {
  const url = new URL(raw);
  if (url.protocol !== 'http:' && url.protocol !== 'https:') throw new Error('only http(s) URLs');
  if (isBlockedHost(url.hostname)) throw new Error('private/local hosts are blocked');
  return url;
}

function sha256(buf) {
  return crypto.createHash('sha256').update(buf).digest('hex');
}

async function writeArtifact(dir, name, data) {
  const buf = Buffer.isBuffer(data) ? data : Buffer.from(data);
  await fs.writeFile(path.join(dir, name), buf);
  return { path: name, bytes: buf.byteLength, sha256: sha256(buf) };
}

function redactHeaders(headers) {
  const blocked = new Set([
    'set-cookie',
    'cookie',
    'authorization',
    'proxy-authorization',
    'x-api-key',
  ]);
  const out = {};
  for (const [key, value] of Object.entries(headers || {})) {
    out[key] = blocked.has(key.toLowerCase()) ? '[redacted]' : value;
  }
  return out;
}

async function runCapture({ id, url, viewport, options, site }) {
  const dir = path.join(CAPTURE_DIR, id);
  await fs.mkdir(dir, { recursive: true });
  const startedAt = Date.now();
  const consoleLines = [];
  const network = [];
  let context;

  try {
    const browser = await getBrowser();
    context = await browser.newContext({
      viewport: { width: viewport.width, height: viewport.height },
      deviceScaleFactor: 1,
      colorScheme: options.colorScheme === 'light' ? 'light' : 'dark',
      reducedMotion: 'reduce',
      locale: options.locale || 'en-US',
      timezoneId: options.timezone || 'UTC',
      acceptDownloads: false,
    });
    const page = await context.newPage();
    page.on('console', (msg) => {
      if (consoleLines.length < 200) consoleLines.push(`[${msg.type()}] ${msg.text()}`);
    });
    page.on('pageerror', (err) => {
      if (consoleLines.length < 200) consoleLines.push(`[pageerror] ${err.message}`);
    });
    page.on('response', (res) => {
      if (network.length < 300) {
        network.push({ url: redactUrl(res.url()), status: res.status() });
      }
    });

    let response = null;
    try {
      response = await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 15_000 });
    } catch (error) {
      // A navigation timeout can still leave a usable page.
      consoleLines.push(`[navigation] ${error.message}`);
    }
    await page.waitForLoadState('networkidle', { timeout: 3_000 }).catch(() => {});
    await page.evaluate(() => (document.fonts ? document.fonts.ready : null)).catch(() => {});
    await page
      .addStyleTag({
        content:
          '::-webkit-scrollbar{display:none!important}html{scroll-behavior:auto!important}*{animation-duration:0s!important;transition-duration:0s!important}',
      })
      .catch(() => {});
    await page.waitForTimeout(400);

    const status = response ? response.status() : 0;
    const statusText = response ? response.statusText() : '';
    const finalUrl = response ? response.url() : url;
    const headers = response ? redactHeaders(await response.allHeaders()) : {};
    let rawBody;
    try {
      rawBody = response ? Buffer.from(await response.body()) : Buffer.alloc(0);
    } catch {
      rawBody = Buffer.from(await (await fetch(url)).arrayBuffer());
    }
    if (!rawBody.length) {
      try {
        rawBody = Buffer.from(await (await fetch(url)).arrayBuffer());
      } catch {
        rawBody = Buffer.alloc(0);
      }
    }

    const rendered = await page.content();
    const text = await page.evaluate(() => (document.body ? document.body.innerText : ''));
    const title = await page.title();
    const domNodes = await page.evaluate(
      () => document.getElementsByTagName('*').length,
    );
    const description = await page
      .evaluate(() => {
        const meta = document.querySelector('meta[name="description"]');
        return meta ? meta.getAttribute('content') || '' : '';
      })
      .catch(() => '');

    const screenshotPath = path.join(dir, 'page.png');
    await page.screenshot({
      path: screenshotPath,
      type: 'png',
      fullPage: false,
      animations: 'disabled',
      caret: 'hide',
    });

    const headersText = [
      `HTTP ${status} ${statusText}`.trim(),
      `FINAL_URL=${finalUrl}`,
      ...Object.entries(headers).map(([key, value]) => `${key}: ${value}`),
    ].join('\n');
    const consoleText = consoleLines.join('\n');
    const networkText = JSON.stringify(network, null, 2);

    const artifactList = [];
    artifactList.push(
      await writeArtifact(dir, 'index.html', rawBody.subarray(0, MAX_HTML)),
    );
    artifactList.push(
      await writeArtifact(dir, 'rendered.html', Buffer.from(rendered).subarray(0, MAX_RENDERED)),
    );
    artifactList.push({
      path: 'page.png',
      bytes: (await fs.stat(screenshotPath)).size,
      sha256: sha256(await fs.readFile(screenshotPath)),
    });
    artifactList.push(
      await writeArtifact(dir, 'text.txt', Buffer.from(text).subarray(0, MAX_TEXT)),
    );
    artifactList.push(await writeArtifact(dir, 'headers.txt', headersText));
    artifactList.push(await writeArtifact(dir, 'console.txt', consoleText));
    artifactList.push(await writeArtifact(dir, 'network.json', networkText));

    const capturedAt = new Date().toISOString();
    const siteId = site?.id || new URL(finalUrl).host;
    const summary = oneLine(description || text, 420) || 'No page summary extracted.';
    const contextText = [
      `SCOPE=web://${siteId}/`,
      'GENERATOR=wasm-fb.web-capture/1',
      `URL=${url}`,
      `FINAL_URL=${finalUrl}`,
      `STATUS=${status}`,
      `TITLE=${oneLine(title, 160)}`,
      `VIEWPORT=${viewport.width}x${viewport.height} @1x`,
      `CAPTURED=${capturedAt}`,
      `RENDER_MS=${Date.now() - startedAt}`,
      `DOM_NODES=${domNodes}`,
      `RESOURCES=${network.length}`,
      `ROLE=Captured web page`,
      `RUNTIME=Chromium headless`,
      `SUMMARY=${summary}`,
      `ARCHITECTURE=Read-only incognito capture; rendered DOM and screenshot are stored as artifacts.`,
    ]
      .join('\n')
      .slice(0, 4000);

    const manifest = {
      schema: 'wasm-fb.web-capture/1',
      captureId: id,
      site: { id: siteId, label: site?.label || siteId },
      request: { url, viewport, scrollY: 0, clip: null },
      result: {
        finalUrl,
        status,
        title,
        capturedAt,
        renderMs: Date.now() - startedAt,
        settled: true,
        colorScheme: options.colorScheme || 'dark',
        reducedMotion: true,
        blockedRequests: 0,
      },
      artifacts: artifactList,
    };
    const captureJson = await writeArtifact(dir, 'capture.json', JSON.stringify(manifest, null, 2));
    artifactList.push(captureJson);
    manifest.artifacts = artifactList;

    const artifacts = {};
    for (const artifact of artifactList) {
      const mime =
        artifact.path.endsWith('.png')
          ? 'image/png'
          : artifact.path.endsWith('.json')
            ? 'application/json'
            : artifact.path.endsWith('.html')
              ? 'text/html; charset=utf-8'
              : 'text/plain; charset=utf-8';
      artifacts[artifact.path] = {
        ...artifact,
        mime,
        url: `${baseUrl}/api/captures/${id}/artifacts/${encodeURIComponent(artifact.path)}`,
      };
    }

    captures.set(id, {
      state: 'ready',
      dir,
      manifest,
      context: contextText,
      artifacts,
      createdAt: Date.now(),
    });
  } catch (error) {
    captures.set(id, {
      state: 'error',
      error: oneLine(error.message || String(error), 300),
      createdAt: Date.now(),
    });
  } finally {
    if (context) await context.close().catch(() => {});
  }
}

let baseUrl = `http://127.0.0.1:${PORT}`;

async function readBody(req) {
  const chunks = [];
  let size = 0;
  for await (const chunk of req) {
    size += chunk.length;
    if (size > 64 * 1024) throw new Error('request body too large');
    chunks.push(chunk);
  }
  return JSON.parse(Buffer.concat(chunks).toString('utf8') || '{}');
}

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url, baseUrl);
  try {
    if (req.method === 'OPTIONS') return send(res, 204, {});
    if (req.method === 'GET' && url.pathname === '/health') {
      return send(res, 200, { ok: true, browser: Boolean(browserPromise), jobs: activeJobs });
    }
    if (req.method === 'POST' && url.pathname === '/api/captures') {
      const body = await readBody(req);
      const target = validateUrl(body.url);
      const width = Math.max(320, Math.min(1920, Number(body.viewport?.width) || 1280));
      const height = Math.max(200, Math.min(1080, Number(body.viewport?.height) || 688));
      const id = crypto.randomUUID().replace(/-/g, '').slice(0, 16);
      const viewport = { width, height, dpr: 1 };
      captures.set(id, { state: 'queued', createdAt: Date.now() });
      enqueue(() =>
        runCapture({
          id,
          url: target.href,
          viewport,
          options: body.options || {},
          site: body.site || { id: target.host, label: target.host },
        }),
      );
      return send(res, 202, {
        id,
        state: 'queued',
        statusUrl: `${baseUrl}/api/captures/${id}`,
      });
    }
    const statusMatch = url.pathname.match(/^\/api\/captures\/([a-f0-9]+)$/);
    if (req.method === 'GET' && statusMatch) {
      const capture = captures.get(statusMatch[1]);
      if (!capture) return send(res, 404, { state: 'error', error: 'not found' });
      if (capture.state === 'ready') {
        return send(res, 200, {
          state: 'ready',
          manifest: capture.manifest,
          context: capture.context,
          artifacts: capture.artifacts,
        });
      }
      return send(res, 200, {
        state: capture.state,
        error: capture.error,
      });
    }
    const artifactMatch = url.pathname.match(
      /^\/api\/captures\/([a-f0-9]+)\/artifacts\/([^/]+)$/,
    );
    if (req.method === 'GET' && artifactMatch) {
      const capture = captures.get(artifactMatch[1]);
      if (!capture || capture.state !== 'ready') {
        return send(res, 404, { state: 'error', error: 'not found' });
      }
      const name = decodeURIComponent(artifactMatch[2]);
      if (!Object.hasOwn(capture.artifacts, name)) {
        return send(res, 404, { state: 'error', error: 'artifact not found' });
      }
      const file = path.join(capture.dir, name);
      const data = await fs.readFile(file);
      const type = capture.artifacts[name].mime;
      return send(res, 200, data, type);
    }
    if (req.method === 'DELETE' && statusMatch) {
      const capture = captures.get(statusMatch[1]);
      if (capture?.dir) await fs.rm(capture.dir, { recursive: true, force: true }).catch(() => {});
      captures.delete(statusMatch[1]);
      return send(res, 200, { ok: true });
    }
    return send(res, 404, { error: 'not found' });
  } catch (error) {
    return send(res, 400, { state: 'error', error: oneLine(error.message || error) });
  }
});

await fs.mkdir(CAPTURE_DIR, { recursive: true });
setInterval(() => {
  const cutoff = Date.now() - 60 * 60 * 1000;
  for (const [id, capture] of captures) {
    if (capture.createdAt < cutoff) {
      if (capture.dir) fs.rm(capture.dir, { recursive: true, force: true }).catch(() => {});
      captures.delete(id);
    }
  }
}, 10 * 60 * 1000).unref();

server.listen(PORT, '127.0.0.1', () => {
  console.log(`web-capture listening on ${baseUrl}`);
});
