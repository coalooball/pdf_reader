use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use pdf_extract::extract_text;
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// PDF file to read
    #[arg(value_name = "FILE")]
    file: PathBuf,
}

struct App {
    pages: Vec<String>,
    current_page: usize,
    scroll_offset: usize,
    should_quit: bool,
}

impl App {
    fn new(pdf_content: Vec<String>) -> Self {
        Self {
            pages: pdf_content,
            current_page: 0,
            scroll_offset: 0,
            should_quit: false,
        }
    }

    fn next_page(&mut self) {
        if self.current_page < self.pages.len().saturating_sub(1) {
            self.current_page += 1;
            self.scroll_offset = 0;
        }
    }

    fn prev_page(&mut self) {
        if self.current_page > 0 {
            self.current_page -= 1;
            self.scroll_offset = 0;
        }
    }

    fn scroll_down(&mut self) {
        self.scroll_offset += 1;
    }

    fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    fn quit(&mut self) {
        self.should_quit = true;
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Read and parse PDF
    let pages = read_pdf(&args.file)?;
    
    if pages.is_empty() {
        println!("PDF file is empty or could not be parsed.");
        return Ok(());
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let mut app = App::new(pages);
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

fn read_pdf(path: &PathBuf) -> Result<Vec<String>> {
    // Try pdf-extract first
    match extract_text(path) {
        Ok(text) => {
            // Split text into pages based on form feed characters or heuristics
            let pages = split_into_pages(&text);
            Ok(pages)
        }
        Err(e) => {
            // Fallback: try to read as plain text or return error
            Err(anyhow::anyhow!("Could not extract text from PDF: {}. The PDF might be image-based or use unsupported encoding.", e))
        }
    }
}

fn split_into_pages(text: &str) -> Vec<String> {
    // Try to split by form feed characters first
    if text.contains('\x0C') {
        return text.split('\x0C')
            .map(|page| format_pdf_content(page))
            .filter(|page| !page.trim().is_empty())
            .collect();
    }
    
    // If no form feed, split by estimated page breaks
    let lines: Vec<&str> = text.lines().collect();
    let mut pages = Vec::new();
    let lines_per_page = 50; // Estimate
    
    for chunk in lines.chunks(lines_per_page) {
        let page_content = chunk.join("\n");
        let formatted = format_pdf_content(&page_content);
        if !formatted.trim().is_empty() {
            pages.push(formatted);
        }
    }
    
    if pages.is_empty() {
        pages.push(format_pdf_content(text));
    }
    
    pages
}

fn format_pdf_content(content: &str) -> String {
    // Basic text processing to maintain some structure
    content
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => app.quit(),
                    KeyCode::Right | KeyCode::Char('n') => app.next_page(),
                    KeyCode::Left | KeyCode::Char('p') => app.prev_page(),
                    KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
                    KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
                    _ => {}
                }
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1), Constraint::Length(3)])
        .split(f.size());

    // Header
    let header = Paragraph::new(format!(
        "PDF Reader - Page {} of {} (Use ←/→ or p/n to navigate, ↑/↓ or j/k to scroll, q/Esc to quit)",
        app.current_page + 1,
        app.pages.len()
    ))
    .block(Block::default().borders(Borders::ALL))
    .style(Style::default().fg(Color::Cyan));
    f.render_widget(header, chunks[0]);

    // Content
    if let Some(content) = app.pages.get(app.current_page) {
        let lines: Vec<Line> = content
            .lines()
            .skip(app.scroll_offset)
            .map(|line| Line::from(vec![Span::raw(line)]))
            .collect();

        let text = Text::from(lines);
        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("Content"))
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::White));
        
        f.render_widget(paragraph, chunks[1]);
    }

    // Footer
    let footer = Paragraph::new("Navigation: ←/→ (pages) | ↑/↓ (scroll) | q (quit)")
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Yellow));
    f.render_widget(footer, chunks[2]);
}
