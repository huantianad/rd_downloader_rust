use crate::prefs::UserPrefs;
use eyre::{eyre, Context, ContextCompat, Result};
use futures::StreamExt;
use hyperx::header::{Charset, ContentDisposition, DispositionParam, DispositionType, Header};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::{header::CONTENT_DISPOSITION, Client, Response};
use std::path::{Path, PathBuf};
use std::str::from_utf8;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

fn get_filename_from_url(response: &Response) -> Option<String> {
    let url_path = PathBuf::from(response.url().path());
    let extension = url_path.extension()?;

    if extension == "rdzip" || extension == "zip" {
        Some(url_path.file_name()?.to_string_lossy().to_string())
    } else {
        None
    }
}

fn get_filename_from_headers(response: &Response) -> Result<String> {
    let raw_header = response
        .headers()
        .get(CONTENT_DISPOSITION)
        .wrap_err("Could not find Content-Disposition header.")?;

    let parsed_header = ContentDisposition::parse_header(&raw_header)?;

    if parsed_header.disposition != DispositionType::Attachment {
        return Err(eyre!(
            "Unknown disposition type: {:?}",
            parsed_header.disposition
        ));
    }

    for param in parsed_header.parameters {
        if let DispositionParam::Filename(charset, _, bytes) = param {
            match charset {
                Charset::Us_Ascii | Charset::Iso_8859_1 => return Ok(from_utf8(&bytes)?.to_owned()),
                Charset::Ext(s) => {
                    if s == "utf-8" || s == "UTF-8" {
                        return Ok(from_utf8(&bytes)?.to_owned());
                    }
                }
                _ => {}
            }
        }
    }

    Ok(String::new())
}

fn get_filename(response: &Response) -> Result<String> {
    let maybe_filename = get_filename_from_url(response);

    if let Some(filename) = maybe_filename {
        Ok(filename)
    } else {
        get_filename_from_headers(response)
    }
}

fn ensure_path(path: PathBuf) -> Result<PathBuf> {
    if !path.try_exists()? {
        return Ok(path);
    }

    let filename = path.file_stem().unwrap().to_string_lossy();
    let extension = path.extension().unwrap().to_string_lossy();
    let mut i = 2;

    while path
        .with_file_name(format!("{} ({}).{}", filename, i, extension))
        .try_exists()?
    {
        i += 1;
    }

    let final_path = path.with_file_name(format!("{} ({}).{}", filename, i, extension));
    Ok(final_path)
}

fn construct_bar(multibar: &MultiProgress, filename: String, length: u64) -> ProgressBar {
    let bar = multibar.insert_from_back(1, ProgressBar::new(length));
    bar.set_prefix(filename);
    bar.set_style(
        ProgressStyle::with_template(
            "{prefix:40!} {bar:40.cyan/blue} {bytes:>10} / {total_bytes:<10}",
        )
        .unwrap()
        .progress_chars("##-"),
    );

    bar
}

async fn download_level(
    client: &Client,
    folder: &Path,
    url: String,
    multibar: &MultiProgress,
    master_bar: &ProgressBar,
) -> Result<()> {
    let mut response = client.get(url).send().await?;

    let filename = get_filename(&response)?;
    let full_path = ensure_path(folder.join(&filename))?;

    let total_bytes = response.content_length().unwrap();
    let bar = construct_bar(multibar, filename, total_bytes);

    let mut dest = File::create(full_path).await?;
    while let Some(chunk) = response.chunk().await? {
        dest.write_all(&chunk).await?;
        bar.inc(chunk.len().try_into().unwrap())
    }

    bar.set_length(total_bytes);
    bar.finish();
    master_bar.inc(1);

    Ok(())
}

pub async fn download_levels(client: &Client, urls: Vec<String>, prefs: UserPrefs) -> Result<()> {
    let folder = prefs.download_path.as_path();
    let threads = prefs.download_threads;

    let multibar = MultiProgress::new();

    let master_bar = multibar.add(ProgressBar::new(urls.len().try_into().unwrap()));
    master_bar.set_style(
        ProgressStyle::with_template("Total: {bar:79.cyan/blue} {human_pos:>5} / {human_len:<5}")
            .unwrap()
            .progress_chars("##-"),
    );

    let fetches = futures::stream::iter(urls)
        .map(|url| download_level(client, folder, url, &multibar, &master_bar))
        .buffer_unordered(threads);

    fetches
        .for_each(|result| async move {
            match result {
                Ok(_) => {}
                Err(_) => println!("not nice"),
            }
        })
        .await;

    master_bar.finish();

    Ok(())
}
