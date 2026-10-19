use std::io::Write;
use std::process::Stdio;

use crate::auth;
use crate::cli::AuthMode;
use crate::config::{Settings, paths};
use crate::errors::{AppError, AppResult};
use crate::i18n::{self, Language, Text};

pub async fn run_auth(mode: AuthMode, settings: &Settings) -> AppResult<()> {
    let text = Text::new(Language::parse(&settings.ui.language));
    let header = match mode {
        AuthMode::Cookie => prompt_cookie(&text)?,
        AuthMode::Clipboard => clipboard_cookie()?,
        AuthMode::File(path) => file_cookie(&path)?,
        AuthMode::Browser => browser_cookie()?,
    };
    auth::validate_with_ytdlp(&settings.yt_dlp_path, &header).await?;
    auth::store_cookie(&header)?;
    println!(
        "{}",
        i18n::fill(
            text.cli_store_ok_fmt,
            &[paths::cookie_file().display().to_string()]
        )
    );
    Ok(())
}

fn prompt_cookie(text: &Text) -> AppResult<auth::CookieHeader> {
    println!("{}", text.cli_paste_prompt);
    print!("> ");
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    auth::CookieHeader::parse(&input)
}

fn clipboard_cookie() -> AppResult<auth::CookieHeader> {
    #[cfg(windows)]
    {
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "Get-Clipboard -Raw"])
            .stdin(Stdio::null())
            .output()
            .map_err(|error| {
                AppError::Provider(format!("no se pudo leer el portapapeles: {error}"))
            })?;
        let text = String::from_utf8_lossy(&output.stdout);
        auth::CookieHeader::parse(&text)
    }
    #[cfg(not(windows))]
    {
        Err(AppError::Provider(
            "clipboard solo está disponible en Windows; usa --auth cookie".into(),
        ))
    }
}

fn file_cookie(path: &str) -> AppResult<auth::CookieHeader> {
    let content = std::fs::read_to_string(path)
        .map_err(|error| AppError::Provider(format!("no se pudo leer {path}: {error}")))?;
    let looks_netscape = content.contains("Netscape")
        || content
            .lines()
            .any(|line| line.matches('\t').count() >= 5);
    if looks_netscape {
        auth::CookieHeader::parse_netscape(&content)
    } else {
        auth::CookieHeader::parse(&content)
    }
}

fn browser_cookie() -> AppResult<auth::CookieHeader> {
    let database = auth::firefox::most_recent_cookie_db().ok_or_else(|| {
        AppError::Provider(
            "no se encontró un perfil de Firefox/Waterfox/LibreWolf/Floorp; usa --auth cookie".into(),
        )
    })?;
    eprintln!("Leyendo cookies de {}", database.display());
    auth::firefox::read_cookies(&database)
}

pub async fn run_diagnose(settings: &Settings) -> AppResult<()> {
    let text = Text::new(Language::parse(&settings.ui.language));
    println!("{} {}", text.app_name, env!("CARGO_PKG_VERSION"));
    println!(
        "yt-dlp: {}",
        describe_yt_dlp(&settings.yt_dlp_path, &text).await
    );
    let caps = crate::terminal::TerminalCaps::detect();
    println!(
        "terminal: truecolor={} windows_terminal={}",
        caps.truecolor, caps.windows_terminal
    );
    println!("{}", text.cli_creds_heading);
    for (label, path) in [
        ("config", paths::config_file()),
        ("state", paths::state_file()),
        ("cookie", paths::cookie_file()),
        ("database", settings.database_path()),
        ("log", paths::log_file()),
    ] {
        let state = if path.exists() {
            text.cli_present
        } else {
            text.cli_absent
        };
        println!("  {label:9} {state} {}", path.display());
    }
    match auth::load_stored_cookie().and_then(|raw| auth::CookieHeader::parse(&raw).ok()) {
        Some(header) => match auth::validate_with_ytdlp(&settings.yt_dlp_path, &header).await {
            Ok(()) => println!("{}: {}", text.cli_library_label, text.cli_library_ok),
            Err(error) => println!("{}: {error}", text.cli_library_label),
        },
        None => println!("{}: {}", text.cli_library_label, text.cli_library_empty),
    }
    match count_search(&settings.yt_dlp_path).await {
        Ok(count) => println!(
            "{}: {}",
            text.cli_search_label,
            i18n::fill(text.cli_search_ok_fmt, &[count.to_string()])
        ),
        Err(error) => println!("{}: {error}", text.cli_search_label),
    }
    Ok(())
}

pub fn print_help(language: Language) {
    println!("{}", crate::cli::usage());
    println!();
    match language {
        Language::Es => {
            println!("  (sin argumentos)   abre la TUI");
            println!("  --auth cookie      valida y guarda una cabecera Cookie pegada");
            println!("  --auth clipboard   lee la cabecera Cookie del portapapeles (Windows)");
            println!("  --auth cookie-file valida y guarda un cookies.txt (Netscape) exportado");
            println!("  --auth browser     lee cookies de Firefox/Waterfox/LibreWolf/Floorp");
            println!("  --diagnose         informa entorno, credenciales y biblioteca");
            println!("  --help             muestra esta ayuda");
        }
        Language::En => {
            println!("  (no arguments)     opens the TUI");
            println!("  --auth cookie      validates and stores a pasted Cookie header");
            println!("  --auth clipboard   reads the Cookie header from the clipboard (Windows)");
            println!("  --auth cookie-file validates and stores an exported cookies.txt (Netscape)");
            println!("  --auth browser     reads cookies from Firefox/Waterfox/LibreWolf/Floorp");
            println!("  --diagnose         reports environment, credentials and library");
            println!("  --help             shows this help");
        }
    }
}

async fn describe_yt_dlp(executable: &str, text: &Text) -> String {
    match tokio::process::Command::new(executable)
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            format!(
                "{} ({executable})",
                String::from_utf8_lossy(&output.stdout).trim()
            )
        }
        _ => format!("{} ({executable})", text.cli_ytdlp_unavailable),
    }
}

async fn count_search(executable: &str) -> Result<usize, String> {
    let output = tokio::process::Command::new(executable)
        .args([
            "--flat-playlist",
            "--dump-json",
            "--skip-download",
            "ytsearch3:lofi",
        ])
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count())
}
