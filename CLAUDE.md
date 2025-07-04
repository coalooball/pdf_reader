# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust-based command-line PDF reader that displays PDF content in the terminal with formatting, similar to viewing PDFs in a web browser. The application uses a terminal UI (TUI) for navigation and display.

## Development Commands

### Build and Run
```bash
cargo build --release          # Build optimized binary
cargo run -- <pdf_file>        # Run with PDF file
cargo run -- --help           # Show help and usage
```

### Development
```bash
cargo check                    # Check compilation
cargo clippy                   # Run linter
cargo fmt                      # Format code
```

## Dependencies

- **lopdf**: PDF parsing and text extraction
- **ratatui**: Terminal UI framework for formatted display
- **crossterm**: Cross-platform terminal manipulation
- **clap**: Command line argument parsing
- **anyhow**: Error handling
- **tokio**: Async runtime (if needed for future features)

## Architecture

### Core Components

1. **main.rs**: Entry point with CLI argument parsing and terminal setup
2. **App struct**: Application state management (current page, scroll position)
3. **PDF Reading**: Uses lopdf to extract text from PDF files
4. **Terminal UI**: Uses ratatui for formatted display with navigation

### Key Features

- Page-by-page PDF navigation (←/→ or p/n keys)
- Vertical scrolling within pages (↑/↓ or j/k keys)
- **Page jumping**: Jump to specific page number (g key)
- **Text search**: Find and highlight text within PDF (/ key)
- **Search navigation**: Navigate between search results (F/B keys)
- Formatted text display with borders and headers
- Text wrapping to fit terminal width
- Colored UI elements (header, footer, content)
- Search result highlighting with yellow background

### UI Layout

- Header: Page counter, navigation instructions, or input prompt
- Content area: Main PDF text display with scrolling and search highlighting
- Footer: Context-sensitive help and navigation controls
- Status bar: Shows search results, page jump confirmations, and error messages

## Usage

```bash
./pdf_reader document.pdf
```

### Navigation Controls

#### Basic Navigation
- `←`/`→` or `p`/`n`: Previous/Next page
- `↑`/`↓` or `j`/`k`: Scroll up/down within page
- `Home`: Go to first page
- `End`: Go to last page
- `q` or `Esc`: Quit application

#### Page Jumping
- `g`: Enter page jump mode
- Enter page number and press `Enter`
- `Esc` to cancel page jump

#### Search Features
- `/`: Enter search mode
- Type search term and press `Enter`
- `F`: Go to next search result
- `B`: Go to previous search result
- Search terms are highlighted in yellow
- `Esc` to cancel search input

#### Input Modes
- **Normal mode**: Standard navigation
- **Page jump mode**: Enter page number
- **Search mode**: Enter search query

## Notes

- The application maintains text formatting while avoiding plain text conversion
- Currently supports text extraction; future versions could add image/table support
- Terminal size affects text wrapping and display quality