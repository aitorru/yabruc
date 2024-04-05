use std::sync::{Arc, Mutex};

use indicatif::{MultiProgress, ProgressBar};

use crate::parser::bru2struct::Dog;

pub async fn send_request(request_parameters: Dog, state: Arc<Mutex<MultiProgress>>) -> bool {
    let bar = state.lock().unwrap().add(ProgressBar::new_spinner());
    bar.enable_steady_tick(std::time::Duration::from_millis(100));
    bar.set_message(format!("🌩️ Sending request to {}", request_parameters.method.url));

    match request_parameters.method.type_ {
        reqwest::Method::GET => {
            
        }
        _ => todo!()
    }

    todo!()
}