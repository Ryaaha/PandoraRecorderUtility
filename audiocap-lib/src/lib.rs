//! audiocap-lib
//!
//! Small library that builds and spawns ffmpeg for recording mic + system audio.
//! Designed to be used by Tauri, another Rust binary, or an orchestrator.

use anyhow::{anyhow, bail, Context, Result};
use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use which::which;

/// Container/format for output files
#[derive(Copy, Clone, Debug)]
pub enum Container {
    Wav,
    Mp3,
}

/// Minimal config controlling platform behavior.
#[derive(Clone, Debug)]
pub struct RecorderConfig {
    pub out_dir: PathBuf,
    pub format: Container,
    pub prefer_pipewire: bool,
    pub wasapi: bool,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            out_dir: PathBuf::from("recordings"),
            format: Container::Wav,
            prefer_pipewire: false,
            wasapi: false,
        }
    }
}

/// Ensure ffmpeg exists in PATH. Returns the path to ffmpeg if successful.
pub fn check_ffmpeg() -> Result<PathBuf> {
    let p = which("ffmpeg").context("ffmpeg is required but not found in PATH.")?;
    Ok(p)
}

/// Generate a timestamped filename in `out_dir` with the given format.
pub fn next_filename(out_dir: &Path, format: Container) -> PathBuf {
    let ts = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let ext = match format {
        Container::Wav => "wav",
        Container::Mp3 => "mp3",
    };
    out_dir.join(format!("recording_{}.{}", ts, ext))
}

/// Build ffmpeg command-line args given a RecorderConfig and optional overrides.
/// `duration`: Option like "00:10:00" or "600"
/// `mic_override`, `system_override`: optional device strings (platform dependent)
pub fn build_ffmpeg_args(
    cfg: &RecorderConfig,
    outfile: &Path,
    duration: Option<&str>,
    mic_override: Option<&str>,
    system_override: Option<&str>,
) -> Result<Vec<String>> {
    // Ensure out_dir exists
    fs::create_dir_all(&cfg.out_dir).context("failed to create output directory")?;

    platform_build_args(cfg, outfile, duration, mic_override, system_override)
}

/// Start a foreground recording (blocks until ffmpeg exits). Handles Ctrl-C to stop ffmpeg.
///
/// Returns () if successful (ffmpeg exit status was success).
pub fn start_foreground(args: &[String]) -> Result<()> {
    let child = Command::new("ffmpeg")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to spawn ffmpeg")?;

    let child_arc = Arc::new(Mutex::new(child));
    let child_for_handler = Arc::clone(&child_arc);

    ctrlc::set_handler(move || {
        if let Ok(mut guard) = child_for_handler.lock() {
            let _ = guard.kill();
        }
    })
    .context("failed to set ctrlc handler")?;

    let status = child_arc
        .lock()
        .unwrap()
        .wait()
        .context("ffmpeg failed to run")?;

    if !status.success() {
        bail!("ffmpeg exited with status: {}", status);
    }
    Ok(())
}

/// Start ffmpeg detached (background). Returns the child's PID on success.
///
/// Note: this uses `setsid()` on Unix and DETACHED_PROCESS flags on Windows.
pub fn start_background(args: &[String]) -> Result<u32> {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                // detach from controlling terminal
                libc::setsid();
                Ok(())
            });
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_FLAGS: u32 = 0x00000008 | 0x00000200; // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
        cmd.creation_flags(CREATE_FLAGS);
    }

    let child = cmd.spawn().context("failed to spawn detached ffmpeg")?;
    Ok(child.id())
}

/// Stop process by pid (cross platform). Returns true if command succeeded.
pub fn stop_pid(pid: u32) -> Result<bool> {
    #[cfg(unix)]
    {
        let status = Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .status()
            .context("failed to run kill")?;
        Ok(status.success())
    }

    #[cfg(windows)]
    {
        let status = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status()
            .context("failed to run taskkill")?;
        Ok(status.success())
    }
}

/// Check if a process is alive by pid.
pub fn is_pid_alive(pid: u32) -> Result<bool> {
    #[cfg(unix)]
    {
        let res = unsafe { libc::kill(pid as i32, 0) };
        Ok(res == 0)
    }

    #[cfg(windows)]
    {
        let output = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid)])
            .output()
            .context("failed to run tasklist")?;
        let s = String::from_utf8_lossy(&output.stdout);
        Ok(s.contains(&pid.to_string()))
    }
}

/// List audio devices for the current platform (prints to stdout/stderr).
/// `_prefer_pipewire` and `_wasapi` are kept for callers who may want to switch behavior.
pub fn list_devices(_prefer_pipewire: bool, _wasapi: bool) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        if _wasapi {
            eprintln!("=== Windows (WASAPI) devices ===");
            let _ = Command::new("ffmpeg")
                .args(["-hide_banner", "-f", "wasapi", "-list_devices", "true", "-i", "dummy"])
                .status();
        } else {
            eprintln!("=== Windows (DirectShow) devices ===");
            let _ = Command::new("ffmpeg")
                .args(["-hide_banner", "-list_devices", "true", "-f", "dshow", "-i", "dummy"])
                .status();
        }
        eprintln!("\nTips: For system audio you may need Stereo Mix or a virtual loopback device.");
    }

    #[cfg(target_os = "macos")]
    {
        eprintln!("=== macOS (AVFoundation) devices ===");
        let _ = Command::new("ffmpeg")
            .args(["-hide_banner", "-f", "avfoundation", "-list_devices", "true", "-i", ""])
            .status();
        eprintln!("Note: macOS requires a loopback device (BlackHole/Loopback) for system audio.");
    }

    #[cfg(target_os = "linux")]
    {
        eprintln!("=== Linux: PulseAudio (pactl) ===");
        let _ = Command::new("pactl").args(["list", "short", "sources"]).status();
        let _ = Command::new("pactl").args(["info"]).status();

        eprintln!("\n=== Linux: PipeWire (pw-cli) ===");
        let _ = Command::new("pw-cli").args(["ls", "Node"]).status();
    }

    Ok(())
}

