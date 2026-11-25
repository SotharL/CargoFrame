//! Finding a value in a file, whatever kind of file it is.
//!
//! * **Value of a name** — in a JSON, YAML, TOML, INI or XML file (known from the file's name),
//!   the key of that name (`version`, `build.number`); in any other file, or when that finds
//!   nothing, the text after `name:`, `name =` or `name` followed by a tab, up to the end of the
//!   line (`GPU: RTX 4070` gives `RTX 4070`). Names are matched without regard to case.
//! * **Text between** a start and an end on one line; with no end, up to the end of the line.
//! * **Line with** a piece of text: the whole line.
//! * **Picked** by selecting it in a file: in the lines with a text (or every line), right after
//!   that text or from the start of the line, up to a stop (or the end of the line). The settings
//!   window runs the same steps on the file shown (`ui/src/settings/data/pick.ts`).
//!
//! Each gives every place it finds; [`pick`] keeps the first, the last or all of them.

use crate::extract::{extract, RuleError};
use crate::model::{Find, Format, Multiple, Next, Rule, Which};
use cargoframe_i18n::t;

/// The kind of a file, from its name, for the formats that hold named values.
pub fn kind_of(path: &str) -> Option<Format> {
    let ext = path.rsplit('.').next()?.to_ascii_lowercase();
    if !path.contains('.') {
        return None;
    }
    Some(match ext.as_str() {
        "json" | "jsonc" => Format::Json,
        "yaml" | "yml" => Format::Yaml,
        "toml" => Format::Toml,
        "ini" | "cfg" => Format::Ini,
        "xml" | "csproj" | "plist" | "config" => Format::Xml,
        _ => return None,
    })
}

/// What [`kind_of`] says, in words for the preview.
pub fn kind_name(path: &str) -> &'static str {
    match kind_of(path) {
        Some(Format::Json) => "JSON",
        Some(Format::Yaml) => "YAML",
        Some(Format::Toml) => "TOML",
        Some(Format::Ini) => "INI",
        Some(Format::Xml) => "XML",
        _ => "text",
    }
}

fn clean(v: &str) -> String {
    let v = v.trim().trim_end_matches(',').trim();
    let quoted = v.len() >= 2
        && ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')));
    if quoted {
        v[1..v.len() - 1].to_string()
    } else {
        v.to_string()
    }
}

/// `name: value`, `name = value` or `name<TAB>value` lines, case aside.
fn after_name(text: &str, name: &str) -> Vec<String> {
    let name = name.trim();
    let lower = name.to_lowercase();
    text.lines()
        .filter_map(|line| {
            // `"version": "3.1"` in a JSON file read as text
            let t = line.trim_start().trim_start_matches(['"', '\'']);
            // a prefix of the same characters, case aside, of the same length in bytes
            let head = t.get(..name.len())?;
            if head.to_lowercase() != lower {
                return None;
            }
            let rest = &t[name.len()..];
            let rest = rest.trim_start_matches([' ', '"', '\'']);
            let value = rest
                .strip_prefix(':')
                .or_else(|| rest.strip_prefix('='))
                .or_else(|| t[name.len()..].strip_prefix('\t'))?;
            Some(clean(value))
        })
        .filter(|v| !v.is_empty())
        .collect()
}

/// The text between `start` and `end` (or the end of the line), at each place `start` is.
fn between(text: &str, start: &str, end: Option<&str>) -> Vec<String> {
    if start.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for line in text.lines() {
        let mut rest = line;
        while let Some(at) = rest.find(start) {
            let after = &rest[at + start.len()..];
            let (value, next) = match end.filter(|e| !e.is_empty()) {
                Some(e) => match after.find(e) {
                    Some(stop) => (&after[..stop], &after[stop + e.len()..]),
                    None => (after, ""),
                },
                None => (after, ""),
            };
            let value = value.trim();
            if !value.is_empty() {
                out.push(value.to_string());
            }
            rest = next;
        }
    }
    out
}

