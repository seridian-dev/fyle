//! Background playback for Quick Look audio previews.
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};
use std::{
    fs::OpenOptions,
    io::BufReader,
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};

enum Command {
    Toggle(PathBuf),
    Stop,
    Seek(Duration),
    Volume(f32),
}

#[derive(Clone, Debug)]
pub(super) struct AudioStatus {
    pub path: Option<PathBuf>,
    pub playing: bool,
    pub position: Duration,
    pub duration: Option<Duration>,
    pub volume: f32,
    pub error: Option<String>,
}

impl Default for AudioStatus {
    fn default() -> Self {
        Self {
            path: None,
            playing: false,
            position: Duration::ZERO,
            duration: None,
            volume: 0.8,
            error: None,
        }
    }
}

#[derive(Clone)]
pub(super) struct AudioPreview {
    commands: mpsc::Sender<Command>,
    status: Arc<Mutex<AudioStatus>>,
}

impl Default for AudioPreview {
    fn default() -> Self {
        let (commands, receiver) = mpsc::channel();
        let status = Arc::new(Mutex::new(AudioStatus::default()));
        let worker_status = Arc::clone(&status);
        std::thread::spawn(move || playback_worker(receiver, worker_status));
        Self { commands, status }
    }
}

impl AudioPreview {
    pub fn status(&self) -> AudioStatus {
        self.status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn toggle(&self, path: PathBuf) {
        let _ = self.commands.send(Command::Toggle(path));
    }

    pub fn stop(&self) {
        let _ = self.commands.send(Command::Stop);
    }

    pub fn seek(&self, position: Duration) {
        let _ = self.commands.send(Command::Seek(position));
    }

    pub fn set_volume(&self, volume: f32) {
        let _ = self.commands.send(Command::Volume(volume.clamp(0.0, 1.0)));
    }
}

fn playback_worker(commands: mpsc::Receiver<Command>, status: Arc<Mutex<AudioStatus>>) {
    let mut output: Option<MixerDeviceSink> = None;
    let mut player: Option<Player> = None;
    loop {
        match commands.recv_timeout(Duration::from_millis(50)) {
            Ok(Command::Toggle(path)) => {
                let current = status
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .clone();
                if current.path.as_ref() == Some(&path)
                    && player.as_ref().is_some_and(|player| !player.empty())
                {
                    let player = player.as_ref().expect("checked above");
                    if current.playing {
                        player.pause();
                    } else {
                        player.play();
                    }
                    status
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .playing = !current.playing;
                    continue;
                }

                if let Some(previous) = player.take() {
                    previous.stop();
                }

                if output.is_none() {
                    match DeviceSinkBuilder::open_default_sink() {
                        Ok(mut device) => {
                            device.log_on_drop(false);
                            output = Some(device);
                        }
                        Err(error) => {
                            set_error(
                                &status,
                                Some(path),
                                format!("Audio output unavailable: {error}"),
                            );
                            continue;
                        }
                    }
                }
                let result = open_audio_file(&path).and_then(|file| {
                    let source = Decoder::try_from(BufReader::new(file))
                        .map_err(|error| anyhow::anyhow!("Cannot decode audio: {error}"))?;
                    let duration = source.total_duration();
                    let player = Player::connect_new(
                        output.as_ref().expect("audio device initialized").mixer(),
                    );
                    player.set_volume(current.volume);
                    player.append(source);
                    player.play();
                    Ok((player, duration))
                });
                match result {
                    Ok((next_player, duration)) => {
                        if let Some(previous) = player.replace(next_player) {
                            previous.stop();
                        }
                        let mut state = status
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        state.path = Some(path);
                        state.playing = true;
                        state.position = Duration::ZERO;
                        state.duration = duration;
                        state.error = None;
                    }
                    Err(error) => set_error(&status, Some(path), format!("{error:#}")),
                }
            }
            Ok(Command::Stop) => {
                if let Some(previous) = player.take() {
                    previous.stop();
                }
                *status
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = AudioStatus::default();
            }
            Ok(Command::Seek(position)) => {
                if let Some(player) = &player {
                    if let Err(error) = player.try_seek(position) {
                        status
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .error = Some(format!("Cannot seek in this audio file: {error}"));
                    }
                }
            }
            Ok(Command::Volume(volume)) => {
                if let Some(player) = &player {
                    player.set_volume(volume);
                }
                status
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .volume = volume;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }

        if let Some(player) = &player {
            let mut state = status
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if state.playing {
                state.position = player.get_pos();
                if player.empty() {
                    state.playing = false;
                    if let Some(duration) = state.duration {
                        state.position = duration;
                    }
                }
            }
        }
    }
}

fn open_audio_file(path: &Path) -> anyhow::Result<std::fs::File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    anyhow::ensure!(
        file.metadata()?.is_file(),
        "Preview only supports regular audio files"
    );
    Ok(file)
}

fn set_error(status: &Arc<Mutex<AudioStatus>>, path: Option<PathBuf>, error: String) {
    let mut state = status
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.path = path;
    state.playing = false;
    state.error = Some(error);
}
