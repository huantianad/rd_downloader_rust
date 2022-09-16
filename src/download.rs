use crate::prefs::UserPrefs;
use crate::print_red;
use eyre::{eyre, Result};
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
        .ok_or(eyre!("Could not find Content-Disposition header."))?;

    let parsed_header = ContentDisposition::parse_header(&raw_header)?;

    if parsed_header.disposition != DispositionType::Attachment {
        return Err(eyre!(
            "Unknown disposition type: {:?}",
            parsed_header.disposition
        ));
    }

    for param in parsed_header.parameters {
        if let DispositionParam::Filename(charset, _, bytes) = param {
            if charset == Charset::Us_Ascii
                || charset == Charset::Iso_8859_1
                || charset == Charset::Ext(String::from("utf-8"))
                || charset == Charset::Ext(String::from("UTF-8"))
            {
                return Ok(from_utf8(&bytes)?.to_owned());
            }
        }
    }

    Err(eyre!(
        "Content-Disposition did not have valid encoding for filename."
    ))
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

    let filename = path
        .file_stem()
        .ok_or(eyre!("Could not get filename from path."))?
        .to_string_lossy();
    let extension = path
        .extension()
        .ok_or(eyre!("Could not get extension from path."))?
        .to_string_lossy();

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
    let bar = multibar.insert_from_back(1, ProgressBar::new(total_bytes));
    bar.set_prefix(filename.clone());
    bar.set_style(
        ProgressStyle::with_template(
            "{prefix:40!} {bar:40.cyan/blue} {bytes:>10} / {total_bytes:<10}",
        )
        .expect("Failed to create progress bar style!!")
        .progress_chars("##-"),
    );

    let mut dest = File::create(full_path).await?;
    while let Some(chunk) = response.chunk().await? {
        dest.write_all(&chunk).await?;
        bar.inc(chunk.len() as u64)
    }

    multibar.remove(&bar);
    multibar.println(&filename)?;
    master_bar.inc(1);

    Ok(())
}

pub async fn download_levels(client: &Client, urls: Vec<String>, prefs: UserPrefs) -> Result<()> {
    let folder = prefs.download_path.as_path();
    let threads = prefs.download_threads;

    let multibar = MultiProgress::new();
    let master_bar = multibar.add(ProgressBar::new(urls.len() as u64));
    master_bar.set_style(
        ProgressStyle::with_template("Total: {bar:79.cyan/blue} {human_pos:>5} / {human_len:<5}")
            .expect("Failed to create master progress bar style!!")
            .progress_chars("##-"),
    );

    let results = futures::stream::iter(urls)
        .map(|url| download_level(client, folder, url, &multibar, &master_bar))
        .buffer_unordered(threads)
        .collect::<Vec<Result<()>>>()
        .await;

    master_bar.finish();

    for result in results {
        if result.is_err() {
            print_red(&format!(
                "Failed to download level, \nerror: {:#?}\n",
                result
            ))
        }
    }

    Ok(())
}
