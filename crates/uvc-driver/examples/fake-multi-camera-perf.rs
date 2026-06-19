use std::{
    env,
    error::Error,
    process::ExitCode,
    time::{Duration, Instant},
};

use uvc_core::{CameraConfig, CameraId, EngineError, FrameFormat, PixelFormat, frame_channel};
use uvc_driver::FakeMultiCameraEngine;

#[derive(Debug)]
struct Options {
    cameras: usize,
    seconds: u64,
    fps: u32,
    width: u32,
    height: u32,
    format: PixelFormat,
    capacity_frames: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            cameras: 4,
            seconds: 3,
            fps: 30,
            width: 640,
            height: 480,
            format: PixelFormat::Yuyv,
            capacity_frames: 256,
        }
    }
}

#[derive(Default)]
struct Metrics {
    frames: u64,
    bytes: u64,
    total_latency: Duration,
    max_latency: Duration,
    buffer_hits: u64,
    buffer_misses: u64,
}

struct BufferPool {
    buffers: Vec<Vec<u8>>,
    max_capacity: usize,
    hits: u64,
    misses: u64,
}

impl BufferPool {
    fn new(max_capacity: usize) -> Self {
        Self {
            buffers: Vec::new(),
            max_capacity,
            hits: 0,
            misses: 0,
        }
    }

    fn take(&mut self, len: usize) -> Vec<u8> {
        if let Some(index) = self
            .buffers
            .iter()
            .position(|buffer| buffer.capacity() >= len)
        {
            self.hits += 1;
            let mut buffer = self.buffers.remove(index);
            buffer.clear();
            buffer
        } else {
            self.misses += 1;
            Vec::with_capacity(len)
        }
    }

    fn recycle(&mut self, mut buffer: Vec<u8>) {
        if buffer.capacity() <= self.max_capacity {
            buffer.clear();
            self.buffers.push(buffer);
        }
    }

    fn hits(&self) -> u64 {
        self.hits
    }

    fn misses(&self) -> u64 {
        self.misses
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let options = parse_options(env::args().skip(1))?;
    let format = FrameFormat::new(options.format, options.width, options.height, options.fps)?;
    let configs = (0..options.cameras)
        .map(|index| {
            let camera_id = CameraId::new(format!("cam-{index}"))?;
            Ok(CameraConfig::new(camera_id, format))
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;

    let (sender, receiver) = frame_channel(options.capacity_frames);
    let mut engine = FakeMultiCameraEngine::spawn(configs, sender)?;
    let started = Instant::now();
    let metrics = receive_for_duration(&receiver, Duration::from_secs(options.seconds))?;
    let elapsed = started.elapsed();
    engine.stop_all()?;

    let frames_per_second = metrics.frames as f64 / elapsed.as_secs_f64();
    let bytes_per_second = metrics.bytes as f64 / elapsed.as_secs_f64();
    let avg_latency =
        Duration::from_secs_f64(metrics.total_latency.as_secs_f64() / metrics.frames.max(1) as f64);
    let reuse = metrics.buffer_hits as f64
        / (metrics.buffer_hits + metrics.buffer_misses).max(1) as f64
        * 100.0;

    println!(
        "cameras={} format={} expected_fps={} duration={:.3}s",
        options.cameras,
        format,
        options.fps,
        elapsed.as_secs_f64()
    );
    println!(
        "throughput frames={} fps={:.2} bytes={} mib_per_sec={:.2}",
        metrics.frames,
        frames_per_second,
        metrics.bytes,
        bytes_per_second / 1024.0 / 1024.0
    );
    println!(
        "latency avg_ms={:.3} max_ms={:.3}",
        avg_latency.as_secs_f64() * 1000.0,
        metrics.max_latency.as_secs_f64() * 1000.0
    );
    println!(
        "buffer_pool hits={} misses={} reuse={:.1}%",
        metrics.buffer_hits, metrics.buffer_misses, reuse
    );

    Ok(())
}

fn receive_for_duration(
    receiver: &uvc_core::FrameReceiver,
    duration: Duration,
) -> Result<Metrics, Box<dyn Error>> {
    let deadline = Instant::now() + duration;
    let mut metrics = Metrics::default();
    let mut pool = BufferPool::new(1024 * 1024);

    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout = remaining.min(Duration::from_millis(200));

        match receiver.recv_timeout(timeout) {
            Ok(frame) => {
                let age = frame.age();
                let bytes = frame.buffer().len();
                let mut data = pool.take(bytes);
                data.extend_from_slice(frame.buffer().as_slice());

                metrics.frames += 1;
                metrics.bytes += bytes as u64;
                metrics.total_latency += age;
                metrics.max_latency = metrics.max_latency.max(age);

                pool.recycle(data);
            }
            Err(EngineError::Timeout) => {}
            Err(error) => return Err(Box::new(error)),
        }
    }

    metrics.buffer_hits = pool.hits();
    metrics.buffer_misses = pool.misses();

    Ok(metrics)
}

fn parse_options<I>(args: I) -> Result<Options, Box<dyn Error>>
where
    I: IntoIterator<Item = String>,
{
    let mut options = Options::default();
    let mut args = args.into_iter();

    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--cameras" => options.cameras = parse_next(&mut args, "--cameras")?,
            "--seconds" => options.seconds = parse_next(&mut args, "--seconds")?,
            "--fps" => options.fps = parse_next(&mut args, "--fps")?,
            "--width" => options.width = parse_next(&mut args, "--width")?,
            "--height" => options.height = parse_next(&mut args, "--height")?,
            "--capacity-frames" => {
                options.capacity_frames = parse_next(&mut args, "--capacity-frames")?
            }
            "--format" => {
                let value = args.next().ok_or("--format requires a value")?;
                options.format = value.parse()?;
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => return Err(format!("unknown option `{other}`").into()),
        }
    }

    if options.cameras == 0 {
        return Err("--cameras must be greater than zero".into());
    }

    if options.seconds == 0 {
        return Err("--seconds must be greater than zero".into());
    }

    if options.fps == 0 {
        return Err("--fps must be greater than zero".into());
    }

    if options.width == 0 {
        return Err("--width must be greater than zero".into());
    }

    if options.height == 0 {
        return Err("--height must be greater than zero".into());
    }

    if options.capacity_frames == 0 {
        return Err("--capacity-frames must be greater than zero".into());
    }

    Ok(options)
}

fn parse_next<T>(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<T, Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: Error + 'static,
{
    let value = args
        .next()
        .ok_or_else(|| format!("{flag} requires a value"))?;
    value
        .parse::<T>()
        .map_err(|error| format!("invalid value for {flag}: {error}").into())
}

fn print_usage() {
    println!(
        "uvc-driver fake-multi-camera-perf [--cameras N] [--seconds N] [--fps N] [--width N] [--height N] [--format mjpeg|yuyv|h264|nv12|rgba] [--capacity-frames N]"
    );
}
