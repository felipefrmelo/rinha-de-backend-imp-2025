use async_trait::async_trait;
use reqwest::Client;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::time::Duration;

#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn get(&self, url: &str) -> Result<HttpResponse, Box<dyn std::error::Error + Send + Sync>>;
}

pub struct HttpResponse {
    pub status_code: u16,
    pub body: String,
    pub is_success: bool,
}

impl HttpResponse {
    pub fn json<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_str(&self.body)
    }

    pub fn status_code(&self) -> u16 {
        self.status_code
    }
}

pub struct ReqwestHttpClient {
    client: Client,
}

impl ReqwestHttpClient {
    pub fn new(timeout: Duration) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .timeout(timeout)
            .build()?;
        
        Ok(Self { client })
    }
}

#[async_trait]
impl HttpClient for ReqwestHttpClient {
    async fn get(&self, url: &str) -> Result<HttpResponse, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.client.get(url).send().await?;
        let status_code = response.status().as_u16();
        let is_success = response.status().is_success();
        let body = response.json().await?;

        
        Ok(HttpResponse {
            status_code,
            body,
            is_success,
        })
    }
}

#[derive(Clone)]
pub struct MockHttpResponse {
    pub status_code: u16,
    pub body: String,
}

pub struct MockHttpClient {
    responses: HashMap<String, MockHttpResponse>,
    default_response: MockHttpResponse,
}

impl MockHttpClient {
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
            default_response: MockHttpResponse {
                status_code: 404,
                body: "Not Found".to_string(),
            },
        }
    }

    pub fn with_response(mut self, url: &str, status_code: u16, body: &str) -> Self {
        self.responses.insert(
            url.to_string(),
            MockHttpResponse {
                status_code,
                body: body.to_string(),
            },
        );
        self
    }

    pub fn with_default_response(mut self, status_code: u16, body: &str) -> Self {
        self.default_response = MockHttpResponse {
            status_code,
            body: body.to_string(),
        };
        self
    }
}

impl Default for MockHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HttpClient for MockHttpClient {
    async fn get(&self, url: &str) -> Result<HttpResponse, Box<dyn std::error::Error + Send + Sync>> {
        let mock_response = self.responses
            .get(url)
            .unwrap_or(&self.default_response);
        
        Ok(HttpResponse {
            status_code: mock_response.status_code,
            body: mock_response.body.clone(),
            is_success: mock_response.status_code >= 200 && mock_response.status_code < 300,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestResponse {
        message: String,
        code: i32,
    }

    #[tokio::test]
    async fn test_mock_http_client() {
        let client = MockHttpClient::new()
            .with_response(
                "http://example.com/test",
                200,
                r#"{"message": "success", "code": 42}"#,
            );

        let response = client.get("http://example.com/test").await.unwrap();
        
        assert_eq!(response.status_code(), 200);
        assert!(response.is_success);
        
        let json_response: TestResponse = response.json().unwrap();
        assert_eq!(json_response, TestResponse {
            message: "success".to_string(),
            code: 42,
        });
    }

    #[tokio::test]
    async fn test_mock_http_client_default_response() {
        let client = MockHttpClient::new()
            .with_default_response(500, "Internal Server Error");

        let response = client.get("http://unknown-url.com").await.unwrap();
        
        assert_eq!(response.status_code(), 500);
        assert!(!response.is_success);
    }
}
