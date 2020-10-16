#![cfg(feature = "edit")]

use std::io::{self, Write};
use tui::backend::CrosstermBackend;
use tui::terminal::Terminal;
use tui::widgets::{Block, Borders, Widget};

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
        let terminal = Terminal::new(CrosstermBackend::new(out))?;
        Ok(Editor { terminal })
    }

    pub fn render(&mut self) -> io::Result<()> {
        self.terminal.draw(|frame| {
            let size = frame.size();
            let block = Block::default()
                .title("nym")
                .borders(Borders::ALL);
            frame.render_widget(block, size);
        })
    }
}
