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

## Controles

- `/`: activar búsqueda
- `Enter`: buscar o seleccionar el resultado
- `Up` / `Down`: navegar resultados
- `Esc`: cancelar búsqueda
- `Space`: reproducir/pausar
- `N` / `P`: siguiente/anterior
- `+` / `-`: volumen
- `1` / `2` / `3` / `4` / `5`: Search / History / Favorites / Playlists / Queue
- `F`: agregar o quitar favorito
- `D`: eliminar el elemento seleccionado
- `C`: crear una playlist local
- `A`: agregar la canción actual a la playlist activa
- `Q`: salir

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
