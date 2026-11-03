/* Services: Mount Catalog, Image Decoders & Wasm Memory Staging */

const decodedRgbaCache = new Map();
const fetchedTextCache = new Map();
const contextXmlCache = new Map();
const textEncoder = new TextEncoder();
const WEB_TEXT_CAP = 768 * 1024;

export async function loadMountsCatalog() {
  try {
    const res = await fetch('./data/mounts.json');
    if (res.ok) {
      return await res.json();
    }
  } catch (err) {
    console.warn('Could not load ./data/mounts.json, using fallback', err);
  }
  return [];
}

export function mountCatalogIntoGuest(guest, mounts = []) {
  if (!guest || !guest.exports || !mounts) return;
  const exp = guest.exports;
  guest.mounts = mounts.slice();
  exp.app_mount_reset();

  for (let idx = 0; idx < mounts.length; idx++) {
    const item = mounts[idx];
    const nameBytes = textEncoder.encode(item.name);
    const ptr = exp.app_mount_name_ptr();
    new Uint8Array(exp.memory.buffer).set(nameBytes, ptr);
    const kindId = item.kind === 'image' ? 1 : 2;
    exp.app_mount_register(kindId, nameBytes.byteLength, item.bytes || 0);
  }
}

export async function decodeBlobToRgba(blob) {
  if (typeof createImageBitmap === 'function') {
    const bmp = await createImageBitmap(blob);
    const cv = document.createElement('canvas');
    cv.width = bmp.width;
    cv.height = bmp.height;
    const ctx = cv.getContext('2d', { willReadFrequently: true });
    ctx.drawImage(bmp, 0, 0);
    const imgData = ctx.getImageData(0, 0, bmp.width, bmp.height);
    return { rgba: imgData.data, width: bmp.width, height: bmp.height };
  }
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => {
      const cv = document.createElement('canvas');
      cv.width = img.width;
      cv.height = img.height;
      const ctx = cv.getContext('2d', { willReadFrequently: true });
      ctx.drawImage(img, 0, 0);
      const imgData = ctx.getImageData(0, 0, img.width, img.height);
      resolve({ rgba: imgData.data, width: img.width, height: img.height });
    };
    img.onerror = reject;
    img.src = URL.createObjectURL(blob);
  });
}

export async function fetchMountPayload(guest, item, idx) {
  if (!guest.exports || guest.disposed) return;
  const exp = guest.exports;

  // Stash context XML if applicable
  if (item.contextUrl) {
    let ctxText = contextXmlCache.get(item.contextUrl);
    if (!ctxText) {
      try {
        const cRes = await fetch(item.contextUrl);
        if (cRes.ok) {
          ctxText = await cRes.text();
          contextXmlCache.set(item.contextUrl, ctxText);
        }
      } catch (err) {
        console.warn(`Context fetch failed for ${item.name}:`, err);
      }
    }
    if (ctxText && exp.app_mount_context_ptr && exp.app_mount_commit_context) {
      const cBytes = textEncoder.encode(ctxText.slice(0, 64 * 1024));
      const cPtr = exp.app_mount_context_ptr();
      new Uint8Array(exp.memory.buffer).set(cBytes, cPtr);
      exp.app_mount_commit_context(idx, cBytes.byteLength);
    }
  }

  if (item.kind === 'image') {
    let decoded = decodedRgbaCache.get(item.url);
    if (!decoded) {
      const res = await fetch(item.url);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const blob = await res.blob();
      decoded = await decodeBlobToRgba(blob);
      decodedRgbaCache.set(item.url, decoded);
    }
    if (guest.disposed || !guest.exports) return;
    const maxBytes = guest.width * guest.height * 4;
    if (decoded.rgba.byteLength > maxBytes) return;
    const stagingPtr = exp.app_mount_staging_ptr();
    new Uint8Array(exp.memory.buffer).set(decoded.rgba, stagingPtr);
    const t0 = performance.now();
    exp.app_mount_commit_rgba(idx, decoded.width, decoded.height);
    const t1 = performance.now();
    if (guest.blitToCanvas) guest.blitToCanvas(t0, t1);
  } else {
    let textBytes = fetchedTextCache.get(item.url);
    if (!textBytes) {
      const res = await fetch(item.url);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const rawBuf = await res.arrayBuffer();
      textBytes = new Uint8Array(rawBuf).subarray(0, WEB_TEXT_CAP);
      fetchedTextCache.set(item.url, textBytes);
    }
    if (guest.disposed || !guest.exports) return;
    const textPtr = exp.app_mount_text_ptr();
    new Uint8Array(exp.memory.buffer).set(textBytes, textPtr);
    const t0 = performance.now();
    exp.app_mount_commit_text(idx, textBytes.byteLength);
    const t1 = performance.now();
    if (guest.blitToCanvas) guest.blitToCanvas(t0, t1);
  }
}
