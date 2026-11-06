import { invoke } from "@tauri-apps/api/core";

export type Format = "epub" | "pdf";

export interface Book {
  id: string;
  title: string;
  author: string | null;
  format: Format;
  addedAt: number;
  lastOpenedAt: number | null;
  location: string | null;
  progress: number;
  hasCover: boolean;
}

export interface Definition {
  word: string;
  phonetic: string | null;
  audio: string | null;
  meanings: {
    partOfSpeech: string;
    definitions: { definition: string; example: string | null }[];
    synonyms: string[];
  }[];
  source: string;
}

export const api = {
  listBooks: () => invoke<Book[]>("list_books"),
  importBooks: (paths: string[]) => invoke<Book[]>("import_books", { paths }),
  openBook: (id: string) => invoke<ArrayBuffer>("open_book", { id }),
  getCover: (id: string) => invoke<ArrayBuffer>("get_cover", { id }),
  saveCover: (id: string, bytes: Uint8Array) =>
    invoke<void>("save_cover", bytes, { headers: { "book-id": id } }),
  saveProgress: (id: string, location: string, progress: number) =>
    invoke<void>("save_progress", { id, location, progress }),
  updateMeta: (id: string, title: string | null, author: string | null) =>
    invoke<void>("update_meta", { id, title, author }),
  getLocations: (id: string) => invoke<string | null>("get_locations", { id }),
  saveLocations: (id: string, data: string) => invoke<void>("save_locations", { id, data }),
  removeBook: (id: string) => invoke<void>("remove_book", { id }),
  define: (word: string, langs: string[]) => invoke<Definition | null>("define", { word, langs }),
};
