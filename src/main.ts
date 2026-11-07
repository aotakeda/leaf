import { open, ask } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { api, type Book } from "./api";
import { hideDefinition, showDefinition } from "./popup";
import type { Reader, Theme, ThemeChoice, TocItem } from "./types";

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;

const THEME_CHOICES: ThemeChoice[] = ["auto", "paper", "sepia", "night"];
const THEME_NAMES: Record<ThemeChoice, string> = { auto: "Auto (follows macOS)", paper: "Paper", sepia: "Sepia", night: "Night" };
const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");
const pref = <T>(key: string, fallback: T): T => {
  try {
    const v = localStorage.getItem(key);
    return v === null ? fallback : (JSON.parse(v) as T);
  } catch {
    return fallback;
  }
};
const setPref = (key: string, v: unknown) => {
  try {
    localStorage.setItem(key, JSON.stringify(v));
  } catch {}
};

let themeChoice: ThemeChoice = pref<ThemeChoice>("theme", "auto");
if (!THEME_CHOICES.includes(themeChoice)) themeChoice = "auto";
const resolveTheme = (): Theme => (themeChoice === "auto" ? (darkQuery.matches ? "night" : "paper") : themeChoice);
let theme: Theme = resolveTheme();
let fontScale: number = pref("fontScale", 100);
let reader: Reader | null = null;
let currentBook: Book | null = null;
let saveTimer = 0;
let chromeTimer = 0;
let lastPointer = "";
const CHROME_IDLE_MS = 2500;
const coverUrls = new Map<string, string>();

document.body.dataset.theme = theme;

function toast(msg: string) {
  const t = $("toast");
  t.textContent = msg;
  t.hidden = false;
  clearTimeout((t as any)._timer);
  (t as any)._timer = setTimeout(() => (t.hidden = true), 3500);
}

function applyTheme() {
  theme = resolveTheme();
  document.body.dataset.theme = theme;
  reader?.setTheme(theme);
  for (const id of ["theme-lib", "theme-btn"]) $(id).title = `Theme: ${THEME_NAMES[themeChoice]}`;
}

function cycleTheme() {
  themeChoice = THEME_CHOICES[(THEME_CHOICES.indexOf(themeChoice) + 1) % THEME_CHOICES.length];
  setPref("theme", themeChoice);
  applyTheme();
  toast(`Theme: ${THEME_NAMES[themeChoice]}`);
}

darkQuery.addEventListener("change", () => themeChoice === "auto" && applyTheme());

async function coverUrl(book: Book): Promise<string | null> {
  if (!book.hasCover) return null;
  if (coverUrls.has(book.id)) return coverUrls.get(book.id)!;
  try {
    const buf = await api.getCover(book.id);
    const url = URL.createObjectURL(new Blob([buf]));
    coverUrls.set(book.id, url);
    return url;
  } catch {
    return null;
  }
}

function hueFor(s: string) {
  let h = 0;
  for (const c of s) h = (h * 31 + c.charCodeAt(0)) % 360;
  return h;
}

