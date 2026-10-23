//! The fallback ladder, used when a share could not be delivered.
//!
//! 1. Save the zip to the configured safe folder.
//! 2. If that is unset, unwritable or full: ask the person for a folder (up to
//!    [`LadderConfig::max_picks`] times, in case the chosen folder is also unusable). A
//!    "remember this folder" tick becomes the new safe folder.
//! 3. If they cancel or nothing worked: keep the zip in CargoFrame's own data folder and say
//!    so, with the path.
//!
//! The zip is written as `<name>.part` and renamed when complete, so a crash never leaves a
//! half-written zip that looks finished. That is the job of [`FallbackIo::save_zip`].

use crate::naming;

/// Why a write failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteFailure {
    /// Folder missing, read-only or access denied.
    NotWritable,
    DiskFull,
    Other(String),
}

/// A folder chosen in the system picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickedFolder {
    pub dir: String,
    /// The "remember this folder" tick.
    pub remember: bool,
}

/// Side effects the ladder needs. The app implements this with real files and dialogs.
pub trait FallbackIo {
    /// Write the finished zip into `dir` as `<file_name>.part`, then rename it to `file_name`.
    /// Returns the final path.
    fn save_zip(&mut self, dir: &str, file_name: &str) -> Result<String, WriteFailure>;
    /// Show the folder picker. `attempt` starts at 1. `None` means the person cancelled.
    fn pick_folder(&mut self, attempt: u32) -> Option<PickedFolder>;
    /// CargoFrame's own data folder for kept zips.
    fn local_data_dir(&self) -> String;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LadderConfig {
    /// The safe folder from settings.
    pub safe_folder: Option<String>,
    /// Folder picker attempts before giving up (default 3).
    pub max_picks: u32,
    /// False when no dialog can be shown (for example a scheduled or headless run).
    pub allow_prompt: bool,
}

impl Default for LadderConfig {
    fn default() -> Self {
        Self {
            safe_folder: None,
            max_picks: 3,
            allow_prompt: true,
        }
    }
}

/// Where the zip ended up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Saved {
    SafeFolder {
        path: String,
    },
    Picked {
        path: String,
        /// Set when the person ticked "remember": store it as the new safe folder.
        remember_dir: Option<String>,
    },
    /// Kept in CargoFrame's data folder; show an error with the path and why.
    LocalData {
        path: String,
        reason: String,
    },
    /// Even the data folder failed. The caller must keep the zip where it is and show an error.
    Unsaved {
        reason: String,
    },
}

/// One thing the ladder tried, for the log and the debug panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempt {
    pub step: Step,
    pub dir: String,
    pub result: Result<(), WriteFailure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    SafeFolder,
    Picked,
    LocalData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LadderReport {
    pub saved: Saved,
    pub attempts: Vec<Attempt>,
}

fn describe(f: &WriteFailure) -> String {
    match f {
        WriteFailure::NotWritable => "folder not writable".to_string(),
        WriteFailure::DiskFull => "disk full".to_string(),
        WriteFailure::Other(m) => m.clone(),
    }
}

