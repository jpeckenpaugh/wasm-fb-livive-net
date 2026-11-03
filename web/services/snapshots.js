/* Services: Framebuffer & Dual Split View Snapshots */

export function fmtHex32(n) {
  return `0x${(n >>> 0).toString(16).padStart(8, '0')}`;
}

export function captureSnapshot({ guest, snapshots, snapList, openDrawer, updateHostStats, setStatus, alreadyRecordedInGuest = false, syncWallClock, selectionLabel }) {
  if (!guest || !guest.exports || !guest.canvas) return null;
  let hash;
  if (!alreadyRecordedInGuest) {
    if (syncWallClock) syncWallClock(guest);
    hash = guest.exports.app_record_snapshot() >>> 0;
    if (guest.render) guest.render();
  } else {
    hash = guest.exports.app_fb_hash() >>> 0;
  }
  guest.fbHash = hash;
  const hashHex = fmtHex32(hash);
  const seq = guest.exports.app_snapshot_count() >>> 0;
  const view = selectionLabel ? selectionLabel(guest) : 'View';
  const dataUrl = guest.canvas.toDataURL('image/png');

  const snap = {
    vm: guest.name,
    guestId: guest.id,
    buildId: guest.buildInfo.id,
    gitCommit: guest.buildInfo.gitCommit,
    seq,
    view,
    width: guest.width,
    height: guest.height,
    hash,
    hashHex,
    timestamp: new Date().toISOString(),
    dataUrl,
    filename: `${guest.name}-${guest.buildInfo.id}-snap-${seq}-${hashHex}.png`,
  };
  recordSnapshotCard({ snap, snapshots, snapList, openDrawer, updateHostStats });
  if (setStatus) setStatus(`${guest.name} · snapshot #${seq} captured (${hashHex})`, true);
  return snap;
}

export function captureSplitSnapshot({ g1, g2, label, snapshots, snapList, openDrawer, updateHostStats, setStatus }) {
  if (!g1 || !g2 || !g1.canvas || !g2.canvas) return null;
  const w1 = g1.width || 1280;
  const h1 = g1.height || 720;
  const w2 = g2.width || 1280;
  const h2 = g2.height || 720;
  const compW = w1 + w2;
  const compH = Math.max(h1, h2);

  const canvas = document.createElement('canvas');
  canvas.width = compW;
  canvas.height = compH;
  const ctx = canvas.getContext('2d');
  if (ctx) {
    ctx.fillStyle = '#0f172a';
    ctx.fillRect(0, 0, compW, compH);
    ctx.drawImage(g1.canvas, 0, 0, w1, h1);
    ctx.drawImage(g2.canvas, w1, 0, w2, h2);
  }

  const h1Hex = g1.exports?.app_fb_hash ? fmtHex32(g1.exports.app_fb_hash() >>> 0) : '0x00000000';
  const h2Hex = g2.exports?.app_fb_hash ? fmtHex32(g2.exports.app_fb_hash() >>> 0) : '0x00000000';
  const dataUrl = canvas.toDataURL('image/png');
  const snapSeq = snapshots.length + 1;

  const snap = {
    vm: `${g1.name} | ${g2.name}`,
    guestId: g1.id,
    buildId: `${g1.buildInfo.id}+${g2.buildInfo.id}`,
    gitCommit: g1.buildInfo.gitCommit,
    seq: snapSeq,
    view: label || 'Split Dual-Perspective',
    width: compW,
    height: compH,
    hash: 0,
    hashHex: `${h1Hex} | ${h2Hex}`,
    timestamp: new Date().toISOString(),
    dataUrl,
    filename: `split-${g1.name}-${g2.name}-snap-${snapSeq}.png`,
  };

  recordSnapshotCard({ snap, snapshots, snapList, openDrawer, updateHostStats });
  if (setStatus) setStatus(`Split View Dual snapshot captured (${h1Hex} | ${h2Hex})`, true);
  return snap;
}

export function recordSnapshotCard({ snap, snapshots, snapList, openDrawer, updateHostStats }) {
  snapshots.unshift(snap);
  if (openDrawer) openDrawer();
  if (updateHostStats) updateHostStats();

  if (snapList) {
    const card = document.createElement('div');
    card.className = 'snap-card';
    card.innerHTML = `
      <img src="${snap.dataUrl}" alt="Snapshot ${snap.seq} from ${snap.vm}">
      <div class="snap-meta">
        <span>${snap.vm} (${snap.buildId}) · #${snap.seq} (${snap.view})</span>
        <b>${snap.hashHex}</b>
      </div>
      <a class="snap-dl" href="${snap.dataUrl}" download="${snap.filename || `${snap.vm}-${snap.buildId}-snap-${snap.seq}-${snap.hashHex}.png`}">
        ↓ download png receipt
      </a>`;
    snapList.prepend(card);
  }
}
