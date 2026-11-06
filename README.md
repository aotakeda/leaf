# Leaf

A fast, lightweight desktop ebook reader for EPUB and PDF. Leaf remembers where you stopped in every book and shows the meaning of any word you highlight.

Built with [Tauri 2](https://tauri.app): a Rust core for the library, file handling and dictionary lookups, and the system webview for rendering pages. The macOS app is about 6 MB.

## Features

- **Library** of EPUB and PDF books with covers. Add books with the button or by dropping files on the window.
- **Remembers your place** in each book, across restarts.
- **Word meanings**: double-click or highlight a word (or a phrase of up to three words) to see its definition in a popup.
  - English books: [Free Dictionary API](https://dictionaryapi.dev)
  - Portuguese books: definitions in Portuguese from [Wikcionário](https://pt.wiktionary.org), with plural forms resolved to the singular
  - Other languages: English meanings from [Wiktionary](https://en.wiktionary.org)
- **Paged reading** with keyboard (← → / Space), side click zones, a table of contents, adjustable text size and two-page spreads on wide windows.
- **Reading themes designed for comfort**: Auto (follows the system appearance), Paper, Sepia and Night.

### About the themes

The palettes follow readability research rather than taste:

- Dark text on a light background is read more accurately in normal lighting ([Piepenbrock et al., 2013](https://doi.org/10.1080/00140139.2013.790485)), so Paper is the daytime default.
- Pure black on pure white (21:1 contrast) causes glare. Body text in every theme sits between 10.7:1 and 13.9:1, above the WCAG AAA minimum of 7:1.
- Night uses a warm dark gray instead of black, with off-white text, which avoids the halation that light-on-black text causes for readers with astigmatism. Its amber accents cut the short-wavelength light linked to poorer sleep after evening e-reading ([Chang et al., 2015](https://doi.org/10.1073/pnas.1418490112)).
- Lines are capped at roughly 75 characters with 1.6 line spacing.

## Building from source

Requirements: [Rust](https://rustup.rs) 1.87 or newer, [Node.js](https://nodejs.org) 22 or newer, [pnpm](https://pnpm.io), and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your platform.

```bash
pnpm install
pnpm tauri dev
```

Build a release app:

```bash
pnpm tauri build
```

Run the tests:

```bash
cd src-tauri && cargo test
```

Leaf has been developed and tested on macOS. Tauri also targets Windows and Linux, but those builds have not been tested yet.

## Privacy

- Books, covers and reading positions stay on your computer, in the app data folder (`~/Library/Application Support/com.leaf.reader/` on macOS).
- When you look up a word, only that word is sent to the dictionary service. Leaf has no accounts, analytics or tracking.

## Project layout

| Path | What it does |
| --- | --- |
| `src-tauri/src/library.rs` | Library storage: importing, deduplicating, progress |
| `src-tauri/src/epub_meta.rs` | Reads EPUB title, author and cover |
| `src-tauri/src/dictionary.rs` | Routes lookups by language, caches results |
| `src-tauri/src/wiktionary.rs` | Wiktionary clients and wikitext parser |
| `src/epub-reader.ts`, `src/pdf-reader.ts` | Paginated readers built on epub.js and pdf.js |
| `src/popup.ts` | Definition popup |
| `src/main.ts` | Library and reader screens |

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Credits

- [epub.js](https://github.com/futurepress/epub.js) (BSD-2-Clause) and [pdf.js](https://github.com/mozilla/pdf.js) (Apache-2.0) render the books.
- Definitions come from [Wiktionary](https://www.wiktionary.org) contributors, directly or through the Free Dictionary API, under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/).

## License

[MIT](LICENSE) © 2026 Arthur Takeda