fn lines_with(text: &str, needle: &str) -> Vec<String> {
    if needle.is_empty() {
        return Vec::new();
    }
    let needle = needle.to_lowercase();
    text.lines()
        .filter(|l| l.to_lowercase().contains(&needle))
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Where one part of a picked value is on `line`, looking from byte `at`: its trimmed start and
/// end. `None` when the line does not have it there. Byte positions are only ever taken from
/// `find`, so every cut falls between characters.
fn part_in(
    line: &str,
    at: usize,
    lines_with: &str,
    from_line_start: bool,
    stop: Option<&str>,
) -> Option<(usize, usize)> {
    let stop = stop.filter(|s| !s.is_empty());
    let found = if lines_with.is_empty() {
        at
    } else {
        at + line[at..].find(lines_with)?
    };
    let mut from = if from_line_start {
        at
    } else {
        found + lines_with.len()
    };
    from += line[from..].len() - line[from..].trim_start_matches(' ').len();
    let to = match stop {
        None => line.len(),
        // the stop is looked for after the value's first character, so a value that starts
        // with the stop (a "[" up to the next "[") is not empty
        Some(s) => {
            let first = line[from..].chars().next().map_or(0, char::len_utf8);
            from + first + line[from + first..].find(s)?
        }
    };
    let part = &line[from..to];
    let start = from + (part.len() - part.trim_start().len());
    let end = to - (part.len() - part.trim_end().len());
    (end > start).then_some((start, end))
}

/// The value [`Find::Pick`] gives on each line it looks in: the first part there, and each of
/// `then` after the one before it (later on that line when it has a text to come after, else on
/// the lines below), joined.
pub fn picked(
    text: &str,
    lines_with: &str,
    from_line_start: bool,
    stop: Option<&str>,
    then: &[Next],
) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for (n, line) in lines.iter().enumerate() {
        let Some((s, e)) = part_in(line, 0, lines_with, from_line_start, stop) else {
            continue;
        };
        let mut value = line[s..e].to_string();
        let (mut at_line, mut at) = (n, e);
        for next in then {
            // on the same line only after a text: "the next line" and "from the start of the
            // line" mean a line below
            let same_line = !next.lines_with.is_empty() && !next.from_line_start;
            let first = if same_line { at_line } else { at_line + 1 };
            let found = (first..lines.len()).find_map(|k| {
                let from = if k == at_line { at } else { 0 };
                let (s, e) = part_in(
                    lines[k],
                    from,
                    &next.lines_with,
                    next.from_line_start,
                    next.stop.as_deref(),
                )?;
                Some((k, s, e))
            });
            if let Some((k, s, e)) = found {
                value.push_str(&next.joined_by);
                value.push_str(&lines[k][s..e]);
                (at_line, at) = (k, e);
            }
        }
        out.push(value);
    }
    out
}

/// Every place `find` finds a value in `text`, the text of the file at `path`. `rule` is only
/// read for [`Find::Advanced`].
pub fn all(
    find: &Find,
    rule: &Rule,
    path: &str,
    text: &str,
    which: Which,
) -> Result<Vec<String>, RuleError> {
    Ok(match find {
        Find::ValueOf { name } => {
            let structured = kind_of(path).and_then(|format| {
                let probe = Rule {
                    format,
                    query: match format {
                        // an element at any depth
                        Format::Xml if !name.contains('/') => format!("//{}", name.trim()),
                        _ => name.trim().to_string(),
                    },
                    multiple: match which {
                        Which::First => Multiple::First,
                        Which::Last => Multiple::Last,
                        Which::All => Multiple::Join,
                    },
                    separator: "\u{1f}".into(),
                    ..Rule::default()
                };
                extract(&probe, text)
                    .ok()
                    .map(|v| v.split('\u{1f}').map(str::to_string).collect::<Vec<_>>())
            });
            match structured {
                Some(found) if !found.is_empty() => found,
                // a JSON file read as text still has `"version": "3.1"` lines
                _ => after_name(text, name),
            }
        }
        Find::Between { start, end } => between(text, start, end.as_deref()),
        Find::LineWith { text: needle } => lines_with(text, needle),
        Find::Pick {
            lines_with,
            from_line_start,
            stop,
            then,
        } => picked(text, lines_with, *from_line_start, stop.as_deref(), then),
        Find::Advanced => vec![extract(rule, text)?],
        // made from other values, after the files are read (see `combine`)
        Find::Combine { .. } => Vec::new(),
    })
}

/// Keep the first, the last or all of what was found.
pub fn pick(mut found: Vec<String>, which: Which, find: &Find) -> Result<String, RuleError> {
    if found.is_empty() {
        return Err(RuleError::NoValue);
    }
    Ok(match which {
        Which::First => found.swap_remove(0),
        Which::Last => found.pop().unwrap_or_default(),
        Which::All => found.join(if matches!(find, Find::LineWith { .. }) {
            "\n"
        } else {
            ", "
        }),
    })
}

