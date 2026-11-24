# 0045. Compatibility with the program a drawing comes from or goes to

Date: 2026-11-24 · Status: Proposed · Phase 03 (03.4, 03.5); every later
entity

## Context

AutoCAD and ProgeCAD (an IntelliCAD) both read and write DWG and DXF.
Some entities they keep in different ways, each reading its own form
well and the other's less well. The Windows comparisons found these:

| Case | AutoCAD | ProgeCAD | Ours |
|---|---|---|---|
| A closed spline through fit points (SP3) | to confirm | clamped, its first fit point again at its end | the same as ProgeCAD's since #262; before, periodic, which ProgeCAD read as open from DXF |
| A polyline's one width all along (M2) | to confirm | the width at each vertex, no constant width | the constant width |
| A spline's fit points after a knot method (SP4) | to confirm | none kept | kept |
| An offset of an ellipse or a spline (SC11–SC13) | to confirm | quadratic, rational with every weight 1 | cubic, not rational |
| A multileader style in DWG 2004 | EED of `ACAD_MLEADERVER` | refuses a drawing without it | written as AutoCAD's ([evidence](../../testing/progecad-dwg2004-mleader-version.md)) |

The maintainer asked, on 2026-11-24:
- for the greatest compatibility possible, in both directions;
- for it to be automatic.

The two directions differ:
- **Reading:** a drawing says, more or less clearly, what made it. Ours
  can know it and read each entity as that program meant it.
- **Writing:** what program will open the file is not known when it is
  saved. Ours cannot write for it; it can only write what both read.

Nobody here has AutoCAD. ProgeCAD runs on the Windows laptop alone.

## Decision

The maintainer chose the following on 2026-11-24.

### D1 — The producer, found on reading

Each drawing read knows its producer: AutoCAD, an IntelliCAD such as
ProgeCAD, ours, another, or unknown.

**How it is found:** by what the file itself says.
- **A DWG of 2004 or later** names the program that wrote it in its
  `AcDb:AppInfo` section. Our fork of AcadRust writes that section but
  does not yet read it; reading it is the first delivery.
- **A DXF:**
  - its header (`$ACADVER`, `$ACADMAINTVER` and what each producer adds);
  - the registered applications and classes it carries.

  Which of these tell producers apart is shown by samples first, then
  used.
- **None of them clear:** unknown, read as today.

**What it is used for:** reading, D2. The producer is not saved in the
drawing, nor asked of the user.

### D2 — Read as the producer meant it

Where a producer keeps an entity in a form of its own, the reader
takes that form into the one model ours holds. For example, it drops
the fit point a closed spline repeats at its end, as `SplineFit::new`
already does for any producer.

So the model does not depend on where a drawing came from, and nothing
drawn changes because of it. A difference is handled where the entity
is read, so every command sees one form.

### D3 — Written as both read

Each entity is written in one form, whoever made the drawing and
whoever will open it:
- the form AutoCAD and ProgeCAD both read the same way, where there is
  one;
- otherwise AutoCAD's, the format's owner, which ProgeCAD sets out to
  read;
- unless a test shows ProgeCAD breaking on AutoCAD's form; then the one
  ProgeCAD reads, recorded as a known difference.

The choice for each entity is recorded with its evidence in a table,
`docs/testing/producer-forms.md`, which this ADR starts. A change of form
goes through it.

### D4 — Evidence

- **AutoCAD:** from public drawings that AutoCAD made, those the corpus
  already uses (LibreDWG's test data, AcadRust's samples) and others
  (ezdxf, ODA), each checked to come from AutoCAD. What no sample shows
  is written "to confirm".
- **ProgeCAD:** from runs on the Windows laptop, as the 02.14
  comparisons are made.
- **Ours:** each entity's form is read back by ours in every case.

### D5 — Deliveries

1. **The producer found:**
   - the codec reads `AcDb:AppInfo`;
   - DXF headers and registered applications are compared across samples
     from each producer;
   - the document knows its producer;
   - tests on the public samples and on files ProgeCAD made.
2. **The forms table:** the cases above, each checked against AutoCAD's
   public samples; SP3's AutoCAD form first, since ours changed in #262.
3. **Adaptations:** each difference found, read in D2 and written in D3,
   in its own pull request.

## Consequences

- **No setting and no question:** the user never picks a target program.
- **Mixed drawings:** a drawing passed from one program to another and
  back is written in one form each time. Nothing depends on where it
  was made.
- **#262 may change again:** SP3's form (ProgeCAD's) stands until
  AutoCAD's is known. If they differ and AutoCAD's is one ProgeCAD reads,
  D3 makes it AutoCAD's.
- **A codec change:** reading `AcDb:AppInfo` is a change in our AcadRust
  fork, under ADR 0017 D2.
- **Room for what comes later:** BIM and IFC (ADR 0014) bring other
  producers. D1's producer and D3's table take them as further columns.
