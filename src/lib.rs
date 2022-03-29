// trying to implement the screen recording method used by
// https://docs.microsoft.com/en-us/windows/uwp/audio-video-camera/screen-capture-video
// parts yoinked from https://github.com/robmikh/screenshot-rs
// and https://github.com/robmikh/displayrecorder
// : adjusted for version 0.34.0 of windows-rs

/*
output size samples:
    1min
    1080p:30fps:8mbit  - 57,9Mb
    1080p:30fps:12mbit - 86,4Mb
    1440p:60fps:8mbit  - 173Mb

    30min
    1080p:30fps:8mbit  - 1,7Gb
    1080p:30fps:12mbit - 2,6Gb
    1440p:60fps:8mbit  - 5,2Gb
*/

use std::sync::{mpsc::Sender, Arc, Condvar, Mutex};

use bitrate::Bitrate;
use framerate::Framerate;
use resolution::Resolution;
use sample_generator::SampleGenerator;
use video_encoder::VideoEncoder;
use windows::{
    core::{Result as WinResult, HRESULT, HSTRING},
    Foundation::{TimeSpan, TypedEventHandler},
    Graphics::Capture::{Direct3D11CaptureFrame, GraphicsCaptureSession},
    Media::Core::{
        MediaStreamSample, MediaStreamSourceSampleRequestedEventArgs,
        MediaStreamSourceStartingEventArgs,
    },
};

pub mod bitrate;
mod capture_item;
mod frame_generator;
pub mod framerate;
pub mod resolution;
mod sample_generator;
mod tests;
mod utils;
mod video_encoder;

pub struct RecorderSettings {
    window_title: String,
    output_resolution: Resolution,
    framerate: Framerate,
    bitrate: Bitrate,
    capture_cursor: bool,
}
pub struct Recorder {
    is_recording: bool,
    stop_sender: Sender<Option<Direct3D11CaptureFrame>>,
    closed_condvar: Arc<(Mutex<bool>, Condvar)>,
    capture_session: GraphicsCaptureSession,
    video_encoder: VideoEncoder,
}

impl Recorder {
    pub fn new(settings: RecorderSettings) -> WinResult<Self> {
        if !GraphicsCaptureSession::IsSupported()? {
            return Err(windows::core::Error::new(
                HRESULT::default(),
                HSTRING::from("Windows Graphics Capture API is not supported!"),
            ));
        }

        let window = capture_item::find_window(settings.window_title);
        if let Some(handle) = window {
            let capture_item = utils::create_capture_item_for_window(handle)?;
            let input_size = capture_item.Size()?;
            let output_size = if let Some(res) = settings.output_resolution.get_size() {
                res
            } else {
                utils::ensure_even(input_size)
            };

            let d3d_device = utils::create_d3d_device()?;

            let mut sample_generator = SampleGenerator::new(d3d_device, capture_item)?;
            let capture_session = sample_generator.capture_session().clone();
            capture_session.SetIsCursorCaptureEnabled(settings.capture_cursor)?;
            match capture_session.SetIsBorderRequired(false) {
                Ok(_) => println!("yellow border removed"),
                Err(e) => println!(
                    "error removing yellow border (only works on windows11): {}",
                    e
                ),
            }

            let sender = sample_generator.sender();

            // media stream source
            let stream_source = utils::get_media_stream_source(&input_size)?;
            stream_source.SetCanSeek(false)?;
            stream_source.Starting(
                TypedEventHandler::<_, MediaStreamSourceStartingEventArgs>::new(|_, args| {
                    args.as_ref()
                        .unwrap()
                        .Request()?
                        .SetActualStartPosition(TimeSpan { Duration: 0 })?;
                    Ok(())
                }),
            )?;
            stream_source.SampleRequested(TypedEventHandler::<
                _,
                MediaStreamSourceSampleRequestedEventArgs,
            >::new(move |_, args| {
                let request = args.as_ref().unwrap().Request()?;
                if let Some(sample) = sample_generator.generate()? {
                    let sample = MediaStreamSample::CreateFromDirect3D11Surface(
                        sample.texture,
                        sample.timestamp,
                    )?;
                    request.SetSample(sample)?;
                } else {
                    request.SetSample(None)?;
                }
                Ok(())
            }))?;

            let pair1 = Arc::new((Mutex::new(false), Condvar::new()));
            let pair2 = Arc::clone(&pair1);
            stream_source.Closed(TypedEventHandler::<_, _>::new(move |_, _| {
                let (lock, cvar) = &*pair2;
                let mut closed = lock.lock().unwrap();
                *closed = true;
                cvar.notify_one();
                Ok(())
            }))?;

            let output_stream = utils::create_output_stream()?;

            let bitrate = if settings.bitrate.is_auto() {
                Bitrate::get_default_bitrate(settings.output_resolution)
            } else {
                settings.bitrate
            };
            let encoding_profile =
                utils::create_media_encoding_profile(output_size, settings.framerate, bitrate)?;

            let video_encoder = VideoEncoder::new(stream_source, output_stream, encoding_profile)?;

            return Ok(Recorder {
                is_recording: false,
                stop_sender: sender,
                closed_condvar: pair1,
                capture_session,
                video_encoder,
            });
        } else {
            return Err(windows::core::Error::new(
                HRESULT::default(),
                HSTRING::from("No window with that name found!"),
            ));
        }
    }

    pub fn start(&mut self, duration: Option<std::time::Duration>) -> Result<(), String> {
        if self.is_recording {
            return Err("Recorder is already running!".to_string());
        }
        match self.try_start(duration) {
            Ok(_) => {
                self.is_recording = true;
                Ok(())
            }
            Err(e) => Err(e.message().to_string_lossy()),
        }
    }

    fn try_start(&mut self, duration: Option<std::time::Duration>) -> WinResult<()> {
        self.capture_session.StartCapture()?;
        self.video_encoder.start()?;

        if let Some(dur) = duration {
            // wait for Closed Event or Duration timeout
            let (lock, cvar) = &*Arc::clone(&self.closed_condvar);
            let closed = lock.lock().unwrap();
            let _ = cvar.wait_timeout(closed, dur);

            match self.try_stop() {
                Ok(_) => {
                    self.cleanup(false);
                    Ok(())
                }
                Err(e) => {
                    self.cleanup(true);
                    Err(windows::core::Error::new(
                        HRESULT(-1),
                        HSTRING::from(e + "Recorder was stopped forcefully!"),
                    ))
                }
            }
        } else {
            Ok(())
        }
    }

    pub fn stop(&mut self) -> Result<(), String> {
        if !self.is_recording {
            return Err("Recorder is not recording!".to_string());
        }
        if let Err(e) = self.try_stop() {
            self.cleanup(true);
            Err(e + "Recorder was stopped forcefully!")
        } else {
            self.cleanup(false);
            Ok(())
        }
    }

    fn try_stop(&mut self) -> Result<(), String> {
        match self.stop_sender.send(None) {
            Ok(_) => Ok(()),
            Err(_) => Err("Stop message could not be sent => ".to_string()),
        }
    }

    fn cleanup(&mut self, force: bool) {
        let _ = self.capture_session.Close();
        if force {
            let _ = self.video_encoder.force_stop();
        } else {
            let _ = self.video_encoder.stop();
        }
    }
}
