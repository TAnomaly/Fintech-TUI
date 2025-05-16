use std::collections::BTreeMap;
use std::io::{self, Write};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph, Row, Table},
};
use reqwest::blocking::get;
use serde::Deserialize;

// Alpha Vantage yanıtı için struct
#[derive(Debug, Deserialize)]
struct TimeSeriesDaily {
    #[serde(rename = "Time Series (Daily)")]
    daily: BTreeMap<String, BTreeMap<String, String>>,
}

fn fetch_alpha_vantage(symbol: &str, api_key: &str, days: usize) -> Result<Vec<f64>, String> {
    let url = format!(
        "https://www.alphavantage.co/query?function=TIME_SERIES_DAILY&symbol={}&apikey={}",
        symbol, api_key
    );
    let resp = get(&url).map_err(|e| format!("HTTP hatası: {}", e))?;
    let text = resp.text().map_err(|e| format!("Yanıt okunamadı: {}", e))?;
    let data: TimeSeriesDaily =
        serde_json::from_str(&text).map_err(|e| format!("JSON hatası: {}", e))?;
    let mut closes: Vec<(String, f64)> = data
        .daily
        .iter()
        .filter_map(|(date, values)| {
            values
                .get("4. close")
                .and_then(|v| v.parse::<f64>().ok())
                .map(|close| (date.clone(), close))
        })
        .collect();
    closes.sort_by(|a, b| a.0.cmp(&b.0));
    let closes: Vec<f64> = closes
        .into_iter()
        .rev()
        .take(days)
        .map(|(_, v)| v)
        .collect();
    if closes.is_empty() {
        return Err("API'den veri alınamadı".to_string());
    }
    Ok(closes.into_iter().rev().collect())
}

// Hareketli ortalama
mod ml_fin {
    pub fn moving_average(prices: &[f64], window: usize) -> Option<f64> {
        if prices.len() < window {
            return None;
        }
        let sum: f64 = prices[prices.len() - window..].iter().sum();
        Some(sum / window as f64)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let api_key = "XOTA84CVGZ6QL713";
    let mut symbol = String::from("AAPL");
    let mut prices: Vec<f64> = fetch_alpha_vantage(&symbol, api_key, 30).unwrap_or_default();
    let mut error_msg = String::new();

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(7),
                    Constraint::Min(10),
                    Constraint::Length(2),
                ])
                .split(f.size());

            let block = Block::default()
                .title(format!(
                    " Alpha Vantage Terminal - Sembol: {} (Çıkmak için Q) ",
                    symbol
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan));
            f.render_widget(block, chunks[0]);

            if prices.is_empty() {
                let info = Paragraph::new("Veri bulunamadı veya API'den fiyat alınamadı.")
                    .block(Block::default().title("Uyarı").borders(Borders::ALL));
                f.render_widget(&info, chunks[1]);
                f.render_widget(&info, chunks[2]);
            } else {
                let last_price = prices.last().cloned().unwrap_or(0.0);
                let rows = vec![
                    Row::new(vec!["Son Kapanış"])
                        .style(Style::default().add_modifier(Modifier::BOLD)),
                    Row::new(vec![format!("{:.2}", last_price)]),
                ];
                let table = Table::new(rows, [Constraint::Length(15)]).block(
                    Block::default()
                        .title("Fiyat Bilgisi")
                        .borders(Borders::ALL),
                );
                f.render_widget(table, chunks[1]);

                let chart_prices: Vec<(f64, f64)> = prices
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (i as f64, *v))
                    .collect();

                let (y_min, y_max) = {
                    let min = prices.iter().cloned().fold(f64::INFINITY, f64::min);
                    let max = prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                    if (max - min).abs() < std::f64::EPSILON {
                        (min - 1.0, max + 1.0)
                    } else {
                        (min.floor(), max.ceil())
                    }
                };

                let datasets = vec![
                    Dataset::default()
                        .name("Kapanış")
                        .graph_type(GraphType::Line) // Çizgi grafik!
                        .style(Style::default().fg(Color::Yellow))
                        .data(&chart_prices),
                ];
                let chart = Chart::new(datasets)
                    .block(
                        Block::default()
                            .title("Son 30 Günlük Kapanış Fiyatı")
                            .borders(Borders::ALL),
                    )
                    .x_axis(
                        Axis::default()
                            .title("Gün")
                            .style(Style::default().fg(Color::Gray))
                            .bounds([0.0, chart_prices.len().max(1) as f64]),
                    )
                    .y_axis(
                        Axis::default()
                            .title("Fiyat")
                            .style(Style::default().fg(Color::Gray))
                            .bounds([y_min, y_max]),
                    );
                f.render_widget(chart, chunks[2]);
            }

            let ma_text = if !prices.is_empty() {
                if let Some(ma) = ml_fin::moving_average(&prices, 5) {
                    format!("Son 5 fiyatın hareketli ortalaması: {:.2}", ma)
                } else {
                    "Hareketli ortalama için yeterli veri yok.".to_string()
                }
            } else {
                "".to_string()
            };
            let error = if !error_msg.is_empty() {
                format!("Hata: {}", error_msg)
            } else {
                "".to_string()
            };
            let info = format!("{}   {}", ma_text, error);
            let info_block = Block::default().borders(Borders::ALL).title("Bilgi");
            f.render_widget(Paragraph::new(info).block(info_block), chunks[3]);
        })?;

        if event::poll(Duration::from_millis(1500))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => break,
                    KeyCode::Enter => {
                        disable_raw_mode()?;
                        execute!(io::stdout(), LeaveAlternateScreen)?;
                        print!("Yeni sembol girin: ");
                        io::stdout().flush().unwrap();
                        let mut new_symbol = String::new();
                        io::stdin().read_line(&mut new_symbol)?;
                        let new_symbol = new_symbol.trim().to_uppercase();
                        if !new_symbol.is_empty() {
                            let new_prices = fetch_alpha_vantage(&new_symbol, api_key, 30);
                            match new_prices {
                                Ok(p) if !p.is_empty() => {
                                    symbol = new_symbol;
                                    prices = p;
                                    error_msg.clear();
                                }
                                Ok(_) => {
                                    error_msg = "Bu sembol için veri bulunamadı.".to_string();
                                    // Eski fiyatlar korunur, grafik kaybolmaz
                                }
                                Err(e) => {
                                    error_msg = e;
                                    // Eski fiyatlar korunur, grafik kaybolmaz
                                }
                            }
                        }
                        enable_raw_mode()?;
                        execute!(io::stdout(), EnterAlternateScreen)?;
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}
