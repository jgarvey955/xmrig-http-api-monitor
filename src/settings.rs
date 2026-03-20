use crate::rpc::ApiConnectionSettings;
use iced::Size;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const SETTINGS_PATH: &str = "settings.json";
const WINDOW_STATE_PATH: &str = "window-state.json";
const DEFAULT_WINDOW_WIDTH: f32 = 1380.0;
const DEFAULT_WINDOW_HEIGHT: f32 = 920.0;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    pub api_host: String,
    pub api_port: u16,
    pub api_transport: String,
    pub api_access_token: String,
    pub poll_frequency_seconds: u64,
    pub preferred_endpoint: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(default)]
pub struct WindowState {
    pub width: f32,
    pub height: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            api_host: "127.0.0.1".to_string(),
            api_port: 80,
            api_transport: "http".to_string(),
            api_access_token: String::new(),
            poll_frequency_seconds: 10,
            preferred_endpoint: "/1/summary".to_string(),
        }
    }
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: DEFAULT_WINDOW_WIDTH,
            height: DEFAULT_WINDOW_HEIGHT,
        }
    }
}

impl Settings {
    pub fn load() -> Result<(Self, bool), Box<dyn std::error::Error>> {
        if Path::new(SETTINGS_PATH).exists() {
            let data = fs::read_to_string(SETTINGS_PATH)?;
            let mut settings: Settings = serde_json::from_str(&data)?;
            settings.normalize();
            Ok((settings, true))
        } else {
            Ok((Self::default(), false))
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let data = serde_json::to_string_pretty(self)?;
        fs::write(SETTINGS_PATH, data)?;
        Ok(())
    }

    pub fn api_connection(&self) -> ApiConnectionSettings {
        ApiConnectionSettings {
            base_url: self.api_url_display(),
            access_token: trimmed_or_none(&self.api_access_token),
        }
    }

    pub fn api_url_display(&self) -> String {
        format!(
            "{}://{}:{}",
            normalize_transport(&self.api_transport, "http"),
            self.api_host.trim(),
            self.api_port
        )
    }

    pub fn summary_url_display(&self) -> String {
        format!("{}/1/summary", self.api_url_display())
    }

    fn normalize(&mut self) {
        self.api_transport = normalize_transport(&self.api_transport, "http");
        self.preferred_endpoint = normalize_endpoint(&self.preferred_endpoint);
    }
}

impl WindowState {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        if Path::new(WINDOW_STATE_PATH).exists() {
            let data = fs::read_to_string(WINDOW_STATE_PATH)?;
            let state: WindowState = serde_json::from_str(&data)?;
            Ok(state.normalized())
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let data = serde_json::to_string_pretty(&self.normalized())?;
        fs::write(WINDOW_STATE_PATH, data)?;
        Ok(())
    }

    pub fn from_size(size: Size) -> Option<Self> {
        if !size.width.is_finite()
            || !size.height.is_finite()
            || size.width <= 1.0
            || size.height <= 1.0
        {
            return None;
        }

        Some(Self {
            width: size.width,
            height: size.height,
        })
    }

    pub fn size(&self) -> Size {
        let normalized = self.normalized();
        Size::new(normalized.width, normalized.height)
    }

    fn normalized(self) -> Self {
        Self {
            width: normalize_dimension(self.width, DEFAULT_WINDOW_WIDTH),
            height: normalize_dimension(self.height, DEFAULT_WINDOW_HEIGHT),
        }
    }
}

fn trimmed_or_none(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_transport(value: &str, fallback: &str) -> String {
    let lowered = value.trim().to_ascii_lowercase();
    match lowered.as_str() {
        "http" | "https" => lowered,
        _ => fallback.to_string(),
    }
}

fn normalize_endpoint(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "/1/summary".to_string()
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

fn normalize_dimension(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 1.0 {
        value
    } else {
        fallback
    }
}
