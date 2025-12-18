use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::protocol::{Packet, VideoFrame, AudioFrame, MetadataFrame, PixelFormat, FrameFlags};
use crate::error::{Result, AqueductError};
use bytes::{BytesMut, Buf};
use std::sync::Arc;
use tokio::sync::broadcast;
use log::{info, error};

// Simple header: [Type: u8] [Length: u32]
// Types: 0x01 = Video, 0x02 = Audio, 0x03 = Metadata

const TYPE_VIDEO: u8 = 0x01;
const TYPE_AUDIO: u8 = 0x02;
const TYPE_METADATA: u8 = 0x03;

use crate::codec::{VideoEncoder, VideoDecoder, Lz4Codec};

#[derive(Clone)]
pub struct Sender {
    tx: broadcast::Sender<Arc<Packet>>,
    compression_buffer: Arc<std::sync::Mutex<BytesMut>>,
}

impl Sender {
    pub async fn new(port: u16) -> Result<Self> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
        let (tx, _) = broadcast::channel(16); // Buffer size 16 frames
        
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            if let Err(e) = run_accept_loop(listener, tx_clone).await {
                error!("Accept loop error: {}", e);
            }
        });

        Ok(Self { 
            tx,
            compression_buffer: Arc::new(std::sync::Mutex::new(BytesMut::with_capacity(8192))),
        })
    }

    pub fn send(&self, mut packet: Packet) -> Result<()> {
        // Encode video frames before sending
        if let Packet::Video(ref mut frame) = packet {
             let original_len = frame.data.len();
             let mut codec = Lz4Codec::new();
             
             // Try to reuse the compression buffer
             let compressed_data_bytes = if let Ok(mut buffer) = self.compression_buffer.lock() {
                 buffer.clear();
                 if let Err(e) = codec.encode_into(frame, &mut buffer) {
                     // Fallback to allocation if buffer error (should not happen)
                     log::warn!("Buffer encode failed, falling back: {}", e);
                     codec.encode(frame)?
                 } else {
                     // Success, freeze the data out
                     buffer.split().freeze()
                 }
             } else {
                 // Lock failed, fallback
                 codec.encode(frame)?
             };
             
             let compressed_len = compressed_data_bytes.len();
             frame.data = compressed_data_bytes;
             
             // Log every 60 frames or so to avoid spam, or just debug
             if log::log_enabled!(log::Level::Debug) {
                 log::debug!("Compressed frame: {} -> {} bytes ({:.2}%)", 
                    original_len, compressed_len, (compressed_len as f64 / original_len as f64) * 100.0);
             }
        }

        // Drop error if no receivers (Err(SendError) means no active subscribers, which is fine)
        let _ = self.tx.send(Arc::new(packet));
        Ok(())
    }
}

async fn run_accept_loop(listener: TcpListener, tx: broadcast::Sender<Arc<Packet>>) -> Result<()> {
    info!("Sender listening on {}", listener.local_addr()?);
    loop {
        let (socket, addr) = listener.accept().await?;
        info!("New receiver connected: {}", addr);
        let rx = tx.subscribe();
        tokio::spawn(handle_receiver(socket, rx));
    }
}

