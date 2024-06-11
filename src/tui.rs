/// Based on the table example from ratatui
use crate::networks::Network;
use crate::types::BlockMessage;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::layout::Direction;
use ratatui::prelude::Alignment;
use ratatui::widgets::Borders;
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Layout, Margin, Rect},
    style::{self, Color, Style, Stylize},
    terminal::{Frame, Terminal},
    text::{Line, Text},
    widgets::{Block, BorderType, Cell, HighlightSpacing, Paragraph, Row, Table, TableState},
};
use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};
use style::palette::tailwind;
use tokio::sync::mpsc::Receiver;
use tokio::time::{self, Duration};
use unicode_width::UnicodeWidthStr;

const PALETTE: tailwind::Palette = tailwind::BLUE;
const INFO_TEXT: &str = "(Esc) quit | (↑) move up | (↓) move down";

#[derive(Clone, Default)]
struct Metrics {
    pub gps: f64,
    pub tps: f64,
    pub dps: f64,
}

impl Metrics {
    fn cells(&self) -> [String; 3] {
        [
            format!("{:.2}", self.tps),
            format!("{:.2}", self.gps / 1024.0 / 1024.0),
            format!("{:.2}", self.dps / 1024.0),
        ]
    }
}

#[derive(Clone, Default)]
struct NetworkMetrics {
    name: String,
    block: u64,
    metrics: Metrics,
}

impl NetworkMetrics {
    fn cells(&self) -> [String; 5] {
        [
            self.name.clone(),
            self.block.to_string(),
            format!("{:.2}", self.metrics.tps),
            format!("{:.2}", self.metrics.gps / 1024.0 / 1024.0),
            format!("{:.2}", self.metrics.dps / 1024.0),
        ]
    }
}

struct App {
    longest_name: u16,
    items: Vec<NetworkMetrics>,
    latest: HashMap<String, Metrics>,
    totals: Metrics,
    state: TableState,
    colors: TableColors,
}

impl App {
    fn new(networks: Vec<Network>) -> Self {
        let items = networks
            .iter()
            .map(|n| NetworkMetrics { name: n.label.clone(), ..Default::default() })
            .collect::<Vec<_>>();
        let longest_name =
            networks.iter().map(|n| UnicodeWidthStr::width(n.label.as_str())).max().unwrap_or(0)
                as u16;
        Self {
            state: TableState::default().with_selected(0),
            longest_name,
            colors: TableColors::new(),
            items,
            latest: HashMap::new(),
            totals: Metrics::default(),
        }
    }

    pub fn next(&mut self) {
        let i =
            self.state.selected().map_or(0, |i| (i + 1).min(self.items.len().saturating_sub(1)));
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = self.state.selected().map_or(0, |i| i.saturating_sub(1));
        self.state.select(Some(i));
    }

    pub fn update(&mut self, message: BlockMessage) {
        match message {
            BlockMessage::UpdateNetwork(nm) => {
                let metrics = Metrics { tps: nm.tps, gps: nm.gps, dps: nm.dps };
                self.latest.insert(nm.network.clone(), metrics.clone());
                if let Some(data) = self.items.iter_mut().find(|d| d.name == nm.network) {
                    data.block = nm.block;
                    data.metrics = metrics.clone();
                }
                self.items.sort_by(|a, b| {
                    b.metrics.tps.partial_cmp(&a.metrics.tps).unwrap_or(std::cmp::Ordering::Equal)
                });
                self.totals = self.latest.values().fold(Metrics::default(), |mut acc, metrics| {
                    acc.gps += metrics.gps;
                    acc.tps += metrics.tps;
                    acc.dps += metrics.dps;
                    acc
                });
            }
            BlockMessage::Log(_) => {}
        }
    }
}

pub async fn tui(networks: Vec<Network>, mut rx: Receiver<BlockMessage>) -> eyre::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = Arc::new(Mutex::new(App::new(networks)));
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            let mut app = app_clone.lock().unwrap();
            app.update(message);
        }
    });

    let res = run_app(&mut terminal, app).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: Arc<Mutex<App>>) -> io::Result<()> {
    let mut interval = time::interval(Duration::from_millis(25));

    loop {
        terminal.draw(|f| {
            let mut app = app.lock().unwrap();
            ui(f, &mut app)
        })?;

        if event::poll(Duration::from_millis(25))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let mut app = app.lock().unwrap();
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Char('j') | KeyCode::Down => app.next(),
                        KeyCode::Char('k') | KeyCode::Up => app.previous(),
                        _ => {}
                    }
                }
            }
        }

        interval.tick().await;
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let outer_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0)]) // full screen
        .split(f.size());

    let inner_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Min(0),    // Main content
            Constraint::Length(1), // Footer
        ])
        .split(outer_layout[0]);

    f.render_widget(
        Block::default().borders(Borders::ALL).title("[Rollup.TUI] by the GhostGraph.xyz team"),
        outer_layout[0],
    );

    let content_area = inner_layout[1].inner(&Margin { vertical: 0, horizontal: 1 });

    let inner_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // Total
            Constraint::Min(0),    // Main table
            Constraint::Length(3), // Footer
        ])
        .split(content_area);

    render_totals(f, app, inner_layout[0]);
    render_table(f, app, inner_layout[1]);
    render_footer(f, app, inner_layout[2]);
}

