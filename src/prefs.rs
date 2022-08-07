use dialoguer::{Confirm, Input};
use eyre::{Context, Result};
use std::{env::current_dir, fs::create_dir_all, path::PathBuf, str::FromStr};

pub struct UserPrefs {
    pub download_path: PathBuf,
    pub download_threads: usize,
    pub verified_only: bool,
}

/// Wrap PathBuf for the ToString trait so we can use it in dialoguer
pub struct PathBufWrapper(PathBuf);
impl ToString for PathBufWrapper {
    fn to_string(&self) -> String {
        self.0.to_string_lossy().to_string()
    }
}
impl Clone for PathBufWrapper {
    fn clone(&self) -> Self {
        PathBufWrapper(self.0.clone())
    }
}
impl FromStr for PathBufWrapper {
    type Err = core::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(PathBufWrapper(PathBuf::from(s)))
    }
}

fn get_default_download_directory() -> Result<PathBufWrapper> {
    Ok(PathBufWrapper(
        current_dir()
            .wrap_err("Failed to get working directory.")?
            .join("rd_downloader"),
    ))
}

fn prompt_download_path() -> Result<PathBuf> {
    fn validator(path_wrapper: &PathBufWrapper) -> Result<(), &'static str> {
        let path = &path_wrapper.0;

        if path.is_file() {
            return Err("Path is file, it should be a directory.");
        }

        let exists = path
            .try_exists()
            .map_err(|_| "Failed to check if folder already exists.")?;

        let message = if exists {
            "Folder already exists, use it anyways?"
        } else {
            "Folder does not exist, create it now?"
        };

        let confirmed = Confirm::new()
            .with_prompt(message)
            .default(true)
            .interact()
            .map_err(|_| "asdf")?;

        if confirmed {
            if !exists {
                create_dir_all(path).map_err(|_| "Failed to create directory!")?;
            }
            Ok(())
        } else {
            Err("User canceled operation.")
        }
    }

    Ok(Input::new()
        .with_prompt("Where should levels be downloaded to?")
        .default(get_default_download_directory()?)
        .validate_with(validator)
        .interact()
        .wrap_err("prompt_download_path error")?
        .0)
}

fn prompt_download_threads() -> Result<usize> {
    fn validator(n: &usize) -> Result<(), &'static str> {
        if n > &0 {
            Ok(())
        } else {
            Err("Download threads must be an integer greater than 0.")
        }
    }

    Input::new()
        .with_prompt("How many concurrent downloads would you like to use? Use the default if you don't know.")
        .default(3)
        .validate_with(validator)
        .interact()
        .wrap_err("prompt_download_threads error")
}

fn prompt_verified_only() -> Result<bool> {
    Confirm::new()
        .with_prompt("Do you want to only download peer-reviewed levels?")
        .default(true)
        .interact()
        .wrap_err("prompt_verified_only error")
}

pub fn prompt_user_prefs() -> Result<UserPrefs> {
    Ok(UserPrefs {
        download_path: prompt_download_path()?,
        download_threads: prompt_download_threads()?,
        verified_only: prompt_verified_only()?,
    })
}
