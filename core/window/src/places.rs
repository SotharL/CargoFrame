//! The folders a Files block looks in, worked out from the program that had focus.
//!
//! All of them sit on one line: the program's own folder, the folders above it, and the project
//! folder (one of those). So the highest of them is above all the others, and a file path
//! relative to that folder is the same whichever block or folder found the file.

use cargoframe_context::climb;
use cargoframe_layout::{BlockKind, Layout, LookFolder, LookIn};
use std::path::{Path, PathBuf};

/// One folder to look in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub which: LookFolder,
    /// The folder the choice means.
    pub base: String,
    /// Where files are listed from: `base`, or one of the subfolders inside it.
    pub dir: String,
}

/// What a block asks for and what could be worked out.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Places {
    pub places: Vec<Place>,
    /// Choices that could not be turned into a folder (no program, or not that many folders
    /// above it), with the reason.
    pub missing: Vec<(LookFolder, String)>,
}

pub fn label(which: LookFolder) -> &'static str {
    match which {
        LookFolder::Project => "Project folder",
        LookFolder::Program => "Program folder",
        LookFolder::Above1 => "One folder above the program",
        LookFolder::Above2 => "Two folders above the program",
        LookFolder::Above3 => "Three folders above the program",
    }
}

fn join(base: &str, sub: &str) -> String {
    let sep = if base.contains('\\') { '\\' } else { '/' };
    let sub = if sep == '\\' {
        sub.replace('/', "\\")
    } else {
        sub.to_string()
    };
    let mut out = base.trim_end_matches(['/', '\\']).to_string();
    out.push(sep);
    out.push_str(&sub);
    out
}

/// The folders `look` means for this capture: each subfolder in each folder. Unsafe subfolders
/// (see [`cargoframe_layout::clean_subfolder`]) are ignored: the folders themselves are used.
pub fn places_for(look: &LookIn, project: Option<&str>, program: Option<&str>) -> Places {
    let subs = look.clean_subfolders().unwrap_or_default();
    let mut out = Places::default();
    for which in look.folders() {
        let base = match which {
            LookFolder::Project => project.map(str::to_string),
            other => program.and_then(|p| climb::folder_above(p, other.levels_above_program()?)),
        };
        let Some(base) = base else {
            let why = match which {
                LookFolder::Project => "no project folder was found for this capture",
                _ if program.is_none() => "no program was in focus",
                _ => "the program is not that deep in its folders",
            };
            out.missing.push((which, why.to_string()));
            continue;
        };
        let dirs: Vec<String> = if subs.is_empty() {
            vec![base.clone()]
        } else {
            subs.iter().map(|s| join(&base, s)).collect()
        };
        for dir in dirs {
            // Two choices can mean the same folder: list it once.
            if out.places.iter().any(|p| p.dir == dir) {
                continue;
            }
            out.places.push(Place {
                which,
                base: base.clone(),
                dir,
            });
        }
    }
    out
}

/// The folder every file path is relative to: the highest folder any Files block looks in
/// (before its subfolder). With only the default that is the project folder, as it always was.
pub fn files_root(
    layout: &Layout,
    project: Option<&str>,
    program: Option<&str>,
) -> Option<PathBuf> {
    layout
        .blocks
        .iter()
        .filter_map(|b| match &b.kind {
            BlockKind::Files(f) => Some(places_for(&f.look_in, project, program).places),
            _ => None,
        })
        .flatten()
        .map(|p| PathBuf::from(p.base))
        .min_by_key(|p| p.components().count())
}