/// Walk the ladder for a finished zip named `file_name`.
pub fn run_ladder(cfg: &LadderConfig, io: &mut dyn FallbackIo, file_name: &str) -> LadderReport {
    let mut attempts: Vec<Attempt> = Vec::new();
    // Why the earlier steps did not work, shown when the zip lands in the data folder.
    let mut why: Vec<String> = Vec::new();

    // Step 1: the safe folder.
    match cfg
        .safe_folder
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
    {
        Some(dir) => match io.save_zip(dir, file_name) {
            Ok(path) => {
                attempts.push(Attempt {
                    step: Step::SafeFolder,
                    dir: dir.to_string(),
                    result: Ok(()),
                });
                return LadderReport {
                    saved: Saved::SafeFolder { path },
                    attempts,
                };
            }
            Err(e) => {
                why.push(format!("safe folder: {}", describe(&e)));
                attempts.push(Attempt {
                    step: Step::SafeFolder,
                    dir: dir.to_string(),
                    result: Err(e),
                });
            }
        },
        None => why.push("no safe folder set".to_string()),
    }

    // Step 2: ask the person.
    if cfg.allow_prompt {
        for attempt in 1..=cfg.max_picks {
            let Some(picked) = io.pick_folder(attempt) else {
                why.push("folder choice cancelled".to_string());
                break;
            };
            match io.save_zip(&picked.dir, file_name) {
                Ok(path) => {
                    attempts.push(Attempt {
                        step: Step::Picked,
                        dir: picked.dir.clone(),
                        result: Ok(()),
                    });
                    return LadderReport {
                        saved: Saved::Picked {
                            path,
                            remember_dir: picked.remember.then_some(picked.dir),
                        },
                        attempts,
                    };
                }
                Err(e) => {
                    why.push(format!("chosen folder: {}", describe(&e)));
                    attempts.push(Attempt {
                        step: Step::Picked,
                        dir: picked.dir,
                        result: Err(e),
                    });
                }
            }
        }
    } else {
        why.push("no prompt allowed".to_string());
    }

    // Step 3: the data folder.
    let dir = io.local_data_dir();
    match io.save_zip(&dir, file_name) {
        Ok(path) => {
            attempts.push(Attempt {
                step: Step::LocalData,
                dir,
                result: Ok(()),
            });
            LadderReport {
                saved: Saved::LocalData {
                    path,
                    reason: why.join("; "),
                },
                attempts,
            }
        }
        Err(e) => {
            why.push(format!("data folder: {}", describe(&e)));
            attempts.push(Attempt {
                step: Step::LocalData,
                dir,
                result: Err(e),
            });
            LadderReport {
                saved: Saved::Unsaved {
                    reason: why.join("; "),
                },
                attempts,
            }
        }
    }
}