fn render_totals(f: &mut Frame, app: &mut App, area: Rect) {
    let header_titles = ["TPS", "MGas/s", "KB/s"];

    let header_style = Style::default().fg(app.colors.header_fg).bg(app.colors.header_bg);
    let header = header_titles
        .into_iter()
        .map(|title| Cell::from(Text::from(title).alignment(Alignment::Center)).style(header_style))
        .collect::<Row>()
        .style(header_style)
        .height(1);

    let totals_row = app
        .totals
        .cells()
        .into_iter()
        .map(|total| {
            Cell::from(Text::from(total).alignment(Alignment::Center))
                .style(Style::default().fg(app.colors.row_fg).bg(app.colors.buffer_bg))
        })
        .collect::<Row>()
        .height(1);

    let totals_table = Table::new(
        vec![totals_row],
        [Constraint::Percentage(33), Constraint::Percentage(33), Constraint::Percentage(34)],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title("Totals"));

    let table_area = area.inner(&Margin { vertical: 1, horizontal: 2 });

    f.render_widget(totals_table, table_area);
}

fn render_table(f: &mut Frame, app: &mut App, area: Rect) {
    let header_style = Style::default().fg(app.colors.header_fg).bg(app.colors.header_bg);
    let header_titles = ["Network", "Block", "TPS", "MGas/s", "KB/s"];
    let header = header_titles
        .iter()
        .enumerate()
        .map(|(i, &title)| {
            let alignment = if i > 0 { Alignment::Right } else { Alignment::Left };
            Cell::from(Text::from(title).alignment(alignment))
        })
        .collect::<Row>()
        .style(header_style)
        .height(1);

    let rows = app.items.iter().map(|data| {
        let color = app.colors.normal_row_color;
        let item = data.cells();
        item.into_iter()
            .enumerate()
            .map(|(i, content)| {
                let alignment = if i > 0 { Alignment::Right } else { Alignment::Left };
                let content = if content == "0.00" { "-".to_string() } else { content };
                Cell::from(Text::from(format!("\n{}\n", content)).alignment(alignment))
            })
            .collect::<Row>()
            .style(Style::default().fg(app.colors.row_fg).bg(color))
            .height(2)
    });

    let bar = " █ ";
    let t = Table::new(
        rows,
        [
            Constraint::Length(app.longest_name + 1),
            Constraint::Min(0),
            Constraint::Min(5),
            Constraint::Min(5),
            Constraint::Min(5),
        ],
    )
    .header(header)
    .highlight_symbol(Text::from(vec!["".into(), bar.into(), bar.into(), "".into()]))
    .bg(app.colors.buffer_bg)
    .highlight_spacing(HighlightSpacing::Always)
    .block(Block::default().borders(Borders::ALL).title("Networks"));

    let table_area = area.inner(&Margin { vertical: 0, horizontal: 2 });

    f.render_stateful_widget(t, table_area, &mut app.state);
}

fn render_footer(f: &mut Frame, app: &mut App, area: Rect) {
    let info_footer = Paragraph::new(Line::from(INFO_TEXT))
        .style(Style::default().fg(app.colors.row_fg).bg(app.colors.buffer_bg))
        .centered()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(app.colors.footer_border_color)),
        );
    f.render_widget(info_footer, area);
}

struct TableColors {
    buffer_bg: Color,
    header_bg: Color,
    header_fg: Color,
    row_fg: Color,
    normal_row_color: Color,
    footer_border_color: Color,
}

impl TableColors {
    const fn new() -> Self {
        Self {
            buffer_bg: tailwind::SLATE.c950,
            header_bg: PALETTE.c900,
            header_fg: tailwind::SLATE.c200,
            row_fg: tailwind::SLATE.c200,
            normal_row_color: tailwind::SLATE.c950,
            footer_border_color: PALETTE.c400,
        }
    }
}
