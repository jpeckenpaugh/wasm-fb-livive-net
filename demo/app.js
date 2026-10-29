// demo module: no dependencies
const state = { count: 0, items: ["clock", "calc", "calendar"] };

export function render(el, items = state.items) {
  el.innerHTML = items.map((name) => `<li>${name}</li>`).join("");
  return items.length;
}

export function summarize(items = state.items) {
  // a deliberately long line so the viewer has something to scroll sideways: the total slot count across every registered application
  return items.reduce((total, name) => total + name.length, 0);
}

function tick(delta) {
  state.count += delta;
  if (state.count > 100) {
    state.count = 0;
    return "reset";
  }
  return "running";
}

const el = document.querySelector("#list");
if (el) {
  render(el);
  state.count += 1;
}

/* A few more statements so the file is taller than the
 * preview window and the vertical scrollbar appears. */
for (let i = 0; i < 12; i++) {
  tick(i % 3);
}

const status = tick(1);
console.log(status, state.count);
