#[cfg(test)]
use crate::{
    bitrate::Bitrate, framerate::Framerate, resolution::Resolution, Recorder, RecorderSettings,
};

#[test]
fn record_league_1080p_30fps_8_mbit_60s() {
    let settings = RecorderSettings {
        window_title: String::from("League of Legends (TM) Client"),
        output_resolution: Resolution::_1080p,
        framerate: Framerate::new(30),
        bitrate: Bitrate::mbit(8),
        capture_cursor: true,
    };
    let mut recorder = Recorder::new(settings).expect("error creating recorder");
    recorder
        .start(Some(std::time::Duration::from_secs(60)))
        .expect("error starting recorder");
}

#[test]
fn record_firefox_1080p_30fps_18_mbit_10s() {
    let settings = RecorderSettings {
        window_title: String::from("Mozilla Firefox"),
        output_resolution: Resolution::_1080p,
        framerate: Framerate::new(30),
        bitrate: Bitrate::mbit(18),
        capture_cursor: true,
    };
    let mut recorder = Recorder::new(settings).expect("error creating recorder");
    recorder
        .start(Some(std::time::Duration::from_secs(10)))
        .expect("error starting recorder");
}
