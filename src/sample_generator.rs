use std::sync::mpsc::Sender;

use windows::{
    core::{Interface, Result},
    Foundation::TimeSpan,
    Graphics::{
        Capture::{Direct3D11CaptureFrame, GraphicsCaptureItem, GraphicsCaptureSession},
        DirectX::Direct3D11::IDirect3DSurface,
    },
    Win32::{
        Graphics::{
            Direct3D11::{
                ID3D11Device, ID3D11DeviceContext, ID3D11Multithread, ID3D11RenderTargetView,
                ID3D11Texture2D, D3D11_BOX,
            },
            Dxgi::IDXGISurface,
        },
        System::WinRT::Direct3D11::CreateDirect3D11SurfaceFromDXGISurface,
    },
};

use crate::{frame_generator::CaptureFrameGenerator, utils};

pub struct VideoEncoderInputSample {
    pub timestamp: TimeSpan,
    pub texture: IDirect3DSurface,
}

impl VideoEncoderInputSample {
    pub fn new(timestamp: TimeSpan, texture: IDirect3DSurface) -> Self {
        Self { timestamp, texture }
    }
}
pub struct SampleGenerator {
    d3d_context: ID3D11DeviceContext,
    multithread: ID3D11Multithread,

    compose_texture: ID3D11Texture2D,
    render_target_view: ID3D11RenderTargetView,

    frame_generator: CaptureFrameGenerator,

    seen_first_time_stamp: bool,
    first_timestamp: TimeSpan,
}

unsafe impl Send for SampleGenerator {}
impl SampleGenerator {
    pub fn new(d3d_device: ID3D11Device, item: GraphicsCaptureItem) -> Result<Self> {
        let d3d_context = utils::get_d3d_context(&d3d_device)?;
        let multithread: ID3D11Multithread = d3d_context.cast()?;
        unsafe { multithread.SetMultithreadProtected(true) };

        let size = item.Size()?;
        let compose_texture = utils::create_compose_texture(&d3d_device, size)?;
        let render_target_view = utils::create_render_target_view(&d3d_device, &compose_texture)?;

        let frame_generator = CaptureFrameGenerator::new(d3d_device, item, size)?;

        Ok(Self {
            d3d_context,
            multithread,

            compose_texture,
            render_target_view,

            frame_generator,

            seen_first_time_stamp: false,
            first_timestamp: TimeSpan::default(),
        })
    }

    pub fn capture_session(&self) -> &GraphicsCaptureSession {
        self.frame_generator.session()
    }

    pub fn sender(&self) -> Sender<Option<Direct3D11CaptureFrame>> {
        self.frame_generator.sender()
    }

    pub fn generate(&mut self) -> Result<Option<VideoEncoderInputSample>> {
        if let Some(frame) = self.frame_generator.try_get_next_frame()? {
            let result = self.generate_from_frame(&frame);
            return Ok(result.ok());
        } else {
            Ok(None)
        }
    }

    fn generate_from_frame(
        &mut self,
        frame: &Direct3D11CaptureFrame,
    ) -> Result<VideoEncoderInputSample> {
        let frame_time = frame.SystemRelativeTime()?;
        let timestamp: TimeSpan;
        if !self.seen_first_time_stamp {
            self.first_timestamp = frame_time;
            self.seen_first_time_stamp = true;
            timestamp = TimeSpan { Duration: 100 }; // just a little bit more than zero
        } else {
            timestamp = TimeSpan {
                Duration: frame_time.Duration - self.first_timestamp.Duration,
            };
        }

        let content_size = frame.ContentSize()?;
        let frame_texture: ID3D11Texture2D =
            utils::get_d3d_interface_from_object(&frame.Surface()?)?;
        let desc = utils::get_texture_description(&frame_texture);

        // In order to support window resizing, we need to only copy out the part of
        // the buffer that contains the window. If the window is smaller than the buffer,
        // then it's a straight forward copy using the ContentSize. If the window is larger,
        // we need to clamp to the size of the buffer. For simplicity, we always clamp.
        let width = content_size.Width.clamp(0, desc.Width as i32) as u32;
        let height = content_size.Height.clamp(0, desc.Height as i32) as u32;

        let region = D3D11_BOX {
            left: 0,
            right: width,
            top: 0,
            bottom: height,
            back: 1,
            front: 0,
        };

        unsafe {
            self.multithread.Enter();

            self.d3d_context
                .ClearRenderTargetView(&self.render_target_view, utils::CLEAR_COLOR.as_ptr());
            self.d3d_context.CopySubresourceRegion(
                &self.compose_texture,
                0,
                0,
                0,
                0,
                &frame_texture,
                0,
                &region,
            );

            let dxgi_surface: IDXGISurface = self.compose_texture.cast()?;
            let d3d_surface: IDirect3DSurface =
                CreateDirect3D11SurfaceFromDXGISurface(dxgi_surface)?.cast()?;

            self.multithread.Leave();
            frame.Surface()?.Close()?;
            frame.Close()?;

            Ok(VideoEncoderInputSample::new(timestamp, d3d_surface))
        }
    }
}
