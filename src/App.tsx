// src/App.tsx
// React + Tauri UI for audiocap recorder library
// Drop this file into your React app (e.g. src/App.tsx).
// Requires @tauri-apps/api installed.

import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";

type Recording = {
  id: string;
  file: string;
  startedAt: string;
  pid?: number;
  status: "recording" | "done";
};

export default function App() {
  return (
    <div className="min-h-screen bg-gray-50 p-6 font-sans">
      <div className="max-w-4xl mx-auto bg-white rounded-lg shadow p-6">
        <Header />
        <main className="mt-6 grid grid-cols-1 md:grid-cols-2 gap-6">
          <div>
            <RecorderPanel />
            <DevicePanel />
            <SettingsPanel />
          </div>

          <div>
            <RecordingsList />
            <SummaryPanel />
          </div>
        </main>
      </div>
    </div>
  );
}

function Header() {
  return (
    <header className="flex items-center justify-between">
      <div>
        <h1 className="text-2xl font-bold">AudioCap — Recorder</h1>
        <p className="text-sm text-gray-600">Start/stop recordings, save files, and summarize meetings.</p>
      </div>
      <div className="text-right text-sm text-gray-500">v0.1</div>
    </header>
  );
}

function RecorderPanel() {
  const [recording, setRecording] = useState<boolean>(false);
  const [background, setBackground] = useState<boolean>(true);
  const [duration, setDuration] = useState<string>("");
  const [lastResult, setLastResult] = useState<any>(null);
  const [isWorking, setIsWorking] = useState(false);
  const [outputPath, setOutputPath] = useState<string | null>(null);

  useEffect(() => {
    // optional: poll status every 3s to keep UI in sync
    let t: any;
    if (recording) {
      t = setInterval(() => checkStatus().then((s) => console.debug("status poll", s)), 3000);
    }
    return () => clearInterval(t);
  }, [recording]);

  async function checkStatus() {
    try {
      const res = await invoke("status");
      return res;
    } catch (e) {
      console.warn("status check failed", e);
      return null;
    }
  }

  async function start() {
    setIsWorking(true);
    setLastResult(null);
    try {
      const payload = {
        output: outputPath || null,
        background,
        duration: duration || null,
        mic: null,
        system: null,
      };

      const res: any = await invoke("start_recording", payload as any);
      setLastResult(res);
      if (res && res.status === "started") {
        setRecording(true);
      } else if (res && res.status === "running") {
        setRecording(true);
      } else if (res && res.status === "done") {
        setRecording(false);
      }

      // If backend returned file path, add to session store
      if (res && res.file) {
        RecordingStore.add({ file: String(res.file) });
      }
    } catch (e: any) {
      alert(String(e));
      console.error(e);
    } finally {
      setIsWorking(false);
    }
  }

  async function stop() {
    setIsWorking(true);
    try {
      const res: any = await invoke("stop_recording");
      setLastResult(res);
      setRecording(false);
    } catch (e: any) {
      alert(String(e));
      console.error(e);
    } finally {
      setIsWorking(false);
    }
  }

  return (
    <section className="border rounded p-4 mb-4">
      <h2 className="font-semibold">Recorder</h2>

      <div className="mt-3 flex items-center gap-3">
        {!recording ? (
          <button
            className={`px-4 py-2 rounded text-white ${isWorking ? "bg-gray-400" : "bg-green-600"}`}
            onClick={start}
            disabled={isWorking}
          >
            🎙 Start
          </button>
        ) : (
          <button
            className={`px-4 py-2 rounded text-white ${isWorking ? "bg-gray-400" : "bg-red-600"}`}
            onClick={stop}
            disabled={isWorking}
          >
            ⏹ Stop
          </button>
        )}

        <label className="flex items-center gap-2">
          <input type="checkbox" checked={background} onChange={(e) => setBackground(e.target.checked)} />
          <span className="text-sm text-gray-700">Background</span>
        </label>

        <input
          type="text"
          placeholder="Duration (e.g. 00:10:00)"
          value={duration}
          onChange={(e) => setDuration(e.target.value)}
          className="px-2 py-1 border rounded text-sm"
        />

        <input
          type="text"
          placeholder="Optional output path (leave empty for auto)"
          value={outputPath || ""}
          onChange={(e) => setOutputPath(e.target.value || null)}
          className="px-2 py-1 border rounded text-sm flex-1"
        />
      </div>

      <div className="mt-3 text-xs text-gray-600">
        <div>Last result: <code>{lastResult ? JSON.stringify(lastResult) : "—"}</code></div>
      </div>
    </section>
  );
}

