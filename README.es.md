# ArqStudioCAD

[English](README.md) | **Español**

CAD 2D/3D de código abierto para arquitectura e ingeniería, con DWG como
formato nativo y DXF como alternativa abierta.

> **Estado:** pre-alfa, fase 0 (fundación). Todavía no es utilizable.

| Capa | Tecnología | Responsable de |
|---|---|---|
| Núcleo CAD | Rust | Geometría, documento, entidades, transacciones, deshacer/rehacer, consultas espaciales |
| Aplicación | Python | Comandos, casos de uso, plugins, scripting, eventos, traducciones |
| Interfaz | PySide6 (Qt 6) | Ventanas, vista gráfica, paneles, línea de comandos, diálogos |

Python se comunica con Rust mediante [PyO3](https://pyo3.rs) y
[maturin](https://www.maturin.rs). Las reglas que debe respetar todo cambio
están en [docs/architecture/technical-contract.md](docs/architecture/technical-contract.md),
y las decisiones que las justifican, en [docs/architecture/adr/](docs/architecture/adr/)
(documentación técnica en inglés).

## Plataformas objetivo

Windows (x86-64, ARM64) · macOS (universal2: Apple Silicon e Intel) ·
Linux (`.deb`, `.rpm`, Arch/AUR, Flatpak, AppImage).

## Idiomas

La interfaz se escribe en inglés y se traduce mediante paquetes de idioma; el
español es la primera traducción mantenida. Los comandos que se escriben en la
línea de comandos se aceptan siempre en inglés (`LINE`, `L`, `_LINE`), y cada
idioma puede añadir sus propios nombres (`LINEA`). Ver
[ADR 0006](docs/architecture/adr/0006-i18n.md).

## Compilar desde el código fuente

Requisitos:

- Rust (la versión exacta está fijada en `rust-toolchain.toml`; `rustup` la instala automáticamente)
- [uv](https://docs.astral.sh/uv/)
- Windows: Visual Studio Build Tools con la carga de trabajo de C++

```sh
uv sync            # crea .venv, compila la extensión Rust e instala las herramientas
uv run pytest      # pruebas de Python
cargo test         # pruebas del núcleo Rust (no necesita Python)
```

Tras modificar código Rust, `uv sync` (o `uv run`) vuelve a compilar la extensión.

Quien contribuya instala además, una sola vez, las comprobaciones locales:

```sh
cargo install cargo-deny --locked
uv run pre-commit install
```

## Licencia

[GPL-3.0-or-later](LICENSE). El código y las ideas de terceros se reconocen en
[THIRD_PARTY.md](THIRD_PARTY.md).
