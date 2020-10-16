#![cfg(feature = "edit")]

use crossterm;
use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal;
use std::io::{self, Write};
use tui::backend::CrosstermBackend;
use tui::terminal::Terminal;
use tui::widgets::{Block, Borders};

pub struct Editor<O>
where
    O: Write,
{
    terminal: Terminal<CrosstermBackend<O>>,
}

impl<O> Editor<O>
where
    O: Write,
{
    pub fn attach(out: O) -> Result<Self, io::Error> {
        // TODO: Don't unwrap.
        terminal::enable_raw_mode().unwrap();
        let terminal = Terminal::new(CrosstermBackend::new(out))?;
        Ok(Editor { terminal })
    }

    pub fn run(&mut self) -> io::Result<()> {
        'run: loop {
            self.terminal.draw(|frame| {
                let size = frame.size();
                let block = Block::default().title("nym").borders(Borders::ALL);
                frame.render_widget(block, size);
            })?;
            // TODO: Don't unwrap.
            match event::read().unwrap() {
                Event::Key(event) => match event.code {
                    KeyCode::Char('q') => {
                        break 'run;
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        Ok(())
    }
}

impl<O> Drop for Editor<O>
where
    O: Write,
{
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}