function DevicePanel() {
  const [output, setOutput] = useState<string>("");
  const [loading, setLoading] = useState(false);

  async function showDevices() {
    setLoading(true);
    try {
      // list_audio_devices prints to stdout/stderr. We call it so the OS logs will contain results.
      // Backend returns a small "done" string by default.
      const res: any = await invoke("list_audio_devices");
      setOutput(String(res));
      alert("Device list command executed. Check your terminal/console output for ffmpeg device list.");
    } catch (e: any) {
      setOutput(String(e));
      alert(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <section className="border rounded p-4 mb-4">
      <h3 className="font-medium">Devices</h3>
      <p className="text-sm text-gray-600">If you need to discover device names (for advanced users), press "Show devices" and inspect console logs.</p>
      <div className="mt-3 flex gap-2">
        <button className="px-3 py-2 bg-blue-600 text-white rounded" onClick={showDevices} disabled={loading}>
          {loading ? "Listing…" : "Show devices"}
        </button>
      </div>
      <pre className="mt-3 text-xs bg-gray-100 p-2 rounded h-24 overflow-auto">{output}</pre>
    </section>
  );
}

function SettingsPanel() {
  // This component stores user preferences locally (could be wired to Tauri settings/API)
  const [format, setFormat] = useState<string>("wav");
  const [wasapi, setWasapi] = useState<boolean>(false);

  return (
    <section className="border rounded p-4">
      <h3 className="font-medium">Settings</h3>
      <div className="mt-2 grid grid-cols-2 gap-2">
        <label className="text-sm">
          Format
          <select value={format} onChange={(e) => setFormat(e.target.value)} className="ml-2 border rounded px-2 py-1 text-sm">
            <option value="wav">WAV</option>
            <option value="mp3">MP3</option>
          </select>
        </label>

        <label className="text-sm">
          <input type="checkbox" checked={wasapi} onChange={(e) => setWasapi(e.target.checked)} />
          <span className="ml-2">Use WASAPI (Windows)</span>
        </label>
      </div>
    </section>
  );
}

/*
  RecordingsList: very small in-memory list for this session.
  For persistent listing, implement a Tauri command (e.g. `list_recordings`) that reads app_data/recordings.
*/
const RecordingStore = (() => {
  let recs: Recording[] = [];
  const subs: Array<() => void> = [];
  return {
    add: (entry: Partial<Recording>) => {
      const r: Recording = {
        id: Math.random().toString(36).slice(2, 9),
        file: entry.file || "",
        startedAt: entry.startedAt || new Date().toISOString(),
        pid: entry.pid,
        status: entry.status || "done",
      };
      recs.unshift(r);
      subs.forEach((s) => s());
      return r;
    },
    all: () => recs.slice(),
    subscribe: (cb: () => void) => {
      subs.push(cb);
      return () => {
        const i = subs.indexOf(cb);
        if (i >= 0) subs.splice(i, 1);
      };
    },
  };
})();

function RecordingsList() {
  const [list, setList] = useState<Recording[]>(RecordingStore.all());

  useEffect(() => {
    const unsub = RecordingStore.subscribe(() => setList(RecordingStore.all()));
    return unsub;
  }, []);

  return (
    <section className="border rounded p-4 mb-4">
      <h3 className="font-medium">Recordings (Session)</h3>
      <p className="text-sm text-gray-600">This list is in-memory for this session. Implement a backend command to list saved files persistently.</p>
      <ul className="mt-3 space-y-2 max-h-64 overflow-auto">
        {list.length === 0 && <li className="text-sm text-gray-500">No recordings yet.</li>}
        {list.map((r) => (
          <li key={r.id} className="flex items-center justify-between">
            <div>
              <div className="text-sm font-medium">{r.file.split("/").pop()}</div>
              <div className="text-xs text-gray-500">{new Date(r.startedAt).toLocaleString()}</div>
            </div>
            <div className="flex items-center gap-2">
              <button
                className="px-2 py-1 bg-gray-200 rounded text-xs"
                onClick={() => window.open("file://" + r.file)}
              >
                Open
              </button>
              <button
                className="px-2 py-1 bg-blue-600 text-white rounded text-xs"
                onClick={async () => {
                  // try to call summarize_meeting; may not be implemented in your backend yet
                  try {
                    const res: any = await invoke("summarize_meeting", { filePath: r.file });
                    alert(String(res));
                  } catch (e: any) {
                    alert("Summarize command not available: " + String(e));
                  }
                }}
              >
                ✨ Summarize
              </button>
            </div>
          </li>
        ))}
      </ul>
    </section>
  );
}

function SummaryPanel() {
  const [text, setText] = useState<string | null>(null);

  return (
    <section className="border rounded p-4">
      <h3 className="font-medium">Summary</h3>
      <div className="mt-3 text-sm text-gray-700">
        {text ? (
          <div className="whitespace-pre-wrap">{text}</div>
        ) : (
          <div className="text-gray-500">Click "Summarize" on a recording to get a summary here.</div>
        )}
      </div>
    </section>
  );
}

