# LexiCog

LexiCog is an AI-powered, agile shortcut-driven Tauri desktop app for frictionless language lookup, translation, OCR, and learning workflows.

It is designed for fast popup workflows:
- look up a selected word or phrase
- translate selected text
- capture part of the screen and run OCR
- revisit saved entries and review them over time

Welcome. If you are trying LexiCog for the first time, start the app, open the `Configure` tab, add at least one vendor API key, choose your models, and then test the default shortcuts. Beyond quick popups, LexiCog also keeps lexical entry history and includes a review flow, making it a practical tool for language learning and exploration.

## Features

- lexical entry lookup
- lexical entry history
- review sessions based on saved entries
- text translation
- OCR from selected screen region
- configurable global shortcuts
- English and Simplified Chinese UI

## Build Requirements

Before building, make sure you have:
- Node.js and npm
- Rust toolchain
- Tauri CLI

For macOS builds, you should also have:
- Xcode Command Line Tools
- Swift toolchain available from Xcode

## Install And Run

```bash
npm install
npm run tauri dev
```

To build a packaged app:

```bash
npm run tauri build
```

## First-Run Notes

- Add at least one vendor API key in `Configure`.
- Select the models you want to use for text, OCR, and text-to-speech.
- Set your target language if the default is not what you want.
- Shortcuts can be changed later in `Configure > Shortcuts`.

On macOS, some features may require system permissions:
- Accessibility: for selected-text capture and shortcut-based workflows
- Screen Recording: for OCR screenshot capture

## Default Shortcuts

- Lookup lexical entry: `Ctrl+Shift+L`
- Translate text: `Ctrl+Shift+T`
- OCR: `Ctrl+Shift+O`

These defaults can be changed from the app settings.

## Tech Stack

- Tauri
- Rust
- React 19
- TypeScript
- Vite

## Status

This repository is an active local project. Some workflows and UI details may continue to change.
