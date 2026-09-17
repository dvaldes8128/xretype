use std::{
    sync::mpsc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, bail};
use arboard::{Clipboard, SetExtLinux};

use crate::input::InputBackend;

use super::Sensitivity;

pub fn paste_with<I: InputBackend + ?Sized>(
    input: &mut I,
    text: String,
    sensitivity: Sensitivity,
    paste_delay: Duration,
    serve_for: Duration,
) -> Result<()> {
    if serve_for <= paste_delay {
        bail!("clipboard_serve_ms must be greater than paste_delay_ms");
    }

    let serve_deadline = Instant::now() + serve_for;
    let expected_text = text.clone();
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

        let setter = clipboard.set().wait_until(serve_deadline);
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

    let paste_keys = vec!["ctrl".to_owned(), "v".to_owned()];
    let mut worker = Some(worker);
    if worker
        .as_ref()
        .expect("clipboard worker was initialized")
        .is_finished()
    {
        // On Wayland, arboard/wl-clipboard may delegate ownership to a
        // background helper and return successfully before input injection.
        // Propagate an early error, but treat early success as a live offer.
        join_worker(worker.take().expect("clipboard worker was initialized"))?;
    }

    let verify_timeout = serve_deadline.saturating_duration_since(Instant::now());
    if let Err(error) = wait_until_clipboard_matches(&expected_text, verify_timeout) {
        if let Some(worker) = worker {
            let _ = join_worker(worker);
        }
        return Err(error);
    }

    let input_result = input.combo(&paste_keys);
    let clipboard_result = match worker {
        Some(worker) => join_worker(worker),
        None => {
            // Wayland's non-foreground wl-clipboard path serves requests from
            // an in-process helper thread. Keep this process alive long enough
            // for the target application to request the value after Ctrl+V.
            thread::sleep(serve_deadline.saturating_duration_since(Instant::now()));
            Ok(())
        }
    };
    input_result?;
    clipboard_result
}

fn wait_until_clipboard_matches(expected: &str, timeout: Duration) -> Result<()> {
    let mut clipboard = Clipboard::new().context("could not open clipboard for verification")?;
    wait_until_matches(expected, timeout, || {
        clipboard
            .get_text()
            .context("could not read clipboard while waiting for the new offer")
    })
}

fn wait_until_matches<F>(expected: &str, timeout: Duration, mut read: F) -> Result<()>
where
    F: FnMut() -> Result<String>,
{
    let deadline = Instant::now() + timeout;
    loop {
        if read().is_ok_and(|actual| actual == expected) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!("new clipboard offer did not become readable before paste input");
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn join_worker(worker: JoinHandle<Result<()>>) -> Result<()> {
    worker
        .join()
        .map_err(|_| anyhow!("clipboard worker panicked"))?
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, time::Duration};

    use anyhow::anyhow;

    use super::wait_until_matches;

    #[test]
    fn waits_past_old_clipboard_contents() {
        let mut values = VecDeque::from([
            Ok("old value".to_owned()),
            Err(anyhow!("temporarily unavailable")),
            Ok("new value".to_owned()),
        ]);

        wait_until_matches("new value", Duration::from_millis(50), || {
            values
                .pop_front()
                .unwrap_or_else(|| Ok("new value".to_owned()))
        })
        .unwrap();
    }

    #[test]
    fn fails_instead_of_pasting_stale_contents() {
        let error = wait_until_matches("new value", Duration::ZERO, || Ok("old value".to_owned()))
            .unwrap_err();

        assert!(error.to_string().contains("did not become readable"));
    }
}
