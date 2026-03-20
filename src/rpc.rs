use reqwest::Method;
use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ApiConnectionSettings {
    pub base_url: String,
    pub access_token: Option<String>,
}

#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    access_token: Option<String>,
    http: Client,
}

impl ApiClient {
    pub fn new(settings: ApiConnectionSettings) -> Result<Self, String> {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        let http = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(10))
            .danger_accept_invalid_certs(true)
            .build()
            .map_err(|error| format!("failed to create HTTP client: {error}"))?;

        Ok(Self {
            base_url: settings.base_url.trim_end_matches('/').to_string(),
            access_token: settings.access_token,
            http,
        })
    }

    pub fn get_json(&self, path: &str) -> Result<Value, String> {
        self.request_json(Method::GET, path, None)
    }

    pub fn request_json(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value, String> {
        let url = self.url(path);
        let response = self
            .build_request(method, &url, body)
            .send()
            .map_err(|error| format!("request failed for {url}: {error}"))?;

        Self::decode_flexible(response)
    }

    fn build_request(&self, method: Method, url: &str, body: Option<&Value>) -> RequestBuilder {
        let mut request = self.http.request(method, url);

        if let Some(token) = &self.access_token {
            request = request.bearer_auth(token);
        }

        if let Some(body) = body {
            request.header(CONTENT_TYPE, "application/json").json(body)
        } else {
            request
        }
    }

    fn url(&self, path: &str) -> String {
        if path.starts_with("http://") || path.starts_with("https://") {
            return path.to_string();
        }

        if path.starts_with('/') {
            format!("{}{}", self.base_url, path)
        } else {
            format!("{}/{}", self.base_url, path)
        }
    }

    fn decode_flexible(response: Response) -> Result<Value, String> {
        let status = response.status();
        let body = response
            .bytes()
            .map_err(|error| format!("failed to read response body: {error}"))?;

        if !status.is_success() {
            let preview = String::from_utf8_lossy(&body);
            return Err(format!("http {}: {}", status.as_u16(), preview));
        }

        if let Ok(json) = serde_json::from_slice::<Value>(&body) {
            return Ok(json);
        }

        if let Ok(text) = String::from_utf8(body.to_vec()) {
            return Ok(json!({ "text": text }));
        }

        let preview = body
            .iter()
            .take(64)
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();

        Ok(json!({
            "binary": true,
            "bytes": body.len(),
            "preview_hex": preview,
        }))
    }
}
