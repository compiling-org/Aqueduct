use aqueduct::{Receiver, Packet, Discovery, PixelFormat};
use std::time::Duration;
use tokio::time;
use minifb::{Window, WindowOptions, Key};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let discovery = Discovery::new()?;
    
    println!("Browsing for sources...");
    
    // In a real app, we'd wait for a specific source.
    // Here we'll just manually connect to localhost:9030 for simplicity 
    // because discovery runs in a separate thread and callback.
    // But let's try to use discovery output.
    
    discovery.browse_sources(|event| {
        println!("Discovery Event: {:?}", event);
    })?;

    // Wait a bit for discovery (optional)
    time::sleep(Duration::from_secs(2)).await;

    println!("Connecting to localhost:9030...");
    let mut receiver = Receiver::connect("127.0.0.1:9030").await?;

    let mut frame_count = 0;
    let mut window: Option<Window> = None;
    let mut fb: Vec<u32> = Vec::new();
    let mut win_w: u32 = 0;
    let mut win_h: u32 = 0;

    loop {
        match receiver.receive().await {
            Ok(packet) => {
                match packet {
                    Packet::Video(frame) => {
                        println!("Received Video: {}x{} [{:?}] @ {:?} ({} bytes)", 
                            frame.width, frame.height, frame.format, frame.timestamp, frame.data.len());
                        
                        if frame.format != PixelFormat::BGRA {
                            eprintln!("Unsupported pixel format for preview: {:?}", frame.format);
                            frame_count += 1;
                            continue;
                        }

                        let width = frame.width;
                        let height = frame.height;
                        let data = &frame.data;

                        if window.is_none() || win_w != width || win_h != height {
                            win_w = width;
                            win_h = height;
                            let title = format!("Aqueduct Preview - {}x{}", width, height);
                            match Window::new(&title, width as usize, height as usize, WindowOptions::default()) {
                                Ok(w) => window = Some(w),
                                Err(e) => {
                                    eprintln!("Failed to create window: {}", e);
                                    frame_count += 1;
                                    continue;
                                }
                            }
                            fb.resize((width as usize) * (height as usize), 0);
                        }

                        if let Some(win) = window.as_mut() {
                            if !win.is_open() || win.is_key_down(Key::Escape) {
                                println!("Window closed by user.");
                                return Ok(());
                            }

                            if fb.len() != (width as usize) * (height as usize) {
                                fb.resize((width as usize) * (height as usize), 0);
                            }

                            // Convert BGRA -> 0x00RRGGBB for minifb
                            // data is tightly packed BGRA
                            let mut pi = 0usize;
                            for chunk in data.chunks_exact(4) {
                                let b = chunk[0] as u32;
                                let g = chunk[1] as u32;
                                let r = chunk[2] as u32;
                                fb[pi] = (r << 16) | (g << 8) | b;
                                pi += 1;
                            }

                            if let Err(e) = win.update_with_buffer(&fb, width as usize, height as usize) {
                                eprintln!("Window update error: {}", e);
                            }
                        }
                        frame_count += 1;
                    }
                    Packet::Audio(frame) => {
                        println!("Received Audio: {}Hz {}ch @ {:?} ({} samples)", 
                            frame.sample_rate, frame.channels, frame.timestamp, frame.data.len() / 4);
                    }
                    Packet::Metadata(frame) => {
                        println!("Received Metadata: {}", frame.content);
                    }
                }
            }
            Err(e) => {
                eprintln!("Receive error: {}", e);
                break;
            }
        }
    }

    Ok(())
}