/// Convenience for callers that build the name from the clock.
pub fn file_name_for(date: (u16, u8, u8), time: (u8, u8, u8), service: &str) -> String {
    naming::zip_file_name(date, time, service)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, VecDeque};

    /// Scripted environment: folders map to a result; picks come from a queue.
    struct Fake {
        folders: HashMap<String, Result<(), WriteFailure>>,
        picks: VecDeque<Option<PickedFolder>>,
        data_dir: String,
        saved_calls: Vec<String>,
        pick_calls: u32,
    }

    impl Fake {
        fn new() -> Self {
            Self {
                folders: HashMap::new(),
                picks: VecDeque::new(),
                data_dir: "/data".to_string(),
                saved_calls: Vec::new(),
                pick_calls: 0,
            }
        }
        fn folder(mut self, dir: &str, r: Result<(), WriteFailure>) -> Self {
            self.folders.insert(dir.to_string(), r);
            self
        }
        fn pick(mut self, p: Option<(&str, bool)>) -> Self {
            self.picks.push_back(p.map(|(d, r)| PickedFolder {
                dir: d.to_string(),
                remember: r,
            }));
            self
        }
    }

    impl FallbackIo for Fake {
        fn save_zip(&mut self, dir: &str, file_name: &str) -> Result<String, WriteFailure> {
            self.saved_calls.push(dir.to_string());
            match self.folders.get(dir) {
                Some(Ok(())) => Ok(format!("{dir}/{file_name}")),
                Some(Err(e)) => Err(e.clone()),
                None => Err(WriteFailure::NotWritable),
            }
        }
        fn pick_folder(&mut self, _attempt: u32) -> Option<PickedFolder> {
            self.pick_calls += 1;
            self.picks.pop_front().flatten()
        }
        fn local_data_dir(&self) -> String {
            self.data_dir.clone()
        }
    }

    fn cfg(safe: Option<&str>) -> LadderConfig {
        LadderConfig {
            safe_folder: safe.map(String::from),
            ..LadderConfig::default()
        }
    }

    #[test]
    fn safe_folder_first() {
        let mut io = Fake::new().folder("/safe", Ok(()));
        let r = run_ladder(&cfg(Some("/safe")), &mut io, "a.zip");
        assert_eq!(
            r.saved,
            Saved::SafeFolder {
                path: "/safe/a.zip".into()
            }
        );
        assert_eq!(io.pick_calls, 0);
        assert_eq!(r.attempts.len(), 1);
    }

    #[test]
    fn full_safe_folder_leads_to_the_picker() {
        let mut io = Fake::new()
            .folder("/safe", Err(WriteFailure::DiskFull))
            .folder("/usb", Ok(()))
            .pick(Some(("/usb", true)));
        let r = run_ladder(&cfg(Some("/safe")), &mut io, "a.zip");
        assert_eq!(
            r.saved,
            Saved::Picked {
                path: "/usb/a.zip".into(),
                remember_dir: Some("/usb".into())
            }
        );
    }

    #[test]
    fn unset_safe_folder_skips_straight_to_the_picker() {
        let mut io = Fake::new()
            .folder("/usb", Ok(()))
            .pick(Some(("/usb", false)));
        let r = run_ladder(&cfg(None), &mut io, "a.zip");
        assert_eq!(
            r.saved,
            Saved::Picked {
                path: "/usb/a.zip".into(),
                remember_dir: None
            }
        );
        assert_eq!(io.saved_calls, vec!["/usb"]);
    }

    #[test]
    fn blank_safe_folder_counts_as_unset() {
        let mut io = Fake::new().pick(None);
        let r = run_ladder(&cfg(Some("   ")), &mut io, "a.zip");
        assert!(matches!(r.saved, Saved::Unsaved { .. }));
        assert!(!io.saved_calls.contains(&"   ".to_string()));
    }

    #[test]
    fn bad_pick_asks_again_then_succeeds() {
        let mut io = Fake::new()
            .folder("/ro", Err(WriteFailure::NotWritable))
            .folder("/ok", Ok(()))
            .pick(Some(("/ro", false)))
            .pick(Some(("/ok", false)));
        let r = run_ladder(&cfg(None), &mut io, "a.zip");
        assert!(matches!(r.saved, Saved::Picked { ref path, .. } if path == "/ok/a.zip"));
        assert_eq!(io.pick_calls, 2);
    }

    #[test]
    fn cancel_keeps_the_zip_in_the_data_folder() {
        let mut io = Fake::new().folder("/data", Ok(())).pick(None);
        let r = run_ladder(&cfg(Some("/safe")), &mut io, "a.zip");
        match r.saved {
            Saved::LocalData { path, reason } => {
                assert_eq!(path, "/data/a.zip");
                assert!(reason.contains("safe folder"));
                assert!(reason.contains("cancelled"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn gives_up_after_max_picks() {
        let mut io = Fake::new()
            .folder("/data", Ok(()))
            .pick(Some(("/a", false)))
            .pick(Some(("/b", false)))
            .pick(Some(("/c", false)))
            .pick(Some(("/d", false)));
        let r = run_ladder(&cfg(None), &mut io, "a.zip");
        assert_eq!(io.pick_calls, 3);
        assert!(matches!(r.saved, Saved::LocalData { .. }));
    }

    #[test]
    fn no_prompt_mode_never_shows_the_picker() {
        let mut io = Fake::new().folder("/data", Ok(()));
        let config = LadderConfig {
            allow_prompt: false,
            ..LadderConfig::default()
        };
        let r = run_ladder(&config, &mut io, "a.zip");
        assert_eq!(io.pick_calls, 0);
        assert!(matches!(r.saved, Saved::LocalData { .. }));
    }

    #[test]
    fn everything_failing_reports_unsaved() {
        let mut io = Fake::new()
            .folder("/safe", Err(WriteFailure::DiskFull))
            .folder("/data", Err(WriteFailure::Other("quota".into())))
            .pick(None);
        let r = run_ladder(&cfg(Some("/safe")), &mut io, "a.zip");
        match r.saved {
            Saved::Unsaved { reason } => {
                assert!(reason.contains("disk full"));
                assert!(reason.contains("quota"));
            }
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(r.attempts.last().unwrap().step, Step::LocalData);
    }

    #[test]
    fn file_name_helper_uses_the_documented_format() {
        assert_eq!(
            file_name_for((2026, 10, 23), (15, 30, 45), "Jira"),
            "CargoFrame_2026-10-23_153045_jira.zip"
        );
    }
}
