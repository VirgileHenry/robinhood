mod app;
mod save_track;
mod spot;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    init_logging();
    tracing::info!("Hello, Robin Hood");

    // crossterm's reader is blocking, so it lives on its own thread and feeds the async loop.
    let (key_tx, key_rx) = tokio::sync::mpsc::unbounded_channel();
    std::thread::spawn(move || {
        while let Ok(ev) = crossterm::event::read() {
            if let crossterm::event::Event::Key(k) = ev {
                if k.kind == crossterm::event::KeyEventKind::Press && key_tx.send(k).is_err() {
                    break;
                }
            }
        }
    });

    let terminal = ratatui::init();

    let spot = spot::Spot::new().await?;
    let app = app::App::new(spot);

    let result = app.run(terminal, key_rx).await;
    ratatui::restore();

    result
}

fn init_logging() {
    #[cfg(debug_assertions)]
    let bin_log_level = "debug";
    #[cfg(not(debug_assertions))]
    let bin_log_level = "info";

    #[cfg(debug_assertions)]
    let lib_log_level = "debug";
    #[cfg(not(debug_assertions))]
    let lib_log_level = "warn";

    let directives = format!("{lib_log_level},robin_hood={bin_log_level}");

    let file = std::fs::File::create("robin-hood.log").expect("create log file");

    let filter = tracing_subscriber::EnvFilter::new(directives);
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(file.try_clone().expect("clone log handle"))
        .with_ansi(false);

    builder.init();
}
