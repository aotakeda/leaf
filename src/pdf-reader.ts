import * as pdfjs from "pdfjs-dist";
import "pdfjs-dist/web/pdf_viewer.css";
import type { PDFDocumentProxy, RenderTask } from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import type { Reader, ReaderCallbacks, Theme, TocItem } from "./types";

pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

export function loadPdf(data: ArrayBuffer): Promise<PDFDocumentProxy> {
  return pdfjs.getDocument({ data: new Uint8Array(data) }).promise;
}

export async function pdfCoverAndMeta(data: ArrayBuffer) {
  const doc = await loadPdf(data);
  try {
    const page = await doc.getPage(1);
    const base = page.getViewport({ scale: 1 });
    const viewport = page.getViewport({ scale: 360 / base.width });
    const canvas = document.createElement("canvas");
    canvas.width = Math.round(viewport.width);
    canvas.height = Math.round(viewport.height);
    await page.render({ canvasContext: canvas.getContext("2d")!, viewport }).promise;
    const blob = await new Promise<Blob | null>((r) => canvas.toBlob(r, "image/jpeg", 0.85));
    const cover = blob ? new Uint8Array(await blob.arrayBuffer()) : null;
    const meta = await doc.getMetadata().catch(() => null);
    const info = (meta?.info ?? {}) as Record<string, unknown>;
    const str = (v: unknown) => (typeof v === "string" && v.trim() ? v.trim() : null);
    let title = str(info.Title);
    if (title && /^(untitled|microsoft word|document\d*$)/i.test(title)) title = null;
    return { cover, title, author: str(info.Author) };
  } finally {
    doc.destroy();
  }
}

export async function openPdf(
  el: HTMLElement,
  data: ArrayBuffer,
  startAt: string | null,
  theme: Theme,
  cb: ReaderCallbacks,
): Promise<Reader> {
  const doc = await loadPdf(data);
  const total = doc.numPages;
  const meta = await doc.getMetadata().catch(() => null);
  const pdfLang = ((meta?.info as Record<string, unknown> | undefined)?.Language as string | undefined) || null;
  let current = Math.min(Math.max(parseInt(startAt ?? "1", 10) || 1, 1), total);
  let renderTask: RenderTask | null = null;
  let textLayer: pdfjs.TextLayer | null = null;
  let generation = 0;

  const stage = document.createElement("div");
  stage.className = "pdf-stage";
  stage.tabIndex = 0;
  el.appendChild(stage);

  const pageEl = document.createElement("div");
  pageEl.className = "pdf-page";
  const canvas = document.createElement("canvas");
  const textEl = document.createElement("div");
  textEl.className = "textLayer";
  pageEl.append(canvas, textEl);
  stage.appendChild(pageEl);

  const applyTheme = (t: Theme) => {
    stage.dataset.theme = t;
  };
  applyTheme(theme);

  const render = async () => {
    const gen = ++generation;
    renderTask?.cancel();
    textLayer?.cancel();
    const page = await doc.getPage(current);
    if (gen !== generation) return;

    const base = page.getViewport({ scale: 1 });
    const availW = stage.clientWidth - 48;
    const availH = stage.clientHeight - 32;
    const scale = Math.max(0.1, Math.min(availW / base.width, availH / base.height));
    const viewport = page.getViewport({ scale });
    const dpr = window.devicePixelRatio || 1;

    const off = document.createElement("canvas");
    off.width = Math.floor(viewport.width * dpr);
    off.height = Math.floor(viewport.height * dpr);
    renderTask = page.render({
      canvasContext: off.getContext("2d")!,
      viewport,
      transform: dpr !== 1 ? [dpr, 0, 0, dpr, 0, 0] : undefined,
    });
    try {
      await renderTask.promise;
    } catch {
      return;
    }
    if (gen !== generation) return;

    canvas.width = off.width;
    canvas.height = off.height;
    canvas.getContext("2d")!.drawImage(off, 0, 0);
    canvas.style.width = `${Math.floor(viewport.width)}px`;
    canvas.style.height = `${Math.floor(viewport.height)}px`;
    pageEl.style.width = canvas.style.width;
    pageEl.style.height = canvas.style.height;
    pageEl.style.setProperty("--scale-factor", String(scale));

    textEl.replaceChildren();
    textLayer = new pdfjs.TextLayer({
      textContentSource: page.streamTextContent(),
      container: textEl,
      viewport,
    });
    await textLayer.render().catch(() => {});

    cb.onRelocate(String(current), total > 1 ? (current - 1) / (total - 1) : 1, `Page ${current} of ${total}`, true);
  };

  const go = (n: number) => {
    const next = Math.min(Math.max(n, 1), total);
    if (next === current) return;
    current = next;
    window.getSelection()?.removeAllRanges();
    render();
  };

  const onMouseUp = () => {
    setTimeout(() => {
      const sel = window.getSelection();
      const text = sel?.toString() ?? "";
      if (!sel || !text.trim() || sel.rangeCount === 0) return;
      if (!stage.contains(sel.anchorNode)) return;
      cb.onSelect(text, sel.getRangeAt(0).getBoundingClientRect());
    }, 0);
  };
  const onMouseDown = () => cb.onClick();
  stage.addEventListener("mouseup", onMouseUp);
  stage.addEventListener("mousedown", onMouseDown);

  let resizeTimer = 0;
  const ro = new ResizeObserver(() => {
    clearTimeout(resizeTimer);
    resizeTimer = window.setTimeout(render, 120);
  });
  ro.observe(stage);

  await render();
  stage.focus();

  const resolveDest = async (dest: unknown): Promise<number | null> => {
    const explicit = typeof dest === "string" ? await doc.getDestination(dest) : (dest as any[]);
    if (!explicit?.length) return null;
    const ref = explicit[0];
    if (typeof ref === "number") return ref + 1;
    return (await doc.getPageIndex(ref)) + 1;
  };

  const mapOutline = (items: any[] | null): TocItem[] =>
    (items ?? []).map((i) => ({ label: i.title, target: i.dest, children: mapOutline(i.items) }));

  return {
    next: () => go(current + 1),
    prev: () => go(current - 1),
    goTo: async (target) => {
      const page = typeof target === "number" ? target : await resolveDest(target);
      if (page) go(page);
    },
    toc: async () => mapOutline(await doc.getOutline()),
    language: () => pdfLang,
    setTheme: applyTheme,
    destroy: () => {
      generation++;
      ro.disconnect();
      renderTask?.cancel();
      textLayer?.cancel();
      stage.removeEventListener("mouseup", onMouseUp);
      stage.removeEventListener("mousedown", onMouseDown);
      stage.remove();
      doc.destroy();
    },
  };
}
