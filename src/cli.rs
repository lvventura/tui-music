#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthMode {
    Cookie,
    Clipboard,
    File(String),
    Browser,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Run,
    Auth(AuthMode),
    Diagnose,
    Help,
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    let mut command = Command::Run;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--help" | "-h" => return Ok(Command::Help),
            "--diagnose" => command = Command::Diagnose,
            "--auth" => {
                let mode = iter.next().ok_or("--auth requiere browser|cookie")?;
                let mode = match mode.as_str() {
                    "cookie" => AuthMode::Cookie,
                    "clipboard" | "cookie-clipboard" => AuthMode::Clipboard,
                    "browser" => AuthMode::Browser,
                    "cookie-file" => AuthMode::File(
                        iter.next()
                            .ok_or("--auth cookie-file requiere una ruta")?
                            .to_owned(),
                    ),
                    other => return Err(format!("modo de auth desconocido: {other}")),
                };
                command = Command::Auth(mode);
            }
            other => return Err(format!("argumento desconocido: {other}")),
        }
    }
    Ok(command)
}

pub fn usage() -> &'static str {
    "yt-dj [--auth cookie|clipboard|cookie-file <ruta>|browser] [--diagnose] [--help]"
}

#[cfg(test)]
mod tests {
    use super::{AuthMode, Command, parse};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn no_arguments_runs_the_tui() {
        assert_eq!(parse(&[]).unwrap(), Command::Run);
    }

    #[test]
    fn auth_modes_are_parsed() {
        assert_eq!(
            parse(&args(&["--auth", "cookie"])).unwrap(),
            Command::Auth(AuthMode::Cookie)
        );
        assert_eq!(
            parse(&args(&["--auth", "browser"])).unwrap(),
            Command::Auth(AuthMode::Browser)
        );
        assert_eq!(
            parse(&args(&["--auth", "clipboard"])).unwrap(),
            Command::Auth(AuthMode::Clipboard)
        );
        assert_eq!(
            parse(&args(&["--auth", "cookie-file", "C:\\tmp\\cookies.txt"])).unwrap(),
            Command::Auth(AuthMode::File("C:\\tmp\\cookies.txt".to_owned()))
        );
    }

    #[test]
    fn diagnose_and_help_are_recognized() {
        assert_eq!(parse(&args(&["--diagnose"])).unwrap(), Command::Diagnose);
        assert_eq!(parse(&args(&["--help"])).unwrap(), Command::Help);
    }

    #[test]
    fn unknown_arguments_are_rejected() {
        assert!(parse(&args(&["--nope"])).is_err());
        assert!(parse(&args(&["--auth"])).is_err());
        assert!(parse(&args(&["--auth", "oauth"])).is_err());
    }
}
