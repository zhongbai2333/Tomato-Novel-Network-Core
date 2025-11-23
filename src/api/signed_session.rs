use std::collections::HashMap;
use std::time::Duration;

use reqwest::blocking::{Client, ClientBuilder};
use reqwest::header::{
    ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CONNECTION, COOKIE, HeaderMap, HeaderName,
    HeaderValue,
};
#[cfg(any(debug_assertions, feature = "charles_proxy"))]
use reqwest::{Certificate, Proxy};
use serde::Deserialize;
use serde_json::Value;

const REGISTER_KEY_URL: &str =
    "https://api5-normal-sinfonlinec.fqnovel.com/reading/crypt/registerkey";
const BATCH_FULL_ENDPOINT: &str =
    "https://api5-normal-sinfonlinec.fqnovel.com/reading/reader/batch_full/v?";
const BATCH_REQUEST_ENDPOINT: &str =
    "https://api5-normal-sinfonlinec.fqnovel.com/reading/reader/batch_full/v?chapter_id=";
const DEFAULT_USER_AGENT: &str = "python-requests/2.31.0";

#[derive(Deserialize)]
struct RegisterKeyRequest {
    install_id: String,
    aid: String,
    body: Value,
    #[serde(default)]
    user_agent: Option<String>,
}

#[derive(Deserialize)]
struct BatchFullRequest {
    query: String,
    headers: HashMap<String, String>,
}

#[derive(Deserialize)]
struct BatchRequestPayload {
    chapter_ids: Vec<String>,
}

pub fn handle_register_key(payload: &[u8]) -> Result<Value, String> {
    let request: RegisterKeyRequest =
        serde_json::from_slice(payload).map_err(|err| err.to_string())?;
    let client = build_client(request.user_agent.as_deref()).map_err(|err| err.to_string())?;
    let mut headers = HeaderMap::new();
    headers.insert(
        COOKIE,
        HeaderValue::from_str(&format!("install_id={}", request.install_id))
            .map_err(|err| err.to_string())?,
    );

    let response = client
        .post(REGISTER_KEY_URL)
        .headers(headers)
        .query(&[("aid", request.aid.as_str())])
        .json(&request.body)
        .send()
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?
        .json::<Value>()
        .map_err(|err| err.to_string())?;
    Ok(response)
}

pub fn handle_batch_full(payload: &[u8]) -> Result<Value, String> {
    let request: BatchFullRequest =
        serde_json::from_slice(payload).map_err(|err| err.to_string())?;
    let client = build_client(None).map_err(|err| err.to_string())?;
    let headers = header_map_from_pairs(request.headers)?;
    let url = format!("{}{}", BATCH_FULL_ENDPOINT, request.query);
    let response = client
        .get(url)
        .headers(headers)
        .send()
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?
        .json::<Value>()
        .map_err(|err| err.to_string())?;
    Ok(response)
}

pub fn handle_batch_request(payload: &[u8]) -> Result<Value, String> {
    let request: BatchRequestPayload =
        serde_json::from_slice(payload).map_err(|err| err.to_string())?;
    if request.chapter_ids.is_empty() {
        return Ok(Value::Array(Vec::new()));
    }
    let client = build_client(None).map_err(|err| err.to_string())?;
    let mut results = Vec::with_capacity(request.chapter_ids.len());
    for chapter_id in request.chapter_ids {
        let url = format!("{}{}", BATCH_REQUEST_ENDPOINT, chapter_id);
        let text = client
            .get(&url)
            .send()
            .map_err(|err| err.to_string())?
            .error_for_status()
            .map_err(|err| err.to_string())?
            .text()
            .map_err(|err| err.to_string())?;
        results.push(Value::String(text));
    }
    Ok(Value::Array(results))
}

fn build_client(user_agent: Option<&str>) -> Result<Client, reqwest::Error> {
    let mut headers = HeaderMap::new();
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/json, text/plain, */*"),
    );
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("zh-CN,zh;q=0.9"));
    headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));
    headers.insert(CONNECTION, HeaderValue::from_static("keep-alive"));

    let builder = Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(15))
        .user_agent(user_agent.unwrap_or(DEFAULT_USER_AGENT));
    configure_charles_proxy(builder).build()
}

fn header_map_from_pairs(pairs: HashMap<String, String>) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    for (key, value) in pairs {
        let name = HeaderName::from_bytes(key.as_bytes()).map_err(|err| err.to_string())?;
        let val = HeaderValue::from_str(&value).map_err(|err| err.to_string())?;
        headers.insert(name, val);
    }
    Ok(headers)
}

#[cfg(any(debug_assertions, feature = "charles_proxy"))]
fn configure_charles_proxy(mut builder: ClientBuilder) -> ClientBuilder {
    if let Some(proxy_url) = std::env::var("FANQIE_CHARLES_PROXY")
        .ok()
        .filter(|s| !s.is_empty())
    {
        if let Ok(proxy) = Proxy::all(&proxy_url) {
            builder = builder.proxy(proxy);
        }

        if let Ok(cert_path) = std::env::var("FANQIE_CHARLES_CA")
            && !cert_path.is_empty()
            && let Ok(pem) = std::fs::read(&cert_path)
            && let Ok(cert) = Certificate::from_pem(&pem)
        {
            builder = builder.add_root_certificate(cert);
        }

        if std::env::var("FANQIE_CHARLES_INSECURE").as_deref() == Ok("1") {
            builder = builder.danger_accept_invalid_certs(true);
        }

        builder = builder.http1_only();
    }

    builder
}

#[cfg(not(any(debug_assertions, feature = "charles_proxy")))]
fn configure_charles_proxy(builder: ClientBuilder) -> ClientBuilder {
    builder
}
