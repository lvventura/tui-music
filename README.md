# yt-dj

Cliente TUI de YouTube Music escrito en Rust para Windows y Linux.

## Requisitos

- Rust y Cargo.
- `yt-dlp` en `PATH` (o configurar `yt_dlp_path` en `config/default.toml`).
- Un dispositivo de salida de audio compatible con Rodio.

`yt-dlp` se usa como adaptador externo porque YouTube Music no ofrece una API
pública oficial para búsqueda y URLs de reproducción. Su salida JSON queda
aislada detrás de `MusicProvider`, por lo que el proveedor puede cambiarse.

## Instalación y ejecución

```text
cargo build --release
cargo run
```

## Autenticación (opcional)

Búsqueda, radio y reproducción funcionan sin credenciales. La biblioteca
personal (Liked Music) requiere una sesión:

```text
yt-dj --auth cookie            # pega la cabecera Cookie de DevTools y la valida
yt-dj --auth clipboard         # lee esa cabecera del portapapeles (Windows)
yt-dj --auth cookie-file FILE  # valida un cookies.txt (Netscape) exportado
yt-dj --auth browser           # lee cookies de Firefox/Waterfox/LibreWolf/Floorp
yt-dj --diagnose               # entorno, credenciales y biblioteca
```

La cookie se valida con `yt-dlp` antes de guardarse en
`%APPDATA%\yt-dj\cookie.txt` (nunca en `%TEMP%`, nunca en los logs). Chrome y
Edge no se leen directamente (cifran las cookies con *app-bound encryption* +
DPAPI, ver yt-dlp issue 10927), así que en esas máquinas usa `--auth clipboard`
(copia el valor de la cabecera `cookie:` en DevTools > Network) o exporta un
`cookies.txt` con una extensión y pásalo con `--auth cookie-file`. Si no hay
cookie guardada, la app delega en yt-dlp con `--cookies-from-browser` usando
`[auth].browser`. Un `429` o "Sign in to confirm you're not a bot" significa
que falta esa sesión.

La tecla `L` en la TUI valida la cookie guardada o inicia el login por
navegador como respaldo.

## Controles

- `1` / `2` / `3` / `4`: vistas Queue / Library / Track / Search
- `/`: activar búsqueda; `Esc` cancelarla; `Enter` buscarla
- `Up` / `Down`: navegar; `Left` / `Right`: cambiar de columna (Library) o avanzar/retroceder
- `Space`: reproducir/pausar; `N` / `P`: siguiente/anterior
- `+` / `-`: volumen; `M`: silencio
- `S`: aleatorio; `R`: repetir
- `F`: favorito; `D`: borrar elemento; `C`: crear playlist; `A`: añadir a playlist
- `L`: validar sesión / iniciar login por navegador
- `Esc` en la vista normal: abre el menú flotante (`←`/`→` cambian el valor)
- `Q`: salir

## Aspecto y temas

Cada tema es una paleta de tres colores (oscuro, medio, brillante) con un
mínimo de contraste. Hay temas integrados (`gruvbox`, `nord`, `dracula`,
`catppuccin`, `tokyonight`, `everforest`, `rosepine`, `solarized`, `matrix`,
`mono`) y `cover` (por defecto), que deriva la paleta de la carátula.

Los temas propios van en `%APPDATA%\yt-dj\themes\*.toml`. El tema y el mapa de
teclas (`default` o `vim`) se eligen en la sección `[ui]` de
`config/default.toml` o desde el menú (`Esc`).

Las carátulas se dibujan como bloques de media altura en cualquier terminal
truecolor y con **Sixel** o **kitty graphics** en terminales compatibles
(WezTerm, foot, mintty, kitty, Ghostty…). La detección es automática;
`YT_DJ_IMAGE_PROTOCOL=kitty|sixel|halfblock|none` la fuerza.

## Arquitectura

- `ui`: representación Ratatui, sin acceso a infraestructura.
- `app`: estado, eventos y acciones.
- `youtube`: modelos y `MusicProvider`; `YtDlpProvider` es la implementación actual.
- `player`: contrato `AudioPlayer` y backend Rodio.
- `database`: SQLite y repositories.

La base de datos usa SQLite y se guarda por defecto en `%APPDATA%/yt-dj/yt-dj.db`
en Windows o `$XDG_DATA_HOME/yt-dj/yt-dj.db` (con fallback a
`~/.local/share/yt-dj/yt-dj.db`) en Linux. `database_path` permite sobrescribir
la ubicación.

La integración externa puede romperse si YouTube cambia sus respuestas o si
`yt-dlp` deja de funcionar; se muestra el error en la TUI en lugar de cerrar
la aplicación.
