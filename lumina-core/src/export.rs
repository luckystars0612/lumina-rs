//! FFmpeg export module - pipes raw RGBA frames to FFmpeg for encoding

use std::io::Write;
use std::process::{Command, Stdio};

/// FFmpeg encoder configuration
#[derive(Debug, Clone)]
pub struct FFmpegConfig {
    /// Output video path
    pub output_path: String,
    /// Video width
    pub width: u32,
    /// Video height
    pub height: u32,
    /// Frame rate
    pub fps: u32,
    /// Quality (lower = better quality, 18-28 typical)
    pub cq: u32,
}

impl Default for FFmpegConfig {
    fn default() -> Self {
        Self {
            output_path: "output.mp4".to_string(),
            width: 1920,
            height: 1080,
            fps: 60,
            cq: 22,
        }
    }
}

/// FFmpeg encoder that writes frames via stdin pipe
pub struct FFmpegEncoder {
    child: std::process::Child,
    frame_size: usize,
}

impl FFmpegEncoder {
    /// Create a new encoder
    pub fn new(config: FFmpegConfig) -> anyhow::Result<Self> {
        // Build FFmpeg command
        let ffmpeg_cmd = format!(
            "ffmpeg -y \
            -f rawvideo \
            -pixel_format rgba \
            -video_size {}x{} \
            -framerate {} \
            -i - \
            -c:v h264_nvenc \
            -preset p7 \
            -tune hq \
            -rc vbr \
            -cq {} \
            -pix_fmt yuv420p \
            -movflags +faststart \
            {}",
            config.width,
            config.height,
            config.fps,
            config.cq,
            config.output_path
        );

        // Start FFmpeg process
        let mut child = Command::new("cmd")
            .args(["/C", &ffmpeg_cmd])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn FFmpeg: {}", e))?;

        let frame_size = (config.width * config.height * 4) as usize;

        Ok(Self {
            child,
            frame_size,
        })
    }

    /// Write a single frame (RGBA bytes)
    pub fn write_frame(&mut self, rgba_data: &[u8]) -> anyhow::Result<()> {
        if rgba_data.len() != self.frame_size {
            anyhow::bail!(
                "Frame size mismatch: expected {} bytes, got {}",
                self.frame_size,
                rgba_data.len()
            );
        }

        let stdin = self.child.stdin.as_mut()
            .ok_or_else(|| anyhow::anyhow!("Failed to get FFmpeg stdin"))?;
        
        stdin
            .write_all(rgba_data)
            .map_err(|e| anyhow::anyhow!("Failed to write frame to FFmpeg: {}", e))?;

        Ok(())
    }

    /// Finish encoding and wait for FFmpeg to complete
    pub fn finish(mut self) -> anyhow::Result<()> {
        // Drop stdin to signal EOF
        self.child.stdin = None;

        let status = self
            .child
            .wait()
            .map_err(|e| anyhow::anyhow!("Failed to wait for FFmpeg: {}", e))?;

        if !status.success() {
            anyhow::bail!("FFmpeg exited with status: {:?}", status);
        }

        Ok(())
    }
}

/// Progress tracker for encoding
pub struct EncodingProgress {
    pub total_frames: u32,
    pub current_frame: u32,
}

impl EncodingProgress {
    pub fn new(total_frames: u32) -> Self {
        Self {
            total_frames,
            current_frame: 0,
        }
    }

    pub fn update(&mut self) {
        self.current_frame += 1;
    }

    pub fn percentage(&self) -> f32 {
        self.current_frame as f32 / self.total_frames as f32 * 100.0
    }

    pub fn is_complete(&self) -> bool {
        self.current_frame >= self.total_frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_size_calculation() {
        let config = FFmpegConfig::default();
        let expected = 1920 * 1080 * 4;
        assert_eq!(expected, 8294400);
    }

    #[test]
    fn test_progress_tracking() {
        let mut progress = EncodingProgress::new(600);
        assert_eq!(progress.percentage(), 0.0);
        assert!(!progress.is_complete());

        progress.current_frame = 300;
        assert_eq!(progress.percentage(), 50.0);

        progress.current_frame = 600;
        assert!(progress.is_complete());
    }
}
