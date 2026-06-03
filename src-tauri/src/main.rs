// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use arboard::Clipboard;
use base64::Engine;
use dotenvy::dotenv;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use reqwest::blocking::{multipart, Client};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::PathBuf;

fn main() {
    dotenv().ok();
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            hello,
            copy_to_clipboard,
            transcribe_audio,
            paste_text,
        ])
        .run(tauri::generate_context!())
        .expect("error running app");
}

#[tauri::command]
fn hello() -> String {
    "Rust is alive".to_string()
}

#[tauri::command]
fn copy_to_clipboard(text: String) -> Result<(), String> {
    let mut clip = Clipboard::new().map_err(|e| e.to_string())?;
    clip.set_text(text).map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct TranscriptionResult {
    raw: String,
    cleaned: String,
}

#[tauri::command]
#[allow(non_snake_case)]
fn transcribe_audio(audioBase64: String, language: String) -> Result<TranscriptionResult, String> {
    let api_key = std::env::var("GROQ_API_KEY").map_err(|_| "GROQ_API_KEY is not set".to_string())?;
    let audio_bytes = base64::engine::general_purpose::STANDARD
        .decode(audioBase64)
        .map_err(|e| e.to_string())?;
    let temp_path = save_temp_audio(&audio_bytes)?;
    let raw = call_groq_whisper(&api_key, &temp_path, &language)?;
    let cleaned = call_groq_cleaner(&api_key, &raw)?;
    Ok(TranscriptionResult { raw, cleaned })
}

#[tauri::command]
fn paste_text(text: String) -> Result<(), String> {
    let mut clip = Clipboard::new().map_err(|e| e.to_string())?;
    clip.set_text(text).map_err(|e| e.to_string())?;

    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    let _ = enigo.key(Key::Control, Direction::Press);
    let _ = enigo.key(Key::Unicode('v'), Direction::Click);
    let _ = enigo.key(Key::Control, Direction::Release);
    Ok(())
}

fn save_temp_audio(bytes: &[u8]) -> Result<PathBuf, String> {
    let mut path = std::env::temp_dir();
    path.push("groq_recorded_audio.webm");
    fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

fn call_groq_whisper(api_key: &str, file_path: &PathBuf, language: &str) -> Result<String, String> {
    let client = Client::new();
    let form = multipart::Form::new()
        .text("model", "whisper-large-v3-turbo")
        .text("temperature", "0")
        .text("response_format", "text")
        .text("language", language.to_string())
        .part(
            "file",
            multipart::Part::bytes(fs::read(file_path).map_err(|e| e.to_string())?)
                .file_name("audio.webm")
                .mime_str("audio/webm")
                .map_err(|e| e.to_string())?,
        );

    let response = client
        .post("https://api.groq.com/openai/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .map_err(|e| e.to_string())?;

    let status = response.status();
    let text = response.text().map_err(|e| e.to_string())?;
    // write raw response to temp for debugging
    let mut dbg_path = std::env::temp_dir();
    dbg_path.push("groq_whisper_response.txt");
    let _ = fs::write(&dbg_path, &text);
    println!("[groq] whisper status={} saved to {:?}", status, dbg_path);
    if status.is_success() {
        return Ok(text.trim().to_string());
    }
    Err(format!("whisper error {}: {}", status, text))
}

fn call_groq_cleaner(api_key: &str, transcript: &str) -> Result<String, String> {
    let client = Client::new();
    let request_body = json!({
        "model": "meta-llama/llama-4-scout-17b-16e-instruct",
        "messages": [
            {
                "role": "user",
                "content": format!(
                    "Clean this transcript by removing filler words like um and uh, fixing minor disfluencies, and returning only the cleaned transcript text with no explanation or commentary:\n\n{}",
                    transcript
                )
            }
        ],
        "temperature": 0.0,
        "max_completion_tokens": 1024,
    });

    let response = client
        .post("https://api.groq.com/openai/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&request_body)
        .send()
        .map_err(|e| e.to_string())?;

    let json: serde_json::Value = response.json().map_err(|e| e.to_string())?;
    // write raw cleaner response for debugging
    let mut dbg = std::env::temp_dir();
    dbg.push("groq_cleaner_response.json");
    let _ = fs::write(&dbg, serde_json::to_string_pretty(&json).unwrap_or_default());
    println!("[groq] cleaner saved to {:?}", dbg);
    let content = json
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .ok_or_else(|| format!("unexpected chat response: {}", json))?;

    Ok(content.trim().to_string())
}


