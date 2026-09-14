use std::sync::{Arc, Mutex};

use indicatif::{MultiProgress, ProgressBar};

use crate::model::Request;

pub async fn send_request(
    request_parameters: Request,
    state: Arc<Mutex<MultiProgress>>,
) -> (bool, Request) {
    let start = std::time::Instant::now();
    let bar = state.lock().unwrap().add(ProgressBar::new_spinner());
    bar.enable_steady_tick(std::time::Duration::from_millis(100));
    bar.set_message(format!(
        "🛠️ {}: Building {} request",
        request_parameters.name, request_parameters.url
    ));

    // Before we start building the request, lets check if the data is correct
    if request_parameters.url.is_empty() {
        bar.finish_with_message(format!(
            "❌ {}: Request for {} failed in {:?}",
            request_parameters.name,
            request_parameters.url,
            start.elapsed()
        ));
        return (false, request_parameters);
    }

    let client = reqwest::Client::new();

    let builder = match request_parameters.method {
        reqwest::Method::GET => client.get(&request_parameters.url),
        reqwest::Method::POST => client.post(&request_parameters.url),
        reqwest::Method::PUT => client.put(&request_parameters.url),
        reqwest::Method::DELETE => client.delete(&request_parameters.url),
        reqwest::Method::PATCH => client.patch(&request_parameters.url),
        reqwest::Method::HEAD => client.head(&request_parameters.url),
        reqwest::Method::OPTIONS => {
            todo!();
        }
        reqwest::Method::CONNECT => {
            todo!();
        }
        reqwest::Method::TRACE => {
            todo!();
        }
        _ => {
            unreachable!("Method not implemented");
        }
    };

    // TODO: Add body and headers
    bar.set_message(format!(
        "🌩️ {}: Sending request to {}",
        request_parameters.name, request_parameters.url
    ));
    let response = builder.send().await;

    let process_response_result = response
        .as_ref()
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    if process_response_result {
        // TODO: Add more tests
        bar.finish_with_message(format!(
            "✅ {}: Request for {} succeeded in {:?} {}",
            request_parameters.name,
            request_parameters.url,
            start.elapsed(),
            response.as_ref().unwrap().status()
        ));
    } else {
        bar.finish_with_message(format!(
            "❌ {}: Request for {} failed in {:?} {}",
            request_parameters.name,
            request_parameters.url,
            start.elapsed(),
            response.as_ref().unwrap().status()
        ));
    }
    (process_response_result, request_parameters)
}
