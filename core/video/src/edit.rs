//! What the person does to a clip: cut the start and the end, crop, and lay marks over it.
//!
//! An [`Edit`] is what the editor window sends; [`resolve`] checks it against the clip and turns
//! it into exactly what will be exported, or says in words what is wrong. All times are in
//! milliseconds on the clip's own timeline (0 is the first frame of the *uncut* clip), so cutting
//! the start never moves a mark.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// The most marks one clip may carry.
pub const MAX_OVERLAYS: usize = 50;
/// The shortest clip that can be exported.
pub const MIN_CLIP_MS: u64 = 500;
/// The smallest crop, in pixels, on each side.
pub const MIN_CROP: u32 = 16;
/// The most places a moving crop goes through.
pub const MAX_PLACES: usize = 400;

/// The recorded clip, as ffprobe describes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clip {
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    /// The recording has a sound track, which the export keeps.
    #[serde(default)]
    pub has_audio: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Crop {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// A transparent picture (a red circle, a text) laid over the clip for a while.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Overlay {
    /// A PNG with transparency, drawn by the editor.
    pub image: PathBuf,
    /// Where its top-left corner goes on the uncropped frame.
    pub x: i32,
    pub y: i32,
    /// When it shows, on the uncut clip's timeline.
    pub from_ms: u64,
    pub to_ms: u64,
}

/// Where the crop's top-left corner is at a moment, when it follows a window that moves. Between
/// two places it slides from one to the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placed {
    /// On the uncut clip's timeline.
    pub at_ms: u64,
    pub x: u32,
    pub y: u32,
}

/// The keys pressed, laid over the clip as it is sent (after the crop): a list of pictures for
/// ffmpeg's concat reader (see [`crate::export::keys_list`]), all the same size, and where their
/// top-left corner goes on the sent picture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeysLayer {
    pub list: PathBuf,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Edit {
    /// Where the exported clip starts.
    pub start_ms: u64,
    /// Where it ends; `None` is the end of the clip.
    pub end_ms: Option<u64>,
    pub crop: Option<Crop>,
    pub overlays: Vec<Overlay>,
    /// With a crop: where it is over time, so it follows a window that was moved. Its size stays
    /// the crop's; the crop's own corner is where it starts.
    pub follow: Vec<Placed>,
    pub keys: Option<KeysLayer>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    EmptyClip,
    EndAfterClip {
        end_ms: u64,
        duration_ms: u64,
    },
    TooShort {
        length_ms: u64,
    },
    CropTooSmall {
        w: u32,
        h: u32,
    },
    CropOutside,
    TooManyOverlays(usize),
    OverlayWithoutImage(usize),
    OverlayEndsBeforeItStarts(usize),
    /// The places a crop follows are out of order, too many, or come without a crop.
    BadFollow,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::EmptyClip => write!(f, "The recording has no picture"),
            Problem::EndAfterClip {
                end_ms,
                duration_ms,
            } => write!(
                f,
                "The end ({}) is after the end of the recording ({})",
                clock(*end_ms),
                clock(*duration_ms)
            ),
            Problem::TooShort { length_ms } => write!(
                f,
                "The clip would be {} long; it must be at least {}",
                clock(*length_ms),
                clock(MIN_CLIP_MS)
            ),
            Problem::CropTooSmall { w, h } => write!(
                f,
                "The crop is {w} by {h} pixels; each side must be at least {MIN_CROP}"
            ),
            Problem::CropOutside => write!(f, "The crop reaches outside the picture"),
            Problem::TooManyOverlays(n) => {
                write!(
                    f,
                    "There are {n} marks; {MAX_OVERLAYS} is the most a clip can carry"
                )
            }
            Problem::OverlayWithoutImage(i) => write!(f, "Mark {} has no picture", i + 1),
            Problem::OverlayEndsBeforeItStarts(i) => {
                write!(f, "Mark {} ends before it starts", i + 1)
            }
            Problem::BadFollow => write!(f, "The crop cannot follow the window as it moved"),
        }
    }
}

/// `1.5 s`, `0.5 s`, `12 s`, for messages.
fn clock(ms: u64) -> String {
    if ms % 1000 == 0 {
        format!("{} s", ms / 1000)
    } else {
        format!("{:.1} s", ms as f64 / 1000.0)
    }
}

