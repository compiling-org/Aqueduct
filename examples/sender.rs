use aqueduct::{Sender, VideoFrame, PixelFormat, FrameFlags, Packet, Discovery, AudioFrame, SineWaveGenerator, MetadataFrame};
use bytes::Bytes;
use std::time::{Duration, Instant};
use tokio::time;
use xcap::Monitor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Start discovery service
    let discovery = Discovery::new()?;
    discovery.register_source("MyComputer", "Screen+Audio Capture", 9000)?;

    // Start TCP Sender
    // Ports 8922-9021 are excluded on this machine. Using 9030.
    let sender = Sender::new(9030).await?;
    
    println!("Sender running on port 9030...");
    
    // Get primary monitor
    let monitors = Monitor::all()?;
    if monitors.is_empty() {
        return Err("No monitors found".into());
    }
    let monitor = monitors.first().unwrap();
    println!("Capturing monitor: {:?}", monitor.name());
    
    // Audio Generator (440Hz beep, 48kHz, Stereo)
    let mut audio_gen = SineWaveGenerator::new(440.0, 48000, 2);

    let start_time = Instant::now();
    let mut interval = time::interval(Duration::from_millis(33)); // ~30fps
    let mut frame_count = 0;

    loop {
        interval.tick().await;
        
        let timestamp = start_time.elapsed();

        // 1. Capture Video
        // xcap returns image::RgbaImage (Vec<u8> in RGBA format)
        let image = match monitor.capture_image() {
            Ok(img) => img,
            Err(e) => {
                eprintln!("Capture error: {}", e);
                continue;
            }
        };

        let width = image.width();
        let height = image.height();
        
        // Convert to Bytes. 
        // xcap returns RGBA8.
        let mut raw_data = image.into_raw();
        
        // Swap R and B to make it BGRA (compatible with our PixelFormat::BGRA)
        for chunk in raw_data.chunks_exact_mut(4) {
             let r = chunk[0];
             let b = chunk[2];
             chunk[0] = b;
             chunk[2] = r;
        }

        let data = Bytes::from(raw_data);
        
        let video_frame = VideoFrame {
            width,
            height,
            format: PixelFormat::BGRA,
            flags: FrameFlags::default(),
            timestamp,
            data,
        };
        
        if let Err(e) = sender.send(Packet::Video(video_frame)) {
            eprintln!("Error sending video frame: {}", e);
        }
        
        // 2. Generate Audio
        // Generate ~33ms of audio (48000 * 0.033 = 1584 samples)
        // Let's use exactly 1600 samples for simplicity (33.33ms)
        let audio_samples = 1600;
        let audio_data = audio_gen.generate(audio_samples);
        
        let audio_frame = AudioFrame {
            sample_rate: 48000,
            channels: 2,
            timestamp,
            data: audio_data,
        };
        
        if let Err(e) = sender.send(Packet::Audio(audio_frame)) {
             eprintln!("Error sending audio frame: {}", e);
        }

        // 3. Send Metadata (every 30 frames ~ 1s)
        if frame_count % 30 == 0 {
            let metadata = MetadataFrame {
                timestamp,
                content: format!("<tally><on_program>true</on_program><source>{}</source></tally>", monitor.name()),
            };
            if let Err(e) = sender.send(Packet::Metadata(metadata)) {
                 eprintln!("Error sending metadata: {}", e);
            }
        }
        
        frame_count += 1;
    }
}
