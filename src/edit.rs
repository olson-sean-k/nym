#![cfg(feature = "edit")]

use crossterm;
use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal;
use std::io::{self, Write};
use tui::backend::CrosstermBackend;
use tui::terminal::Terminal;
use tui::widgets::{Block, Borders};

pub struct Edit<W>
where
    W: Write,
{
    terminal: Terminal<CrosstermBackend<W>>,
}

impl<W> Edit<W>
where
    W: Write,
{
    pub fn attach(out: W) -> Result<Self, io::Error> {
        // TODO: Don't unwrap.
        terminal::enable_raw_mode().unwrap();
        let terminal = Terminal::new(CrosstermBackend::new(out))?;
        Ok(Edit { terminal })
    }

    pub fn execute(&mut self) -> io::Result<()> {
        self.terminal.clear()?;
        loop {
            self.terminal.draw(|frame| {
                let size = frame.size();
                let block = Block::default().title("nym").borders(Borders::ALL);
                frame.render_widget(block, size);
            })?;
            // TODO: Don't unwrap.
            match event::read().unwrap() {
                Event::Key(event) => match event.code {
                    KeyCode::Char('q') => {
                        break;
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        Ok(())
    }
}

impl<W> Drop for Edit<W>
where
    W: Write,
{
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}
