use std::{ops::Not, path::PathBuf};

use eyre::{Context, ContextCompat, Result};
use inquire::ConfirmPromptAction;
use reaper_save_rs::high_level::{ReaperProject, Track};
use rfd::FileDialog;
use tap::prelude::*;

fn prompt_confirm_enter(prompt: &str) -> Result<()> {
    inquire::Text::new(prompt)
        .with_help_message("press [ENTER] to continue")
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
            "{}",
            self.track
                .name()
                .expect("this is a bug, please report it to [wojciech.brozek@niedzwiedz.it]")
        )
    }
}

fn main() -> Result<()> {
    Ok(())
        .and_then(|_| {
            prompt_confirm_enter(
                "You will now be prompted for project file you wish to import FROM (source)",
            )
            .and_then(|_| {
                FileDialog::new()
                    .pick_file()
                    .context("no source file selected")
            })
            .and_then(|source_file| {
                prompt_confirm_enter(
                    "Select the reaper project file you wish to import INTO (target)",
                )
                .and_then(|_| {
                    FileDialog::new()
                        .pick_file()
                        .context("no target file selected")
                })
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
                    .and_then(|copied_tracks| {
                        target_project
                            .modify_tracks(move |target_tracks| {
                                target_tracks
                                    .into_iter()
                                    .chain(copied_tracks.into_iter().map(|s| s.track))
                                    .collect()
                            })
                            .context("modifying target file failed")
                    })
                    .and_then(|_| {
                        target_project
                            .serialize_to_string()
                            .context("serializing to string")
                            .and_then(|serialized| {
                                prompt_confirm_enter(
                                    format!(
                                        "Do you want to save the modified file at [{}]? Remember \
                                         to backup your project file just in case, no changes \
                                         were applied yet.",
                                        target_path.display()
                                    )
                                    .as_str(),
                                )
                                .and_then(|_| {
                                    std::fs::write(target_path, serialized)
                                        .context("writing modified project file")
                                })
                            })
                    })
                    .context("applying changes")
            },
        )
        .tap(|res| match res.as_ref() {
            Ok(_) => prompt_confirm_enter("SUCCESS").unwrap(),
            Err(message) => {
                prompt_confirm_enter(format!("ERROR: {message:?}").as_str()).unwrap();
            }
        })
}
