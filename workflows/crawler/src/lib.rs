use std::{collections::HashSet, time::Duration};

use flawless::{
    workflow,
    workflow::{sleep, Input},
};
use flawless_http::ErrorKind;
use log::warn;
use select::{document::Document, predicate::Name};
use serde::{Deserialize, Serialize};

flawless::module! { name = "crawler", version = "0.1.7" }

const MAX_CRAWL_SIZE: usize = 64;

#[derive(Debug, Serialize, Deserialize)]
pub struct Job {
    pub id: usize,
    pub url: String,
}

#[workflow("crawl")]
pub fn start_crawler(input: Input<Job>) {
    let id = input.id;
    let url = input.url.clone();

    let mut urls = vec![url];
    let mut visited_urls = HashSet::new();

    let mut links_crawled = 0;
    while let Some(url) = urls.pop() {
        links_crawled += 1;

        // Skip visited URLs.
        if visited_urls.contains(&url) {
            continue;
        }
        visited_urls.insert(url.clone());

        // If maximum number of URLs reached, stop.
        if links_crawled > MAX_CRAWL_SIZE {
            return;
        }

        sleep(Duration::from_millis(300));
        update_ui(UpdateUI::new(id, Status::Request, &url, urls.len() + 1));
        let response = flawless_http::get(url.as_str()).send();
        if let Err(err) = response {
            if err.kind() == ErrorKind::RequestInterrupted {
                warn!("The request to '{}' was interrupted", url);
            }
            update_ui(UpdateUI::new(id, Status::Error, "last", urls.len()));
            continue;
        }
        sleep(Duration::from_millis(300));

        let document = response.unwrap().text();
        if let Err(_err) = document {
            update_ui(UpdateUI::new(id, Status::Error, "last", urls.len()));
            continue;
        }

        update_ui(UpdateUI::new(id, Status::Parse, "last", urls.len()));
        let links = extract_links(document.unwrap());
        urls.extend(links);
        sleep(Duration::from_millis(300));

        update_ui(UpdateUI::new(id, Status::Done, "last", urls.len()));
    }
}

// Extract links from HTML document.
fn extract_links(document: String) -> Vec<String> {
    Document::from(document.as_str())
        .find(Name("a"))
        .filter_map(|n| n.attr("href"))
        .filter(|url| url.starts_with("https://"))
        .map(|url| url.to_string())
        .collect()
}

// Send update to UI server.
fn update_ui(update: UpdateUI) {
    flawless_http::post("http://localhost:3000/ui-update")
        .body(serde_json::to_value(update).expect("UpdateUI serialization"))
        .send()
        .expect("UI update");
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateUI {
    pub id: usize,
    pub status: Status,
    pub url: String,
    pub urls_left: usize,
}

impl UpdateUI {
    fn new(id: usize, status: Status, url: &str, urls_left: usize) -> Self {
        UpdateUI {
            id,
            status,
            url: url.to_string(),
            urls_left,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Status {
    // Request in progress.
    Request,
    // Parsing in progress.
    Parse,
    // Finished.
    Done,
    // Error
    Error,
}
