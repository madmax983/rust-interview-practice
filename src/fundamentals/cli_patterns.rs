//! # CLI Patterns with Ratatui
//!
//! Building production-quality terminal user interfaces with ratatui.
//! Master event handling, layouts, widgets, and TUI architecture.
//!
//! **Enable with:** `cargo build --features cli-patterns`

#![cfg(feature = "cli-patterns")]

use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph},
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

use std::io;

// ============================================================================
// Terminal Initialization and Cleanup
// ============================================================================

/// Initialize terminal for TUI.
fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

/// Restore terminal to normal state.
fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// Restore terminal on panic.
#[allow(dead_code)]
fn setup_panic_hook() {
    let original_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(panic_info);
    }));
}

// ============================================================================
// Basic App Structure and Event Loop
// ============================================================================

/// Application state.
struct App {
    running: bool,
    counter: u32,
    input: String,
}

impl App {
    fn new() -> Self {
        App {
            running: true,
            counter: 0,
            input: String::new(),
        }
    }

    fn on_tick(&mut self) {
        self.counter += 1;
    }

    fn quit(&mut self) {
        self.running = false;
    }
}

/// Basic event loop pattern.
#[allow(dead_code)]
fn run_basic_app() -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = App::new();

    while app.running {
        // Draw UI
        terminal.draw(|f| ui_basic(f, &app))?;

        // Handle events
        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()? {
                handle_key_basic(&mut app, key);
            }

        // Update state
        app.on_tick();
    }

    restore_terminal(&mut terminal)?;
    Ok(())
}

/// Render UI.
fn ui_basic(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(f.size());

    let block = Block::default().title("Counter").borders(Borders::ALL);
    let text = Paragraph::new(format!("Count: {}", app.counter))
        .block(block)
        .alignment(Alignment::Center);
    f.render_widget(text, chunks[0]);

    let input_block = Block::default().title("Input").borders(Borders::ALL);
    let input = Paragraph::new(app.input.as_str()).block(input_block);
    f.render_widget(input, chunks[1]);
}

/// Handle keyboard input.
fn handle_key_basic(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Char(c) => app.input.push(c),
        KeyCode::Backspace => {
            app.input.pop();
        }
        _ => {}
    }
}

// ============================================================================
// Component Architecture
// ============================================================================

/// Component trait for composable widgets.
trait Component {
    fn handle_key(&mut self, key: KeyEvent) -> bool;
    fn update(&mut self);
    fn render(&mut self, f: &mut Frame, area: ratatui::layout::Rect);
}

/// Stateful list component.
struct ListComponent {
    items: Vec<String>,
    state: ListState,
}

impl ListComponent {
    fn new(items: Vec<String>) -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        ListComponent { items, state }
    }

    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }
}

impl Component for ListComponent {
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Down => {
                self.next();
                true
            }
            KeyCode::Up => {
                self.previous();
                true
            }
            _ => false,
        }
    }

    fn update(&mut self) {
        // Update logic
    }

    fn render(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let items: Vec<ListItem> = self
            .items
            .iter()
            .map(|item| ListItem::new(item.as_str()))
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("List"))
            .highlight_style(Style::default().bg(Color::DarkGray))
            .highlight_symbol("> ");

        f.render_stateful_widget(list, area, &mut self.state);
    }
}

// ============================================================================
// State Management
// ============================================================================

/// Application with multiple views.
enum View {
    Main,
    Settings,
    Help,
}

struct MultiViewApp {
    running: bool,
    current_view: View,
    list: ListComponent,
    show_popup: bool,
}

impl MultiViewApp {
    fn new() -> Self {
        MultiViewApp {
            running: true,
            current_view: View::Main,
            list: ListComponent::new(vec![
                "Item 1".to_string(),
                "Item 2".to_string(),
                "Item 3".to_string(),
            ]),
            show_popup: false,
        }
    }

    fn switch_view(&mut self, view: View) {
        self.current_view = view;
    }

    fn toggle_popup(&mut self) {
        self.show_popup = !self.show_popup;
    }
}

// ============================================================================
// Event Handling with Dispatch
// ============================================================================

/// Event dispatch pattern.
enum AppEvent {
    Quit,
    SwitchView(View),
    TogglePopup,
    Input(char),
}

fn handle_events(app: &mut MultiViewApp, key: KeyEvent) -> Option<AppEvent> {
    match key.code {
        KeyCode::Char('q') => Some(AppEvent::Quit),
        KeyCode::Char('1') => Some(AppEvent::SwitchView(View::Main)),
        KeyCode::Char('2') => Some(AppEvent::SwitchView(View::Settings)),
        KeyCode::Char('?') => Some(AppEvent::SwitchView(View::Help)),
        KeyCode::Char('p') => Some(AppEvent::TogglePopup),
        KeyCode::Char(c) => Some(AppEvent::Input(c)),
        _ => {
            app.list.handle_key(key);
            None
        }
    }
}

