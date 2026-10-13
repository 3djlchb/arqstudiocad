/* LibreDWG adapter for dwg-bench (spike): reads a drawing into a JSON
 * summary and writes DWG or DXF, with the calls LibreDWG's own dwg2dxf and
 * dwgwrite programs use. Compiled against LibreDWG's source tree (DXF
 * output needs its internal Bit_Chain).
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "dwg.h"
#include "bits.h"
#include "out_dxf.h"

typedef struct
{
  char *name;
  unsigned long count;
} tally;

typedef struct
{
  tally *items;
  size_t len, cap;
} tallies;

static void
count (tallies *t, const char *name)
{
  for (size_t i = 0; i < t->len; i++)
    if (!strcmp (t->items[i].name, name))
      {
        t->items[i].count++;
        return;
      }
  if (t->len == t->cap)
    {
      t->cap = t->cap ? t->cap * 2 : 64;
      t->items = realloc (t->items, t->cap * sizeof (tally));
    }
  t->items[t->len].name = strdup (name);
  t->items[t->len].count = 1;
  t->len++;
}

typedef struct
{
  char *s;
  size_t len, cap;
} buffer;

static void
put (buffer *b, const char *text)
{
  size_t n = strlen (text);
  if (b->len + n + 1 > b->cap)
    {
      b->cap = (b->len + n + 1) * 2;
      b->s = realloc (b->s, b->cap);
    }
  memcpy (b->s + b->len, text, n + 1);
  b->len += n;
}

static void
put_name (buffer *b, const char *name)
{
  put (b, "\"");
  for (const char *c = name; *c; c++)
    {
      char one[8];
      if (*c == '"' || *c == '\\')
        snprintf (one, sizeof one, "\\%c", *c);
      else if ((unsigned char)*c < 0x20)
        snprintf (one, sizeof one, "?");
      else
        snprintf (one, sizeof one, "%c", *c);
      put (b, one);
    }
  put (b, "\"");
}

static void
put_tallies (buffer *b, const tallies *t)
{
  put (b, "{");
  for (size_t i = 0; i < t->len; i++)
    {
      char n[32];
      if (i)
        put (b, ",");
      put_name (b, t->items[i].name);
      snprintf (n, sizeof n, ":%lu", t->items[i].count);
      put (b, n);
      free (t->items[i].name);
    }
  put (b, "}");
  free (t->items);
}

static int
is_dxf (const char *path)
{
  size_t n = strlen (path);
  return n > 4 && (!strcmp (path + n - 4, ".dxf") || !strcmp (path + n - 4, ".DXF"));
}

/* Reads `path`; returns the error bits (>= DWG_ERR_CRITICAL: failed). */
static int
read_any (const char *path, Dwg_Data *dwg)
{
  memset (dwg, 0, sizeof (Dwg_Data));
  dwg->opts = 0;
  return is_dxf (path) ? dxf_read_file (path, dwg) : dwg_read_file (path, dwg);
}

static void
fingerprint (buffer *b, const char *key, unsigned long n, double sum, int last)
{
  char text[160];
  snprintf (text, sizeof text, "\"%s\":{\"count\":%lu,\"sum\":%.17g}%s", key, n,
            sum, last ? "" : ",");
  put (b, text);
}

