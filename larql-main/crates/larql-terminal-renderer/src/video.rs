use crate::image_buffer::Image;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::Duration;

#[derive(Clone, Copy)]
pub struct VideoProbe {
    pub width: usize,
    pub height: usize,
    pub fps: f32,
}

pub struct VideoFrame {
    pub image: Image,
    pub index: u64,
    pub pts: Duration,
}

pub struct FfmpegDecoder {
    path: PathBuf,
    probe: VideoProbe,
    fps_cap: Option<f32>,
    child: Child,
    stdout: ChildStdout,
    frame_bytes: usize,
    frame_index: u64,
}

impl VideoProbe {
    pub fn open(path: &Path) -> Result<Self, String> {
        let output = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=width,height,avg_frame_rate",
                "-of",
                "default=noprint_wrappers=1:nokey=1",
            ])
            .arg(path)
            .output()
            .map_err(|e| format!("failed to run ffprobe: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "ffprobe failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        let stdout = String::from_utf8(output.stdout)
            .map_err(|e| e.to_string())?;
        let mut lines = stdout
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty());
        let width = lines
            .next()
            .ok_or("ffprobe missing width")?
            .parse::<usize>()
            .map_err(|e| format!("invalid video width: {e}"))?;
        let height = lines
            .next()
            .ok_or("ffprobe missing height")?
            .parse::<usize>()
            .map_err(|e| format!("invalid video height: {e}"))?;
        let fps = parse_fps(lines.next().ok_or("ffprobe missing avg_frame_rate")?)?;

        Ok(Self { width, height, fps })
    }
}

impl FfmpegDecoder {
    pub fn spawn(path: &Path, probe: VideoProbe, fps_cap: Option<f32>) -> Result<Self, String> {
        let mut command = Command::new("ffmpeg");
        command.args(["-v", "error", "-nostdin", "-i"]);
        command.arg(path);
        if let Some(fps) = fps_cap {
            command.args(["-vf", &format!("fps={fps}")]);
        }
        command.args(["-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"]);
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|e| format!("failed to run ffmpeg: {e}"))?;
        let stdout = child.stdout.take().ok_or("ffmpeg stdout unavailable")?;

        let frame_bytes = probe.width * probe.height * 3;
        Ok(Self {
            path: path.to_path_buf(),
            probe,
            fps_cap,
            child,
            stdout,
            frame_bytes,
            frame_index: 0,
        })
    }

    pub fn frame_duration(&self) -> Duration {
        let fps = self.fps_cap.unwrap_or(self.probe.fps).max(0.001);
        Duration::from_secs_f32(1.0 / fps)
    }

    pub fn next_frame(&mut self) -> Result<Option<VideoFrame>, String> {
        let mut buf = vec![0u8; self.frame_bytes];
        let mut read = 0usize;
        while read < self.frame_bytes {
            let n = self
                .stdout
                .read(&mut buf[read..])
                .map_err(|e| format!("ffmpeg read failed: {e}"))?;
            if n == 0 {
                if read == 0 {
                    return Ok(None);
                }
                return Err("ffmpeg ended mid-frame".to_string());
            }
            read += n;
        }

        let index = self.frame_index;
        self.frame_index += 1;
        Ok(Some(VideoFrame {
            image: Image::from_rgb(self.probe.width, self.probe.height, buf),
            index,
            pts: self.frame_duration().mul_f32(index as f32),
        }))
    }

    pub fn restart(&mut self) -> Result<(), String> {
        let _ = self.child.kill();
        let probe = VideoProbe {
            width: self.probe.width,
            height: self.probe.height,
            fps: self.probe.fps,
        };
        let replacement = Self::spawn(&self.path, probe, self.fps_cap)?;
        *self = replacement;
        Ok(())
    }
}

fn parse_fps(raw: &str) -> Result<f32, String> {
    if let Some((num, den)) = raw.split_once('/') {
        let num = num
            .parse::<f32>()
            .map_err(|e| format!("invalid fps numerator: {e}"))?;
        let den = den
            .parse::<f32>()
            .map_err(|e| format!("invalid fps denominator: {e}"))?;
        if den == 0.0 {
            return Err("ffprobe returned zero fps denominator".to_string());
        }
        Ok(num / den)
    } else {
        raw.parse::<f32>()
            .map_err(|e| format!("invalid fps value: {e}"))
    }
}
