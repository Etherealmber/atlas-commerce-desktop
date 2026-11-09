//! Isolated WebKitGTK process. The URL is received only through a private
//! inherited pipe, never command-line arguments or logs.
#![forbid(unsafe_code)]

#[cfg(target_os = "linux")]
mod window;

#[cfg(target_os = "linux")]
fn main() {
    if run().is_err() {
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {}

#[cfg(target_os = "linux")]
fn run() -> Result<(), ()> {
    use std::io::{BufRead, BufReader, Read};
    use std::sync::mpsc;
    use std::time::Duration;
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        url: String,
        server: String,
        code: String,
    }
    let mut reader = BufReader::new(std::io::stdin());
    let mut line = String::new();
    if reader
        .by_ref()
        .take(16_385)
        .read_line(&mut line)
        .map_err(|_| ())?
        > 16_384
    {
        return Err(());
    }
    let request: Request = serde_json::from_str(&line).map_err(|_| ())?;
    let url = url::Url::parse(&request.url).map_err(|_| ())?;
    let view = window::LoginView::open(&url, &request.server, &request.code).map_err(|_| ())?;
    let (closed_tx, closed_rx) = mpsc::channel();
    // The parent keeps this pipe open until cancellation, success or exit.
    // EOF also handles a crash without leaving an orphaned login window.
    std::thread::spawn(move || {
        let mut byte = [0u8; 1];
        let _ = reader.read(&mut byte);
        let _ = closed_tx.send(());
    });
    while closed_rx.try_recv().is_err() && view.pump() {
        std::thread::sleep(Duration::from_millis(16));
    }
    Ok(())
}
