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

#[derive(Clone, PartialEq)]
enum InputMode {
    Normal,
    PageJump,
    Search,
}

#[derive(Clone)]
struct SearchResult {
    page: usize,
    line: usize,
}

struct App {
    pages: Vec<String>,
    current_page: usize,
    scroll_offset: usize,
    should_quit: bool,
    input_mode: InputMode,
    input_buffer: String,
    search_query: String,
    search_results: Vec<SearchResult>,
    current_search_result: usize,
    status_message: String,
}

impl App {
    fn new(pdf_content: Vec<String>) -> Self {
        Self {
            pages: pdf_content,
            current_page: 0,
            scroll_offset: 0,
            should_quit: false,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            search_query: String::new(),
            search_results: Vec::new(),
            current_search_result: 0,
            status_message: String::new(),
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

    fn jump_to_page(&mut self, page_num: usize) {
        if page_num > 0 && page_num <= self.pages.len() {
            self.current_page = page_num - 1;
            self.scroll_offset = 0;
            self.status_message = format!("Jumped to page {}", page_num);
        } else {
            self.status_message = format!("Invalid page number: {}", page_num);
        }
    }

    fn start_page_jump(&mut self) {
        self.input_mode = InputMode::PageJump;
        self.input_buffer.clear();
        self.status_message = "Enter page number:".to_string();
    }

    fn start_search(&mut self) {
        self.input_mode = InputMode::Search;
        self.input_buffer.clear();
        self.status_message = "Enter search term:".to_string();
    }

    fn execute_search(&mut self) {
        if self.input_buffer.is_empty() {
            self.status_message = "Search query is empty".to_string();
            return;
        }

        self.search_query = self.input_buffer.clone();
        self.search_results.clear();
        
        let query_lower = self.search_query.to_lowercase();
        
        for (page_idx, page_content) in self.pages.iter().enumerate() {
            for (line_idx, line) in page_content.lines().enumerate() {
                if line.to_lowercase().contains(&query_lower) {
                    self.search_results.push(SearchResult {
                        page: page_idx,
                        line: line_idx,
                    });
                }
            }
        }

        if self.search_results.is_empty() {
            self.status_message = format!("No results found for '{}'", self.search_query);
        } else {
            self.current_search_result = 0;
            self.go_to_search_result();
        }
    }

    fn go_to_search_result(&mut self) {
        if let Some(result) = self.search_results.get(self.current_search_result) {
            self.current_page = result.page;
            self.scroll_offset = result.line.saturating_sub(5); // Show some context
            self.status_message = format!(
                "Result {} of {} for '{}'",
                self.current_search_result + 1,
                self.search_results.len(),
                self.search_query
            );
        }
    }

    fn next_search_result(&mut self) {
        if !self.search_results.is_empty() {
            self.current_search_result = (self.current_search_result + 1) % self.search_results.len();
            self.go_to_search_result();
        }
    }

    fn prev_search_result(&mut self) {
        if !self.search_results.is_empty() {
            self.current_search_result = if self.current_search_result == 0 {
                self.search_results.len() - 1
            } else {
                self.current_search_result - 1
            };
            self.go_to_search_result();
        }
    }

    fn cancel_input(&mut self) {
        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
        self.status_message.clear();
    }

    fn clear_search(&mut self) {
        self.search_query.clear();
        self.search_results.clear();
        self.current_search_result = 0;
        self.status_message = "Search cleared".to_string();
    }

    fn handle_input(&mut self, c: char) {
        match self.input_mode {
            InputMode::PageJump => {
                if c.is_ascii_digit() {
                    self.input_buffer.push(c);
                }
            }
            InputMode::Search => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
    }

    fn submit_input(&mut self) {
        match self.input_mode {
            InputMode::PageJump => {
                if let Ok(page_num) = self.input_buffer.parse::<usize>() {
                    self.jump_to_page(page_num);
                } else {
                    self.status_message = "Invalid page number".to_string();
                }
                self.input_mode = InputMode::Normal;
                self.input_buffer.clear();
            }
            InputMode::Search => {
                self.execute_search();
                self.input_mode = InputMode::Normal;
                self.input_buffer.clear();
            }
            _ => {}
        }
    }

    fn backspace(&mut self) {
        self.input_buffer.pop();
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
                match app.input_mode {
                    InputMode::Normal => {
                        match key.code {
                            KeyCode::Char('q') => app.quit(),
                            KeyCode::Esc => {
                                if !app.search_query.is_empty() {
                                    app.clear_search();
                                } else {
                                    app.quit();
                                }
                            },
                            KeyCode::Right | KeyCode::Char('n') => app.next_page(),
                            KeyCode::Left | KeyCode::Char('p') => app.prev_page(),
                            KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
                            KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
                            KeyCode::Char('g') => app.start_page_jump(),
                            KeyCode::Char('/') => app.start_search(),
                            KeyCode::Char('F') => app.next_search_result(),
                            KeyCode::Char('B') => app.prev_search_result(),
                            KeyCode::Home => {
                                app.current_page = 0;
                                app.scroll_offset = 0;
                            },
                            KeyCode::End => {
                                app.current_page = app.pages.len().saturating_sub(1);
                                app.scroll_offset = 0;
                            },
                            _ => {}
                        }
                    }
                    InputMode::PageJump | InputMode::Search => {
                        match key.code {
                            KeyCode::Enter => app.submit_input(),
                            KeyCode::Esc => app.cancel_input(),
                            KeyCode::Backspace => app.backspace(),
                            KeyCode::Char(c) => app.handle_input(c),
                            _ => {}
                        }
                    }
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
        .constraints([
            Constraint::Length(3), 
            Constraint::Min(1), 
            Constraint::Length(3),
            Constraint::Length(if app.input_mode != InputMode::Normal || !app.status_message.is_empty() { 3 } else { 0 })
        ])
        .split(f.size());

    // Header
    let header_text = if app.input_mode != InputMode::Normal {
        match app.input_mode {
            InputMode::PageJump => format!("Enter page number (1-{}): {}", app.pages.len(), app.input_buffer),
            InputMode::Search => format!("Search: {}", app.input_buffer),
            _ => format!("PDF Reader - Page {} of {}", app.current_page + 1, app.pages.len()),
        }
    } else {
        format!("PDF Reader - Page {} of {}", app.current_page + 1, app.pages.len())
    };
    
    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(if app.input_mode != InputMode::Normal { Color::Yellow } else { Color::Cyan }));
    f.render_widget(header, chunks[0]);

    // Content with search highlighting
    if let Some(content) = app.pages.get(app.current_page) {
        let search_query_lower = app.search_query.to_lowercase();
        
        let lines: Vec<Line> = content
            .lines()
            .skip(app.scroll_offset)
            .enumerate()
            .map(|(_line_idx, line)| {
                if !app.search_query.is_empty() && line.to_lowercase().contains(&search_query_lower) {
                    // Highlight search results
                    let mut spans = Vec::new();
                    let line_lower = line.to_lowercase();
                    let mut last_end = 0;
                    
                    while let Some(start) = line_lower[last_end..].find(&search_query_lower) {
                        let actual_start = last_end + start;
                        let actual_end = actual_start + app.search_query.len();
                        
                        // Add text before match
                        if actual_start > last_end {
                            spans.push(Span::raw(&line[last_end..actual_start]));
                        }
                        
                        // Add highlighted match
                        spans.push(Span::styled(
                            &line[actual_start..actual_end],
                            Style::default().fg(Color::Black).bg(Color::Yellow)
                        ));
                        
                        last_end = actual_end;
                    }
                    
                    // Add remaining text
                    if last_end < line.len() {
                        spans.push(Span::raw(&line[last_end..]));
                    }
                    
                    Line::from(spans)
                } else {
                    Line::from(vec![Span::raw(line)])
                }
            })
            .collect();

        let text = Text::from(lines);
        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("Content"))
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::White));
        
        f.render_widget(paragraph, chunks[1]);
    }

    // Controls footer
    let controls = if app.input_mode == InputMode::Normal {
        if !app.search_query.is_empty() {
            "g (goto page) | / (search) | F/B (next/prev result) | ←/→ (pages) | ↑/↓ (scroll) | Home/End | Esc (clear search) | q (quit)"
        } else {
            "g (goto page) | / (search) | ←/→ (pages) | ↑/↓ (scroll) | Home/End | q/Esc (quit)"
        }
    } else {
        "Enter (submit) | Esc (cancel) | Backspace (delete)"
    };
    
    let footer = Paragraph::new(controls)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Yellow));
    f.render_widget(footer, chunks[2]);

    // Status message
    if app.input_mode != InputMode::Normal || !app.status_message.is_empty() {
        let status = Paragraph::new(app.status_message.as_str())
            .block(Block::default().borders(Borders::ALL).title("Status"))
            .style(Style::default().fg(Color::Green));
        f.render_widget(status, chunks[3]);
    }
}