/// An edit that has been checked against its clip: what the exporter does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub start_ms: u64,
    pub end_ms: u64,
    /// Even-sized and inside the picture (video needs even sides); `None` when the crop is the
    /// whole picture.
    pub crop: Option<Crop>,
    /// Only the marks that show during the clip, with their times cut to it.
    pub overlays: Vec<Overlay>,
    /// Marks left out because they only show in the part that was cut.
    pub dropped_overlays: usize,
    /// Where the crop goes over time (even, inside the picture); empty when it stays put.
    pub follow: Vec<Placed>,
    pub keys: Option<KeysLayer>,
}

impl Resolved {
    pub fn length_ms(&self) -> u64 {
        self.end_ms - self.start_ms
    }

    /// The size of the exported picture.
    pub fn size(&self, clip: &Clip) -> (u32, u32) {
        match self.crop {
            Some(c) => (c.w, c.h),
            // yuv420 needs even sides, and the exporter rounds the same way
            None => (clip.width & !1, clip.height & !1),
        }
    }
}

/// Check the edit and make it exact. Every problem is reported, not only the first.
pub fn resolve(edit: &Edit, clip: &Clip) -> Result<Resolved, Vec<Problem>> {
    let mut problems = Vec::new();
    if clip.width < 2 || clip.height < 2 || clip.duration_ms == 0 {
        return Err(vec![Problem::EmptyClip]);
    }

    let end_ms = edit.end_ms.unwrap_or(clip.duration_ms);
    let start_ms = edit.start_ms;
    if end_ms > clip.duration_ms {
        problems.push(Problem::EndAfterClip {
            end_ms,
            duration_ms: clip.duration_ms,
        });
    } else if end_ms.saturating_sub(start_ms) < MIN_CLIP_MS {
        problems.push(Problem::TooShort {
            length_ms: end_ms.saturating_sub(start_ms),
        });
    }

    let mut crop = None;
    if let Some(c) = edit.crop {
        // rounded down to even numbers, inside the picture
        let (x, y) = (c.x & !1, c.y & !1);
        let (w, h) = (c.w & !1, c.h & !1);
        if w < MIN_CROP || h < MIN_CROP {
            problems.push(Problem::CropTooSmall { w: c.w, h: c.h });
        } else if u64::from(x) + u64::from(w) > u64::from(clip.width)
            || u64::from(y) + u64::from(h) > u64::from(clip.height)
        {
            problems.push(Problem::CropOutside);
        } else if !(x == 0 && y == 0 && w >= clip.width & !1 && h >= clip.height & !1) {
            crop = Some(Crop { x, y, w, h });
        }
    }

    if edit.overlays.len() > MAX_OVERLAYS {
        problems.push(Problem::TooManyOverlays(edit.overlays.len()));
    }
    let mut overlays = Vec::new();
    let mut dropped = 0;
    for (i, o) in edit.overlays.iter().enumerate().take(MAX_OVERLAYS) {
        if o.image.as_os_str().is_empty() {
            problems.push(Problem::OverlayWithoutImage(i));
            continue;
        }
        if o.to_ms <= o.from_ms {
            problems.push(Problem::OverlayEndsBeforeItStarts(i));
            continue;
        }
        let (from, to) = (o.from_ms.max(start_ms), o.to_ms.min(end_ms));
        if to <= from {
            dropped += 1;
            continue;
        }
        overlays.push(Overlay {
            from_ms: from,
            to_ms: to,
            ..o.clone()
        });
    }

    let mut follow = Vec::new();
    if !edit.follow.is_empty() {
        let ordered = edit.follow.windows(2).all(|w| w[0].at_ms <= w[1].at_ms);
        match (crop, edit.crop) {
            (_, None) => problems.push(Problem::BadFollow),
            _ if !ordered || edit.follow.len() > MAX_PLACES => problems.push(Problem::BadFollow),
            (c, Some(asked)) => {
                // the size is the crop's (or the whole picture's, when it covers all of it)
                let (w, h) = c.map_or((clip.width & !1, clip.height & !1), |c| (c.w, c.h));
                let (max_x, max_y) = (clip.width.saturating_sub(w), clip.height.saturating_sub(h));
                let moves = edit
                    .follow
                    .iter()
                    .any(|p| (p.x & !1, p.y & !1) != (asked.x & !1, asked.y & !1));
                if moves {
                    follow = edit
                        .follow
                        .iter()
                        .map(|p| Placed {
                            at_ms: p.at_ms,
                            x: p.x.min(max_x) & !1,
                            y: p.y.min(max_y) & !1,
                        })
                        .collect();
                    if crop.is_none() {
                        crop = Some(Crop { x: 0, y: 0, w, h });
                    }
                }
            }
        }
    }

    if problems.is_empty() {
        Ok(Resolved {
            start_ms,
            end_ms,
            crop,
            overlays,
            dropped_overlays: dropped,
            follow,
            keys: edit.keys.clone(),
        })
    } else {
        Err(problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIP: Clip = Clip {
        has_audio: false,
        width: 1920,
        height: 1080,
        duration_ms: 10_000,
    };

    fn mark(from: u64, to: u64) -> Overlay {
        Overlay {
            image: "circle.png".into(),
            x: 100,
            y: 100,
            from_ms: from,
            to_ms: to,
        }
    }

    fn words(e: Vec<Problem>) -> Vec<String> {
        e.iter().map(|p| p.to_string()).collect()
    }

    #[test]
    fn no_edit_exports_the_whole_clip() {
        let r = resolve(&Edit::default(), &CLIP).unwrap();
        assert_eq!((r.start_ms, r.end_ms), (0, 10_000));
        assert_eq!(r.length_ms(), 10_000);
        assert_eq!(r.crop, None);
        assert_eq!(r.size(&CLIP), (1920, 1080));
        assert!(r.overlays.is_empty());
    }

    #[test]
    fn an_odd_sized_picture_is_exported_one_pixel_smaller() {
        let odd = Clip {
            has_audio: false,
            width: 1279,
            height: 719,
            duration_ms: 3000,
        };
        assert_eq!(
            resolve(&Edit::default(), &odd).unwrap().size(&odd),
            (1278, 718)
        );
    }

    #[test]
    fn the_cut_must_fit_the_clip_and_leave_something() {
        let edit = |start, end| Edit {
            start_ms: start,
            end_ms: end,
            ..Edit::default()
        };
        assert!(resolve(&edit(2_000, Some(9_000)), &CLIP).is_ok());
        assert!(resolve(&edit(9_500, None), &CLIP).is_ok());
        assert_eq!(
            words(resolve(&edit(0, Some(10_500)), &CLIP).unwrap_err()),
            ["The end (10.5 s) is after the end of the recording (10 s)"]
        );
        assert_eq!(
            words(resolve(&edit(9_800, None), &CLIP).unwrap_err()),
            ["The clip would be 0.2 s long; it must be at least 0.5 s"]
        );
        // a start after the end is a clip of no length, not an underflow
        assert_eq!(
            words(resolve(&edit(8_000, Some(2_000)), &CLIP).unwrap_err()),
            ["The clip would be 0 s long; it must be at least 0.5 s"]
        );
    }

    #[test]
    fn a_crop_is_made_even_and_checked() {
        let crop = |x, y, w, h| Edit {
            crop: Some(Crop { x, y, w, h }),
            ..Edit::default()
        };
        let r = resolve(&crop(101, 51, 641, 361), &CLIP).unwrap();
        assert_eq!(
            r.crop,
            Some(Crop {
                x: 100,
                y: 50,
                w: 640,
                h: 360
            })
        );
        assert_eq!(r.size(&CLIP), (640, 360));

        // the whole picture is no crop at all
        assert_eq!(resolve(&crop(0, 0, 1920, 1080), &CLIP).unwrap().crop, None);
        // one pixel too many is rounded away; two is outside
        assert_eq!(resolve(&crop(0, 0, 1921, 1081), &CLIP).unwrap().crop, None);
        assert_eq!(
            resolve(&crop(0, 0, 1922, 1082), &CLIP).unwrap_err(),
            [Problem::CropOutside]
        );
        assert_eq!(
            words(resolve(&crop(0, 0, 10, 400), &CLIP).unwrap_err()),
            ["The crop is 10 by 400 pixels; each side must be at least 16"]
        );
        assert_eq!(
            resolve(&crop(1900, 0, 100, 100), &CLIP).unwrap_err(),
            [Problem::CropOutside]
        );
        // a huge corner cannot overflow
        assert!(resolve(&crop(u32::MAX, u32::MAX, u32::MAX, u32::MAX), &CLIP).is_err());
    }

    #[test]
    fn marks_are_cut_to_the_clip_and_those_in_the_cut_part_are_dropped() {
        let edit = Edit {
            start_ms: 2_000,
            end_ms: Some(8_000),
            overlays: vec![
                mark(1_000, 3_000), // starts before the cut: shortened
                mark(4_000, 5_000), // inside: unchanged
                mark(0, 1_500),     // only in the cut start: dropped
                mark(7_500, 9_900), // ends after the cut end: shortened
                mark(8_000, 9_000), // starts where the clip ends: dropped
            ],
            ..Edit::default()
        };
        let r = resolve(&edit, &CLIP).unwrap();
        let times: Vec<(u64, u64)> = r.overlays.iter().map(|o| (o.from_ms, o.to_ms)).collect();
        assert_eq!(times, [(2_000, 3_000), (4_000, 5_000), (7_500, 8_000)]);
        assert_eq!(r.dropped_overlays, 2);
    }

    #[test]
    fn broken_marks_and_every_other_problem_are_all_reported_together() {
        let edit = Edit {
            end_ms: Some(20_000),
            crop: Some(Crop {
                x: 0,
                y: 0,
                w: 8,
                h: 8,
            }),
            overlays: vec![
                Overlay {
                    image: PathBuf::new(),
                    ..mark(0, 1_000)
                },
                mark(2_000, 2_000),
            ],
            ..Edit::default()
        };
        let p = words(resolve(&edit, &CLIP).unwrap_err());
        assert_eq!(p.len(), 4, "{p:?}");
        assert!(p.contains(&"Mark 1 has no picture".to_string()));
        assert!(p.contains(&"Mark 2 ends before it starts".to_string()));
    }

    #[test]
    fn a_following_crop_is_kept_inside_the_picture_and_needs_a_crop() {
        let place = |at_ms, x, y| Placed { at_ms, x, y };
        let edit = Edit {
            crop: Some(Crop {
                x: 100,
                y: 100,
                w: 640,
                h: 360,
            }),
            follow: vec![place(0, 100, 100), place(1_000, 1_301, 9_999)],
            ..Edit::default()
        };
        let r = resolve(&edit, &CLIP).unwrap();
        // the size stays; the corner is even and cannot go past the picture
        assert_eq!(r.follow, vec![place(0, 100, 100), place(1_000, 1_280, 720)]);
        // a window that never moved needs nothing more than its crop
        let still = Edit {
            follow: vec![place(0, 101, 100), place(1_000, 100, 101)],
            ..edit.clone()
        };
        assert!(resolve(&still, &CLIP).unwrap().follow.is_empty());
        // out of order, or without a crop
        let backwards = Edit {
            follow: vec![place(1_000, 0, 0), place(0, 10, 10)],
            ..edit.clone()
        };
        assert_eq!(
            resolve(&backwards, &CLIP).unwrap_err(),
            [Problem::BadFollow]
        );
        let no_crop = Edit { crop: None, ..edit };
        assert_eq!(resolve(&no_crop, &CLIP).unwrap_err(), [Problem::BadFollow]);
    }

    #[test]
    fn too_many_marks_and_an_empty_clip_are_refused() {
        let edit = Edit {
            overlays: (0..=MAX_OVERLAYS as u64).map(|i| mark(i, i + 10)).collect(),
            ..Edit::default()
        };
        assert_eq!(
            resolve(&edit, &CLIP).unwrap_err(),
            [Problem::TooManyOverlays(MAX_OVERLAYS + 1)]
        );
        let empty = Clip {
            has_audio: false,
            width: 0,
            height: 0,
            duration_ms: 0,
        };
        assert_eq!(
            resolve(&Edit::default(), &empty).unwrap_err(),
            [Problem::EmptyClip]
        );
    }

    #[test]
    fn an_edit_survives_being_sent_to_and_from_the_editor() {
        let edit = Edit {
            start_ms: 500,
            end_ms: Some(4_000),
            crop: Some(Crop {
                x: 2,
                y: 4,
                w: 320,
                h: 240,
            }),
            overlays: vec![mark(600, 900)],
            follow: vec![Placed {
                at_ms: 700,
                x: 10,
                y: 12,
            }],
            keys: Some(KeysLayer {
                list: "keys.txt".into(),
                x: 4,
                y: 200,
            }),
        };
        let json = serde_json::to_string(&edit).unwrap();
        assert_eq!(serde_json::from_str::<Edit>(&json).unwrap(), edit);
        // a bare object is an edit that changes nothing
        assert_eq!(serde_json::from_str::<Edit>("{}").unwrap(), Edit::default());
    }
}