/* ----------------- Internal helpers below ----------------- */

#[derive(Clone, Default)]
struct DevPrelude {
    system_prelude: Vec<String>,
    mic_prelude: Vec<String>,
}

#[derive(Clone, Default)]
struct FormatInputs {
    system: Vec<String>,
    mic: Vec<String>,
}

// Builds args using platform-specific choices and optional overrides.
fn platform_build_args(
    cfg: &RecorderConfig,
    outfile: &Path,
    duration_override: Option<&str>,
    mic_override: Option<&str>,
    system_override: Option<&str>,
) -> Result<Vec<String>> {
    // Choose device strings and preludes
    let (mic_spec, sys_spec, dev_opts, format_inputs) =
        platform_inputs_with_overrides(cfg, mic_override, system_override)?;

    let mut args: Vec<String> = Vec::new();
    args.extend(["-hide_banner".into(), "-y".into()]);

    if let Some(dur) = duration_override {
        args.extend(["-t".into(), dur.to_string()]);
    }

    // Input 0: system
    args.extend(dev_opts.system_prelude.clone());
    for s in &format_inputs.system {
        args.push(s.clone());
    }
    args.extend(["-i".into(), sys_spec]);

    // Input 1: mic
    args.extend(dev_opts.mic_prelude.clone());
    for s in &format_inputs.mic {
        args.push(s.clone());
    }
    args.extend(["-i".into(), mic_spec]);

    // Mix
    args.extend([
        "-filter_complex".into(),
        "amix=inputs=2:duration=longest:dropout_transition=2".into(),
    ]);

    // Output codec/container
    match cfg.format {
        Container::Wav => {
            args.extend(["-c:a".into(), "pcm_s16le".into()]);
        }
        Container::Mp3 => {
            args.extend(["-c:a".into(), "libmp3lame".into(), "-b:a".into(), "192k".into()]);
        }
    }

    args.push(outfile.to_string_lossy().to_string());
    Ok(args)
}

fn platform_inputs_with_overrides(
    cfg: &RecorderConfig,
    mic_override: Option<&str>,
    system_override: Option<&str>,
) -> Result<(String, String, DevPrelude, FormatInputs)> {
    #[cfg(target_os = "windows")]
    {
        let mut pre = DevPrelude::default();
        let fmt = FormatInputs::default();

        if cfg.wasapi {
            pre.system_prelude = vec!["-f".into(), "wasapi".into()];
            pre.mic_prelude = vec!["-f".into(), "wasapi".into()];

            let sys = if let Some(s) = system_override {
                s.to_string()
            } else {
                // WASAPI loopback via default: insert -loopback 1 before prelude
                let mut new_prelude = pre.system_prelude.clone();
                new_prelude.splice(0..0, vec!["-loopback".into(), "1".into()]);
                pre.system_prelude = new_prelude;
                "default".into()
            };

            let mic = mic_override.map(|s| s.to_string()).unwrap_or_else(|| "default".into());
            return Ok((mic, sys, pre, fmt));
        } else {
            pre.system_prelude = vec!["-f".into(), "dshow".into()];
            pre.mic_prelude = vec!["-f".into(), "dshow".into()];

            let sys = system_override.map(|s| s.to_string()).unwrap_or_else(|| "audio=virtual-audio-capturer".into());
            let mic = mic_override.map(|s| s.to_string()).unwrap_or_else(|| "audio=Microphone (default)".into());
            return Ok((mic, sys, pre, fmt));
        }
    }

    #[cfg(target_os = "macos")]
    {
        let mut pre = DevPrelude::default();
        pre.system_prelude = vec!["-f".into(), "avfoundation".into()];
        pre.mic_prelude = vec!["-f".into(), "avfoundation".into()];

        let mic = mic_override.map(|s| s.to_string()).unwrap_or_else(|| ":0".into());

        let sys = if let Some(s) = system_override { s.to_string() } else {
            return Err(anyhow!(
                "On macOS you must provide a loopback device via system_override (e.g. 'BlackHole 2ch')."
            ));
        };

        return Ok((mic, sys, pre, FormatInputs::default()));
    }

    #[cfg(target_os = "linux")]
    {
        let mut pre = DevPrelude::default();
        let mut fmt = FormatInputs::default();

        pre.system_prelude = vec!["-f".into(), "pulse".into()];
        pre.mic_prelude = vec!["-f".into(), "pulse".into()];

        let mic = mic_override.map(|s| s.to_string()).unwrap_or_else(|| "@DEFAULT_SOURCE@".into());
        let sys = system_override.map(|s| s.to_string()).unwrap_or_else(|| "@DEFAULT_SINK@.monitor".into());

        fmt.system.extend(vec!["-thread_queue_size".into(), "1024".into()]);
        fmt.mic.extend(vec!["-thread_queue_size".into(), "1024".into()]);

        return Ok((mic, sys, pre, fmt));
    }

    Err(anyhow!("Unsupported platform"))
}

