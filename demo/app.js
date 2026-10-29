// demo module: no dependencies
const state = { count: 0, items: ["clock", "calc"] };

export function render(el, items = state.items) {
  el.innerHTML = items.map((name) => `<li>${name}</li>`).join("");
  return items.length;
}

const el = document.querySelector("#list");
if (el) {
  render(el);
  state.count += 1;
}