/// `dir` relative to `root`, for showing.
pub fn relative(root: &Path, dir: &Path) -> String {
    dir.strip_prefix(root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| dir.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargoframe_layout::{Block, FilesBlock, Rect};

    const PROGRAM: &str = "/dev/foo/bin/release/foo";
    const PROJECT: &str = "/dev/foo";

    fn look(folders: &[LookFolder], sub: &str) -> LookIn {
        LookIn {
            folders: folders.to_vec(),
            subfolder: sub.into(),
        }
    }

    fn dirs(p: &Places) -> Vec<&str> {
        p.places.iter().map(|p| p.dir.as_str()).collect()
    }

    #[test]
    fn the_default_is_the_project_folder() {
        let p = places_for(&LookIn::default(), Some(PROJECT), Some(PROGRAM));
        assert_eq!(dirs(&p), ["/dev/foo"]);
        assert_eq!(p.places[0].which, LookFolder::Project);
        assert!(p.missing.is_empty());
    }

    #[test]
    fn folders_above_the_program_and_a_subfolder_in_each() {
        let l = look(
            &[LookFolder::Program, LookFolder::Above2, LookFolder::Above3],
            "logs",
        );
        let p = places_for(&l, Some(PROJECT), Some(PROGRAM));
        assert_eq!(
            dirs(&p),
            ["/dev/foo/bin/release/logs", "/dev/foo/logs", "/dev/logs"]
        );
        assert_eq!(p.places[1].base, "/dev/foo");
    }

    #[test]
    fn windows_paths_keep_their_separators() {
        let l = look(&[LookFolder::Above1], "build/logs");
        let p = places_for(&l, None, Some(r"C:\Dev\Foo\bin\Release\Foo.exe"));
        assert_eq!(dirs(&p), [r"C:\Dev\Foo\bin\build\logs"]);
    }

    #[test]
    fn two_choices_for_the_same_folder_list_it_once() {
        // The project folder is two above the program here.
        let l = look(&[LookFolder::Project, LookFolder::Above2], "");
        let p = places_for(&l, Some(PROJECT), Some(PROGRAM));
        assert_eq!(dirs(&p), ["/dev/foo"]);
    }

    #[test]
    fn what_cannot_be_found_is_named_not_guessed() {
        let l = look(&[LookFolder::Project, LookFolder::Above3], "");
        let none = places_for(&l, None, None);
        assert!(none.places.is_empty());
        assert_eq!(none.missing.len(), 2);
        assert!(none.missing[0].1.contains("project"));
        assert!(none.missing[1].1.contains("no program"));
        let shallow = places_for(&look(&[LookFolder::Above3], ""), None, Some("/a/b/prog"));
        assert!(shallow.places.is_empty());
        assert!(shallow.missing[0].1.contains("not that deep"));
    }

    #[test]
    fn several_subfolders_are_each_looked_in() {
        let p = places_for(
            &look(&[LookFolder::Project, LookFolder::Program], "logs, crash"),
            Some(PROJECT),
            Some(PROGRAM),
        );
        assert_eq!(
            dirs(&p),
            [
                "/dev/foo/logs",
                "/dev/foo/crash",
                "/dev/foo/bin/release/logs",
                "/dev/foo/bin/release/crash"
            ]
        );
    }

    #[test]
    fn an_unsafe_subfolder_is_ignored() {
        let l = look(&[LookFolder::Program], "../../etc");
        let p = places_for(&l, None, Some(PROGRAM));
        assert_eq!(dirs(&p), ["/dev/foo/bin/release"]);
    }

    fn files_block(id: &str, look_in: LookIn) -> Block {
        Block {
            id: id.into(),
            rect: Rect {
                x: 0,
                y: 0,
                w: 6,
                h: 2,
            },
            kind: BlockKind::Files(FilesBlock {
                title: id.into(),
                include: vec!["*.log".into()],
                preselect: vec![],
                look_in,
            }),
        }
    }

    fn layout(blocks: Vec<Block>) -> Layout {
        Layout {
            accepts_input: false,
            id: "l".into(),
            name: "L".into(),
            blocks,
        }
    }

    #[test]
    fn the_root_is_the_project_folder_unless_a_block_looks_higher() {
        let plain = layout(vec![files_block("a", LookIn::default())]);
        assert_eq!(
            files_root(&plain, Some(PROJECT), Some(PROGRAM)),
            Some(PathBuf::from("/dev/foo"))
        );
        let high = layout(vec![
            files_block("a", LookIn::default()),
            files_block("b", look(&[LookFolder::Above3], "logs")),
        ]);
        assert_eq!(
            files_root(&high, Some(PROJECT), Some(PROGRAM)),
            Some(PathBuf::from("/dev"))
        );
        let none = layout(vec![]);
        assert_eq!(files_root(&none, Some(PROJECT), Some(PROGRAM)), None);
    }

    #[test]
    fn relative_shows_where_inside_the_root() {
        assert_eq!(
            relative(Path::new("/dev"), Path::new("/dev/foo/logs")),
            "foo/logs"
        );
        assert_eq!(relative(Path::new("/x"), Path::new("/dev")), "/dev");
    }
}
