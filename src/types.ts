export type Theme = "paper" | "sepia" | "night";
export type ThemeChoice = "auto" | Theme;

export interface TocItem {
  label: string;
  target: unknown;
  children: TocItem[];
}

export interface ReaderCallbacks {
  onRelocate: (location: string, progress: number, label: string, persist: boolean) => void;
  onSelect: (text: string, rect: DOMRect) => void;
  onClick: () => void;
  onKey: (e: KeyboardEvent) => void;
  onMouseMove: (e: MouseEvent) => void;
}

export interface Reader {
  next(): void;
  prev(): void;
  goTo(target: unknown): void;
  toc(): Promise<TocItem[]>;
  language(): string | null;
  setTheme(theme: Theme): void;
  setFontScale?(percent: number): void;
  setCursorHidden?(hidden: boolean): void;
  destroy(): void;
}