async function renderLibrary() {
  const books = await api.listBooks();
  const grid = $("grid");
  grid.replaceChildren();
  $("empty").hidden = books.length > 0;
  grid.hidden = books.length === 0;

  for (const book of books) {
    const card = document.createElement("article");
    card.className = "card";
    card.tabIndex = 0;
    card.title = book.title;

    const cover = document.createElement("div");
    cover.className = "cover";
    const url = await coverUrl(book);
    if (url) {
      const img = document.createElement("img");
      img.src = url;
      img.alt = "";
      cover.appendChild(img);
    } else {
      cover.classList.add("placeholder");
      cover.style.setProperty("--hue", String(hueFor(book.title)));
      cover.innerHTML = `<span></span><small></small>`;
      cover.querySelector("span")!.textContent = book.title;
      cover.querySelector("small")!.textContent = book.author ?? "";
    }
    const badge = document.createElement("span");
    badge.className = "badge";
    badge.textContent = book.format.toUpperCase();
    cover.appendChild(badge);

    const remove = document.createElement("button");
    remove.className = "remove";
    remove.title = "Remove from library";
    remove.textContent = "×";
    remove.addEventListener("click", async (e) => {
      e.stopPropagation();
      const ok = await ask(`Remove “${book.title}” from your library? Your reading position will be lost.`, {
        title: "Remove book",
        kind: "warning",
        okLabel: "Remove",
      });
      if (!ok) return;
      await api.removeBook(book.id);
      renderLibrary();
    });
    cover.appendChild(remove);

    const meta = document.createElement("div");
    meta.className = "meta";
    const title = document.createElement("div");
    title.className = "title";
    title.textContent = book.title;
    const author = document.createElement("div");
    author.className = "author";
    author.textContent = book.author ?? "";
    const bar = document.createElement("div");
    bar.className = "mini-progress";
    bar.innerHTML = `<div style="width:${Math.round(book.progress * 100)}%"></div>`;
    const pct = document.createElement("div");
    pct.className = "pct";
    pct.textContent = book.lastOpenedAt ? `${Math.round(book.progress * 100)}% read` : "New";
    meta.append(title, author, bar, pct);

    card.append(cover, meta);
    card.addEventListener("click", () => openReader(book));
    card.addEventListener("keydown", (e) => e.key === "Enter" && openReader(book));
    grid.appendChild(card);
  }
}

async function enrichPdf(book: Book) {
  try {
    const data = await api.openBook(book.id);
    const { pdfCoverAndMeta } = await import("./pdf-reader");
    const { cover, title, author } = await pdfCoverAndMeta(data);
    if (cover) await api.saveCover(book.id, cover);
    if (title || author) await api.updateMeta(book.id, title, author);
  } catch (e) {
    console.warn("pdf enrich", e);
  }
}

async function importPaths(paths: string[]) {
  const supported = paths.filter((p) => /\.(epub|pdf)$/i.test(p));
  if (!supported.length) {
    toast("Only EPUB and PDF files are supported.");
    return;
  }
  try {
    const books = await api.importBooks(supported);
    for (const b of books) if (b.format === "pdf" && !b.hasCover) await enrichPdf(b);
    toast(books.length === 1 ? `Added “${books[0].title}”` : `Added ${books.length} books`);
  } catch (e) {
    toast(String(e));
  }
  renderLibrary();
}

async function pickFiles() {
  const picked = await open({
    multiple: true,
    filters: [{ name: "Books", extensions: ["epub", "pdf"] }],
  });
  if (!picked) return;
  importPaths(Array.isArray(picked) ? picked : [picked]);
}

function renderToc(items: TocItem[]) {
  const toc = $("toc");
  toc.replaceChildren();
  if (!items.length) {
    toc.innerHTML = `<p class="muted">No table of contents</p>`;
    return;
  }
  const build = (list: TocItem[], depth: number) => {
    for (const item of list) {
      const b = document.createElement("button");
      b.className = "toc-item";
      b.style.paddingLeft = `${16 + depth * 14}px`;
      b.textContent = item.label;
      b.addEventListener("click", () => {
        reader?.goTo(item.target);
        toc.hidden = true;
      });
      toc.appendChild(b);
      build(item.children, depth + 1);
    }
  };
  build(items, 0);
}

function setChromeHidden(hidden: boolean) {
  $("reader").classList.toggle("chrome-hidden", hidden);
  reader?.setCursorHidden?.(hidden);
}

function scheduleChromeHide() {
  clearTimeout(chromeTimer);
  chromeTimer = window.setTimeout(() => {
    const busy = !$("toc").hidden || document.querySelector(".reader-bar:hover, .reader-foot:hover");
    if (busy) scheduleChromeHide();
    else setChromeHidden(true);
  }, CHROME_IDLE_MS);
}

function showChrome(e?: MouseEvent) {
  if (e) {
    const pos = `${e.screenX},${e.screenY}`;
    if (pos === lastPointer) return;
    lastPointer = pos;
  }
  setChromeHidden(false);
  scheduleChromeHide();
}

function changeFontScale(delta: number) {
  fontScale = Math.min(200, Math.max(70, fontScale + delta));
  setPref("fontScale", fontScale);
  reader?.setFontScale?.(fontScale);
}

