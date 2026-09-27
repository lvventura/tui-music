use std::process::Stdio;

use crate::errors::{AppError, AppResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthStatus {
    Unauthenticated,
    Authenticated,
}

#[derive(Clone, Debug)]
pub struct AuthManager {
    status: AuthStatus,
    browser: String,
    login_pending: bool,
}

impl AuthManager {
    pub fn new(browser: impl Into<String>) -> Self {
        Self {
            status: AuthStatus::Unauthenticated,
            browser: browser.into(),
            login_pending: false,
        }
    }

    pub fn status(&self) -> AuthStatus {
        self.status
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