fn process_event(app: &mut MultiViewApp, event: AppEvent) {
    match event {
        AppEvent::Quit => app.running = false,
        AppEvent::SwitchView(view) => app.switch_view(view),
        AppEvent::TogglePopup => app.toggle_popup(),
        AppEvent::Input(_) => {}
    }
}

// ============================================================================
// Layout Patterns
// ============================================================================

/// Responsive layout with multiple panes.
fn render_layout(f: &mut Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Body
            Constraint::Length(3), // Footer
        ])
        .split(f.size());

    // Header
    let header = Paragraph::new("My TUI App")
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);
    f.render_widget(header, chunks[0]);

    // Body with horizontal split
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(chunks[1]);

    let sidebar = Block::default().title("Sidebar").borders(Borders::ALL);
    f.render_widget(sidebar, body[0]);

    let main = Block::default().title("Main").borders(Borders::ALL);
    f.render_widget(main, body[1]);

    // Footer
    let footer = Paragraph::new("Press 'q' to quit").block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

/// Nested layouts for complex UIs.
fn render_nested_layout(f: &mut Frame) {
    let outer = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(f.size());

    // Left side with vertical split
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(outer[0]);

    let top_left = Block::default().title("Top Left").borders(Borders::ALL);
    f.render_widget(top_left, left[0]);

    let bottom_left = Block::default().title("Bottom Left").borders(Borders::ALL);
    f.render_widget(bottom_left, left[1]);

    // Right side
    let right = Block::default().title("Right").borders(Borders::ALL);
    f.render_widget(right, outer[1]);
}

// ============================================================================
// Common Widgets
// ============================================================================

/// Progress bar widget.
fn render_progress(f: &mut Frame, area: ratatui::layout::Rect, progress: u16) {
    let gauge = Gauge::default()
        .block(Block::default().title("Progress").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Green))
        .percent(progress);

    f.render_widget(gauge, area);
}

/// Styled text with spans.
fn render_styled_text(f: &mut Frame, area: ratatui::layout::Rect) {
    let spans = vec![
        Span::raw("Normal text, "),
        Span::styled("bold", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(", "),
        Span::styled("colored", Style::default().fg(Color::Green)),
        Span::raw(", "),
        Span::styled(
            "bold+colored",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
    ];

    let paragraph = Paragraph::new(Line::from(spans))
        .block(Block::default().title("Styled Text").borders(Borders::ALL));

    f.render_widget(paragraph, area);
}

// ============================================================================
// Styling and Themes
// ============================================================================

/// Color scheme definition.
struct ColorScheme {
    bg: Color,
    fg: Color,
    accent: Color,
    error: Color,
    success: Color,
}

impl ColorScheme {
    fn dark() -> Self {
        ColorScheme {
            bg: Color::Black,
            fg: Color::White,
            accent: Color::Cyan,
            error: Color::Red,
            success: Color::Green,
        }
    }

    fn light() -> Self {
        ColorScheme {
            bg: Color::White,
            fg: Color::Black,
            accent: Color::Blue,
            error: Color::Red,
            success: Color::Green,
        }
    }
}

/// Apply theme to block.
fn themed_block<'a>(title: &'a str, scheme: &ColorScheme) -> Block<'a> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(scheme.accent))
        .style(Style::default().bg(scheme.bg).fg(scheme.fg))
}

// ============================================================================
// Advanced Patterns
// ============================================================================

/// Async TUI with tokio.
#[cfg(feature = "async-parallel")]
#[allow(dead_code)]
async fn run_async_tui() -> io::Result<()> {
    use tokio::sync::mpsc;

    let mut terminal = setup_terminal()?;
    let (tx, mut rx) = mpsc::channel(100);

    // Spawn background task
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        let mut counter = 0;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            counter += 1;
            if tx_clone.send(format!("Update {counter}")).await.is_err() {
                break;
            }
        }
    });

    // Event loop
    loop {
        terminal.draw(|f| {
            let area = f.size();
            let block = Block::default()
                .title("Async Updates")
                .borders(Borders::ALL);
            f.render_widget(block, area);
        })?;

        tokio::select! {
            Some(msg) = rx.recv() => {
                println!("{msg}");
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                if event::poll(std::time::Duration::from_millis(0))?
                    && let Event::Key(key) = event::read()?
                        && key.code == KeyCode::Char('q') {
                            break;
                        }
            }
        }
    }

    restore_terminal(&mut terminal)?;
    Ok(())
}

/// Scrollable content.
struct ScrollableText {
    content: Vec<String>,
    scroll: usize,
}

impl ScrollableText {
    fn new(content: Vec<String>) -> Self {
        ScrollableText { content, scroll: 0 }
    }

    fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    fn scroll_down(&mut self) {
        if self.scroll < self.content.len().saturating_sub(1) {
            self.scroll += 1;
        }
    }

