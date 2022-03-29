use crate::resolution::Resolution;

#[derive(Debug, Copy, Clone)]
pub struct Bitrate(u32);

impl Bitrate {
    pub fn auto() -> Self {
        Bitrate(0)
    }
    pub fn kbit(kbit: u32) -> Self {
        Bitrate(kbit * 1000)
    }
    pub fn mbit(mbit: u32) -> Self {
        Bitrate(mbit * 1000000)
    }
    pub fn gbit(gbit: u32) -> Self {
        Bitrate(gbit * 1000000000)
    }

    pub fn get_default_bitrate(resolution: Resolution) -> Self {
        let mbit = match resolution {
            Resolution::Native => 15,
            Resolution::_720p => 5,
            Resolution::_1080p => 8,
            Resolution::_1440p => 16,
            Resolution::_2160p => 45,
            Resolution::_4320p => 175,
        };
        Self::mbit(mbit)
    }

    pub fn is_auto(&self) -> bool {
        self.0 > 0
    }
}

impl Into<u32> for Bitrate {
    fn into(self) -> u32 {
        self.0
    }
}
impl From<u32> for Bitrate {
    fn from(bitrate: u32) -> Self {
        Self(bitrate)
    }
}
