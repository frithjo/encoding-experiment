//! Trait-based HTTP client abstraction for swappable implementations.
//!
//! This allows using different HTTP clients (reqwest blocking, reqwest async,
//! ureq, curl, etc.) without changing calling code. Implementations are gated
//! behind feature flags to keep dependencies optional.

use std::collections::HashMap;

/// Error type for HTTP operations.
#[derive(Debug, thiserror::Error)]
pub enum HttpClientError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(String),
    
    #[error("HTTP error: {0} {1}")]
    HttpError(u16, String),
    
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Result type for HTTP operations.
pub type HttpResult<T> = Result<T, HttpClientError>;

/// Trait for HTTP client implementations.
///
/// This trait provides a minimal interface for HTTP operations needed by
/// LARQL (GET, POST, JSON serialization). Implementations can be blocking
/// or async depending on the use case.
pub trait HttpClient: Send + Sync {
    /// Perform a GET request to the given URL.
    fn get(&self, url: &str) -> HttpResult<HttpResponse>;
    
    /// Perform a GET request with query parameters.
    fn get_with_query(&self, url: &str, query: &HashMap<String, String>) -> HttpResult<HttpResponse>;
    
    /// Perform a POST request with a JSON body.
    fn post_json(&self, url: &str, body: &serde_json::Value) -> HttpResult<HttpResponse>;
    
    /// Perform a PUT request with raw bytes.
    fn put_bytes(&self, url: &str, body: Vec<u8>, headers: &HashMap<String, String>) -> HttpResult<HttpResponse>;
}

/// HTTP response.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response body as bytes.
    pub body: Vec<u8>,
    /// Response headers.
    pub headers: HashMap<String, String>,
}

impl HttpResponse {
    /// Parse response body as JSON.
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> HttpResult<T> {
        serde_json::from_slice(&self.body)
            .map_err(|e| HttpClientError::SerializationError(e.to_string()))
    }
    
    /// Get response body as string.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }
    
    /// Check if response was successful (2xx status).
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// Blocking HTTP client using reqwest.
#[cfg(feature = "http")]
pub struct ReqwestBlockingClient {
    client: reqwest::blocking::Client,
}

#[cfg(feature = "http")]
impl ReqwestBlockingClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
        }
    }
    
    pub fn with_timeout(timeout_secs: u64) -> Self {
        Self {
            client: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(timeout_secs))
                .build()
                .expect("failed to build HTTP client"),
        }
    }
}

#[cfg(feature = "http")]
impl HttpClient for ReqwestBlockingClient {
    fn get(&self, url: &str) -> HttpResult<HttpResponse> {
        let resp = self.client.get(url).send()
            .map_err(|e| HttpClientError::RequestFailed(e.to_string()))?;
        
        let status = resp.status().as_u16();
        let headers = resp.headers()
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.to_string(), v.to_string())))
            .collect();
        let body = resp.bytes()
            .map_err(|e| HttpClientError::RequestFailed(e.to_string()))?
            .to_vec();
        
        Ok(HttpResponse { status, body, headers })
    }
    
    fn get_with_query(&self, url: &str, query: &HashMap<String, String>) -> HttpResult<HttpResponse> {
        let mut req = self.client.get(url);
        for (key, value) in query {
            req = req.query(&[(key.as_str(), value.as_str())]);
        }
        
        let resp = req.send()
            .map_err(|e| HttpClientError::RequestFailed(e.to_string()))?;
        
        let status = resp.status().as_u16();
        let headers = resp.headers()
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.to_string(), v.to_string())))
            .collect();
        let body = resp.bytes()
            .map_err(|e| HttpClientError::RequestFailed(e.to_string()))?
            .to_vec();
        
        Ok(HttpResponse { status, body, headers })
    }
    
    fn post_json(&self, url: &str, body: &serde_json::Value) -> HttpResult<HttpResponse> {
        let resp = self.client.post(url).json(body).send()
            .map_err(|e| HttpClientError::RequestFailed(e.to_string()))?;
        
        let status = resp.status().as_u16();
        let headers = resp.headers()
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.to_string(), v.to_string())))
            .collect();
        let response_body = resp.bytes()
            .map_err(|e| HttpClientError::RequestFailed(e.to_string()))?
            .to_vec();
        
        Ok(HttpResponse { status, body: response_body, headers })
    }
    
    fn put_bytes(&self, url: &str, body: Vec<u8>, headers: &HashMap<String, String>) -> HttpResult<HttpResponse> {
        let mut req = self.client.put(url).body(body);
        for (key, value) in headers {
            req = req.header(key, value);
        }
        
        let resp = req.send()
            .map_err(|e| HttpClientError::RequestFailed(e.to_string()))?;
        
        let status = resp.status().as_u16();
        let resp_headers = resp.headers()
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.to_string(), v.to_string())))
            .collect();
        let response_body = resp.bytes()
            .map_err(|e| HttpClientError::RequestFailed(e.to_string()))?
            .to_vec();
        
        Ok(HttpResponse { status, body: response_body, headers: resp_headers })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_response_is_success() {
        let resp = HttpResponse {
            status: 200,
            body: b"ok".to_vec(),
            headers: HashMap::new(),
        };
        assert!(resp.is_success());
        
        let resp = HttpResponse {
            status: 404,
            body: b"not found".to_vec(),
            headers: HashMap::new(),
        };
        assert!(!resp.is_success());
    }
}
