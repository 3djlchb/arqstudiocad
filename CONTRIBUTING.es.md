# Contribuir a ArqStudioCAD

[English](CONTRIBUTING.md) | **Español**

Son bienvenidos los parches, los informes de errores, los dibujos que se
comportan mal y las traducciones. ArqStudioCAD es GPL-3.0-or-later; al
contribuir aceptas que tu trabajo se distribuya bajo esa licencia. Quienes
contribuyen figuran en [AUTHORS](AUTHORS), y el proyecto sigue el
[Contributor Covenant](CODE_OF_CONDUCT.md).

**Escribe los issues y pull requests en español o en inglés, como prefieras**:
se leen los dos. Solo el código, sus comentarios y los mensajes de commit
tienen que ir en inglés, para que cualquier persona pueda leerlos.

No necesitas ningún permiso en este repositorio para contribuir: haz un fork,
sube tu cambio a una rama de tu fork y abre un pull request contra `main`.

> El proyecto está en pre-alfa. Antes de un cambio grande, abre primero un
> issue para acordar el enfoque: te ahorrará trabajo.

## Ponerlo en marcha

Sigue [Compilar desde el código fuente](README.es.md#compilar-desde-el-código-fuente)
e instala una vez las comprobaciones locales:

```sh
cargo install cargo-deny --locked
uv run pre-commit install
```

A partir de ahí, cada commit ejecuta el formato, los linters, la comprobación
de tipos y la revisión de licencias de las dependencias. La lista completa de
comandos está en [AGENTS.md](AGENTS.md). **Las pruebas empiezan en local**: se
espera que un pull request llegue con las comprobaciones ya en verde en tu
máquina. Después, la CI de GitHub las ejecuta en cada pull request a `main`, en
Windows, Linux y macOS.

## Reglas de la casa

Las reglas vinculantes están en el [contrato técnico](docs/architecture/technical-contract.md)
(en inglés); estas son las que suele encontrarse una primera contribución:

- **Cada capa es dueña de lo suyo.** Rust es dueño del documento y de toda la
  geometría; Python coordina los comandos; PySide6 solo presenta. Un comando
  nunca edita directamente el interior del documento ni los widgets, y ningún
  algoritmo se duplica entre Rust y Python.
- **Todo cambio en el dibujo pasa por la API del núcleo**, dentro de una
  transacción, para que deshacer y rehacer sean exactos y una operación fallida
  deje el dibujo intacto.
- **Los comandos son acciones sin interfaz**, nunca lógica pegada a una tecla o
  a un evento del ratón. Así se pueden probar sin GUI y automatizar.
- **El documento DWG/DXF es el modelo.** Lo que ArqStudioCAD no entiende de un
  dibujo (proxies, XDATA, diccionarios, sólidos 3D) debe sobrevivir intacto a
  abrir y guardar. Los tipos del motor (AcadRust) nunca salen de `arq-document`.
- **Solo `crates/arq-python` puede depender de Python**, y ningún crate depende de Qt.
- **Las coordenadas son `f64` en unidades del modelo y los ángulos, radianes**;
  se rechazan NaN e infinito.
- **Todo comportamiento nuevo viene con una prueba, en la capa adecuada.** Una
  prueba que pasa con el error todavía presente no demuestra nada: comprueba
  que falla al revertir tu corrección. Una comprobación que no inspecciona nada
  es un fallo, no un acierto.
- **El código, los comentarios, los docstrings y los mensajes de commit van en
  inglés.** La interfaz se traduce (ver abajo). `README.md` y `README.es.md`
  cambian juntos, igual que esta guía y su versión en inglés.
- **El código copiado o adaptado** debe tener una licencia compatible con
  GPL-3.0-or-later y se registra en [THIRD_PARTY.md](THIRD_PARTY.md).
- **Las decisiones de arquitectura** (una dependencia nueva, un cambio de
  enfoque) vienen con un ADR en [docs/architecture/adr/](docs/architecture/adr/).
- **Se aceptan contribuciones hechas con ayuda de IA.** Eres responsable de
  cada línea: léela, ejecútala y pruébala. [AGENTS.md](AGENTS.md) contiene las
  reglas para los agentes de programación.

### Trabajar con agentes de programación

Cualquier agente de programación puede trabajar en el repositorio, y una tarea
puede continuar otro día o con otro agente. Antes de planificar o cambiar la
lógica, el agente debe escanear el estado actual del repositorio y continuar a
partir de lo que realmente encuentre, usando el plan maestro y los documentos
rectores como fuente común. Así se evita que cada agente invente un enfoque
incompatible. Sigue el [flujo de trabajo para agentes de IA](docs/AI_AGENT_WORKFLOW.md);
el contexto de chats anteriores no sustituye revisar la rama, los cambios, el
código, las pruebas y los ADR aplicables.

## Ramas, commits y pull requests

- Una rama por funcionalidad o corrección, llamada `feature/<tema>` o
  `fix/<tema>` (`feature/native-format`, `fix/document-roundtrip`).
- Un tema por pull request; lo pequeño es más fácil de revisar y de revertir.
- Mensajes de commit: una primera línea en inglés, en imperativo
  (`Add LINE command`), y un cuerpo que explique el *porqué* cuando no sea obvio.
- En el pull request, explica **cómo verificaste** el cambio; la plantilla lo
  pide. La prueba más convincente es un test que falla sin el cambio, un dibujo
  concreto antes y después, o una medición.
- Quien mantiene el proyecto fusiona en `main` cuando las comprobaciones están
  en verde y la revisión ha terminado.

## Informar de un dibujo que se abre, se dibuja o se guarda mal

Es el tipo de informe más valioso, y es imposible de resolver sin el archivo.
Adjunta el DWG/DXF (comprímelo en zip si GitHub rechaza la extensión). Si es
trabajo de un cliente que no puedes compartir, una copia reducida a las
entidades que fallan vale incluso más que el original.

Antes de informar, puedes pasar el corpus de ida y vuelta por tus propios
dibujos sin compartirlos: abre cada uno, lo guarda como DWG y DXF, lo vuelve
a leer, lo edita y comprueba que no se ha perdido nada de lo que el programa
muestra.

```sh
# Dibujos generados por las pruebas: parte de cada `cargo test`.
cargo test -p arq-core --test corpus
# Las muestras del propio motor DWG/DXF, descargadas y verificadas antes.
uv run python tools/fetch_codec_samples.py
cargo test -p arq-core --test corpus -- --ignored codec
# Tus dibujos: un archivo .dwg/.dxf, o una carpeta recorrida con subcarpetas.
# Solo se leen; nunca se copian al repositorio.
ARQ_CORPUS_DIR=/ruta/a/dibujos cargo test -p arq-core --test corpus -- --ignored local --nocapture
```

## Traducir ArqStudioCAD

**Todo lo que se lee se traduce; todo lo que se escribe es inglés.**

Quien viene de AutoCAD escribe lo que ya saben sus dedos —`L`, `TR`, `Z`— y eso
debe funcionar en cualquier idioma de la interfaz. Se traducen los menús, los
diálogos, los paneles y el *texto* de los mensajes; no se traducen los nombres
de comando ni las teclas de las opciones. Las reglas completas están en el
[ADR 0006](docs/architecture/adr/0006-i18n.md).

La única regla estricta para traducir: un mensaje se interpreta según sus
teclas de opción en inglés, así que la traducción debe mantener cada tecla visible:

```json
"Specify rotation angle or [Reference]:":
    "Especifique el ángulo de rotación o [Referencia(R)]:"
```

Cada idioma es una carpeta en `python/arqstudiocad/locales/<código>/` con un
`meta.json` (su código y su nombre en su propio idioma, p. ej.
`"name": "Español"`) y un `ui.json` (`{"texto en inglés": "traducción"}`);
copia `es/` como modelo. No hace falta tocar código: el idioma aparece solo en
*Herramientas > Idioma*. Las traducciones parciales están bien: un texto sin
traducir se muestra en inglés, y nunca se retrasa una versión por una
traducción. Respeta los `{marcadores}` como `{app}` o `{version}` tal cual: las
pruebas lo comprueban. Los nombres de comando localizados van en
`commands.json` (`{"LINE": ["LINEA"]}`), en mayúsculas y sin tildes, como los
escribe AutoCAD (el usuario puede escribirlos con tilde). Se *añaden*: nunca
sustituyen ni tapan un nombre o alias en inglés, ni toman uno de los alias
ingleses de AutoCAD reservados para comandos futuros.

---

El modelo de contribución está adaptado de [IngeCAD](https://github.com/ingelibre/ingecad)
(GPL-3.0-or-later).
