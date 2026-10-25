# Drawing templates

`arqstudiocad-sample.dwt` is ArqStudioCAD's sample template: an empty model
space and ten simple architectural block definitions (a door, a window, a
table with chairs, a bed, a toilet, a washbasin, a column, a tree, a north
arrow and a level mark), drawn in metres on coloured `A-` layers. The Blocks
palette offers it as a template (ADR 0025 D7).

It is generated, never edited by hand: run
`uv run python tools/make_sample_template.py` to rebuild it. It is drawn only
by that script; nothing in it comes from another application's templates.

## License

The sample template and every block in it are dedicated to the public
domain under [CC0 1.0 Universal](https://creativecommons.org/publicdomain/zero/1.0/):
use, copy, change and redistribute them, in any drawing, commercial or not,
without asking and without attribution. The rest of ArqStudioCAD keeps its
own licenses (GPL-3.0-or-later code, CC BY-SA 4.0 documentation).
