use std::{ops::Not, path::PathBuf, str::FromStr};

use eyre::{Context, ContextCompat, Result};
use reaper_save_rs::high_level::{ReaperProject, Track};
use rfd::FileDialog;
use tap::prelude::*;
use tracing::info;

fn prompt_confirm_enter(prompt: &str) -> Result<()> {
    inquire::Text::new(prompt)
        .with_help_message("press [ENTER] to confirm")
        .prompt()
        .context("action not confirmed")
        .map(|_| ())
}

fn load(path: PathBuf) -> Result<(PathBuf, ReaperProject)> {
    std::fs::read_to_string(&path)
        .with_context(|| format!("reading [{}]", path.display()))
        .and_then(|content| ReaperProject::parse_from_str(&content).context("parsing"))
        .map(|project| (path, project))
}

#[macro_export]
macro_rules! zip_results {
    (Error = $ret:ty, $($result:expr),*) => {
        {
            let mut __extract = || -> std::result::Result<_, _> {
                std::result::Result::<_, $ret>::Ok(($($result?),*))
            };
            __extract()
        }
    };
    ($($result:expr),*) => {
        {
            let mut __extract = || -> std::result::Result<_, _> {
                Ok(($($result?),*))
            };
            __extract()
        }
    };

}

struct TrackSelection {
    track: Track,
}

impl std::fmt::Display for TrackSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}, ({} items)",
            self.track
                .name()
                .expect("this is a bug, please report it to [wojciech.brozek@niedzwiedz.it]"),
            self.track.items().len(),
        )
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt().init();
    Ok(())
        .and_then(|_| {
            FileDialog::new()
                .set_title("Project file you wish to import FROM (source)")
                .pick_file()
                .context("no source file selected")
                .and_then(|source_file| {
                    FileDialog::new()
                        .set_title("Project file you wish to import INTO (target)")
                        .pick_file()
                        .context("no target file selected")
                        .map(|target_file| (source_file, target_file))
                })
        })
        .and_then(|(source_file, target_file)| -> Result<_> {
            (
                load(source_file).context("loading source file")?,
                load(target_file).context("loading target file")?,
            )
                .pipe(Ok)
        })
        .context("loading both projects")
        .and_then(
            |((source_path, source_project), (target_path, mut target_project))| {
                source_project
                    .tracks()
                    .into_iter()
                    .map(|track| TrackSelection { track })
                    .collect::<Vec<_>>()
                    .pipe(|options| {
                        inquire::MultiSelect::new("Select tracks you wish to copy", options)
                    })
                    .prompt()
                    .context("selecting source tracks")
                    .and_then(|v| {
                        v.is_empty()
                            .not()
                            .then_some(v)
                            .context("no tracks selected")
                    })
                    .map(|tracks| tracks.into_iter().map(|t| t.track).collect::<Vec<_>>())
                    .and_then(|mut copied_tracks| {
                        copied_tracks
                            .iter_mut()
                            .flat_map(|track| {
                                track.modify_items(|item| {
                                    item.with_source_waves_mut(|source| match source.file_mut() {
                                        Some(source) => {
                                            source.context("invalid file").and_then(|file| {
                                                PathBuf::from_str(file.as_str())
                                                    .context("invalid path")
                                                    .and_then(|item_path| {
                                                        match item_path.is_absolute() {
                                                            true => Ok(file.clone()),
                                                            false => source_path
                                                                .parent()
                                                                .context(
                                                                    "source path has no parent",
                                                                )
                                                                .map(|parent| {
                                                                    parent
                                                                        .join(item_path)
                                                                        .display()
                                                                        .to_string()
                                                                }),
                                                        }
                                                    })
                                                    .map(|corrected| {
                                                        info!(
                                                            "correcting path [{file}] -> \
                                                             [{corrected}]"
                                                        );
                                                        *file = corrected;
                                                    })
                                            })
                                        }
                                        None => Ok(()),
                                    })
                                })
                            })
                            .flatten()
                            .collect::<Result<()>>()
                            .map(|_| copied_tracks)
                    })
                    .and_then(|copied_tracks| {
                        target_project
                            .modify_tracks(move |target_tracks| {
                                target_tracks.into_iter().chain(copied_tracks).collect()
                            })
                            .context("modifying target file failed")
                    })
                    .and_then(|_| {
                        target_project
                            .serialize_to_string()
                            .context("serializing to string")
                            .and_then(|serialized| {
                                inquire::Confirm::new(
                                    format!(
                                        "Do you want to save the modified file at [{}]? Remember \
                                         to backup your project file just in case, no changes \
                                         were applied yet.",
                                        target_path.display()
                                    )
                                    .as_str(),
                                )
                                .prompt()
                                .context("asking for confirmation on save")
                                .and_then(|confirmed| {
                                    confirmed.then_some(()).context("not confirmed")
                                })
                                .and_then(|_| {
                                    std::fs::write(&target_path, serialized)
                                        .context("writing modified project file")
                                })
                            })
                    })
                    .context("applying changes")
                    .tap(move |res| match res.as_ref() {
                        Ok(_) => {
                            println!(
                                "source: {}\n ->\ntarget: {}",
                                source_path.display(),
                                target_path.display()
                            );
                            prompt_confirm_enter("SUCCESS").unwrap()
                        }
                        Err(message) => {
                            prompt_confirm_enter(format!("ERROR: {message:?}").as_str()).unwrap();
                        }
                    })
            },
        )
}
