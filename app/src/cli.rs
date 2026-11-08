//! Command-line switches: two internal ones, and `open`, which other programs use to fill a
//! layout:
//!
//! ```text
//! cargoframe open --trigger bug --set summary="It crashed" --set build=1.4.2
//! ```
//!
//! runs the trigger `bug` in the copy that is already running (the single-instance channel
//! carries it there: same user, no network port) and puts the values in the blocks with those
//! ids. Only a layout with *Other programs can fill this* accepts them, and nothing is sent
//! until the person presses a button.

/// Most values one `open` can carry, and the longest one.
pub const MAX_VALUES: usize = 50;
pub const MAX_VALUE_CHARS: usize = 20_000;

/// What this launch was asked to do.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Cli {
    /// Started by the operating system at sign-in: stay in the tray, do not open Settings.
    pub autostart: bool,
    /// Ask the running copy to quit (used by the installer before it upgrades files).
    pub quit: bool,
    /// Another program asks for a capture with values filled in.
    pub open: Option<Result<OpenRequest, String>>,
}

/// `open`: which trigger to run, and the value for each block.
#[derive(Clone, PartialEq, Eq)]
pub struct OpenRequest {
    pub trigger: String,
    pub values: Vec<(String, String)>,
}

// The values come from another program and may be anything: the log says how many, never what.
impl std::fmt::Debug for OpenRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenRequest")
            .field("trigger", &self.trigger)
            .field("values", &self.values.len())
            .finish()
    }
}

/// A block id or trigger id: letters, digits, `-` and `_`.
fn is_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The arguments after `open`.
pub fn parse_open<S: AsRef<str>>(args: &[S]) -> Result<OpenRequest, String> {
    let mut trigger = None;
    let mut values: Vec<(String, String)> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_ref();
        let (name, inline) = match arg.split_once('=') {
            Some((n, v)) if n.starts_with("--") => (n, Some(v.to_string())),
            _ => (arg, None),
        };
        let mut next = || -> Result<String, String> {
            if let Some(v) = inline.clone() {
                return Ok(v);
            }
            i += 1;
            args.get(i)
                .map(|a| a.as_ref().to_string())
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match name {
            "--trigger" => trigger = Some(next()?),
            "--set" => {
                let pair = next()?;
                let (block, value) = pair
                    .split_once('=')
                    .ok_or_else(|| format!("--set {pair}: write it as block=value"))?;
                if !is_id(block) {
                    return Err(format!("\"{block}\" is not a block id"));
                }
                if value.chars().count() > MAX_VALUE_CHARS {
                    return Err(format!(
                        "The value for {block} is longer than {MAX_VALUE_CHARS} characters"
                    ));
                }
                values.retain(|(b, _)| b != block);
                values.push((block.to_string(), value.to_string()));
                if values.len() > MAX_VALUES {
                    return Err(format!("At most {MAX_VALUES} values can be sent at once"));
                }
            }
            other => return Err(format!("Unknown option for open: {other}")),
        }
        i += 1;
    }
    let trigger = trigger.ok_or("open needs --trigger and the id of a trigger")?;
    if !is_id(&trigger) {
        return Err(format!("\"{trigger}\" is not a trigger id"));
    }
    Ok(OpenRequest { trigger, values })
}

impl Cli {
    /// Reads switches from an argument list that includes the program name first.
    /// Anything unknown is ignored.
    pub fn parse<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut cli = Cli::default();
        let args: Vec<String> = args
            .into_iter()
            .skip(1)
            .map(|a| a.as_ref().to_string())
            .collect();
        if args.first().map(String::as_str) == Some("open") {
            cli.open = Some(parse_open(&args[1..]));
            return cli;
        }
        for arg in &args {
            match arg.as_str() {
                "--autostart" => cli.autostart = true,
                "--quit" => cli.quit = true,
                _ => {}
            }
        }
        cli
    }
}

/// What the first instance does once it is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Startup {
    /// Exit at once (asked to quit while nothing was running).
    Exit,
    /// Tray and notification only.
    TrayOnly,
    /// Tray, notification, and open Settings.
    OpenSettings,
}

/// Running the app always opens Settings; a sign-in start does not, unless there is no tray
/// to click (then Settings is the only way in).
pub fn startup(cli: &Cli, tray_available: bool) -> Startup {
    if cli.quit {
        Startup::Exit
    } else if (cli.autostart || cli.open.is_some()) && tray_available {
        Startup::TrayOnly
    } else {
        Startup::OpenSettings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_switches_and_ignores_the_rest() {
        assert_eq!(Cli::parse(["app"]), Cli::default());
        assert!(Cli::parse(["app", "--autostart"]).autostart);
        assert!(Cli::parse(["app", "--quit"]).quit);
        let c = Cli::parse(["app", "--whatever", "--autostart", "x"]);
        assert!(c.autostart && !c.quit);
    }

    #[test]
    fn open_reads_the_trigger_and_the_values() {
        let cli = Cli::parse([
            "app",
            "open",
            "--trigger",
            "bug",
            "--set",
            "summary=It crashed: a=b",
            "--set=build=1.4",
            "--set",
            "summary=Again",
        ]);
        let open = cli.open.unwrap().unwrap();
        assert_eq!(open.trigger, "bug");
        // a block named twice keeps its last value
        assert_eq!(
            open.values,
            vec![
                ("build".to_string(), "1.4".to_string()),
                ("summary".to_string(), "Again".to_string())
            ]
        );
        // the log never shows what was sent
        assert!(!format!("{open:?}").contains("Again"));
    }

    #[test]
    fn open_refuses_what_it_cannot_use_and_says_why() {
        let err = |args: &[&str]| parse_open(args).unwrap_err();
        assert!(err(&["--set", "a=b"]).contains("--trigger"));
        assert!(err(&["--trigger", "../x"]).contains("not a trigger id"));
        assert!(err(&["--trigger", "t", "--set", "novalue"]).contains("block=value"));
        assert!(err(&["--trigger", "t", "--set", "bad id=1"]).contains("not a block id"));
        assert!(err(&["--trigger", "t", "--whatever"]).contains("Unknown option"));
        assert!(err(&["--trigger"]).contains("needs a value"));
        let long = format!("x={}", "y".repeat(MAX_VALUE_CHARS + 1));
        assert!(err(&["--trigger", "t", "--set", &long]).contains("longer than"));
    }

    #[test]
    fn an_open_from_a_first_start_stays_in_the_tray() {
        let cli = Cli::parse(["app", "open", "--trigger", "bug"]);
        assert_eq!(startup(&cli, true), Startup::TrayOnly);
    }

    #[test]
    fn the_program_name_is_never_a_switch() {
        assert_eq!(Cli::parse(["--autostart"]), Cli::default());
    }

    #[test]
    fn running_the_app_opens_settings() {
        assert_eq!(startup(&Cli::default(), true), Startup::OpenSettings);
    }

    #[test]
    fn sign_in_start_stays_in_the_tray() {
        let cli = Cli {
            autostart: true,
            quit: false,
            open: None,
        };
        assert_eq!(startup(&cli, true), Startup::TrayOnly);
    }

    #[test]
    fn sign_in_start_without_a_tray_opens_settings() {
        let cli = Cli {
            autostart: true,
            quit: false,
            open: None,
        };
        assert_eq!(startup(&cli, false), Startup::OpenSettings);
    }

    #[test]
    fn quit_wins() {
        let cli = Cli {
            autostart: true,
            quit: true,
            open: None,
        };
        assert_eq!(startup(&cli, true), Startup::Exit);
    }
}
