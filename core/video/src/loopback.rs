//! Hearing what a Windows computer plays, or what one program plays.
//!
//! ffmpeg can only record sound *inputs* on Windows (microphones, and "Stereo Mix" if the driver has
//! one), never what the computer plays. The Windows sound system can give that, so CargoFrame asks
//! it ("loopback") and hands the samples to ffmpeg through [`crate::pcm::Feed`].
//!
//! Only Windows has this; elsewhere `open` says so (Linux gets the same from `pulse` in ffmpeg).

use crate::pcm::PcmSource;

/// Listen to everything the computer plays (`program` is `None`), or to one program and the
/// programs it started (a process id). The returned source belongs on the thread that made it.
pub fn open(program: Option<u32>) -> Result<Box<dyn PcmSource>, String> {
    imp::open(program)
}

#[cfg(windows)]
mod imp {
    use std::collections::VecDeque;
    use std::time::Duration;

    use wasapi::{
        deinitialize, initialize_mta, AudioCaptureClient, AudioClient, DeviceEnumerator, Direction,
        Handle, SampleType, StreamMode, WasapiError, WaveFormat,
    };

    use crate::pcm::{PcmFormat, PcmSource};

    /// What ffmpeg is told to expect. The Windows sound system converts to it, whatever the
    /// speakers use.
    const RATE: usize = 48_000;
    const CHANNELS: usize = 2;
    /// A fifth of a second of room (units of 100 ns), the same as Microsoft's own loopback sample.
    const ROOM: i64 = 200_000;

    pub fn open(program: Option<u32>) -> Result<Box<dyn PcmSource>, String> {
        let com = Com::start();
        let stream = Stream::start(program, RATE, CHANNELS).or_else(|first| {
            // Some drivers refuse a rate or a channel count: ask for the format they use themselves
            // (a program's sound has none to ask for, so only the whole computer gets a second try)
            if program.is_some() {
                return Err(first);
            }
            let (rate, channels) = own_format().ok_or(first.clone())?;
            Stream::start(program, rate, channels).map_err(|_| first)
        })?;
        Ok(Box::new(Listener {
            format: PcmFormat {
                rate: stream.rate as u32,
                channels: stream.channels as u16,
            },
            program,
            stream: Some(stream),
            waiting: VecDeque::new(),
            _com: com,
        }))
    }

    /// The rate and channel count the speakers use, when they say.
    fn own_format() -> Option<(usize, usize)> {
        let client = new_client(None).ok()?;
        let format = client.get_mixformat().ok()?;
        let (rate, channels) = (format.get_samplespersec(), format.get_nchannels());
        (rate > 0 && channels > 0).then_some((rate as usize, usize::from(channels)))
    }

    fn new_client(program: Option<u32>) -> Result<AudioClient, String> {
        match program {
            Some(pid) => AudioClient::new_application_loopback_client(pid, true)
                .map_err(|e| format!("the program's sound could not be reached ({e})")),
            None => DeviceEnumerator::new()
                .and_then(|devices| devices.get_default_device(Direction::Render))
                .and_then(|speakers| speakers.get_iaudioclient())
                .map_err(|e| format!("the computer's sound output could not be reached ({e})")),
        }
    }

    /// COM has to be started on a thread before the sound system is used there, and ended after
    /// everything it gave out is gone (this is the last field of the listener for that reason).
    struct Com(bool);

    impl Com {
        fn start() -> Self {
            Com(initialize_mta().is_ok())
        }
    }

    impl Drop for Com {
        fn drop(&mut self) {
            if self.0 {
                deinitialize();
            }
        }
    }

    /// One open listening session.
    struct Stream {
        // kept alive: the capture client and the event only work while the client does
        client: AudioClient,
        capture: AudioCaptureClient,
        event: Handle,
        rate: usize,
        channels: usize,
    }

    impl Stream {
        fn start(program: Option<u32>, rate: usize, channels: usize) -> Result<Self, String> {
            let mut client = new_client(program)?;
            let wanted = WaveFormat::new(32, 32, SampleType::Float, rate, channels, None);
            let mode = StreamMode::EventsShared {
                autoconvert: true,
                buffer_duration_hns: ROOM,
            };
            let fail = |e: WasapiError| format!("the sound could not be listened to ({e})");
            client
                .initialize_client(&wanted, Direction::Capture, mode, None)
                .map_err(fail)?;
            let event = client.set_get_eventhandle().map_err(fail)?;
            let capture = client.get_audiocaptureclient().map_err(fail)?;
            client.start_stream().map_err(fail)?;
            Ok(Self {
                client,
                capture,
                event,
                rate,
                channels,
            })
        }

        /// Everything that has come in since the last time, up to `wait` for the first of it.
        fn take(&self, wait: Duration, into: &mut VecDeque<u8>) -> Result<(), WasapiError> {
            let ms = u32::try_from(wait.as_millis()).unwrap_or(u32::MAX).max(1);
            match self.event.wait_for_event(ms) {
                Ok(()) => {}
                // the computer played nothing in that time: not an error
                Err(WasapiError::EventTimeout) => return Ok(()),
                Err(e) => return Err(e),
            }
            while matches!(self.capture.get_next_packet_size()?, Some(frames) if frames > 0) {
                self.capture.read_from_device_to_deque(into)?;
            }
            Ok(())
        }
    }

    impl Drop for Stream {
        fn drop(&mut self) {
            let _ = self.client.stop_stream();
        }
    }

    struct Listener {
        format: PcmFormat,
        program: Option<u32>,
        /// `None` only for the moment between a stream that broke and the one that replaces it
        stream: Option<Stream>,
        waiting: VecDeque<u8>,
        _com: Com,
    }

    impl PcmSource for Listener {
        fn format(&self) -> PcmFormat {
            self.format
        }

        fn read(&mut self, out: &mut Vec<u8>, wait: Duration) -> Result<(), String> {
            let taken = match &self.stream {
                Some(stream) => stream.take(wait, &mut self.waiting),
                None => Ok(()),
            };
            if let Err(broken) = taken {
                // the speakers were unplugged or changed: the new default gets a second chance, in
                // the same format, since ffmpeg has been told that one
                self.stream = None;
                let (rate, channels) =
                    (self.format.rate as usize, usize::from(self.format.channels));
                match Stream::start(self.program, rate, channels) {
                    Ok(stream) => self.stream = Some(stream),
                    Err(_) => return Err(format!("the sound stopped ({broken})")),
                }
            }
            out.extend(self.waiting.drain(..));
            Ok(())
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use crate::pcm::PcmSource;

    pub fn open(_program: Option<u32>) -> Result<Box<dyn PcmSource>, String> {
        Err("hearing what the computer plays is only built for Windows".into())
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    #[test]
    fn off_windows_it_says_it_is_not_there() {
        assert!(super::open(None).is_err());
        assert!(super::open(Some(1)).is_err());
    }
}
