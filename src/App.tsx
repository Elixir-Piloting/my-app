import { invoke } from "@tauri-apps/api/core";
import { useRef, useState } from "react";

function App() {
  const [status, setStatus] = useState("Ready");
  const [raw, setRaw] = useState("");
  const [cleaned, setCleaned] = useState("");
  const [language, setLanguage] = useState("en");
  const [isRecording, setIsRecording] = useState(false);
  const recorderRef = useRef<MediaRecorder | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const streamRef = useRef<MediaStream | null>(null);

  async function startRecording() {
    try {
      setStatus("Requesting microphone...");
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      streamRef.current = stream;
      const mimeType = MediaRecorder.isTypeSupported("audio/webm")
        ? "audio/webm"
        : "audio/webm;codecs=opus";
      const recorder = new MediaRecorder(stream, { mimeType });
      chunksRef.current = [];

      recorder.ondataavailable = (event) => {
        if (event.data.size > 0) {
          chunksRef.current.push(event.data);
        }
      };

      recorder.onstop = async () => {
        setStatus("Processing audio...");
        const blob = new Blob(chunksRef.current, { type: mimeType });
        const reader = new FileReader();

        reader.onloadend = async () => {
          const dataUrl = reader.result as string;
          const base64 = dataUrl.split(",")[1] || "";
          try {
            const result: any = await invoke("transcribe_audio", {
              audioBase64: base64,
              language,
            });
            setRaw(result.raw || "");
            setCleaned(result.cleaned || "");
            // copy cleaned text to clipboard and paste
            const toCopy = result.cleaned || result.raw || "";
            if (toCopy) await navigator.clipboard.writeText(toCopy);
            setStatus("Pasting cleaned transcript...");
            if (toCopy) await invoke("paste_text", { text: toCopy });
            setStatus("Done");
          } catch (err) {
            setStatus("Error processing audio");
            console.error(err);
          }
        };

        reader.readAsDataURL(blob);
        setIsRecording(false);
      };

      recorder.start();
      recorderRef.current = recorder;
      setIsRecording(true);
      setStatus("Recording...");
    } catch (err) {
      setStatus("Microphone access denied");
      console.error(err);
    }
  }

  function stopRecording() {
    if (recorderRef.current) {
      recorderRef.current.stop();
      recorderRef.current = null;
    }
    if (streamRef.current) {
      streamRef.current.getTracks().forEach((track) => track.stop());
      streamRef.current = null;
    }
    setStatus("Stopped");
    setIsRecording(false);
  }

  return (
    <div className="min-h-screen bg-slate-50 flex items-center justify-center p-6">
      <div className="w-full max-w-3xl bg-white rounded-2xl shadow-lg p-8">
        <div className="flex items-start justify-between gap-6">
          <div>
            <h1 className="text-2xl font-semibold text-slate-900">Voice Dictation</h1>
            <p className="mt-2 text-sm text-slate-500">Start recording, speak, stop — result will be copied and pasted.</p>
          </div>
          <div className="flex items-center gap-3">
            <select
              value={language}
              onChange={(event) => setLanguage(event.target.value)}
              className="px-2 py-1 rounded-md border text-sm"
            >
              <option value="en">English (en)</option>
              <option value="es">Spanish (es)</option>
              <option value="fr">French (fr)</option>
              <option value="de">German (de)</option>
            </select>
            <button
              onClick={() => {
                if (isRecording) stopRecording();
                else startRecording();
              }}
              className={`px-2 py-1 rounded-md text-sm font-semibold ${isRecording ? 'bg-red-600 text-white' : 'bg-blue-600 text-white'}`}
            >
              {isRecording ? 'Stop' : 'Record'}
            </button>
          </div>
        </div>

        <div className="mt-6 grid grid-cols-1 md:grid-cols-2 gap-4">
          <div>
            <div className="text-sm font-medium text-slate-700">Raw Transcript</div>
            <div className="mt-2 p-4 bg-slate-100 rounded-lg min-h-[120px] text-slate-900 whitespace-pre-wrap">{raw || 'No transcript yet.'}</div>
          </div>
          <div>
            <div className="text-sm font-medium text-slate-700">Cleaned Transcript</div>
            <div className="mt-2 p-4 bg-slate-100 rounded-lg min-h-[120px] text-slate-900 whitespace-pre-wrap">{cleaned || 'No cleaned transcript yet.'}</div>
          </div>
        </div>

        <div className="mt-6">
          <div className="text-sm font-medium text-slate-700">Status</div>
          <div className="mt-2 p-3 bg-indigo-50 text-slate-800 rounded-md">{status}</div>
        </div>
      </div>
    </div>
  );
}

export default App;
