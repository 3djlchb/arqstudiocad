# ArqStudioCAD

[English](README.md) | **Español**

CAD 2D/3D de código abierto para arquitectura e ingeniería, con DWG como
formato nativo y DXF como alternativa abierta.

> **Estado:** pre-alfa. La aplicación arranca, cambia entre inglés y español y
> se empaqueta para todas las plataformas objetivo (fase 1). El núcleo
> geométrico está terminado —líneas, círculos, arcos, transformaciones,
> intersecciones y puntos de referencia (fase 2)— y también el núcleo del
> documento: abre y guarda DWG y DXF, edita líneas, círculos, arcos y capas
> con deshacer y rehacer, y conserva intacto todo lo demás del dibujo
> (fase 3). La interfaz todavía no permite dibujar.

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
Linux (`.deb`, `.rpm`, AppImage).

## Idiomas

La interfaz se escribe en inglés y se traduce mediante paquetes de idioma; el
español es la primera traducción mantenida. Los comandos que se escriben en la
línea de comandos se aceptan siempre en inglés (`LINE`, `L`, `_LINE`), y cada
idioma puede añadir sus propios nombres (`LINEA`). Ver
[ADR 0006](docs/architecture/adr/0006-i18n.md).

## Compilar desde el código fuente

En todas las plataformas se necesitan cuatro cosas: **Git**, un **enlazador
C/C++**, **rustup** (instala Rust) y **[uv](https://docs.astral.sh/uv/)**
(instala Python y las herramientas de Python). **No** hace falta instalar
Python ni una versión concreta de Rust por tu cuenta: uv descarga Python 3.12+
si no lo encuentra, y rustup instala la versión exacta de Rust fijada en
`rust-toolchain.toml` la primera vez que compilas.

### Windows 10/11 (x86-64 o ARM64)

1. Instala las herramientas desde PowerShell:

   ```powershell
   winget install --source winget --id Git.Git
   winget install --source winget --id Rustlang.Rustup
   winget install --source winget --id astral-sh.uv
   winget install --source winget --id Microsoft.VisualStudio.BuildTools `
     --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
   ```

   El último comando instala Visual Studio Build Tools con la carga de trabajo
   de C++ (enlazador MSVC y SDK de Windows). Si ya tienes Visual Studio con
   "Desarrollo para el escritorio con C++", puedes omitirlo.

2. **Cierra y vuelve a abrir la terminal** para que cargue el nuevo `PATH`.

3. Descarga el código y compila:

   ```powershell
   git clone https://github.com/3djlchb/arqstudiocad.git
   cd arqstudiocad
   uv sync
   ```

### Linux

1. Instala un compilador, Git y curl con el gestor de paquetes de tu distribución:

   | Distribución | Comando |
   |---|---|
   | Debian, Ubuntu, Linux Mint, LMDE | `sudo apt install build-essential git curl` |
   | Fedora | `sudo dnf install gcc gcc-c++ make git curl` |
   | openSUSE | `sudo zypper install -t pattern devel_basis && sudo zypper install git curl` |
   | Arch, Manjaro, EndeavourOS | `sudo pacman -S --needed base-devel git curl` |

   Las distribuciones de escritorio ya traen las librerías que Qt necesita para
   abrir ventanas, salvo a veces la del cursor de X11: `libxcb-cursor0` (Debian,
   Ubuntu), `xcb-util-cursor` (Fedora, openSUSE, Arch).

2. Instala rustup y uv (ambos se instalan en tu carpeta personal, sin `sudo`):

   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
   curl -LsSf https://astral.sh/uv/install.sh | sh
   ```

3. **Abre una terminal nueva** (o ejecuta `source ~/.cargo/env`) para que
   `cargo` y `uv` estén en el `PATH`.

4. Descarga el código y compila:

   ```sh
   git clone https://github.com/3djlchb/arqstudiocad.git
   cd arqstudiocad
   uv sync
   ```

### macOS (Apple Silicon o Intel)

Ejecuta `xcode-select --install` para el compilador y sigue los pasos 2–4 de Linux.

### Ejecutarla (todas las plataformas)

```sh
uv run arqstudiocad              # abre la aplicación
uv run arqstudiocad --lang es    # en español (también: Herramientas > Idioma)
uv run arqstudiocad --check      # verifica la instalación y muestra un informe
```

Y las pruebas:

```sh
cargo test         # pruebas del núcleo Rust (sin Python)
uv run pytest      # pruebas de Python
```

`uv sync` crea la carpeta `.venv`, compila la extensión Rust en modo depuración
e instala las herramientas de desarrollo. Tras modificar código Rust, `uv sync`
(o cualquier `uv run ...`) vuelve a compilar la extensión automáticamente.

Para generar un paquete optimizado e instalable (un *wheel*) en `target/wheels/`:

```sh
uv run maturin build --release
```

Los instaladores y paquetes portables (instalador y ZIP portable de Windows,
`.dmg` de macOS, AppImage/`.deb`/`.rpm` de Linux) se generan con los scripts
descritos en [packaging/README.md](packaging/README.md) (en inglés).

### Para quien contribuya

Instala una vez las comprobaciones locales; después se ejecutan en cada commit:

```sh
cargo install cargo-deny --locked
uv run pre-commit install
```

La lista completa de comprobaciones está en [AGENTS.md](AGENTS.md).

### Solución de problemas

| Síntoma | Solución |
|---|---|
| No se encuentra `cargo`, `rustup` o `uv` | Abre una terminal nueva (Windows: cierra todas las terminales o reinicia VS Code) |
| `link.exe not found` (Windows) | Instala la carga de trabajo de C++ de Build Tools (paso 1) |
| `linker 'cc' not found` (Linux) | Instala el paquete del compilador del paso 1 |
| `Could not load the Qt platform plugin "xcb"` (Linux) | Instala la librería del cursor de X11 indicada en el paso 1 |
| `invalid peer certificate: UnknownIssuer` en uv | Un proxy o antivirus intercepta HTTPS. Define `UV_SYSTEM_CERTS=1` para que uv use los certificados del sistema |

## Contribuir

Las contribuciones son bienvenidas, en español o en inglés. Lee
[CONTRIBUTING.es.md](CONTRIBUTING.es.md) para conocer el flujo de trabajo y las
reglas, y el [Código de Conducta](CODE_OF_CONDUCT.md).

## Licencia

[GPL-3.0-or-later](LICENSE). El código y las ideas de terceros se reconocen en
[THIRD_PARTY.md](THIRD_PARTY.md).
