#![warn(
    clippy::all,
    clippy::cargo,
    clippy::unwrap_used,
    clippy::str_to_string,
    clippy::inefficient_to_string,
    clippy::suspicious
)]
#![feature(once_cell)]

mod api;
mod download;
mod prefs;

use console::style;
use eyre::{Context, Result};
use reqwest::Client;

fn print_yellow(string: &str) {
    println!("{}", style(string).yellow())
}
fn print_green(string: &str) {
    println!("{}", style(string).green())
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let client = Client::new();
    let prefs =
        prefs::prompt_user_prefs().wrap_err("Error when prompting for user preferences.")?;

    print_yellow("Querying rhythm.cafe for levels..");

    let urls = api::get_urls(&client, prefs.verified_only)
        .await
        .wrap_err("Failed to fetch levels to download from rhythm.cafe api.")?;

    print_green(&format!("Got {} levels.", urls.len()));

    download::download_levels(&client, urls, prefs).await?;

    Ok(())
}
