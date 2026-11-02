/* Fullscreen overlay manager: enterFullscreen(guest), exitFullscreen(), toggleFullscreen(guest), updateFullscreenHud(guest), HUD auto-fade timers, and Esc key handling. */

const fullscreenBtn = document.getElementById('fullscreen-btn');
const fullscreenOverlay = document.getElementById('fullscreen-overlay');
const fullscreenContainer = document.getElementById('fullscreen-canvas-container');
const fullscreenHud = document.getElementById('fullscreen-hud');
const fsHudTitle = document.getElementById('fs-hud-title');
const fsHudPill = document.getElementById('fs-hud-pill');
const fsHudRes = document.getElementById('fs-hud-res');
const fsExitBtn = document.getElementById('fs-exit-btn');

let fullscreenGuest = null;
let hudFadeTimer = null;

// Callbacks for host decoupling
let _getActiveGuest = () => null;
let _setActiveId = (_id) => {};
let _setStatus = (_msg, _ok) => {};
let _selectionLabel = (_guest) => 'desktop';

export function configureFullscreenDeps(deps = {}) {
  if (deps.getActiveGuest) _getActiveGuest = deps.getActiveGuest;
  if (deps.setActiveId) _setActiveId = deps.setActiveId;
  if (deps.setStatus) _setStatus = deps.setStatus;
  if (deps.selectionLabel) _selectionLabel = deps.selectionLabel;
}

export function getFullscreenGuest() {
  return fullscreenGuest;
}

export function isFullscreen() {
  return Boolean(fullscreenGuest);
}

export function updateFullscreenHud(guest) {
  if (!guest || !fullscreenOverlay || fullscreenOverlay.hidden) return;
  if (fsHudTitle) {
    fsHudTitle.textContent = guest.name;
  }
  if (fsHudRes) {
    fsHudRes.textContent = `${guest.width}×${guest.height}`;
  }
  if (fsHudPill) {
    if (guest.mpSeat && guest.mpMatchId) {
      if (guest.mpGame === 'chess' && guest.exports?.app_chess_turn) {
        const turn = guest.exports.app_chess_turn();
        const white = guest.exports.app_chess_white_count ? guest.exports.app_chess_white_count() : 16;
        const blk = guest.exports.app_chess_black_count ? guest.exports.app_chess_black_count() : 16;
        const turnBadge = guest.mpSeat === turn ? '🟢 Your Turn' : '⏳ Opponent Turn';
        fsHudPill.textContent = `Match #${guest.mpMatchId} · Chess Seat P${guest.mpSeat} · Turn P${turn} (${white}W:${blk}B) · ${turnBadge}`;
      } else if (guest.mpGame === 'checkers' && guest.exports?.app_checkers_turn) {
        const turn = guest.exports.app_checkers_turn();
        const red = guest.exports.app_checkers_red_count ? guest.exports.app_checkers_red_count() : 12;
        const blk = guest.exports.app_checkers_black_count ? guest.exports.app_checkers_black_count() : 12;
        const turnBadge = guest.mpSeat === turn ? '🟢 Your Turn' : '⏳ Opponent Turn';
        fsHudPill.textContent = `Match #${guest.mpMatchId} · Checkers Seat P${guest.mpSeat} · Turn P${turn} (${red}R:${blk}B) · ${turnBadge}`;
      } else {
        const turn = guest.exports?.app_mines_turn ? guest.exports.app_mines_turn() : 1;
        const p1 = guest.exports?.app_mines_p1_score ? guest.exports.app_mines_p1_score() : 0;
        const p2 = guest.exports?.app_mines_p2_score ? guest.exports.app_mines_p2_score() : 0;
        const turnBadge = guest.mpSeat === turn ? '🟢 Your Turn' : '⏳ Opponent Turn';
        fsHudPill.textContent = `Match #${guest.mpMatchId} · Mines Seat P${guest.mpSeat} · Turn P${turn} (${p1}:${p2}) · ${turnBadge}`;
      }
    } else {
      fsHudPill.textContent = `${guest.buildInfo.shortLabel} · ${_selectionLabel(guest)}`;
    }
  }
}

