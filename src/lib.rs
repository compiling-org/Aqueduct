pub mod protocol;
pub mod discovery;
pub mod transport;
pub mod error;
pub mod codec;
pub mod audio_source;

pub use protocol::{Packet, VideoFrame, AudioFrame, MetadataFrame, PixelFormat, FrameFlags};
pub use discovery::Discovery;
pub use transport::{Sender, Receiver};
pub use error::{AqueductError, Result};
pub use codec::{VideoEncoder, VideoDecoder, Lz4Codec};
pub use audio_source::SineWaveGenerator;
