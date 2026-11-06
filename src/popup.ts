import { api, type Definition } from "./api";

const popup = document.createElement("div");
popup.className = "def-popup";
popup.hidden = true;
document.body.appendChild(popup);

let requestId = 0;

function esc(s: string) {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

function place(rect: DOMRect) {
  const margin = 12;
  const w = popup.offsetWidth;
  const h = popup.offsetHeight;
  let left = rect.left + rect.width / 2 - w / 2;
  left = Math.max(margin, Math.min(left, window.innerWidth - w - margin));
  let top = rect.bottom + 10;
  if (top + h > window.innerHeight - margin) top = rect.top - h - 10;
  top = Math.max(margin, top);
  popup.style.left = `${left}px`;
  popup.style.top = `${top}px`;
}

function renderDefinition(d: Definition) {
  const meanings = d.meanings
    .map(
      (m) => `
      <section>
        <div class="pos">${esc(m.partOfSpeech)}</div>
        <ol>${m.definitions
          .map((s) => `<li>${esc(s.definition)}${s.example ? `<div class="ex">“${esc(s.example)}”</div>` : ""}</li>`)
          .join("")}</ol>
        ${m.synonyms.length ? `<div class="syn">Similar: ${m.synonyms.map(esc).join(", ")}</div>` : ""}
      </section>`,
    )
    .join("");
  return `
    <header>
      <h3>${esc(d.word)}</h3>
      ${d.phonetic ? `<span class="phon">${esc(d.phonetic)}</span>` : ""}
      ${d.audio ? `<button class="play" title="Pronounce" aria-label="Pronounce">🔊</button>` : ""}
    </header>
    <div class="body">${meanings}</div>
    <footer>Source: ${esc(d.source)} · CC BY-SA 4.0</footer>`;
}

export function showDefinition(text: string, rect: DOMRect, langs: string[]) {
  const word = text.replace(/\u00AD/g, "").trim().replace(/\s+/g, " ");
  if (!word || word.split(" ").length > 3 || !/\p{L}/u.test(word)) return;
  const id = ++requestId;
  popup.hidden = false;
  popup.innerHTML = `<header><h3>${esc(word)}</h3></header><div class="body muted">Looking up…</div>`;
  place(rect);

  api
    .define(word, langs)
    .then((d) => {
      if (id !== requestId) return;
      if (!d) {
        popup.innerHTML = `<header><h3>${esc(word)}</h3></header><div class="body muted">No definition found.</div>`;
      } else {
        popup.innerHTML = renderDefinition(d);
        const audio = d.audio;
        popup.querySelector<HTMLButtonElement>(".play")?.addEventListener("click", (e) => {
          e.stopPropagation();
          if (audio) new Audio(audio).play().catch(() => {});
        });
      }
      place(rect);
    })
    .catch((err) => {
      if (id !== requestId) return;
      const msg = String(err).includes("429") ? "The dictionary is busy. Try again in a moment." : String(err);
      popup.innerHTML = `<header><h3>${esc(word)}</h3></header><div class="body muted">${esc(msg)}</div>`;
      place(rect);
    });
}

export function hideDefinition() {
  requestId++;
  popup.hidden = true;
}

export function popupContains(node: Node | null) {
  return !!node && popup.contains(node);
}

document.addEventListener("mousedown", (e) => {
  if (!popup.hidden && !popupContains(e.target as Node)) hideDefinition();
});
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") hideDefinition();
});
