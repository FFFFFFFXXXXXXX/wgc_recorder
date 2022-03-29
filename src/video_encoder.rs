use windows::{
    core::Result,
    Foundation::IAsyncActionWithProgress,
    Media::{
        Core::MediaStreamSource, MediaProperties::MediaEncodingProfile,
        Transcoding::MediaTranscoder,
    },
    Storage::Streams::IRandomAccessStream,
};

use crate::utils;
pub struct VideoEncoder {
    transcoder: MediaTranscoder,
    stream_source: MediaStreamSource,
    output_stream: IRandomAccessStream,
    encoding_profile: MediaEncodingProfile,
    async_transcode: Option<IAsyncActionWithProgress<f64>>,
}

impl VideoEncoder {
    pub fn new(
        stream_source: MediaStreamSource,
        output_stream: IRandomAccessStream,
        encoding_profile: MediaEncodingProfile,
    ) -> Result<Self> {
        let transcoder = utils::create_media_transcoder()?;
        transcoder.SetHardwareAccelerationEnabled(true)?;

        Ok(VideoEncoder {
            transcoder,
            stream_source,
            output_stream,
            encoding_profile,
            async_transcode: None,
        })
    }

    pub fn start(&mut self) -> Result<()> {
        let transcoder = self
            .transcoder
            .PrepareMediaStreamSourceTranscodeAsync(
                &self.stream_source,
                &self.output_stream,
                &self.encoding_profile,
            )?
            .get()?;
        self.async_transcode = Some(transcoder.TranscodeAsync()?);
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        self.async_transcode.as_ref().unwrap().get()?;
        self.output_stream.FlushAsync()?.get()?;
        self.output_stream.Close()?;
        Ok(())
    }

    pub fn force_stop(&self) -> Result<()> {
        self.async_transcode.as_ref().unwrap().Close()?;
        self.output_stream.Close()?;
        Ok(())
    }
}