function onKey(e: KeyboardEvent) {
  if (!reader) return;
  if ((e.metaKey || e.ctrlKey) && ["=", "+", "-"].includes(e.key)) {
    e.preventDefault();
    if (reader.setFontScale) changeFontScale(e.key === "-" ? -10 : 10);
  } else if (["ArrowRight", "PageDown", " "].includes(e.key)) {
    e.preventDefault();
    hideDefinition();
    reader.next();
  } else if (["ArrowLeft", "PageUp"].includes(e.key)) {
    e.preventDefault();
    hideDefinition();
    reader.prev();
  } else if (e.key === "Escape" && !$("toc").hidden) {
    $("toc").hidden = true;
  }
}

async function openReader(book: Book) {
  currentBook = book;
  $("library").hidden = true;
  $("reader").hidden = false;
  showChrome();
  $("loading").hidden = false;
  $("toc").hidden = true;
  $("book-title").textContent = book.title;
  $("font-controls").hidden = book.format !== "epub";
  $("loc-label").textContent = "";
  $("pct-label").textContent = "";
  $<HTMLDivElement>("progress-fill").style.width = `${book.progress * 100}%`;

  const viewer = $("viewer");
  viewer.replaceChildren();
  viewer.classList.toggle("pdf", book.format === "pdf");

  const callbacks = {
    onRelocate: (location: string, progress: number, label: string, persist: boolean) => {
      $("loc-label").textContent = label;
      $("pct-label").textContent = `${Math.round(progress * 100)}%`;
      $<HTMLDivElement>("progress-fill").style.width = `${progress * 100}%`;
      if (!persist) return;
      clearTimeout(saveTimer);
      const id = book.id;
      saveTimer = window.setTimeout(() => api.saveProgress(id, location, progress).catch(console.warn), 300);
    },
    onSelect: (text: string, rect: DOMRect) => {
      const langs = [reader?.language(), book.format === "pdf" ? navigator.language : null].filter((l): l is string => !!l);
      showDefinition(text, rect, langs);
    },
    onClick: hideDefinition,
    onKey,
    onMouseMove: showChrome,
  };

  try {
    const data = await api.openBook(book.id);
    if (currentBook !== book) return;
    reader =
      book.format === "epub"
        ? await (await import("./epub-reader")).openEpub(viewer, book.id, data, book.location, theme, fontScale, callbacks)
        : await (await import("./pdf-reader")).openPdf(viewer, data, book.location, theme, callbacks);
    reader.toc().then(renderToc).catch(() => renderToc([]));
  } catch (e) {
    toast(`Couldn't open this book: ${e}`);
    closeReader();
  } finally {
    $("loading").hidden = true;
    if (currentBook === book) showChrome();
  }
}

function closeReader() {
  hideDefinition();
  reader?.destroy();
  reader = null;
  currentBook = null;
  clearTimeout(chromeTimer);
  $("reader").classList.remove("chrome-hidden");
  $("reader").hidden = true;
  $("library").hidden = false;
  renderLibrary();
}

$("add-books").addEventListener("click", pickFiles);
$("add-books-empty").addEventListener("click", pickFiles);
$("theme-lib").addEventListener("click", cycleTheme);
$("theme-btn").addEventListener("click", cycleTheme);
$("back").addEventListener("click", closeReader);
$("toc-btn").addEventListener("click", () => ($("toc").hidden = !$("toc").hidden));
$("prev").addEventListener("click", () => reader?.prev());
$("next").addEventListener("click", () => reader?.next());
$("font-dec").addEventListener("click", () => changeFontScale(-10));
$("font-inc").addEventListener("click", () => changeFontScale(10));
document.addEventListener("keydown", (e) => {
  if (!$("reader").hidden) onKey(e);
});
document.addEventListener("mousemove", (e) => {
  if (!$("reader").hidden) showChrome(e);
});

getCurrentWebview().onDragDropEvent((event) => {
  const p = event.payload;
  if (p.type === "enter" || p.type === "over") $("drop").hidden = false;
  else if (p.type === "leave") $("drop").hidden = true;
  else if (p.type === "drop") {
    $("drop").hidden = true;
    if (!$("reader").hidden) closeReader();
    importPaths(p.paths);
  }
});

applyTheme();
renderLibrary();
