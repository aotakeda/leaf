# Contributing to Leaf

Thanks for your interest in improving Leaf.

## Getting started

1. Install the requirements listed in the [README](README.md#building-from-source).
2. Run `pnpm install`, then `pnpm tauri dev` to start the app with hot reload.

## Before opening a pull request

Please make sure these pass:

```bash
pnpm exec tsc --noEmit
cd src-tauri && cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

Keep pull requests focused on one change, and describe what you changed and how you tested it. For UI changes, a screenshot helps.

## Reporting bugs

Open an issue with:

- your operating system and version
- what you did, what you expected and what happened
- for book-specific problems, whether the book is EPUB or PDF and, if you can share it, a public-domain sample that shows the problem

## Ideas

Some directions that would be welcome:

- Windows and Linux testing
- Native-language definitions for more languages
- Highlights and notes
- Search inside a book

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE).
