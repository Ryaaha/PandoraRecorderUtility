// src-tauri/src/main.rs
#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

use tauri::{Manager, State};
use std::path::PathBuf;
use anyhow::Context;

use audiocap_lib::{
    build_ffmpeg_args, check_ffmpeg, is_pid_alive, list_devices, next_filename, start_background,
    start_foreground, stop_pid, Container, RecorderConfig,
};

/// Small state struct so we can keep track of last background pid if desired.
/// (We still write a pidfile in app_data for cross-process control.)
struct AppState {
    // Optionally you can keep an in-memory handle; we only store last pid for convenience.
    last_pid: std::sync::Mutex<Option<u32>>,
}


#[tauri::command]
async fn start_recording(
    app_handle: tauri::AppHandle,
    // optional output path relative or absolute
    output: Option<String>,
    // background or foreground
    background: bool,
    // optional duration "00:10:00" or seconds "600"
    duration: Option<String>,
    mic: Option<String>,
    system: Option<String>,
) -> Result<serde_json::Value, String> {
    // ---- Simplified: require system ffmpeg on PATH ----
    // audiocap-lib::check_ffmpeg() returns Err if ffmpeg is not found on PATH.
    if let Err(e) = audiocap_lib::check_ffmpeg() {
        // Friendly error with short install hints
        let msg = format!(
            "ffmpeg not found in PATH: {}.\n\n\
             Please install ffmpeg and ensure it is available on your PATH.\n\n\
             Quick install examples:\n  • macOS (Homebrew): brew install ffmpeg\n  • Ubuntu/Debian: sudo apt update && sudo apt install ffmpeg\n  • Windows (Scoop/Chocolatey): scoop install ffmpeg  OR  choco install ffmpeg\n\n\
             After installing, restart the app and try again.",
            e
        );
        return Err(msg);
    }

    // Build RecorderConfig (you can pass additional flags from front-end if needed)
    let mut cfg = RecorderConfig::default();

    // Use app data directory as out_dir to avoid permission issues when app is installed
    let data_dir = app_handle
        .path_resolver()
        .app_data_dir()
        .ok_or("failed to resolve app data dir".to_string())?;
    cfg.out_dir = data_dir.join("recordings");
    cfg.format = Container::Wav; // or accept a parameter from frontend

    // ensure it exists
    std::fs::create_dir_all(&cfg.out_dir).map_err(|e| e.to_string())?;

    // Choose output file
    let outfile: PathBuf = match output {
        Some(s) => PathBuf::from(s),
        None => audiocap_lib::next_filename(&cfg.out_dir, cfg.format),
    };

    let args = build_ffmpeg_args(
        &cfg,
        &outfile,
        duration.as_deref(),
        mic.as_deref(),
        system.as_deref(),
    )
    .map_err(|e| e.to_string())?;

    if background {
        // Start background: spawn detached and write pidfile
        let pid = start_background(&args).map_err(|e| e.to_string())?;

        // write pidfile to app data dir
        let pidfile = data_dir.join("audiocap.pid");
        std::fs::write(&pidfile, pid.to_string()).map_err(|e| e.to_string())?;

        // store to in-memory state
        {
            let state = app_handle.state::<AppState>();
            *state.last_pid.lock().unwrap() = Some(pid);
        }

        Ok(serde_json::json!({
            "status": "started",
            "pid": pid,
            "file": outfile.to_string_lossy()
        }))
    } else {
        // Foreground mode: run ffmpeg in blocking thread so UI thread is not blocked
        let args_clone = args.clone();
        // spawn a blocking task so Tauri's async runtime isn't blocked
        tauri::async_runtime::spawn_blocking(move || {
            start_foreground(&args_clone)
                .map_err(|e| format!("ffmpeg failed: {}", e))
        })
        .await
        .map_err(|e| format!("task failed: {}", e))?
        .map_err(|e| e)?;

        Ok(serde_json::json!({
            "status": "done",
            "file": outfile.to_string_lossy()
        }))
    }
}










#[tauri::command]
async fn stop_recording(app_handle: tauri::AppHandle) -> Result<serde_json::Value, String> {
    // Read pidfile from app_data
    let data_dir = app_handle
        .path_resolver()
        .app_data_dir()
        .ok_or("failed to resolve app data dir")?;
    let pidfile = data_dir.join("audiocap.pid");
    if !pidfile.exists() {
        return Err("pidfile not found".into());
    }
    let pid_str = std::fs::read_to_string(&pidfile).map_err(|e| e.to_string())?;
    let pid: u32 = pid_str.trim().parse().map_err(|e| e.to_string())?;
    let ok = stop_pid(pid).map_err(|e| e.to_string())?;
    if ok {
        let _ = std::fs::remove_file(&pidfile);
        // clear in-memory state
        let state = app_handle.state::<AppState>();
        *state.last_pid.lock().unwrap() = None;
        Ok(serde_json::json!({ "status": "stopped", "pid": pid }))
    } else {
        Err("failed to stop process".into())
    }
}

#[tauri::command]
async fn status(app_handle: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let data_dir = app_handle
        .path_resolver()
        .app_data_dir()
        .ok_or("failed to resolve app data dir")?;
    let pidfile = data_dir.join("audiocap.pid");
    if !pidfile.exists() {
        return Ok(serde_json::json!({ "status": "no_pidfile" }));
    }
    let pid_str = std::fs::read_to_string(&pidfile).map_err(|e| e.to_string())?;
    let pid: u32 = pid_str.trim().parse().map_err(|e| e.to_string())?;
    let alive = is_pid_alive(pid).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "status": if alive { "running" } else { "not_running" }, "pid": pid }))
}

#[tauri::command]
async fn list_audio_devices() -> Result<String, String> {
    // run list_devices in blocking thread (it prints to stdout/stderr)
    tauri::async_runtime::spawn_blocking(move || {
        list_devices(false, false).map_err(|e| format!("list_devices failed: {}", e))
    })
    .await
    .map_err(|e| e.to_string())?
    .map(|_| "done".into())
}

fn add_resource_dir_to_path(app_handle: &tauri::AppHandle) -> anyhow::Result<()> {
    // If you bundle ffmpeg in resource dir, add that dir to PATH so `ffmpeg` resolves.
    if let Some(res_dir) = app_handle.path_resolver().resource_dir() {
        // resource_dir()/binaries should contain ffmpeg executable(s)
        let bin_dir = res_dir.join("binaries");
        if bin_dir.exists() {
            // set executable perms on unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let ff = bin_dir.join("ffmpeg");
                if ff.exists() {
                    let mut perms = std::fs::metadata(&ff)?.permissions();
                    perms.set_mode(0o755);
                    std::fs::set_permissions(ff, perms)?;
                }
            }
            // prepend to PATH (platform-specific separator)
            let path_key = "PATH";
            let sep = if cfg!(target_os = "windows") { ";" } else { ":" };
            let cur = std::env::var(path_key).unwrap_or_default();
            let new = format!("{}{}{}", bin_dir.display(), sep, cur);
            std::env::set_var(path_key, new);
        }
    }
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            last_pid: std::sync::Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_recording,
            status,
            list_audio_devices
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

