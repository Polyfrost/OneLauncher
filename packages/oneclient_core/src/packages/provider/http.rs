use reqwest::Method;
use serde::de::DeserializeOwned;

use crate::http::{RequestClient, RequestError, ResponseNotifyOptions, ResponseOptions};
use crate::notification::GroupedProgressChild;
use crate::state::LauncherServices;

pub async fn fetch_json<T: DeserializeOwned>(
    client: &RequestClient,
    method: Method,
    url: impl reqwest::IntoUrl,
    body: Option<serde_json::Value>,
) -> Result<T, RequestError> {
    let url = url.into_url()?;
    client.send_json(method, url, body, &[]).await
}

pub async fn fetch_json_with_headers<T: DeserializeOwned>(
    client: &RequestClient,
    method: Method,
    url: impl reqwest::IntoUrl,
    body: Option<serde_json::Value>,
    headers: &[(String, String)],
) -> Result<T, RequestError> {
    let url = url.into_url()?;
    let borrowed: Vec<(&str, &str)> = headers
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    client.send_json(method, url, body, &borrowed).await
}

pub async fn download_url(
    client: &RequestClient,
    url: &str,
    dest: impl AsRef<std::path::Path> + Send,
    child: Option<&GroupedProgressChild>,
    services: &LauncherServices,
) -> Result<(), RequestError> {
    let request = reqwest::Request::new(Method::GET, url.parse()?);
    let options = ResponseOptions {
        notify: child.map(|c| ResponseNotifyOptions::grouped(c.clone())),
    };
    client
        .download_file(request, dest, options, services)
        .await
}