/// Check what a person wrote for one value.
pub fn check(find: &Find) -> Result<(), String> {
    match find {
        Find::ValueOf { name } if name.trim().is_empty() => {
            Err(t("Write the name written before the value, like GPU or version").into())
        }
        Find::Between { start, .. } if start.is_empty() => {
            Err(t("Write the text that comes just before the value").into())
        }
        Find::LineWith { text } if text.trim().is_empty() => {
            Err(t("Write a piece of text the line holds").into())
        }
        Find::Pick {
            lines_with,
            from_line_start: false,
            ..
        } if lines_with.is_empty() => Err(t("Choose the text it comes right after").into()),
        Find::Pick { then, .. }
            if then
                .iter()
                .any(|n| n.lines_with.is_empty() && !n.from_line_start) =>
        {
            Err(t("Choose the text each next part comes right after").into())
        }
        Find::Combine { choices } => crate::combine::check(choices),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value_of(name: &str) -> Find {
        Find::ValueOf { name: name.into() }
    }

    fn run(find: &Find, path: &str, text: &str, which: Which) -> Option<String> {
        all(find, &Rule::default(), path, text, which)
            .ok()
            .and_then(|v| pick(v, which, find).ok())
    }

    const SYSTEM: &str = "Host Name:                 DEV-07\n\
                          OS Name:                   Microsoft Windows 11 Pro\n\
                          GPU: NVIDIA RTX 4070\n\
                          gpu: Intel UHD 770\n\
                          RAM = 32 GB\n\
                          Driver\t551.86\n";

    #[test]
    fn a_name_gives_what_is_written_after_it_whatever_the_file() {
        assert_eq!(
            run(
                &value_of("OS Name"),
                "system_info.txt",
                SYSTEM,
                Which::First
            )
            .as_deref(),
            Some("Microsoft Windows 11 Pro")
        );
        assert_eq!(
            run(&value_of("ram"), "system_info.txt", SYSTEM, Which::First).as_deref(),
            Some("32 GB")
        );
        assert_eq!(
            run(&value_of("Driver"), "x.log", SYSTEM, Which::First).as_deref(),
            Some("551.86")
        );
        // case aside, first, last or all
        assert_eq!(
            run(&value_of("GPU"), "a.txt", SYSTEM, Which::Last).as_deref(),
            Some("Intel UHD 770")
        );
        assert_eq!(
            run(&value_of("GPU"), "a.txt", SYSTEM, Which::All).as_deref(),
            Some("NVIDIA RTX 4070, Intel UHD 770")
        );
        assert_eq!(run(&value_of("CPU"), "a.txt", SYSTEM, Which::First), None);
        // "GPUs: …" is another name
        assert_eq!(run(&value_of("GP"), "a.txt", SYSTEM, Which::First), None);
    }

    #[test]
    fn in_a_data_file_the_name_is_a_key_and_reaches_inside() {
        let json = r#"{"version": "3.1.4", "build": {"number": 12}, "gpu": ["a", "b"]}"#;
        assert_eq!(
            run(&value_of("version"), "package.json", json, Which::First).as_deref(),
            Some("3.1.4")
        );
        assert_eq!(
            run(
                &value_of("build.number"),
                "package.json",
                json,
                Which::First
            )
            .as_deref(),
            Some("12")
        );
        let toml = "[package]\nname = \"foo\"\nversion = \"0.3.0\"\n";
        assert_eq!(
            run(
                &value_of("package.version"),
                "Cargo.toml",
                toml,
                Which::First
            )
            .as_deref(),
            Some("0.3.0")
        );
        let yaml = "app:\n  version: 2.0\n";
        assert_eq!(
            run(&value_of("app.version"), "x.yml", yaml, Which::First).as_deref(),
            Some("2.0")
        );
        let xml = "<project><props><Version>1.2</Version></props></project>";
        assert_eq!(
            run(&value_of("Version"), "app.csproj", xml, Which::First).as_deref(),
            Some("1.2")
        );
        let ini = "[gpu]\nname=RTX\n";
        assert_eq!(
            run(&value_of("gpu.name"), "settings.ini", ini, Which::First).as_deref(),
            Some("RTX")
        );
    }

    #[test]
    fn a_data_file_that_does_not_parse_is_read_as_text() {
        // a broken JSON file still has its "version" line
        let broken = "{\n  \"version\": \"3.1.4\",\n  oops\n";
        assert_eq!(
            run(&value_of("version"), "package.json", broken, Which::First).as_deref(),
            Some("3.1.4")
        );
    }

    #[test]
    fn text_between_two_pieces_or_to_the_end_of_the_line() {
        let log = "[12:00] start build=41 ok\n[12:05] start build=42 ok\nlast line: done";
        let b = Find::Between {
            start: "build=".into(),
            end: Some(" ".into()),
        };
        assert_eq!(run(&b, "app.log", log, Which::First).as_deref(), Some("41"));
        assert_eq!(run(&b, "app.log", log, Which::Last).as_deref(), Some("42"));
        assert_eq!(
            run(&b, "app.log", log, Which::All).as_deref(),
            Some("41, 42")
        );
        let eol = Find::Between {
            start: "last line:".into(),
            end: None,
        };
        assert_eq!(
            run(&eol, "app.log", log, Which::First).as_deref(),
            Some("done")
        );
        // an end that is not on the line: up to its end
        let missing_end = Find::Between {
            start: "last line:".into(),
            end: Some("]".into()),
        };
        assert_eq!(
            run(&missing_end, "app.log", log, Which::First).as_deref(),
            Some("done")
        );
        // two on one line
        let two = Find::Between {
            start: "[".into(),
            end: Some("]".into()),
        };
        assert_eq!(
            run(&two, "a.txt", "[a] and [b]", Which::All).as_deref(),
            Some("a, b")
        );
    }

    #[test]
    fn the_line_with_a_text_comes_whole_and_all_of_them_one_per_line() {
        let log = "INFO ok\nERROR disk full\nINFO ok\nerror: again\n";
        let l = Find::LineWith {
            text: "error".into(),
        };
        assert_eq!(
            run(&l, "a.log", log, Which::Last).as_deref(),
            Some("error: again")
        );
        assert_eq!(
            run(&l, "a.log", log, Which::All).as_deref(),
            Some("ERROR disk full\nerror: again")
        );
    }

    #[test]
    fn the_kind_of_a_file_comes_from_its_name() {
        assert_eq!(kind_of("a/package.json"), Some(Format::Json));
        assert_eq!(kind_of("x.YML"), Some(Format::Yaml));
        assert_eq!(kind_of("app.csproj"), Some(Format::Xml));
        assert_eq!(kind_of("system_info.txt"), None);
        assert_eq!(kind_of("Makefile"), None);
        assert_eq!(kind_name("run.log"), "text");
    }

    #[test]
    fn what_a_person_wrote_is_checked() {
        assert!(check(&value_of(" ")).is_err());
        assert!(check(&Find::Between {
            start: String::new(),
            end: None
        })
        .is_err());
        assert!(check(&Find::LineWith { text: "x".into() }).is_ok());
    }

    const GAME_LOG: &str = "Log file open, 11/16/26 13:58:01\n\
        [2026.11.16-13.58.09:877][  0]LogLoad: LoadMap: /Game/Maps/MainMenu?listen\n\
        [2026.11.16-13.59.12:440][  0]LogLoad: LoadMap: /Game/Maps/Harbor_P?listen\n\
        [2026.11.16-13.59.19:251][  0]LogLoad: Took 6.81 seconds to LoadMap(/Game/Maps/Harbor_P)\n\
        [2026.11.16-13.59.28:300][  0]LogAI: Spawned 4 gulls on Harbor_P\n\
        [2026.11.16-13.59.31:090][  0]LogAI: Spawned 12 crew on Harbor_P\n";

    fn pick_find(lines_with: &str, from_line_start: bool, stop: Option<&str>) -> Find {
        Find::Pick {
            lines_with: lines_with.into(),
            from_line_start,
            stop: stop.map(str::to_string),
            then: vec![],
        }
    }

    // what ffmpeg writes about a file it made (issue #60)
    const FFMPEG: &str = "  Stream #0:0: Video: h264 (avc1 / 0x31637661), 1920x1080\n\
    \x20   Metadata:\n\
    \x20     encoder         : Lavc58.134.100 libopenh264\n\
    \x20 Stream #0:1: Audio: aac (LC) (mp4a / 0x6134706D), 48000 Hz, stereo, fltp (24 bit), 128 kb/s\n\
    \x20   Metadata:\n\
    \x20     encoder         : Lavc58.134.100 aac\n\
    frame=    1 fps=0.0\n";

    fn next(joined_by: &str, lines_with: &str) -> Next {
        Next {
            joined_by: joined_by.into(),
            lines_with: lines_with.into(),
            ..Next::default()
        }
    }

    #[test]
    fn a_value_of_two_parts_takes_the_next_one_found_after_the_first() {
        let audio = Find::Pick {
            lines_with: "Stream #0:1:".into(),
            from_line_start: false,
            stop: None,
            then: vec![next(" - ", "encoder         :")],
        };
        assert_eq!(
            run(&audio, "Choca.mp4.log", FFMPEG, Which::Last).as_deref(),
            Some("Audio: aac (LC) (mp4a / 0x6134706D), 48000 Hz, stereo, fltp (24 bit), 128 kb/s - Lavc58.134.100 aac")
        );
        // the first stream's encoder is the one after it, not the audio's
        let video = Find::Pick {
            lines_with: "Stream #0:0:".into(),
            from_line_start: false,
            stop: Some(",".into()),
            then: vec![next("\n", "encoder         :")],
        };
        assert_eq!(
            run(&video, "a.log", FFMPEG, Which::Last).as_deref(),
            Some("Video: h264 (avc1 / 0x31637661)\nLavc58.134.100 libopenh264")
        );
    }

    #[test]
    fn a_next_part_can_be_later_on_the_same_line_or_missing() {
        // after the first part on its own line: "fps=" after "frame="
        let p = Find::Pick {
            lines_with: "frame=".into(),
            from_line_start: false,
            stop: Some(" ".into()),
            then: vec![next(" at ", "fps="), next(", ", "speed=")],
        };
        // no speed anywhere after it: the parts found
        assert_eq!(
            run(&p, "a.log", FFMPEG, Which::Last).as_deref(),
            Some("1 at 0.0")
        );
        // "the next line": the line below, whole
        let below = Find::Pick {
            lines_with: "Stream #0:1:".into(),
            from_line_start: false,
            stop: Some(":".into()),
            then: vec![Next {
                joined_by: " / ".into(),
                from_line_start: true,
                ..Next::default()
            }],
        };
        assert_eq!(
            run(&below, "a.log", FFMPEG, Which::Last).as_deref(),
            Some("Audio / Metadata:")
        );
        assert!(check(&Find::Pick {
            lines_with: "a".into(),
            from_line_start: false,
            stop: None,
            then: vec![next(", ", "")],
        })
        .is_err());
    }

    #[test]
    fn a_value_picked_in_a_file_is_found_again_the_same_way() {
        // right after a text, up to the next sign; a line without the sign gives nothing
        let map = pick_find("Maps/", false, Some("?"));
        assert_eq!(
            picked(GAME_LOG, "Maps/", false, Some("?"), &[]),
            ["MainMenu", "Harbor_P"]
        );
        assert_eq!(
            run(&map, "Game.log", GAME_LOG, Which::Last).as_deref(),
            Some("Harbor_P")
        );
        assert_eq!(
            run(&map, "Game.log", GAME_LOG, Which::First).as_deref(),
            Some("MainMenu")
        );
        // right after a text, to the end of the line
        let spawned = pick_find("LogAI:", false, None);
        assert_eq!(
            run(&spawned, "Game.log", GAME_LOG, Which::Last).as_deref(),
            Some("Spawned 12 crew on Harbor_P")
        );
        // every line, from its start up to the next "[": the time stamp, its own "[" kept
        let stamp = pick_find("", true, Some("["));
        assert_eq!(
            run(&stamp, "Game.log", GAME_LOG, Which::Last).as_deref(),
            Some("[2026.11.16-13.59.31:090]")
        );
        // the whole line with a text
        assert_eq!(
            run(
                &pick_find("LogAI:", true, None),
                "Game.log",
                GAME_LOG,
                Which::First
            )
            .as_deref(),
            Some("[2026.11.16-13.59.28:300][  0]LogAI: Spawned 4 gulls on Harbor_P")
        );
        // a value with letters of any script is cut between characters
        assert_eq!(
            picked("Carte: Hafen_é?x", "Carte:", false, Some("?"), &[]),
            ["Hafen_é"]
        );
        assert_eq!(
            run(
                &pick_find("Nowhere:", false, None),
                "a.log",
                GAME_LOG,
                Which::Last
            ),
            None
        );
    }

    #[test]
    fn a_pick_needs_where_it_starts() {
        assert!(check(&pick_find("", false, None)).is_err());
        assert!(check(&pick_find("", true, Some("["))).is_ok());
        assert!(check(&pick_find("GPU:", false, None)).is_ok());
    }
}
