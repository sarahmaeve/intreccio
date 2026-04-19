//! Google Cloud Text-to-Speech client.
//!
//! Scope is intentionally narrow: plain-text synthesis with one voice at
//! a time, MP3 output, over a single HTTP POST. No SSML, no dialog
//! concatenation, no silence generation, no multi-voice assignment —
//! intreccio synthesizes Italian words and phrases in isolation.

use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// A Google Cloud TTS voice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Voice {
    /// BCP 47 language code, e.g. `"it-IT"`.
    pub language_code: &'static str,
    /// Full Google Cloud voice name, e.g. `"it-IT-Chirp3-HD-Aoede"`.
    pub name: &'static str,
}

const GOOGLE_TTS_URL: &str = "https://texttospeech.googleapis.com/v1/text:synthesize";

#[derive(Debug, thiserror::Error)]
pub enum TtsError {
    #[error("GOOGLE_TTS_API_KEY environment variable not set")]
    MissingApiKey,
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("API returned error status {status}: {body}")]
    ApiError { status: u16, body: String },
    #[error("failed to decode audio content: {0}")]
    Decode(#[from] base64::DecodeError),
    #[error("file I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Serialize)]
struct SynthesizeRequest<'a> {
    input: SynthesisInput<'a>,
    voice: VoiceSelection<'a>,
    #[serde(rename = "audioConfig")]
    audio_config: AudioConfig,
}

#[derive(Serialize)]
struct SynthesisInput<'a> {
    text: &'a str,
}

#[derive(Serialize)]
struct VoiceSelection<'a> {
    #[serde(rename = "languageCode")]
    language_code: &'a str,
    name: &'a str,
}

#[derive(Serialize)]
struct AudioConfig {
    #[serde(rename = "audioEncoding")]
    audio_encoding: &'static str,
}

#[derive(Deserialize)]
struct SynthesizeResponse {
    #[serde(rename = "audioContent")]
    audio_content: String,
}

/// Google Cloud TTS client.
#[derive(Debug)]
pub struct GoogleTts {
    client: Client,
    api_key: String,
}

impl GoogleTts {
    /// Construct a client from the `GOOGLE_TTS_API_KEY` environment variable.
    pub fn from_env() -> Result<Self, TtsError> {
        let api_key = std::env::var("GOOGLE_TTS_API_KEY").map_err(|_| TtsError::MissingApiKey)?;
        Ok(Self {
            client: Client::new(),
            api_key,
        })
    }

    /// Synthesize `text` with `voice` and return the raw MP3 bytes.
    pub async fn synthesize(&self, text: &str, voice: &Voice) -> Result<Vec<u8>, TtsError> {
        let body = SynthesizeRequest {
            input: SynthesisInput { text },
            voice: VoiceSelection {
                language_code: voice.language_code,
                name: voice.name,
            },
            audio_config: AudioConfig { audio_encoding: "MP3" },
        };

        let url = format!("{}?key={}", GOOGLE_TTS_URL, self.api_key);
        let resp = self.client.post(&url).json(&body).send().await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(TtsError::ApiError {
                status: status.as_u16(),
                body,
            });
        }

        let synth: SynthesizeResponse = resp.json().await?;
        let bytes = base64::engine::general_purpose::STANDARD.decode(&synth.audio_content)?;
        Ok(bytes)
    }

    /// Synthesize `text` and write the MP3 to `output_path`, creating
    /// parent directories as needed.
    pub async fn synthesize_to_file(
        &self,
        text: &str,
        voice: &Voice,
        output_path: &std::path::Path,
    ) -> Result<(), TtsError> {
        let bytes = self.synthesize(text, voice).await?;
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(output_path, &bytes)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_VOICE: Voice = Voice {
        language_code: "it-IT",
        name: "it-IT-Chirp3-HD-Aoede",
    };

    #[test]
    fn from_env_fails_without_key() {
        // Clear any inherited value from the developer's shell.
        // SAFETY: single-threaded test; no other code reads the env.
        unsafe { std::env::remove_var("GOOGLE_TTS_API_KEY"); }
        let result = GoogleTts::from_env();
        assert!(matches!(result, Err(TtsError::MissingApiKey)));
    }

    #[test]
    fn request_body_shape() {
        let req = SynthesizeRequest {
            input: SynthesisInput { text: "la storia" },
            voice: VoiceSelection {
                language_code: TEST_VOICE.language_code,
                name: TEST_VOICE.name,
            },
            audio_config: AudioConfig { audio_encoding: "MP3" },
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["input"]["text"], "la storia");
        assert_eq!(json["voice"]["languageCode"], "it-IT");
        assert_eq!(json["voice"]["name"], "it-IT-Chirp3-HD-Aoede");
        assert_eq!(json["audioConfig"]["audioEncoding"], "MP3");
    }
}