    fn render(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let visible_lines = (area.height as usize).saturating_sub(2);
        let end = (self.scroll + visible_lines).min(self.content.len());
        let visible = &self.content[self.scroll..end];

        let text: Vec<Line> = visible
            .iter()
            .map(|line| Line::from(line.clone()))
            .collect();

        let paragraph =
            Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Scrollable"));

        f.render_widget(paragraph, area);
    }
}

/// Search/filter UI.
struct SearchableList {
    items: Vec<String>,
    filtered: Vec<String>,
    query: String,
    state: ListState,
}

impl SearchableList {
    fn new(items: Vec<String>) -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        SearchableList {
            filtered: items.clone(),
            items,
            query: String::new(),
            state,
        }
    }

    fn update_filter(&mut self) {
        self.filtered = self
            .items
            .iter()
            .filter(|item| item.to_lowercase().contains(&self.query.to_lowercase()))
            .cloned()
            .collect();

        if !self.filtered.is_empty() {
            self.state.select(Some(0));
        }
    }

    fn handle_input(&mut self, c: char) {
        self.query.push(c);
        self.update_filter();
    }

    fn handle_backspace(&mut self) {
        self.query.pop();
        self.update_filter();
    }
}

// ============================================================================
// Testing TUI Apps
// ============================================================================

#[cfg(test)]
mod tui_tests {
    use super::*;

    #[test]
    fn test_app_creation() {
        let app = App::new();
        assert!(app.running);
        assert_eq!(app.counter, 0);
    }

    #[test]
    fn test_list_navigation() {
        let mut list = ListComponent::new(vec!["a".to_string(), "b".to_string()]);
        assert_eq!(list.state.selected(), Some(0));

        list.next();
        assert_eq!(list.state.selected(), Some(1));

        list.next();
        assert_eq!(list.state.selected(), Some(0)); // Wraps around
    }

    #[test]
    fn test_searchable_list() {
        let items = vec![
            "apple".to_string(),
            "banana".to_string(),
            "cherry".to_string(),
        ];
        let mut list = SearchableList::new(items);

        list.handle_input('a');
        assert_eq!(list.filtered.len(), 2); // apple, banana

        list.handle_input('p');
        assert_eq!(list.filtered.len(), 1); // apple

        list.handle_backspace();
        assert_eq!(list.filtered.len(), 2); // apple, banana
    }
}

// ============================================================================
// Production Patterns
// ============================================================================

/// Error handling in TUI context.
#[allow(dead_code)]
fn show_error(f: &mut Frame, error: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(5)
        .constraints([Constraint::Percentage(50)])
        .split(f.size());

    let error_text = Paragraph::new(error)
        .block(
            Block::default()
                .title("Error")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red)),
        )
        .style(Style::default().fg(Color::Red))
        .alignment(Alignment::Center);

    f.render_widget(error_text, chunks[0]);
}

/// Help screen pattern.
fn render_help(f: &mut Frame) {
    let help_text = vec![
        "Keyboard Shortcuts:",
        "",
        "q       - Quit",
        "?       - Show this help",
        "↑/↓     - Navigate list",
        "Enter   - Select item",
        "Esc     - Go back",
        "",
        "Press any key to close",
    ];

    let text: Vec<Line> = help_text.iter().map(|&s| Line::from(s)).collect();

    let paragraph = Paragraph::new(text)
        .block(Block::default().title("Help").borders(Borders::ALL))
        .alignment(Alignment::Left);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(5)
        .constraints([Constraint::Percentage(50)])
        .split(f.size());

    f.render_widget(paragraph, chunks[0]);
}

/// Configuration with clap.
#[derive(clap::Parser)]
#[command(name = "my-tui")]
#[command(about = "A terminal UI application", long_about = None)]
struct Cli {
    /// Color scheme (dark or light)
    #[arg(short, long, default_value = "dark")]
    theme: String,

    /// Enable debug mode
    #[arg(short, long)]
    debug: bool,
}

// ============================================================================
// Usage Notes
// ============================================================================

/// Running the TUI:
///
/// ```bash
/// cargo run --features cli-patterns
/// ```
///
/// Key ratatui patterns:
/// - Use Layout for responsive layouts
/// - Stateful widgets for interactive components
/// - Component trait for composition
/// - Event dispatch for complex apps
/// - Separate rendering from state
///
/// Best practices:
/// - Always restore terminal on exit
/// - Set panic hook to restore terminal
/// - Use raw_mode for input handling
/// - Poll events with timeout for responsive UI
/// - Keep render logic pure (no side effects)
/// - Test components in isolation
///
/// Resources:
/// - ratatui examples: https://github.com/ratatui-org/ratatui/tree/main/examples
/// - ratatui book: https://ratatui.rs/
#[allow(dead_code)]
const TUI_GUIDE: &str = "See module docs";
