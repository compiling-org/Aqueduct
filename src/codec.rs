use crate::error::Result;
use bytes::{Bytes, BytesMut, BufMut};
use crate::protocol::VideoFrame;
use lz4_flex::block::get_maximum_output_size;

pub trait VideoEncoder {
    fn encode(&mut self, frame: &VideoFrame) -> Result<Bytes>;
    fn encode_into(&mut self, frame: &VideoFrame, dst: &mut BytesMut) -> Result<()>;
}

pub trait VideoDecoder {
    fn decode(&mut self, data: &[u8]) -> Result<Bytes>;
    fn decode_into(&mut self, data: &[u8], dst: &mut BytesMut) -> Result<()>;
}

// Implementation of LZ4 codec as a placeholder for VMX
pub struct Lz4Codec;

impl Lz4Codec {
    pub fn new() -> Self {
        Self
    }
}

impl VideoEncoder for Lz4Codec {
    fn encode(&mut self, frame: &VideoFrame) -> Result<Bytes> {
        let mut dst = BytesMut::with_capacity(frame.data.len() + 8); // Heuristic
        self.encode_into(frame, &mut dst)?;
        Ok(dst.freeze())
    }

    fn encode_into(&mut self, frame: &VideoFrame, dst: &mut BytesMut) -> Result<()> {
        // Use lz4_flex's helper to get the exact maximum size required
        let max_len = get_maximum_output_size(frame.data.len());
        
        // Ensure capacity for size header (4 bytes) + compressed data
        let total_required = 4 + max_len;
        
        // Reserve space if needed
        // Note: reserve takes the *additional* capacity required
        if dst.capacity() < dst.len() + total_required {
             dst.reserve(total_required);
        }
        
        // Write uncompressed size header
        dst.put_u32_le(frame.data.len() as u32);
        
        // Compress directly into dst
        // We resize to accommodate worst case, then truncate to actual size
        let start_len = dst.len();
        dst.resize(start_len + max_len, 0);
        
        let compressed_size = lz4_flex::compress_into(&frame.data, &mut dst[start_len..])
            .map_err(|e| crate::error::AqueductError::Protocol(format!("Compression error: {}", e)))?;
            
        dst.truncate(start_len + compressed_size);
        Ok(())
    }
}

impl VideoDecoder for Lz4Codec {
    fn decode(&mut self, data: &[u8]) -> Result<Bytes> {
        // Read uncompressed size
        if data.len() < 4 {
             return Err(crate::error::AqueductError::Protocol("Data too short for header".to_string()));
        }
        let uncompressed_size = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        
        let mut dst = BytesMut::with_capacity(uncompressed_size);
        self.decode_into(data, &mut dst)?;
        Ok(dst.freeze())
    }

    fn decode_into(&mut self, data: &[u8], dst: &mut BytesMut) -> Result<()> {
        if data.len() < 4 {
             return Err(crate::error::AqueductError::Protocol("Data too short for header".to_string()));
        }
        // lz4_flex::decompress_into needs the exact uncompressed size usually?
        // decompress_size_prepended handles the header.
        // But it allocates.
        
        // We use `decompress_into` (without size prepended logic, we handle header manually).
        let uncompressed_size = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        
        // Ensure dst has space
        if dst.capacity() < uncompressed_size {
             dst.reserve(uncompressed_size - dst.len());
        }
        
        // We need to write into `dst`.
        // Safe way: resize to uncompressed_size (zeroes it), then decompress into slice.
        // Zeroing is cost.
        // unsafe { dst.set_len(uncompressed_size) } is the way to avoid zeroing but requires `unsafe`.
        // For this task, let's just use `resize` (safe) first.
        dst.resize(uncompressed_size, 0);
        
        let size = lz4_flex::decompress_into(&data[4..], dst.as_mut())
             .map_err(|e| crate::error::AqueductError::Protocol(format!("Decompression error: {}", e)))?;
             
        if size != uncompressed_size {
             // Should not happen if header is correct
        }
        
        Ok(())
    }
}
