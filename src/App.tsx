// src/App.tsx
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Recording = {
  id: string;
  file: string;
  startedAt: string;
  pid?: number;
  status: "recording" | "done";
};

// Simple in-memory store for recordings
const RecordingStore = {
  items: [] as Recording[],
  add(rec: { file: string }) {
    const newRec: Recording = {
      id: String(Date.now()),
      file: rec.file,
      startedAt: new Date().toISOString(),
      status: "done",
    };
    this.items.push(newRec);
  },
  all() {
    return this.items;
  },
};

function DevicePanel() {
  const [output, setOutput] = useState("");
  const [loading, setLoading] = useState(false);

  async function showDevices() {
    setLoading(true);
    try {
      const res: any = await invoke("list_audio_devices");
      setOutput(JSON.stringify(res, null, 2));
      alert("Device list command executed. Check logs for details.");
    } catch (e: any) {
      setOutput(String(e));
      alert(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <section className="rounded-xl border bg-white p-4 shadow-sm">
      <h3 className="font-medium text-lg">Devices</h3>
      <div className="mt-3 flex flex-col gap-3">
        <button
          onClick={showDevices}
          disabled={loading}
          className={`px-4 py-2 rounded-lg text-white font-medium transition ${
            loading ? "bg-gray-400" : "bg-blue-600 hover:bg-blue-700"
          }`}
        >
          {loading ? "Loading..." : "Show Devices"}
        </button>
        <pre className="text-xs text-gray-600 whitespace-pre-wrap break-words bg-gray-50 p-2 rounded">
          {output || "No devices listed yet."}
        </pre>
      </div>
    </section>
  );
}

function SummaryPanel() {
  const [text] = useState<string | null>(null);

  return (
    <section className="rounded-xl border bg-white p-4 shadow-sm">
      <h3 className="font-medium text-lg">Summary</h3>
      <div className="mt-3 text-sm text-gray-700">
        {text ? (
          <div className="whitespace-pre-wrap">{text}</div>
        ) : (
          <div className="text-gray-400">
            Click "Summarize" on a recording to see the summary here.
          </div>
        )}
      </div>
    </section>
  );
}

function SettingsPanel() {
  const [format, setFormat] = useState("wav");
  const [wasapi, setWasapi] = useState(false);

  return (
    <section className="rounded-xl border bg-white p-4 shadow-sm">
      <h3 className="font-medium text-lg">Settings</h3>
      <div className="mt-3 grid grid-cols-2 gap-4">
        <label className="text-sm flex items-center gap-2">
          Format
          <select
            value={format}
            onChange={(e) => setFormat(e.target.value)}
            className="rounded-lg border px-2 py-1 text-sm focus:ring focus:ring-blue-200"
          >
            <option value="wav">WAV</option>
            <option value="mp3">MP3</option>
          </select>
        </label>

        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={wasapi}
            onChange={(e) => setWasapi(e.target.checked)}
            className="rounded border-gray-300"
          />
          Use WASAPI (Windows)
        </label>
      </div>
    </section>
  );
}

function RecorderPanel() {
  const [recording, setRecording] = useState(false);
  const [background, setBackground] = useState(true);
  const [duration, setDuration] = useState("");
  const [lastResult, setLastResult] = useState<any>(null);
  const [isWorking, setIsWorking] = useState(false);
  const [outputPath, setOutputPath] = useState<string | null>(null);

  useEffect(() => {
    let t: any;
    if (recording) {
      t = setInterval(() => checkStatus().then(console.debug), 3000);
    }
    return () => clearInterval(t);
  }, [recording]);

  async function checkStatus() {
    try {
      return await invoke("status");
    } catch (e) {
      console.warn("status check failed", e);
      return null;
    }
  }

  async function start() {
    setIsWorking(true);
    setLastResult(null);
    try {
      const res: any = await invoke("start_recording", {
        output: outputPath || null,
        background,
        duration: duration || null,
        mic: null,
        system: null,
      });
      setLastResult(res);
      if (res?.status === "started" || res?.status === "running") {
        setRecording(true);
      }
      if (res?.file) RecordingStore.add({ file: String(res.file) });
    } catch (e: any) {
      alert(String(e));
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
    } finally {
      setIsWorking(false);
    }
  }

  return (
    <section className="border rounded-xl p-4 sm:p-6 bg-gray-50 shadow-sm">
      <h2 className="font-semibold text-lg">Recorder</h2>

      <div className="mt-4 flex flex-col gap-3">
        
        <div className="flex flex-wrap items-center gap-3">
  {/* Start button */}
  <button
    className={`px-5 py-2 rounded-lg text-white font-medium transition ${
      isWorking || recording
        ? "bg-gray-400 cursor-not-allowed"
        : "bg-green-600 hover:bg-green-700"
    }`}
    onClick={start}
    disabled={isWorking || recording}
  >
    🎙 Start
  </button>

  {/* Small Stop button (always visible when recording) */}
  {recording && (
    <button
      className={`px-3 py-1 rounded-md text-white text-sm font-medium transition ${
        isWorking
          ? "bg-gray-400 cursor-not-allowed"
          : "bg-red-600 hover:bg-red-700"
      }`}
      onClick={stop}
      disabled={isWorking}
    >
      ⏹ Stop
    </button>
  )}

  <label className="flex items-center gap-2 text-sm">
    <input
      type="checkbox"
      checked={background}
      onChange={(e) => setBackground(e.target.checked)}
      className="w-4 h-4"
    />
    <span>Background</span>
  </label>
</div>


        <input
          type="text"
          placeholder="Duration (e.g. 00:10:00)"
          value={duration}
          onChange={(e) => setDuration(e.target.value)}
          className="px-3 py-2 border rounded-lg text-sm w-full"
        />

        <input
          type="text"
          placeholder="Optional output path (leave empty for auto)"
          value={outputPath || ""}
          onChange={(e) => setOutputPath(e.target.value || null)}
          className="px-3 py-2 border rounded-lg text-sm w-full"
        />
      </div>

      <div className="mt-3 text-xs text-gray-600 break-words">
        <div>
          Last result:{" "}
          <code className="bg-gray-100 px-1 rounded">
            {lastResult ? JSON.stringify(lastResult) : "—"}
          </code>
        </div>
      </div>
    </section>
  );
}

function RecordingsList() {
  const [recs, setRecs] = useState<Recording[]>([]);

  useEffect(() => {
    setRecs(RecordingStore.all());
  }, []);

  return (
    <section className="rounded-xl border bg-white p-4 shadow-sm">
      <h3 className="font-medium text-lg">Recordings</h3>
      <div className="mt-3 space-y-2 text-sm">
        {recs.length === 0 ? (
          <div className="text-gray-400">No recordings yet.</div>
        ) : (
          recs.map((r) => (
            <div
              key={r.id}
              className="flex items-center justify-between rounded bg-gray-50 p-2"
            >
              <span className="truncate">{r.file}</span>
              <span className="text-xs text-gray-500">{r.status}</span>
            </div>
          ))
        )}
      </div>
    </section>
  );
}

function Header() {
  return (
    <header className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2">
      <div>
        <h1 className="text-2xl sm:text-3xl font-bold text-gray-800">
          🎧 AudioCap — Recorder
        </h1>
        <p className="text-sm text-gray-600">
          Start/stop recordings, save files, and summarize meetings.
        </p>
      </div>
      <div className="text-right text-xs sm:text-sm text-gray-500">v0.1</div>
    </header>
  );
}

export default function App() {
  return (
    <div className="min-h-screen bg-gradient-to-b from-gray-50 to-gray-100 p-4 sm:p-6 font-sans">
      <div className="max-w-6xl mx-auto bg-white rounded-2xl shadow-lg p-4 sm:p-8">
        <Header />
        <main className="mt-6 grid gap-6 md:grid-cols-2">
          {/* Left column */}
          <div className="flex flex-col gap-6">
            <RecorderPanel />
            <DevicePanel />
            <SettingsPanel />
          </div>

          {/* Right column */}
          <div className="flex flex-col gap-6">
            <RecordingsList />
            <SummaryPanel />
          </div>
        </main>
      </div>
    </div>
  );
}