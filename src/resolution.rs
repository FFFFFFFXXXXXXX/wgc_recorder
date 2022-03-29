use windows::Graphics::SizeInt32;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Resolution {
    Native,
    _720p,
    _1080p,
    _1440p,
    _2160p,
    _4320p,
}

impl Resolution {
    pub fn get_size(&self) -> Option<SizeInt32> {
        match self {
            Resolution::Native => None,
            Resolution::_720p => Some(SizeInt32 {
                Width: 1280,
                Height: 720,
            }),
            Resolution::_1080p => Some(SizeInt32 {
                Width: 1920,
                Height: 1080,
            }),
            Resolution::_1440p => Some(SizeInt32 {
                Width: 2560,
                Height: 1440,
            }),
            Resolution::_2160p => Some(SizeInt32 {
                Width: 3840,
                Height: 2160,
            }),
            Resolution::_4320p => Some(SizeInt32 {
                Width: 7680,
                Height: 4320,
            }),
        }
    }
}
