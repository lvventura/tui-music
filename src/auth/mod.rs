pub mod cookie;
pub mod firefox;

use std::{path::Path, process::Stdio};

use crate::config::paths;
use crate::errors::{AppError, AppResult};

pub use cookie::CookieHeader;

/// Writes a credential file inside the per-user application directory. On
/// Windows the directory ACL of `%APPDATA%` is the protection; on Unix we also
/// strip group/other access. The contents are never logged or printed.
pub fn write_private(path: &Path, contents: &str) -> AppResult<()> {
    paths::ensure_parent(path)?;
    std::fs::write(path, contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Reads the stored cookie header, if any. Never returns it through logs.
pub fn load_stored_cookie() -> Option<String> {
    std::fs::read_to_string(paths::cookie_file())
        .ok()
        .map(|contents| contents.trim().to_owned())
        .filter(|contents| !contents.is_empty())
}

/// Stores a validated cookie header in the per-user directory.
pub fn store_cookie(header: &CookieHeader) -> AppResult<()> {
    write_private(&paths::cookie_file(), &header.to_header_string())
}

/// Exports the stored cookie as a Netscape file so `yt-dlp --cookies` can use
/// it while resolving audio URLs (age/region-restricted tracks need this).
pub fn export_netscape(header: &CookieHeader) -> AppResult<std::path::PathBuf> {
    let expiry = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| (elapsed.as_secs() + 31_536_000) as i64)
        .unwrap_or(2_147_483_647);
    let path = paths::cookies_export_file();
    write_private(&path, &header.to_netscape(expiry))?;
    Ok(path)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthStatus {
    Unauthenticated,
    Verifying,
    Authenticated,
}

impl AuthStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unauthenticated => "Unauthenticated",
            Self::Verifying => "Verificando…",
            Self::Authenticated => "Authenticated",
        }
    }
}

#[derive(Clone, Debug)]
pub struct AuthManager {
    status: AuthStatus,
    browser: String,
    login_pending: bool,
    cookie: Option<CookieHeader>,
}

impl AuthManager {
    pub fn with_cookie(browser: impl Into<String>, cookie: Option<CookieHeader>) -> Self {
        Self {
            status: AuthStatus::Unauthenticated,
            browser: browser.into(),
            login_pending: false,
            cookie,
        }
    }

    pub fn status(&self) -> AuthStatus {
        self.status
    }

    pub fn cookie_header(&self) -> Option<&CookieHeader> {
        self.cookie.as_ref()
    }

    pub fn set_status(&mut self, status: AuthStatus) {
        self.status = status;
    }

    pub fn login_pending(&self) -> bool {
        self.login_pending
    }

    pub fn begin_login(&mut self) -> AppResult<()> {
        if self.browser.trim().is_empty() {
            return Err(AppError::Provider(
                "configura [auth].browser para usar login por navegador".into(),
            ));
        }
        let url = "https://music.youtube.com";
        #[cfg(windows)]
        let status = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .status()?;
        #[cfg(target_os = "macos")]
        let status = std::process::Command::new("open").arg(url).status()?;
        #[cfg(all(unix, not(target_os = "macos")))]
        let status = std::process::Command::new("xdg-open").arg(url).status()?;
        if !status.success() {
            return Err(AppError::Provider(
                "no se pudo abrir el navegador predeterminado".into(),
            ));
        }
        self.login_pending = true;
        Ok(())
    }

    pub async fn verify(&mut self, yt_dlp: &str) -> AppResult<()> {
        if !self.login_pending {
            return Err(AppError::Provider("primero inicia el login con L".into()));
        }
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(20),
            tokio::process::Command::new(yt_dlp)
                .args([
                    "--cookies-from-browser",
                    self.browser.trim(),
                    "--skip-download",
                    "--dump-single-json",
                    "https://music.youtube.com",
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .output(),
        )
        .await
        .map_err(|_| AppError::Provider("timeout verificando la sesión del navegador".into()))?
        .map_err(|error| AppError::Provider(format!("no se pudo ejecutar yt-dlp: {error}")))?;
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            if error.contains("Could not copy Chrome cookie database") {
                return Err(AppError::Provider(
                    "yt-dlp no puede copiar las cookies de Chrome. Cierra Chrome por completo, verifica que YT-DJ corre con el mismo usuario de Windows y pulsa L de nuevo; también puedes usar Edge/Firefox en [auth].browser".into(),
                ));
            }
            if error.contains("Failed to decrypt with DPAPI") {
                return Err(AppError::Provider(
                    "Windows no permitió descifrar las cookies de Chrome. Ejecuta YT-DJ con el mismo usuario de Windows que Chrome, cierra Chrome por completo o configura otro navegador en [auth].browser".into(),
                ));
            }
            return Err(AppError::Provider(error.trim().to_owned()));
        }
        self.status = AuthStatus::Authenticated;
        self.login_pending = false;
        Ok(())
    }

}

/// Validates a cookie header by asking yt-dlp for the account's Liked Music
/// playlist. The header is written to a private, ephemeral file inside the
/// per-user directory and always removed afterwards.
pub async fn validate_with_ytdlp(yt_dlp: &str, header: &CookieHeader) -> AppResult<()> {
    let expiry = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| (elapsed.as_secs() + 31_536_000) as i64)
        .unwrap_or(2_147_483_647);
    let validation_file = paths::app_dir().join(".validate-cookies.txt");
    write_private(&validation_file, &header.to_netscape(expiry))?;
    let outcome = run_validation(yt_dlp, &validation_file).await;
    let _ = std::fs::remove_file(&validation_file);
    outcome
}

async fn run_validation(yt_dlp: &str, cookies_file: &Path) -> AppResult<()> {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(25),
        tokio::process::Command::new(yt_dlp)
            .arg("--cookies")
            .arg(cookies_file)
            .args([
                "--flat-playlist",
                "--dump-single-json",
                "--skip-download",
                "--playlist-end",
                "1",
                "https://music.youtube.com/playlist?list=LM",
            ])
            .stdin(Stdio::null())
            .output(),
    )
    .await
    .map_err(|_| AppError::Provider("timeout validando la cookie".into()))?
    .map_err(|error| AppError::Provider(format!("no se pudo ejecutar yt-dlp: {error}")))?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Provider(format!(
            "la cookie no permitió acceder a la biblioteca: {}",
            error.trim()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::write_private;

    #[test]
    fn private_write_round_trips() {
        let path = std::env::temp_dir().join(format!(
            "yt-dj-auth-test-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0)
        ));
        write_private(&path, "SAPISID=secret-that-is-never-logged;").unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "SAPISID=secret-that-is-never-logged;"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let _ = std::fs::remove_file(&path);
    }
}
