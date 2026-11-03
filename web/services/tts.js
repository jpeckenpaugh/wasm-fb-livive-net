/* Services: Text-To-Speech Synthesis */

let ttsUtterance = null;
let ttsGuest = null;

export function stopTts(guest) {
  if (typeof window === 'undefined' || !window.speechSynthesis) return;
  window.speechSynthesis.cancel();
  ttsUtterance = null;
  ttsGuest = null;
  if (guest && guest.panel) {
    const banner = guest.panel.querySelector('.tts-banner');
    if (banner) banner.hidden = true;
  }
}

export function handleTtsToggle(guest, onStatus) {
  if (typeof window === 'undefined' || !window.speechSynthesis) return;
  if (window.speechSynthesis.speaking || window.speechSynthesis.pending) {
    stopTts(guest);
    if (onStatus) onStatus(`${guest.name} · TTS speech halted`, true);
    return;
  }

  if (!guest.exports || !guest.exports.app_selected) return;
  const sel = guest.exports.app_selected();
  let text = '';
  if (sel === 3 && guest.exports.app_code_content_ptr) {
    const ptr = guest.exports.app_code_content_ptr();
    const len = guest.exports.app_code_content_len ? guest.exports.app_code_content_len() : 0;
    if (ptr && len > 0) {
      const slice = new Uint8Array(guest.exports.memory.buffer, ptr, Math.min(len, 4000));
      text = new TextDecoder().decode(slice);
    }
  }

  if (!text || text.trim().length === 0) {
    text = `${guest.name} running ${guest.buildInfo.shortLabel} with framebuffer active.`;
  }

  try {
    const utt = new SpeechSynthesisUtterance(text.slice(0, 1000));
    utt.rate = 1.0;
    utt.pitch = 1.0;
    utt.onend = () => {
      ttsUtterance = null;
      ttsGuest = null;
      if (guest && guest.panel) {
        const b = guest.panel.querySelector('.tts-banner');
        if (b) b.hidden = true;
      }
    };
    utt.onerror = () => {
      ttsUtterance = null;
      ttsGuest = null;
    };
    ttsUtterance = utt;
    ttsGuest = guest;
    window.speechSynthesis.speak(utt);
    if (guest.panel) {
      const b = guest.panel.querySelector('.tts-banner');
      if (b) b.hidden = false;
    }
    if (onStatus) onStatus(`${guest.name} · TTS reading text aloud...`, true);
  } catch (err) {
    console.warn('TTS playback error:', err);
  }
}
