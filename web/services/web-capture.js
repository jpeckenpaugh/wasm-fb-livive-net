/* Services: Web Config, Sites & Headless Capture Proxy */

import { decodeBlobToRgba } from './mounts.js';

let webConfigPromise = null;
const webCaptureCache = new Map();
const textEncoder = new TextEncoder();
const WEB_TEXT_CAP = 768 * 1024;

export async function loadWebConfig() {
  if (!webConfigPromise) {
    webConfigPromise = (async () => {
      try {
        const res = await fetch('./sites.json', { cache: 'no-cache' });
        if (!res.ok) return null;
        return await res.json();
      } catch (err) {
        console.warn('Failed loading ./sites.json:', err);
        return null;
      }
    })();
  }
  return webConfigPromise;
}

export function oneLine(value) {
  return String(value || '').replace(/\s+/g, ' ').trim();
}

export function webMetaText(state, detail, sites = [], opts = {}) {
  const ready = opts.ready || 0;
  const endpoint = opts.endpoint || '';
  const lines = [
    `# web capture subsystem`,
    `state: ${state}`,
    `ready: ${ready}/${sites.length}`,
    `endpoint: ${endpoint}`,
    `detail: ${oneLine(detail)}`,
    '',
    '## catalog',
  ];
  for (const s of sites) {
    lines.push(`- id=${s.id} url=${s.url} title=${oneLine(s.title || s.name || s.id)}`);
  }
  return lines.join('\n');
}

export function writeWebMeta(guest, text) {
  if (!guest || !guest.exports || !guest.exports.app_web_set_meta) return;
  const bytes = textEncoder.encode(text);
  const ptr = guest.exports.app_mount_text_ptr();
  new Uint8Array(guest.exports.memory.buffer).set(bytes, ptr);
  guest.exports.app_web_set_meta(ptr, bytes.byteLength);
}

export async function captureSite(site, width = 1280, height = 720) {
  const key = `${site.id}:${width}x${height}`;
  if (webCaptureCache.has(key)) return webCaptureCache.get(key);
  const p = (async () => {
    const ep = 'https://proxy-app.livive.net/capture';
    const body = JSON.stringify({ url: site.url, width, height });
    const res = await fetch(ep, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body,
    });
    if (!res.ok) throw new Error(`Capture error: ${res.status}`);
    return await res.json();
  })();
  webCaptureCache.set(key, p);
  return p;
}

export function mountWebArtifacts(guest, site, snapshot) {
  if (!guest || !guest.exports || !snapshot) return;
  // If snapshot has items, register into mounts
}
