use eyre::{Context, Result};
use reqwest::{Client, Response};
use serde::Deserialize;
use tap::Pipe;

fn get_link_header(response: &Response) -> Option<String> {
    Some(
        response
            .headers()
            .get("link")?
            .to_str()
            .ok()?
            // The header is in the form <url>; rel="next"
            .strip_prefix('<')?
            .strip_suffix(">; rel=\"next\"")?
            .to_owned(),
    )
}

async fn get_data(response: Response) -> Result<Vec<String>> {
    Ok(response
        .json::<Vec<SiteData>>()
        .await
        .wrap_err("Failed to convert api response to JSON.")?
        .into_iter()
        .map(|data| data.url)
        .collect())
}

#[derive(Deserialize)]
struct SiteData {
    url: String,
}

pub async fn get_urls(client: &Client, verified_only: bool) -> Result<Vec<String>> {
    let initial_response = client
        .get("https://api.rhythm.cafe/datasette/combined/levels.json")
        .query(&[("_shape", "array"), ("_col", "url"), ("_size", "max")])
        .pipe(|builder| {
            if verified_only {
                builder.query(&[("approval__gt", "0")])
            } else {
                builder
            }
        })
        .send()
        .await
        .wrap_err("Network error sending initial request to rhythm.cafe api.")?;

    let mut next_url = get_link_header(&initial_response);
    let mut result = get_data(initial_response).await?;

    while let Some(url) = next_url {
        let response = client
            .get(url)
            .send()
            .await
            .wrap_err("Network error sending initial request to rhythm.cafe api.")?;

        next_url = get_link_header(&response);
        result.append(&mut get_data(response).await?);
    }

    Ok(result)
}
