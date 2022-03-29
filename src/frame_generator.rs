use std::{
    sync::mpsc::{channel, Receiver, Sender, TryRecvError},
    time::Duration,
};

use windows::{
    core::{IInspectable, Result},
    Foundation::TypedEventHandler,
    Graphics::{
        Capture::{
            Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem,
            GraphicsCaptureSession,
        },
        DirectX::DirectXPixelFormat,
        SizeInt32,
    },
    Win32::Graphics::Direct3D11::ID3D11Device,
};

use crate::utils;

pub struct CaptureFrameGenerator {
    _d3d_device: ID3D11Device,
    _item: GraphicsCaptureItem,
    frame_pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    sender: Sender<Option<Direct3D11CaptureFrame>>,
    receiver: Receiver<Option<Direct3D11CaptureFrame>>,
}

impl CaptureFrameGenerator {
    pub fn new(
        d3d_device: ID3D11Device,
        item: GraphicsCaptureItem,
        size: SizeInt32,
    ) -> Result<Self> {
        let device = utils::create_direct3d_device(&d3d_device)?;
        let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            1,
            size,
        )?;
        device.Close()?;

        let session = frame_pool.CreateCaptureSession(&item)?;

        let (sender, receiver) = channel();
        frame_pool.FrameArrived(
            TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new({
                let sender = sender.clone();
                move |frame_pool, _| {
                    let frame_pool = frame_pool.as_ref().unwrap();
                    let frame = frame_pool.TryGetNextFrame()?;
                    if sender.send(Some(frame)).is_err() {
                        frame_pool.Close()?;
                    }
                    Ok(())
                }
            }),
        )?;

        Ok(Self {
            _d3d_device: d3d_device,
            _item: item,
            frame_pool,
            session,
            sender,
            receiver,
        })
    }

    pub fn session(&self) -> &GraphicsCaptureSession {
        &self.session
    }

    pub fn sender(&self) -> Sender<Option<Direct3D11CaptureFrame>> {
        self.sender.clone()
    }

    pub fn try_get_next_frame(&mut self) -> Result<Option<Direct3D11CaptureFrame>> {
        // wait for at least one frame or cahannel disconnect
        let mut last_item;
        match self.receiver.recv_timeout(Duration::from_secs(1)) {
            Ok(item) => last_item = item,
            Err(_) => return Ok(None),
        }

        // consume all buffered frames
        loop {
            // unless there is a None in it, then return that to close the transcoder
            if last_item.is_none() {
                return Ok(None);
            }
            // when there are no frames left return the last one
            match self.receiver.try_recv() {
                Ok(item) => {
                    let _ = last_item.expect("this error should never happen").Close();
                    last_item = item
                }
                Err(_) => return Ok(last_item),
            }
        }
    }
}

impl Drop for CaptureFrameGenerator {
    fn drop(&mut self) {
        self.session.Close().unwrap();
        self.frame_pool.Close().unwrap();
    }
}
