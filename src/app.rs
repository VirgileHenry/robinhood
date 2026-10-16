mod ui;

pub struct App {
    running: bool,
    spot: crate::spot::Spot,
    downloading: std::collections::BTreeMap<String, crate::save_track::SaveTrackHandle>,
    downloading_channel: tokio::sync::mpsc::Receiver<crate::save_track::SaveTrackEvent>,
    downloading_sender: tokio::sync::mpsc::Sender<crate::save_track::SaveTrackEvent>,
    downloaded: Vec<String>,
}

impl App {
    pub fn new(spot: crate::spot::Spot) -> Self {
        let (downloading_sender, downloading_channel) = tokio::sync::mpsc::channel(1024);
        Self {
            running: true,
            spot,
            downloading: std::collections::BTreeMap::new(),
            downloading_channel,
            downloading_sender,
            downloaded: Vec::new(),
        }
    }

    pub async fn run(
        mut self,
        mut terminal: ratatui::DefaultTerminal,
        mut keys: tokio::sync::mpsc::UnboundedReceiver<crossterm::event::KeyEvent>,
    ) -> std::io::Result<()> {
        while self.running {
            if let Err(e) = terminal.draw(|f| ui::draw(f, &self)) {
                tracing::error!("Failed to draw ui to terminal: {e}");
            }

            tokio::select! {
                Some(key) = keys.recv() => self.handle_key(key),
                Some(track) = self.spot.hijack_receiver() => self.incoming_track(track),
                Some(event) = self.downloading_channel.recv() => self.update_save_thread(event),
                _ = tokio::signal::ctrl_c() => self.running = false,
            }
        }

        if let Err(e) = self.spot.shutdown().await {
            tracing::error!("Failed to shutdown spot client: {e}");
        }

        Ok(())
    }

    fn handle_key(&mut self, event: crossterm::event::KeyEvent) {
        match event.code {
            crossterm::event::KeyCode::Char('c') if event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                self.running = false;
            }
            _ => {}
        }
    }

    fn incoming_track(&mut self, track: librespot_playback::player::PlayerLoadedTrackData) {
        let (name, save_thread) = crate::save_track::SaveTrackHandle::start(track, &self.downloading_sender.clone());
        self.downloading.insert(name, save_thread);
    }

    fn update_save_thread(&mut self, event: crate::save_track::SaveTrackEvent) {
        let track = event.track.clone();
        if let Some(mut save_thread) = self.downloading.remove(&track) {
            match save_thread.update(event) {
                Ok(None) => _ = self.downloading.insert(track, save_thread),
                Ok(Some(name)) => self.downloaded.push(name),
                Err(e) => tracing::error!("Failed to update save thread: {e}"),
            }
        } else {
            tracing::warn!("Unable to update save thread: not found {}", event.track);
        }
    }
}
