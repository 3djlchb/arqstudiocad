# STRETCH and BREAK: independent review (2026-11-23)

The roadmap left BREAK's implementation waiting on a fresh independent
acceptance. STRETCH and BREAK were written by another agent, and their
work passed to Claude Code (maintainer, 2026-11-23). This is that review,
of both commands, made on LMDE 7 against their designs:
- [STRETCH](../design/stretch-command.md) and its
  [partial arcs](../design/stretch-partial-arcs.md);
- [BREAK](../design/break-command.md).

Like their own checks, it is internal. It runs no ProgeCAD; that stays
on the Windows laptop.

## Method

1. **Read** each command's Python flow, its core operation and the
   geometry behind it, against its design and AutoCAD 2021's behavior.
2. **Ran edge cases** through the core from a scratch script outside the
   repository, each result worked out by hand:
   - **BREAK:**
     - a line broken in either order, at one interior point, past one end
       and past both;
     - points off the line, and coordinates of a million;
     - an arc broken in either order, across 0°, and by points beyond its
       sweep;
     - a circle and a whole ellipse, both ways round;
     - open and closed polylines, across a vertex and across the closing
       segment.
   - **STRETCH:**
     - lines, circles, arcs, texts, ellipses, and open and closed
       polylines, each with windows holding a part, all of it or none;
     - a stretch that would collapse a segment.
3. **Sabotaged** eight rules the commands rest on, to see whether the
   existing tests guard them:
   - the span BREAK takes from a closed curve, and the piece before an
     open curve's gap;
   - a partial arc's sagitta;
   - a closed polyline's traversal, and its constant width;
   - STRETCH's remembered displacement, and Enter at its second point;
   - BREAK's First point option.

## Findings

- **The geometry and the edits match their designs.** Each result agreed
  with the value worked out by hand. For example, an arc's start moved to
  (6,1) keeps its sagitta: its radius becomes 5.1707, as the formula
  gives.
- **The existing tests guard the rules:** all eight sabotages were
  caught.
- **Two things a user meets were wrong, and are fixed:**
  - **BREAK on a locked layer:** an object picked on a locked layer gave
    the core's error, untranslated: `Error: entity 49 is on locked layer
    L`, in Spanish too. It now says what the other commands say, and asks
    for another object.
  - **STRETCH's displacement prompt** did not show the displacement Enter
    takes. As AutoCAD does, it now shows the last one, `Specify
    displacement <2,3>:`.
    - **Exact:** the value shown is the very one remembered, digits and
      all, so Enter does not round it.
    - **A dead branch removed:** the fallback for an empty answer could
      no longer run, so it is gone.
- **Left as designed:**
  - **No center snap on a circle:** a circle's center, typed as a break
    point, projects onto an arbitrary point of the circle, since it has
    no nearest one. AutoCAD's pick cannot land there.
  - **The singular message:** `{count} were on a locked layer.` read
    "1 were" for one object. Since its follow-up (the same day) one reads
    `1 was on a locked layer.` in every command, and OFFSET, FILLET,
    CHAMFER and JOIN say it too where they gave the core's error.

## Checks

- Each new test was observed failing with its fix taken out, and passing
  with it.
- The STRETCH and BREAK Python suites pass: 54 cases.
- The local checks of AGENTS.md pass.
