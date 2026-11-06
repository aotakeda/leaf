import ePub from "epubjs";
import type { NavItem } from "epubjs";
import { api } from "./api";
import type { Reader, ReaderCallbacks, Theme, TocItem } from "./types";

const THEME_COLORS: Record<Theme, { fg: string; bg: string; link: string; sel: string }> = {
  paper: { fg: "#262624", bg: "#F7F5F0", link: "#2D5DA8", sel: "rgba(45, 93, 168, 0.22)" },
  sepia: { fg: "#3B2F23", bg: "#F2E8D5", link: "#8A4A12", sel: "rgba(138, 74, 18, 0.2)" },
  night: { fg: "#D6D1C7", bg: "#1E1D1B", link: "#E0A96D", sel: "rgba(224, 169, 109, 0.28)" },
};

function mapToc(items: NavItem[]): TocItem[] {
  return items.map((i) => ({
    label: i.label.trim(),
    target: i.href,
    children: mapToc(i.subitems ?? []),
  }));
}

export async function openEpub(
  el: HTMLElement,
  bookId: string,
  data: ArrayBuffer,
  startAt: string | null,
  theme: Theme,
  fontScale: number,
  cb: ReaderCallbacks,
): Promise<Reader> {
  const book = ePub(data as unknown as string);
  (book.spine as any).hooks.content.register((doc: Document) => {
    doc.querySelectorAll("script, iframe, object, embed, frame, frameset, form, base").forEach((el) => el.remove());
    doc.querySelectorAll("*").forEach((el) => {
      for (const attr of [...el.attributes]) {
        const name = attr.name.toLowerCase();
        const value = attr.value.trim().toLowerCase();
        if (name.startsWith("on") || ((name === "href" || name.endsWith(":href") || name === "src") && value.startsWith("javascript:"))) {
          el.removeAttribute(attr.name);
        }
      }
    });
  });
  const rendition = book.renderTo(el, {
    width: "100%",
    height: "100%",
    flow: "paginated",
    spread: "auto",
    minSpreadWidth: 900,
    allowScriptedContent: true,
  } as any);

  const applyTheme = (t: Theme) => {
    const c = THEME_COLORS[t];
    rendition.themes.override("color", c.fg, true);
    rendition.themes.override("background", c.bg, true);
    rendition.themes.register(`leaf-${t}`, { a: { color: `${c.link} !important` }, "::selection": { background: c.sel } } as any);
    rendition.themes.select(`leaf-${t}`);
  };
  applyTheme(theme);
  rendition.themes.fontSize(`${fontScale}%`);
  rendition.themes.default({
    body: { "line-height": "1.6 !important", "-webkit-font-smoothing": "antialiased" },
  } as any);

  let settled = false;
  const report = (loc: any, persist = settled) => {
    if (!loc?.start) return;
    const cfi: string = loc.start.cfi;
    const hasLocations = (book.locations as any).length() > 0;
    const progress = hasLocations ? book.locations.percentageFromCfi(cfi) : loc.start.percentage ?? 0;
    const page = loc.start.displayed?.page;
    const total = loc.start.displayed?.total;
    const label = page && total ? `Page ${page} of ${total} in chapter` : "";
    cb.onRelocate(cfi, progress || 0, label, persist);
  };

  rendition.on("relocated", report);

  (rendition as any).hooks.content.register((contents: any) => {
    const doc: Document = contents.document;
    doc.addEventListener("mouseup", () => {
      setTimeout(() => {
        const sel = contents.window.getSelection() as Selection | null;
        const text = sel?.toString() ?? "";
        if (!sel || !text.trim() || sel.rangeCount === 0) return;
        const r = sel.getRangeAt(0).getBoundingClientRect();
        const frame = (contents.window.frameElement as HTMLElement).getBoundingClientRect();
        cb.onSelect(text, new DOMRect(frame.left + r.left, frame.top + r.top, r.width, r.height));
      }, 0);
    });
  });

  rendition.on("click", () => cb.onClick());
  rendition.on("keydown", (e: KeyboardEvent) => cb.onKey(e));

  await rendition.display(startAt ?? undefined).catch(() => rendition.display());
  report(rendition.currentLocation(), false);
  settled = true;

  book.ready
    .then(async () => {
      const cached = await api.getLocations(bookId);
      if (cached) {
        book.locations.load(cached);
      } else {
        await book.locations.generate(1200);
        await api.saveLocations(bookId, book.locations.save());
      }
      report(rendition.currentLocation(), false);
    })
    .catch((e) => console.warn("locations", e));

  return {
    next: () => rendition.next(),
    prev: () => rendition.prev(),
    goTo: (target) => rendition.display(target as string),
    toc: async () => mapToc((await book.loaded.navigation).toc),
    language: () => ((book as any).packaging?.metadata?.language as string | undefined) || null,
    setTheme: applyTheme,
    setFontScale: (p) => rendition.themes.fontSize(`${p}%`),
    destroy: () => {
      rendition.destroy();
      book.destroy();
    },
  };
}
