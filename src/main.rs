#![warn(
    clippy::all,
    clippy::cargo,
    clippy::unwrap_used,
    clippy::str_to_string,
    clippy::inefficient_to_string,
    clippy::suspicious
)]

use reqwest::{Client, Response};
use serde::Deserialize;

#[derive(Deserialize)]
struct Thing {
    id: String,
    url: String,
}

fn get_link_header(response: &Response) -> Option<String> {
    Some(
        response
            .headers()
            .get("link")?
            .to_str()
            .ok()?
            .strip_prefix('<')?
            .strip_suffix(">; rel=\"next\"")?
            .to_owned(),
    )
}

async fn get_urls(client: Client, verified_only: bool) -> Result<Vec<Thing>, reqwest::Error> {
    let initial_response = client
        .get("https://api.rhythm.cafe/datasette/combined/levels.json")
        .query(&[
            if verified_only {
                ("approval__gt", "0")
            } else {
                ("", "")
            },
            ("_shape", "array"),
            ("_col", "url"),
            ("_size", "max"),
        ])
        .send()
        .await?;

    let mut next_url = get_link_header(&initial_response);
    let mut result = initial_response.json::<Vec<Thing>>().await?;

    while let Some(url) = next_url {
        let response = client.get(url).send().await?;

        next_url = get_link_header(&response);
        result.append(&mut response.json::<Vec<Thing>>().await?);
    }

    Ok(result)
}

#[tokio::main]
async fn main() {
    let client = Client::new();
    println!("{}", get_urls(client, false).await.unwrap().len());
}
