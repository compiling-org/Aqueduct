use bytes::Bytes;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PixelFormat {
    // 8-bit
    UYVY = 0, // 4:2:2
    UYVA = 1, // 4:2:2:4
    BGRA = 2, // 4:4:4
    NV12 = 3, // 4:2:0 Planar
    YV12 = 4, // 4:2:0 Planar
    
    // 16-bit
    P216 = 5, // Planar 4:2:2 16-bit
    PA16 = 6, // Planar 4:2:2:4 16-bit
}

impl PixelFormat {
    pub fn from_u8(n: u8) -> Option<Self> {
        match n {
            0 => Some(Self::UYVY),
            1 => Some(Self::UYVA),
            2 => Some(Self::BGRA),
            3 => Some(Self::NV12),
            4 => Some(Self::YV12),
            5 => Some(Self::P216),
            6 => Some(Self::PA16),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameFlags {
    pub alpha: bool,
    pub premultiplied: bool,
    pub high_bit_depth: bool,
}

impl Default for FrameFlags {
    fn default() -> Self {
        Self {
            alpha: false,
            premultiplied: false,
            high_bit_depth: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub flags: FrameFlags,
    pub timestamp: Duration,
    pub data: Bytes,
}

#[derive(Debug, Clone)]
pub struct AudioFrame {
    pub sample_rate: u32,
    pub channels: u32,
    pub timestamp: Duration,
    pub data: Bytes, // 32-bit float samples
}

#[derive(Debug, Clone)]
pub struct MetadataFrame {
    pub timestamp: Duration,
    pub content: String, // XML content
}

#[derive(Debug, Clone)]
pub enum Packet {
    Video(VideoFrame),
    Audio(AudioFrame),
    Metadata(MetadataFrame),
}
