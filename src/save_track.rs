use librespot_metadata::audio::AudioItem;
use librespot_playback::player::PlayerLoadedTrackData;

pub struct SaveTrackHandle {
    _handle: tokio::task::JoinHandle<()>,
    pub progress: f32,
}

impl SaveTrackHandle {
    pub fn start(track: PlayerLoadedTrackData, sender: &tokio::sync::mpsc::Sender<SaveTrackEvent>) -> (String, Self) {
        tracing::info!("Start downloading track: {}", track.audio_item.name);

        let name = track_name(&track.audio_item);

        let handle = tokio::runtime::Handle::current();
        let _handle = handle.spawn(save_track(track, sender.clone()));

        (name, Self { _handle, progress: 0.0 })
    }

    pub fn update(&mut self, event: SaveTrackEvent) -> std::io::Result<Option<String>> {
        match event.kind {
            SaveTrackEventKind::Progress { downloaded, total } => {
                self.progress = (downloaded as f32) / (total as f32);
                Ok(None)
            }
            SaveTrackEventKind::Error { err } => Err(std::io::Error::other(err)),
            SaveTrackEventKind::Done { file } => Ok(Some(file)),
        }
    }
}

pub struct SaveTrackEvent {
    pub track: String,
    kind: SaveTrackEventKind,
}

enum SaveTrackEventKind {
    Progress { downloaded: u64, total: u64 },
    Done { file: String },
    Error { err: String },
}

pub async fn save_track(mut track: PlayerLoadedTrackData, sender: tokio::sync::mpsc::Sender<SaveTrackEvent>) {
    let track_name = track_name(&track.audio_item);

    let sample_rate = librespot_playback::SAMPLE_RATE as u64;
    let num_channels = librespot_playback::NUM_CHANNELS as u64;
    let duration_ms = track.audio_item.duration_ms as u64;

    let expected_samples = sample_rate * num_channels * duration_ms;
    let expected_samples = expected_samples / 1000;

    let mut last_samples_on_error = 0;
    let mut error_count = 0;

    let mut samples = Vec::new();

    tracing::debug!(
        "Start downloading track: {track_name}, expecting {} samples",
        expected_samples
    );
    log_err(
        sender
            .send(SaveTrackEvent {
                track: track_name.clone(),
                kind: SaveTrackEventKind::Progress {
                    downloaded: samples.len() as u64,
                    total: expected_samples,
                },
            })
            .await,
    );

    loop {
        match track.decoder.as_mut().next_packet() {
            Ok(Some((position, audio_packet))) => {
                if position.skipped {
                    tracing::warn!("Received a packet with a `skipped` flag");
                }
                if let librespot_playback::decoder::AudioPacket::Samples(packet_samples) = audio_packet {
                    let samples_count = packet_samples.len() as u64;
                    let samples_duration = (1000 * samples_count) / (sample_rate * num_channels);

                    let sleep_time = std::time::Duration::from_millis(samples_duration) / 20;

                    log_err(
                        sender
                            .send(SaveTrackEvent {
                                track: track_name.clone(),
                                kind: SaveTrackEventKind::Progress {
                                    downloaded: samples.len() as u64,
                                    total: expected_samples,
                                },
                            })
                            .await,
                    );
                    tokio::time::sleep(sleep_time).await;

                    samples.extend_from_slice(packet_samples.as_slice());
                } else {
                    tracing::warn!("Unhandled raw audio packet");
                }
            }
            Ok(None) => break,
            Err(e) => {
                tracing::warn!("Error when fetching the next packet: {e}");
                if last_samples_on_error == samples.len() {
                    error_count += 1;
                } else {
                    last_samples_on_error = samples.len();
                    error_count = 1;
                }
                if error_count >= 10 {
                    tracing::error!("10th error without more samples, exiting...");
                    log_err(
                        sender
                            .send(SaveTrackEvent {
                                track: track_name.clone(),
                                kind: SaveTrackEventKind::Error {
                                    err: format!("10th error without more samples: {e}"),
                                },
                            })
                            .await,
                    );
                    return;
                }
            }
        }
    }

    let track_path = format!("{track_name}.wav");
    let result = save_wav(
        &track_path,
        samples.as_slice(),
        librespot_playback::SAMPLE_RATE,
        u16::from(librespot_playback::NUM_CHANNELS),
    );

    if let Err(e) = result {
        tracing::error!("Failed to save the wav file: {e}");
        log_err(
            sender
                .send(SaveTrackEvent {
                    track: track_name.clone(),
                    kind: SaveTrackEventKind::Error {
                        err: format!("Failed to save the wav file: {e}"),
                    },
                })
                .await,
        );
    } else {
        log_err(
            sender
                .send(SaveTrackEvent {
                    track: track_name.clone(),
                    kind: SaveTrackEventKind::Done { file: track_path },
                })
                .await,
        );
    }
}

fn track_name(audio_item: &AudioItem) -> String {
    let (artist_name, album_name) = match &audio_item.unique_fields {
        librespot_metadata::audio::UniqueFields::Track { artists, album, .. } => match artists.as_slice() {
            [] => ("Unknown Artist".to_string(), album.clone()),
            artists => {
                let artists = artists.iter().map(|art| art.name.as_str()).collect::<Vec<_>>().join(", ");
                (artists, album.clone())
            }
        },
        _ => unimplemented!(),
    };
    let track_name = audio_item.name.clone();
    let album_name = if album_name != track_name {
        format!(" ({album_name})")
    } else {
        "".to_string()
    };
    format!("{artist_name} - {track_name}{album_name}")
}

fn save_wav(path: &str, samples: &[f64], sample_rate: u32, channels: u16) -> Result<(), hound::Error> {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &s in samples {
        writer.write_sample(s as f32)?;
    }
    writer.finalize()?;

    tracing::info!("Saved file: {path}");
    Ok(())
}

fn log_err(err: Result<(), tokio::sync::mpsc::error::SendError<SaveTrackEvent>>) {
    if let Err(e) = err {
        tracing::error!("Failed to send event from save thread: {e}")
    }
}