async fn handle_receiver(mut socket: TcpStream, mut rx: broadcast::Receiver<Arc<Packet>>) {
    loop {
        match rx.recv().await {
            Ok(packet) => {
                if let Err(e) = write_packet(&mut socket, &packet).await {
                    error!("Failed to send packet: {}", e);
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                info!("Receiver lagged by {} packets", n);
            }
            Err(broadcast::error::RecvError::Closed) => {
                break;
            }
        }
    }
}

async fn write_packet(socket: &mut TcpStream, packet: &Packet) -> Result<()> {
    match packet {
        Packet::Video(frame) => {
            socket.write_u8(TYPE_VIDEO).await?;
            // We need to serialize the frame metadata + data. 
            // Simplified: [Width: u32][Height: u32][Format: u8][Timestamp: u64 (micros)][DataLen: u32][Data]
            // buf was unused in previous draft, removed.
            
            // This is a placeholder serialization. Real OMT might differ.
            // TODO: Implement proper serialization based on OMT spec
            let len = 4 + 4 + 1 + 8 + frame.data.len() as u32; 
            socket.write_u32(len).await?;
            
            socket.write_u32(frame.width).await?;
            socket.write_u32(frame.height).await?;
            socket.write_u8(frame.format as u8).await?; // Assuming enum matches u8 representation
            socket.write_u64(frame.timestamp.as_micros() as u64).await?;
            socket.write_all(&frame.data).await?;
        }
        Packet::Audio(frame) => {
            socket.write_u8(TYPE_AUDIO).await?;
            let len = 4 + 4 + 8 + frame.data.len() as u32;
            socket.write_u32(len).await?;
            
            socket.write_u32(frame.sample_rate).await?;
            socket.write_u32(frame.channels).await?;
            socket.write_u64(frame.timestamp.as_micros() as u64).await?;
            socket.write_all(&frame.data).await?;
        }
        Packet::Metadata(frame) => {
            socket.write_u8(TYPE_METADATA).await?;
            let bytes = frame.content.as_bytes();
            let len = 8 + bytes.len() as u32;
            socket.write_u32(len).await?;
            
            socket.write_u64(frame.timestamp.as_micros() as u64).await?;
            socket.write_all(bytes).await?;
        }
    }
    Ok(())
}

pub struct Receiver {
    stream: TcpStream,
    buffer: BytesMut,
    decompress_buffer: BytesMut,
}

impl Receiver {
    pub async fn connect(addr: &str) -> Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        Ok(Self { 
            stream,
            buffer: BytesMut::with_capacity(4096),
            decompress_buffer: BytesMut::with_capacity(4096),
        })
    }

    pub async fn receive(&mut self) -> Result<Packet> {
        // Ensure we have the header (Type + Length = 1 + 4 = 5 bytes)
        loop {
            if self.buffer.len() >= 5 {
                break;
            }
            if self.stream.read_buf(&mut self.buffer).await? == 0 {
                 if self.buffer.is_empty() {
                     return Err(AqueductError::Io(std::io::Error::from(std::io::ErrorKind::UnexpectedEof)));
                 } else {
                     return Err(AqueductError::Protocol("Connection closed incomplete".to_string()));
                 }
            }
        }

        // Peek header
        let type_id = self.buffer[0];
        let mut len_bytes = [0u8; 4];
        len_bytes.copy_from_slice(&self.buffer[1..5]);
        let len = u32::from_be_bytes(len_bytes) as usize; // read_u32 is big endian? 
        // Wait, tokio read_u32 is Big Endian. My write_u32 was...
        // AsyncWriteExt::write_u32 is Big Endian.
        // So from_be_bytes is correct.

        // Safety check
        if len > 100_000_000 {
             return Err(AqueductError::Protocol("Packet too large".to_string()));
        }

        // Ensure we have the full packet
        let total_len = 5 + len;
        loop {
            if self.buffer.len() >= total_len {
                break;
            }
            // Reserve space if needed
            if self.buffer.capacity() < total_len {
                self.buffer.reserve(total_len - self.buffer.len());
            }
            if self.stream.read_buf(&mut self.buffer).await? == 0 {
                 return Err(AqueductError::Io(std::io::Error::from(std::io::ErrorKind::UnexpectedEof)));
            }
        }

        // Consume header
        self.buffer.advance(5);
        
        // Split payload
        let payload = self.buffer.split_to(len);
        // buffer now contains the *next* packet data (if any)
        
        let mut cursor = std::io::Cursor::new(payload.freeze());

        match type_id {
            TYPE_VIDEO => {
                // [Width: u32][Height: u32][Format: u8][Timestamp: u64][Data...]
                if len < 21 { return Err(AqueductError::Protocol("Video packet too short".to_string())); }
                
                let width = cursor.get_u32();
                let height = cursor.get_u32();
                let format_byte = cursor.get_u8();
                let timestamp_micros = cursor.get_u64();
                
                // Rest is data
                let data_pos = cursor.position() as usize;
                // cursor.into_inner() gives Bytes.
                let data_bytes = cursor.into_inner();
                let compressed_data = data_bytes.slice(data_pos..);

                // Map format_byte to enum
                let format = PixelFormat::from_u8(format_byte)
                    .ok_or_else(|| AqueductError::Protocol(format!("Invalid pixel format: {}", format_byte)))?;

                let mut codec = Lz4Codec::new();
                
                // Read uncompressed size from header to reserve space?
                // decode_into handles reading the size from the first 4 bytes of compressed_data
                // We use our persistent buffer.
                // We need to ensure it's empty of previous data but keeps capacity?
                // split() removes the data. So it is empty.
                
                // codec.decode_into appends to the buffer.
                codec.decode_into(&compressed_data, &mut self.decompress_buffer)?;
                
                // The data is now in self.decompress_buffer.
                // We split it out to get a Bytes object.
                let data = self.decompress_buffer.split().freeze();

                Ok(Packet::Video(VideoFrame {
                    width,
                    height,
                    format,
                    flags: FrameFlags::default(),
                    timestamp: std::time::Duration::from_micros(timestamp_micros),
                    data,
                }))
            }
            TYPE_AUDIO => {
                let sample_rate = cursor.get_u32();
                let channels = cursor.get_u32();
                let timestamp_micros = cursor.get_u64();
                
                let data_pos = cursor.position() as usize;
                let data = cursor.into_inner().slice(data_pos..);

                Ok(Packet::Audio(AudioFrame {
                    sample_rate,
                    channels,
                    timestamp: std::time::Duration::from_micros(timestamp_micros),
                    data,
                }))
            }
            TYPE_METADATA => {
                let timestamp_micros = cursor.get_u64();
                let data_pos = cursor.position() as usize;
                let content = String::from_utf8_lossy(&cursor.into_inner()[data_pos..]).to_string();

                Ok(Packet::Metadata(MetadataFrame {
                    timestamp: std::time::Duration::from_micros(timestamp_micros),
                    content,
                }))
            }
            _ => Err(AqueductError::Protocol(format!("Unknown packet type: {}", type_id))),
        }
    }
}