/* A JSON summary of `path` (free with lr_free), or NULL with *error set. */
char *
lr_read_json (const char *path, int *error)
{
  Dwg_Data dwg;
  *error = read_any (path, &dwg);
  if (*error >= DWG_ERR_CRITICAL)
    {
      dwg_free (&dwg);
      return NULL;
    }
  tallies types = { 0 }, objects = { 0 };
  unsigned long entities = 0, model = 0, layers = 0, blocks = 0;
  unsigned long n_lines = 0, n_circles = 0, n_inserts = 0;
  double s_lines = 0, s_circles = 0, s_inserts = 0;
  unsigned long n_solids = 0, solids_with_data = 0, xdata_entities = 0;
  double s_solids = 0;
  for (BITCODE_BL i = 0; i < dwg.num_objects; i++)
    {
      Dwg_Object *o = &dwg.object[i];
      const char *name = o->dxfname ? o->dxfname : "UNKNOWN";
      if (o->supertype == DWG_SUPERTYPE_ENTITY)
        {
          entities++;
          count (&types, name);
          if (o->tio.entity && o->tio.entity->entmode == 2)
            model++;
          if (o->tio.entity && o->tio.entity->num_eed > 0)
            xdata_entities++;
          size_t nl = strlen (name);
          if (o->tio.entity && o->tio.entity->tio.BODY
              && (!strcmp (name, "3DSOLID") || !strcmp (name, "REGION")
                  || !strcmp (name, "BODY")
                  || (nl > 7 && !strcmp (name + nl - 7, "SURFACE"))))
            {
              /* Every ACIS entity starts with the same fields. */
              Dwg_Entity_3DSOLID *sol
                  = (Dwg_Entity_3DSOLID *)o->tio.entity->tio.BODY;
              double bytes = 0;
              if (!sol->acis_empty)
                {
                  if (sol->version == 2)
                    bytes = sol->sab_size;
                  else if (sol->block_size)
                    for (BITCODE_BL k = 0; k < sol->num_blocks; k++)
                      bytes += sol->block_size[k];
                }
              n_solids++;
              s_solids += bytes;
              if (bytes > 0)
                solids_with_data++;
            }
          if (o->fixedtype == DWG_TYPE_LINE && o->tio.entity->tio.LINE)
            {
              Dwg_Entity_LINE *l = o->tio.entity->tio.LINE;
              n_lines++;
              s_lines += l->start.x + l->start.y + l->end.x + l->end.y;
            }
          else if (o->fixedtype == DWG_TYPE_CIRCLE && o->tio.entity->tio.CIRCLE)
            {
              Dwg_Entity_CIRCLE *c = o->tio.entity->tio.CIRCLE;
              n_circles++;
              s_circles += c->center.x + c->center.y + c->radius;
            }
          else if (o->fixedtype == DWG_TYPE_INSERT && o->tio.entity->tio.INSERT)
            {
              Dwg_Entity_INSERT *ins = o->tio.entity->tio.INSERT;
              n_inserts++;
              s_inserts += ins->ins_pt.x + ins->ins_pt.y;
            }
        }
      else
        {
          count (&objects, name);
          if (o->fixedtype == DWG_TYPE_LAYER)
            layers++;
          else if (o->fixedtype == DWG_TYPE_BLOCK_HEADER)
            blocks++;
        }
    }
  buffer b = { 0 };
  char text[256];
  snprintf (text, sizeof text,
            "{\"version\":\"%s\",\"entities\":%lu,\"model\":%lu,\"layers\":%lu,"
            "\"blocks\":%lu,\"recovered\":%s,\"diagnostics\":%d,\"types\":",
            dwg_version_type (dwg.header.from_version ? dwg.header.from_version
                                                      : dwg.header.version),
            entities, model, layers, blocks, *error ? "true" : "false",
            __builtin_popcount ((unsigned)*error));
  put (&b, text);
  put_tallies (&b, &types);
  put (&b, ",\"objects\":");
  put_tallies (&b, &objects);
  put (&b, ",");
  fingerprint (&b, "lines", n_lines, s_lines, 0);
  fingerprint (&b, "circles", n_circles, s_circles, 0);
  fingerprint (&b, "inserts", n_inserts, s_inserts, 0);
  fingerprint (&b, "solids", n_solids, s_solids, 0);
  snprintf (text, sizeof text, "\"solids_with_data\":%lu,\"xdata_entities\":%lu}",
            solids_with_data, xdata_entities);
  put (&b, text);
  dwg_free (&dwg);
  return b.s;
}

void
lr_free (char *s)
{
  free (s);
}

/* Reads `in` and writes `out` (DWG or DXF by extension) as `version`
 * ("r2000", "r2018"…); returns the error bits, DWG_ERR_CRITICAL or more on
 * failure. */
int
lr_write (const char *in, const char *out, const char *version)
{
  Dwg_Data dwg;
  int error = read_any (in, &dwg);
  if (error >= DWG_ERR_CRITICAL)
    {
      dwg_free (&dwg);
      return error;
    }
  Dwg_Version_Type target = dwg_version_as (version);
  if (target == R_INVALID)
    {
      dwg_free (&dwg);
      return DWG_ERR_CRITICAL;
    }
  if (dwg.header.from_version != dwg.header.version)
    dwg.header.from_version = dwg.header.version;
  dwg.header.version = target;
  remove (out);
  if (is_dxf (out))
    {
      Bit_Chain dat = { 0 };
      dat.version = dwg.header.version;
      dat.from_version = dwg.header.from_version;
      dat.fh = fopen (out, "wb");
      if (!dat.fh)
        error = DWG_ERR_IOERROR;
      else
        {
          error = dwg_write_dxf (&dat, &dwg);
          fclose (dat.fh);
        }
    }
  else
    error = dwg_write_file (out, &dwg);
  dwg_free (&dwg);
  return error;
}

int
lr_critical (void)
{
  return DWG_ERR_CRITICAL;
}
