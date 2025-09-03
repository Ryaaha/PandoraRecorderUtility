// examples/cli.rs
use audiocap_lib::{build_ffmpeg_args, check_ffmpeg, next_filename, start_background, start_foreground, list_devices, stop_pid, is_pid_alive, RecorderConfig, Container};
use std::path::PathBuf;
use anyhow::Result;
use std::fs;

fn main() -> Result<()> {
    // Example: start background recording
    check_ffmpeg()?;
    let mut cfg = RecorderConfig::default();
    cfg.format = Container::Wav;
    fs::create_dir_all(&cfg.out_dir)?;

    let outfile = next_filename(&cfg.out_dir, cfg.format);
    let args = build_ffmpeg_args(&cfg, &outfile, None, None, None)?;
    let pid = start_background(&args)?;
    println!("started background pid={} file={}", pid, outfile.display());

    // Later you can stop:
    // let stopped = stop_pid(pid)?;
    // println!("stop result: {}", stopped);

    Ok(())
}

