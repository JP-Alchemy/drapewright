# Open questions

Questions I would rather ask than guess. Each one has a **provisional
default** that the code uses until it is answered. Every default is a single
setting, so changing it later is cheap.

Status key: ☐ open · ☑ answered (answer recorded inline)

---

## For you (JP)

### Licence
☐ **Which licence?** I have not added one; the manifests say "licence to be
decided".

| Option | For | Against |
|---|---|---|
| **MIT OR Apache-2.0** (dual, the Rust convention) | Anyone can embed the geometry kernel or importers, including tools like FreeSewing (MIT) and commercial cutter vendors. Compatible with an App Store build later. All our dependencies already use these licences. | Someone could ship a closed fork of the app. |
| **MPL-2.0** | Changes to our files must stay open, but the code can still be combined with other code. Firefox for iOS ships under it, so App Store distribution works. | Slightly more friction for embedders. |
| **GPLv3** | Every fork must stay open. Lets us copy code from Seamly2D/Valentina. | GPLv3 conflicts with App Store terms, so no future native iPad app without a different licence. We do not need their code anyway: reading their file formats is fine under any licence. |

My recommendation: **MIT OR Apache-2.0**, because interoperability is one of
the product principles and nothing we need is GPL.

### Other
☐ **Acceptance garments** — the `[FILL IN]` in the brief. Which 3–4 garments,
and can I have her pattern files (or the names of the patterns) as private
test fixtures?
☐ **"Protected" PDF patterns.** Many bought patterns are PDFs encrypted
with only an owner password: they open in any viewer without a password,
but say "no copying" or "no changes". Drapewright reads their drawing as
viewers do, keeps it on the device, and refuses PDFs that need a password
to open. *Provisional: open them, for the buyer's own use.* Is that the
policy you want, or should the import warn first (or refuse)?
☑ **Hosting**: our own server (Hetzner, nginx) at https://drapewright.com,
deployed from the `production` branch (README → Hosting).
☑ **Name**: Drapewright.

---

## For the seamstress

Each question says what I do now (the provisional default) and why it
matters.

### Seam allowance
1. ☐ **Default seam allowance for a pattern that has none.** 1.5 cm (5/8″,
   the common commercial standard) or 1 cm (common in European patterns)?
   *Provisional: 1.5 cm seams, 1.5 cm hems, set per piece and per edge.*
2. ☐ **Default hem allowance**, if different from seams. *Provisional: same as
   seams.*
3. ☐ **Corners of the allowance.** I support four shapes (pictures in the
   app): *pointed* (lines extended until they meet), *squared* (cut at a
   right angle to one edge), *folded back* (shaped so that when a hem is
   turned up, or a seam pressed open, the end lines up exactly), and
   *automatic*. *Provisional: automatic = folded back where an edge meets a
   hem, pointed elsewhere, with very sharp points cut square.* Is that what
   she expects to see on a printed pattern? FreeSewing patterns cut every
   corner off at 45° (a bevel), so on import their cutting line differs
   from ours by up to 1 cm at corners. Should Drapewright offer that corner
   too?
4. ☐ **"Squared" and "reverse for darts".** The brief lists these corner
   styles. I read *squared* as "cut at a right angle to one of the two
   seams", and *reverse for darts* as the shaped peak in the cutting line at
   a dart so the allowance is straight when the dart is folded. Is that what
   she means? (Darts come in Phase 4.)
5. ☐ **Patterns that already include allowance.** When she imports a
   pattern with allowance included, would she rather work on the cutting
   line (as on paper, allowance implied) or have it converted to the
   stitching line with the allowance added back? *Provisional: keep the
   cutting line exactly as printed and show the stitching line dashed inside
   it.* Nothing is converted unless she asks. When a file does not say
   which line its outlines are, the import asks her (nothing pre-chosen),
   with the allowance width; garment CAD files that mark their cutting
   lines pre-choose *Already included*.

### Notches and marks
6. ☐ **How notches should look in print**: a short slit into the allowance,
   or a small triangle outside the cutting line? *Provisional: slit.*
7. ☐ **When an edge gets longer or shorter**, should a notch keep its
   distance from the corner it was measured from, or slide proportionally?
   *Provisional: keep its distance from the nearer corner at the time it was
   placed.*
8. ☐ **Double and triple notches**: is 6 mm (¼″) between the marks right?
9. ☐ **Grainline**: double-headed arrow by default, single-headed for
   napped or one-way fabrics. Correct?

### Cutting instructions
10. ☐ When a piece says **"Cut 2"**, does she assume a mirrored pair (fabric
    folded) unless stated otherwise? *Provisional: yes.*
11. ☐ **Place on fold**: the fold edge gets no allowance and is printed with
    the usual bracket arrow. Correct?

### Printing
12. ☐ **Assembling tiled pages**: does she prefer *trim and butt* (cut off
    the margin on two sides and butt pages edge to edge) or *overlap and
    match*? *Provisional: trim and butt, with matching marks and A1/B2-style
    page labels.*
13. ☐ **Paper and units**: A4 and centimetres, or US Letter and inches?
    *Provisional: new projects start on A4 in centimetres; Letter and
    inches are one tap away and saved with the project. Choosing from the
    device's language comes with project storage (day 10).*
14. ☐ **Print the stitching line too** (dashed, inside the cutting line)?
    *Provisional: yes.*
15. ☐ **Copy shop / projector**: does she ever print A0 at a copy shop or use
    a projector? (Both are planned; this sets the priority.)

### Her patterns
16. ☐ **Where do her patterns come from?** Big 4 tissue, indie PDFs (which
    companies?), magazines (Burda), or self-drafted? This decides which
    importers and size systems get the most testing.
17. ☐ **DXF files from pattern CAD systems** (Gerber, Lectra, Optitex,
    Valentina/Seamly2D, CLO…): does she have any? The AAMA/ASTM reader is
    now tested on 44 public exports from Gerber, Lectra, Assyst, CLO and
    Browzwear, kept privately outside the repository. Real exports vary
    between vendors, so a few of her own files would still show what needs
    adjusting.

### Fitting (Phases 3 and 4)
18. ☐ **Which measurements are taken**, and over light clothing or on the
    skin? *Provisional: the step 17 list (height,
    bust, high bust, underbust, waist, high hip, hip, neck, shoulder
    width, shoulder length, across front, across back, back and front
    waist length, bust span, shoulder to bust point, waist to hip, arm
    length, upper arm, wrist, inseam, outseam, crotch depth, thigh, knee,
    calf, ankle), over light clothing.*
19. ☐ **Which alterations come up most?** This sets their order in
    Phase 4. *Provisional: lengthen and shorten, full bust, swayback, waist
    and hip, then the rest.*
20. ☐ **Which fabrics are sewn most?** The drape starts from presets.
    *Provisional: cotton lawn, poplin, linen, chambray, denim, twill,
    crepe, satin, jersey, rib knit, wool suiting, fleece.*
21. ☐ **Ease expected** between a fitted blouse and the body at bust,
    waist and hip, so the ease map's colours mean something. *Provisional:
    the map shows millimetres, with colours changing at 0 (tight), 20 and
    50 mm.*
