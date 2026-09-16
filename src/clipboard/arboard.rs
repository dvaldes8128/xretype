use std::{
    sync::mpsc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, bail};
use arboard::{Clipboard, SetExtLinux};

use crate::input::InputBackend;

use super::Sensitivity;

pub fn paste_with<I: InputBackend>(
    input: &mut I,
    text: String,
    sensitivity: Sensitivity,
    paste_delay: Duration,
    serve_for: Duration,
) -> Result<()> {
    if serve_for <= paste_delay {
        bail!("clipboard_serve_ms must be greater than paste_delay_ms");
    }

    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || -> Result<()> {
        let mut clipboard = match Clipboard::new().context("could not open the clipboard") {
            Ok(clipboard) => clipboard,
            Err(error) => {
                let _ = ready_tx.send(Err(error.to_string()));
                return Err(error);
            }
        };

        ready_tx
            .send(Ok(()))
            .map_err(|_| anyhow!("clipboard coordinator stopped before the offer was ready"))?;

        let setter = clipboard.set().wait_until(Instant::now() + serve_for);
        match sensitivity {
            Sensitivity::Sensitive => setter
                .exclude_from_history()
                .text(text)
                .context("could not offer sensitive clipboard text"),
            Sensitivity::Public => setter
                .text(text)
                .context("could not offer public clipboard text"),
        }
    });

    match ready_rx
        .recv()
        .context("clipboard worker stopped during setup")?
    {
        Ok(()) => {}
        Err(message) => return join_worker(worker).and_then(|_| Err(anyhow!(message))),
    }

    thread::sleep(paste_delay);
    if worker.is_finished() {
        join_worker(worker)?;
        bail!("clipboard offer ended before paste input could be injected");
    }

    let paste_keys = vec!["ctrl".to_owned(), "v".to_owned()];
    let input_result = input.combo(&paste_keys);
    let clipboard_result = join_worker(worker);
    input_result?;
    clipboard_result
}

fn join_worker(worker: JoinHandle<Result<()>>) -> Result<()> {
    worker
        .join()
        .map_err(|_| anyhow!("clipboard worker panicked"))?
}
