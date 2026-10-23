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
22. ☐ **Full bust adjustment: where the dart's tip stops.** *Provisional:
    2.5 cm short of the bust point, on the line from the bust point to the
    middle of the dart's mouth.*
23. ☐ **Pivoting a dart: about its tip, or about the bust point?** Books
    pivot about the bust point and then shorten the new dart. *Provisional:
    about the dart's own tip, so a side dart moved to the waist keeps the
    side dart's tip (2.5 cm from the bust point, towards the side).*
24. ☐ **A swayback on a back cut on the fold.** *Provisional: refused, with
    the advice to make the centre back a seam first. Alternatives: take the
    wedge out and then straighten the fold (losing some of it at the
    centre), or move it to the side seams.*
25. ☐ **Broad and narrow shoulders: which method?** *Provisional: slashed
    down the grain from the middle of the shoulder seam to the level of a
    hinge on the armhole (level with its notch), then across to it, and the
    shoulder's end turned out until it has moved the amount along the
    shoulder seam. The shoulder seam is then trued straight.*
26. ☐ **Waist and hip: where the amount goes.** *Provisional: at each side
    seam, square to the grain; the waist tapers to nothing at the hip, the
    hip carries straight on down to the hem. A front and a back cut on the
    fold therefore change the whole garment by four times the amount.*
27. ☐ **Truing: how far either side of a jog a smooth truing blends.**
    *Provisional: 2.5 cm, less where jogs are close together.*
28. ☐ **Measuring her own fabric.** Is the at-home test practical: a 10 cm
    square weighed; a strip 5 cm wide with 500 g hung from it, along and
    across the grain; a strip 2.5 cm wide slid over a table's edge until it
    droops to 45°? Would she rather enter the weight printed on the bolt
    (g/m²) and choose "stretches a little / a lot"? *Provisional: the
    tests, each optional.*
29. ☐ **Projector set-up.** Does she have, or want, a projector over her
    cutting table? Which size of rectangle can she mark on her mat?
    *Provisional: 40 × 30 cm to start, changeable.*
30. ☐ **Facings and linings on the avatar.** *Provisional (2026-10-14):
    draped inside, shown paler, and the status line names them.* Each
    starts laid on the piece it was traced from and is kept behind the
    cloth over it. Those sewn to nothing are still left off. Lining is
    draped as the main fabric for now: should it have a fabric of its own?
    Known shortcoming: sitting in a lined skirt, the skirt rides up round
    the waist (each layer pushed out along its own slant, step by step);
    standing, walking and reaching are right.
31. ☐ **Buttoning on the avatar.** Buttons marked along a front are done
    up when it is draped: to the other front's buttonholes, or to its edge
    where it has loops instead. *Provisional (2026-10-14): done up, with a
    **Buttons done up** switch under the avatar to see it worn open.*
32. ☑ **Pieces drawn once, for one side.** Corset patterns often draw each
    panel once and don't print how many to cut. Should Drapewright set them
    itself? *Answered (JP, 2026-10-12): yes, and say so.* **Find seams**
    sets a band of panels cut once, from an open centre front to an open
    centre back, to cut 2 as mirrored pairs (one undo step), and the status
    line names them.
33. ☐ **A busk and lacing.** A corset opens at the centre front (a busk)
    and laces at the centre back. *Provisional: Find seams sews each centre
    to its other half, and the drape keeps both closed.* Should the back
    drape laced with a gap (often about 5 cm)?
34. ☐ **Crotch pieces on the avatar.** A bodysuit's crotch pieces are sewn
    between the legs. *Provisional: left off the drape, and the status line
    says so, until the avatar can keep cloth between its legs.*
35. ☐ **A corset smaller than the body.** A corset's waist is made smaller
    than the waist it goes on, to pull it in. The avatar's body doesn't give
    way, so the corset is stretched round it. *Provisional: stretched
    cloth.* Should the avatar's waist draw in to the corset's, and say by
    how much?
36. ☐ **Pockets drafted into a panel.** FreeSewing's Sasha draws its side
    front as two pieces: one above the pocket's opening that runs on down
    as the pocket's bag, and one below that lies over it. Sewing and
    draping a piece partly worn outside and partly inside isn't possible
    yet. *Provisional (2026-10-14): Sasha is drafted without its pockets,
    and the sample dress is that.* Should pockets be sewn in, and shown?
    *Note (2026-10-23):* patch pockets are now sewn onto their placement
    and draped over the panel (Step 80), but this is a different case: one
    panel that is the garment's outside in one part and behind another
    panel in another. The drape keeps layers by panel, so it would need
    each point to know its own layer (`docs/design-pockets.md` §5).
37. ☐ **Fronts done up with nothing marked.** A coat done up with snaps
    often marks neither snaps nor buttonholes: the instructions say how
    many, and the centre front line drawn on each front says where.
    *Provisional (2026-10-14): fronts that cross past a centre front line
    drawn on them, with no buttons marked, are done up on that line every
    10 cm down two thirds of the opening; **Buttons done up** shows it
    open.* Should she mark her snaps herself instead (and where they go)?
38. ☐ **Facings with nothing to lie on.** A coat's neck and front facings
    are sewn to each other at the shoulders, then to the collar and the
    front edges, between the coat and its lining. Find seams sews them to
    each other only, so they have nowhere to go inside. *Provisional
    (2026-10-14): left off the drape (with the armhole facings, which
    can't yet be draped without them), and the status line says they are
    sewn but have nothing to lie on.*
39. ☐ **Two pieces drawn as one.** A pocket's two bags are often drawn on
    one outline, one bag a little longer (each named, with "front" and
    "back" marking its line). *Provisional: imported as the shorter bag and
    a narrow strip, the difference, which can be deleted.* Should both bags
    be made?
40. ☐ **Pocket placements marked by dots only.** Many patterns mark only a
    patch pocket's top corners with dots, not its outline. *Provisional
    (2026-10-23): not found; a pocket is sewn on only where a line is drawn
    on the panel.* Should a pocket be placed by its top corners' dots?
41. ☐ **A pocket's turn-down when it isn't marked.** A patch pocket's top is
    turned down, so its sides are longer than its placement's. *Provisional
    (2026-10-23): the turn-down is the difference, shared between its two
    sides, up to 3 cm each; the side seams stop there.* Is that how she
    reads a pocket with no fold line drawn?
