use std::time::Duration;

use windows::{
    core::{Abi, Interface, Result, HSTRING},
    Graphics::{Capture::GraphicsCaptureItem, DirectX::Direct3D11::IDirect3DDevice, SizeInt32},
    Media::{
        Core::{MediaStreamSource, VideoStreamDescriptor},
        MediaProperties::{MediaEncodingProfile, MediaEncodingSubtypes, VideoEncodingProperties},
        Transcoding::MediaTranscoder,
    },
    Storage::{
        CreationCollisionOption, FileAccessMode, KnownFolders, Streams::IRandomAccessStream,
    },
    Win32::{
        Foundation::HWND,
        Graphics::{
            Direct3D,
            Direct3D11::{
                self, ID3D11Device, ID3D11DeviceContext, ID3D11RenderTargetView, ID3D11Texture2D,
                D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_TEXTURE2D_DESC,
                D3D11_USAGE_DEFAULT,
            },
            Dxgi::{
                Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
                IDXGIDevice,
            },
        },
        System::WinRT::{
            Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess},
            Graphics::Capture::IGraphicsCaptureItemInterop,
        },
    },
};

use crate::{bitrate::Bitrate, framerate::Framerate};

pub const CLEAR_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

pub fn create_render_target_view(
    d3d_device: &ID3D11Device,
    compose_texture: &ID3D11Texture2D,
) -> Result<ID3D11RenderTargetView> {
    unsafe { d3d_device.CreateRenderTargetView(compose_texture, std::ptr::null()) }
}

pub fn create_compose_texture(
    d3d_device: &ID3D11Device,
    size: SizeInt32,
) -> Result<ID3D11Texture2D> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: size.Width.try_into().unwrap(),
        Height: size.Height.try_into().unwrap(),
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_SHADER_RESOURCE | D3D11_BIND_RENDER_TARGET,
        ..Default::default()
    };

    unsafe { d3d_device.CreateTexture2D(&desc as *const _, std::ptr::null()) }
}

pub fn create_capture_item_for_window(window_handle: HWND) -> Result<GraphicsCaptureItem> {
    let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
    unsafe { interop.CreateForWindow(window_handle) }
}

pub fn ensure_even(size: SizeInt32) -> SizeInt32 {
    SizeInt32 {
        Width: if size.Width % 2 == 0 {
            size.Width
        } else {
            size.Width + 1
        },
        Height: if size.Height % 2 == 0 {
            size.Height
        } else {
            size.Height + 1
        },
    }
}

pub fn create_d3d_device() -> Result<ID3D11Device> {
    let mut device = None;
    let _result = unsafe {
        Direct3D11::D3D11CreateDevice(
            None,
            Direct3D::D3D_DRIVER_TYPE_HARDWARE,
            None,
            Direct3D11::D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            &[],
            Direct3D11::D3D11_SDK_VERSION,
            &mut device,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };

    Ok(device.expect("failed creating d3d11device"))
}

pub fn get_d3d_context(d3d_device: &ID3D11Device) -> Result<ID3D11DeviceContext> {
    unsafe {
        let mut d3d_context = None;
        d3d_device.GetImmediateContext(&mut d3d_context);
        Ok(d3d_context.expect("error getting d3d_context"))
    }
}

pub fn create_direct3d_device(d3d_device: &ID3D11Device) -> Result<IDirect3DDevice> {
    let dxgi_device: IDXGIDevice = d3d_device.cast()?;
    let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(Some(dxgi_device))? };
    inspectable.cast()
}

pub fn create_media_encoding_profile(
    size: SizeInt32,
    framerate: Framerate,
    bitrate: Bitrate,
) -> Result<MediaEncodingProfile> {
    let encoding_profile = MediaEncodingProfile::new()?;
    encoding_profile
        .Container()?
        .SetSubtype(MediaEncodingSubtypes::Mpeg4()?)?;
    encoding_profile
        .Video()?
        .SetSubtype(MediaEncodingSubtypes::H264()?)?;
    encoding_profile
        .Video()?
        .SetWidth(size.Width.try_into().unwrap())?;
    encoding_profile
        .Video()?
        .SetHeight(size.Height.try_into().unwrap())?;
    encoding_profile.Video()?.SetBitrate(bitrate.into())?;
    encoding_profile
        .Video()?
        .FrameRate()?
        .SetNumerator(framerate.into())?;
    encoding_profile.Video()?.FrameRate()?.SetDenominator(1)?;
    encoding_profile
        .Video()?
        .PixelAspectRatio()?
        .SetNumerator(1)?;
    encoding_profile
        .Video()?
        .PixelAspectRatio()?
        .SetDenominator(1)?;
    Ok(encoding_profile)
}

pub fn get_media_stream_source(size: &SizeInt32) -> Result<MediaStreamSource> {
    let video_properties = VideoEncodingProperties::CreateUncompressed(
        MediaEncodingSubtypes::Bgra8()?,
        size.Width.try_into().unwrap(),
        size.Height.try_into().unwrap(),
    )?;
    let video_descriptor = VideoStreamDescriptor::Create(video_properties)?;
    let media_stream_source = MediaStreamSource::CreateFromDescriptor(video_descriptor)?;
    media_stream_source.SetBufferTime(Duration::ZERO)?;
    Ok(media_stream_source)
}

pub fn create_media_transcoder() -> Result<MediaTranscoder> {
    let transcoder = MediaTranscoder::new()?;
    transcoder.SetHardwareAccelerationEnabled(true)?;
    Ok(transcoder)
}

pub fn get_d3d_interface_from_object<S: Interface, R: Interface + Abi>(object: &S) -> Result<R> {
    let access: IDirect3DDxgiInterfaceAccess = object.cast()?;
    let object = unsafe { access.GetInterface::<R>()? };
    Ok(object)
}

pub fn get_texture_description(texture: &ID3D11Texture2D) -> D3D11_TEXTURE2D_DESC {
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe { texture.GetDesc(&mut desc) };
    return desc;
}

pub fn create_output_stream() -> Result<IRandomAccessStream> {
    let folder = KnownFolders::VideosLibrary()?;
    let filename = chrono::offset::Local::now().format("%Y-%m-%d_%H-%M-%S.mp4");
    let filename = format!("{}", filename);

    let file = folder
        .CreateFileAsync(
            HSTRING::from(filename),
            CreationCollisionOption::GenerateUniqueName,
        )?
        .get()?;

    let output_stream = file.OpenAsync(FileAccessMode::ReadWrite)?.get()?;
    Ok(output_stream)
}