export function showFullscreenHud() {
  if (!fullscreenHud) return;
  fullscreenHud.classList.remove('hud-hidden');
  if (hudFadeTimer !== null) {
    clearTimeout(hudFadeTimer);
  }
  hudFadeTimer = setTimeout(() => {
    if (fullscreenOverlay && !fullscreenOverlay.hidden) {
      fullscreenHud.classList.add('hud-hidden');
    }
  }, 3000);
}

export function enterFullscreen(guest) {
  if (!guest || !guest.canvas) return;
  if (fullscreenGuest && fullscreenGuest !== guest) {
    exitFullscreen();
  }
  fullscreenGuest = guest;
  _setActiveId(guest.id);

  // Move canvas into overlay container
  if (fullscreenContainer && guest.canvas) {
    fullscreenContainer.replaceChildren(guest.canvas);
  }
  if (fullscreenOverlay) {
    fullscreenOverlay.hidden = false;
    fullscreenOverlay.removeAttribute('aria-hidden');
  }

  updateFullscreenHud(guest);
  showFullscreenHud();
  guest.canvas.focus({ preventScroll: true });

  // Native fullscreen if available and not already in fullscreen
  if (!document.fullscreenElement) {
    try {
      const docEl = document.documentElement;
      if (docEl.requestFullscreen) {
        docEl.requestFullscreen().catch(() => {});
      } else if (docEl.webkitRequestFullscreen) {
        docEl.webkitRequestFullscreen();
      }
    } catch (_) {}
  }

  _setStatus(`${guest.name} · entered focused fullscreen overlay`, true);
}

export function exitFullscreen() {
  if (!fullscreenGuest) return;
  const guest = fullscreenGuest;
  fullscreenGuest = null;

  // Restore canvas to guest's panel .screen
  const screenEl = guest.panel?.querySelector('.screen');
  if (screenEl && guest.canvas) {
    screenEl.replaceChildren(guest.canvas);
  }

  if (fullscreenOverlay) {
    fullscreenOverlay.hidden = true;
    fullscreenOverlay.setAttribute('aria-hidden', 'true');
  }

  if (hudFadeTimer !== null) {
    clearTimeout(hudFadeTimer);
    hudFadeTimer = null;
  }

  // Exit native fullscreen if active
  if (document.fullscreenElement) {
    try {
      if (document.exitFullscreen) {
        document.exitFullscreen().catch(() => {});
      } else if (document.webkitExitFullscreen) {
        document.webkitExitFullscreen();
      }
    } catch (_) {}
  }

  if (guest.canvas) {
    guest.canvas.focus({ preventScroll: true });
  }
  _setStatus(`${guest.name} · exited fullscreen`, true);
}

export function toggleFullscreen(guest) {
  const target = guest || _getActiveGuest();
  if (!target) return;
  if (fullscreenGuest === target) {
    exitFullscreen();
  } else {
    enterFullscreen(target);
  }
}

export function initFullscreenListeners() {
  if (fullscreenBtn) {
    fullscreenBtn.addEventListener('click', () => {
      toggleFullscreen();
    });
  }

  if (fsExitBtn) {
    fsExitBtn.addEventListener('click', () => {
      exitFullscreen();
    });
  }

  if (fullscreenOverlay) {
    fullscreenOverlay.addEventListener('mousemove', () => {
      showFullscreenHud();
    });
    fullscreenOverlay.addEventListener('touchstart', () => {
      showFullscreenHud();
    }, { passive: true });
  }

  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && fullscreenGuest) {
      e.preventDefault();
      exitFullscreen();
    }
  });

  document.addEventListener('fullscreenchange', () => {
    if (!document.fullscreenElement && fullscreenGuest) {
      // User exited browser native fullscreen (e.g. via browser Esc)
      exitFullscreen();
    }
  });

  document.addEventListener('webkitfullscreenchange', () => {
    if (!document.webkitFullscreenElement && fullscreenGuest) {
      exitFullscreen();
    }
  });

  window.addEventListener('resize', () => {
    if (fullscreenGuest) {
      updateFullscreenHud(fullscreenGuest);
    }
  });

  window.addEventListener('orientationchange', () => {
    if (fullscreenGuest) {
      updateFullscreenHud(fullscreenGuest);
    }
  });
}
