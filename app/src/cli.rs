//! Command-line switches. CargoFrame has only two, both internal.

/// What this launch was asked to do.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Cli {
    /// Started by the operating system at sign-in: stay in the tray, do not open Settings.
    pub autostart: bool,
    /// Ask the running copy to quit (used by the installer before it upgrades files).
    pub quit: bool,
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
        for arg in args.into_iter().skip(1) {
            match arg.as_ref() {
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
    } else if cli.autostart && tray_available {
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
    fn the_program_name_is_never_a_switch() {
        assert_eq!(Cli::parse(["--autostart"]), Cli::default());
    }

    #[test]
    fn running_the_app_opens_settings() {
        assert_eq!(startup(&Cli::default(), true), Startup::OpenSettings);
    }

    #[test]
    fn sign_in_start_stays_in_the_tray() {
        let cli = Cli { autostart: true, quit: false };
        assert_eq!(startup(&cli, true), Startup::TrayOnly);
    }

    #[test]
    fn sign_in_start_without_a_tray_opens_settings() {
        let cli = Cli { autostart: true, quit: false };
        assert_eq!(startup(&cli, false), Startup::OpenSettings);
    }

    #[test]
    fn quit_wins() {
        let cli = Cli { autostart: true, quit: true };
        assert_eq!(startup(&cli, true), Startup::Exit);
    }
}
