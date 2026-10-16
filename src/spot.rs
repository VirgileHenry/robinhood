/// Stores spotify related stuff
pub struct Spot {
    /// Spotify connect handle
    spirc: librespot_connect::Spirc,
    /// Channel to receive events from the player
    _player_events: tokio::sync::mpsc::UnboundedReceiver<librespot_playback::player::PlayerEvent>,
    /// Handle for the running player task
    spirc_handle: tokio::task::JoinHandle<()>,
    /// Channel to receive each track from spotify
    hijack_receiver: tokio::sync::mpsc::Receiver<librespot_playback::player::PlayerLoadedTrackData>,
}

impl Spot {
    pub async fn new() -> std::io::Result<Self> {
        // 1. Credentials: cached blob if we've logged in before, otherwise browser OAuth.
        let session_config = librespot_core::SessionConfig::default();
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| std::io::Error::other("No cache directory"))?
            .join("robin-hood");
        let cache = librespot_core::cache::Cache::new(Some(&cache_dir), None, None, None).map_err(to_io)?;

        let credentials = match cache.credentials() {
            Some(creds) => creds,
            None => {
                let client_id = session_config.client_id.clone();
                let token = tokio::task::spawn_blocking(move || {
                    librespot_oauth::OAuthClientBuilder::new(&client_id, "http://127.0.0.1:8898/login", vec!["streaming"])
                        .open_in_browser()
                        .build()?
                        .get_access_token()
                })
                .await?
                .map_err(to_io)?;
                librespot_core::authentication::Credentials::with_access_token(token.access_token)
            }
        };

        // 2. Session: not connected yet, Spirc does the login itself.
        let session = librespot_core::Session::new(session_config, Some(cache));

        // 3. Audio: mixer (volume) + player writing to the default sink.
        let mixer_fn = librespot_playback::mixer::find(None).ok_or_else(|| std::io::Error::other("No mixer"))?;
        let mixer = mixer_fn(librespot_playback::mixer::MixerConfig::default()).map_err(to_io)?;

        /* track hijacker allows to extract the raw files out */
        let (hijack_sender, hijack_receiver) = tokio::sync::mpsc::channel(256);

        let sink_builder =
            librespot_playback::audio_backend::find(None).ok_or_else(|| std::io::Error::other("No audio backend"))?;
        let player = librespot_playback::player::Player::new(
            librespot_playback::config::PlayerConfig::default(),
            session.clone(),
            mixer.get_soft_volume(),
            move || sink_builder(None, librespot_playback::config::AudioFormat::default()),
            hijack_sender,
        );

        // Grab the event channel before Spirc takes the player.
        let player_events = player.get_player_event_channel();

        // 4. Spirc: logs in, registers "Robin Hood" as a Connect device, and from then on
        //    receives the shared playback state of your account.
        let connect_config = librespot_connect::ConnectConfig {
            name: "Robin Hood".into(),
            ..Default::default()
        };
        let (spirc, spirc_task) = librespot_connect::Spirc::new(connect_config, session.clone(), credentials, player, mixer)
            .await
            .map_err(to_io)?;

        tracing::info!(
            "connected as {} — look for \"Robin Hood\" in your devices list",
            session.username()
        );

        // spirc_task is the device's main loop: it must be driven for anything to happen.
        let spirc_handle = tokio::spawn(spirc_task);

        Ok(Self {
            spirc,
            _player_events: player_events,
            spirc_handle,
            hijack_receiver,
        })
    }

    pub async fn hijack_receiver(&mut self) -> Option<librespot_playback::player::PlayerLoadedTrackData> {
        self.hijack_receiver.recv().await
    }

    pub async fn shutdown(self) -> std::io::Result<()> {
        self.spirc.shutdown().map_err(to_io)?;
        self.spirc_handle.await?;
        Ok(())
    }
}

fn to_io<E>(error: E) -> std::io::Error
where
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    std::io::Error::other(error)
}
