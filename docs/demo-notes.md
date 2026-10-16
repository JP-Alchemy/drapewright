# Demo notes

What you can try after each step. Newest first.

To run the app locally:

```bash
pnpm install
pnpm wasm
pnpm dev        # then open http://localhost:5173/app/ (the site is at the root)
```

---

## Step 74 — Seams under the pieces they sew, joined on the table

The side panel listed the pieces, then every seam apart from them: to see
which pieces fit together, you matched each seam's number to the numbers
on the table by eye. Now each piece's seams are under it in the list, as
an outliner shows what belongs to what, and picking one draws a line on
the table from one of its edges to the other.

**Try it:** on an empty table press **A dress from a PDF pattern** and
**Add 5 pieces**.

1. Press **Find seams**, under the pieces. Tap a suggestion's words (not
   its buttons): its two edges are drawn dashed on the table, joined by a
   dashed line. Tap it again to hide it. Then **Sew all**.
2. Tap **Front inside**, in the list or on the table. It opens to show the
   pieces it is sewn to, each with its seam's number (as on the table) and
   the edges sewn, its own first: *Sleeve, edge 4 ↔ part of edge 1*. The
   chevron beside a piece with seams opens and closes it.
3. Tap **Sleeve** under it. A line joins the front's armhole to the
   sleeve's head, and the details show the seam: both sides, how they
   measure against each other, **Turn round** and **Unpick**. Had either
   edge been off screen, the table would have moved to show both.
4. Tap the sleeve on the table, or **Sleeve** in the list: the seam stays
   picked, now under the sleeve. Drag the sleeve and the line goes with it.
   Tap the seam again to put it down; tapping the table where there is no
   piece, or Escape, puts down the piece too.
5. Pick an edge, **Sew to another edge…** and tap another: the new seam is
   picked, so the line shows what was sewn to what, **Turn round** at hand.

With the keyboard, the arrow keys go through the pieces and their seams,
picking each seam they come to. A seam whose notches miss each other by a
centimetre or more (sewn the wrong way round, say) has a warning sign
beside it.

**What changed:**

* **The pieces list is a tree.** A seam is listed under each piece it
  sews (once, for a piece sewn to itself), saying the piece it is sewn to
  and the edges. A piece chosen on its own opens, and stays open until
  closed, so nothing moves under the pointer. **Select** (to tick several)
  lists the pieces alone.
* **The seams' own section is gone.** **Find seams** and its suggestions
  are under the pieces; a picked seam's lengths and notches, **Turn round**
  and **Unpick** are in the details, above its piece's.
* **The line** leaves each edge at its middle (or the middle of the part
  sewn), outwards, as a thread pulled from one edge to the other would,
  over a halo of paper so it shows on the mat and across pieces alike. A
  seam sewing an edge to its own other half (a centre back) is a dot on
  the edge.
* **The view** moves only when an edge of the seam picked (or the
  suggestion shown) is off screen: as little as it can, zoomed out only
  if both edges don't fit.

---

## Step 73 — An assistant names pieces by the words printed on them

The importer names each piece by rules: the biggest words beside its
number, not the pattern's title, not a translation. Every such rule fixes
one pattern and fools another (steps 67 to 71 kept finding more). An
assistant reading all the words printed on a piece is not fooled. Now it
can: the import keeps the words printed on each piece, and the MCP server
has a tool to read them.

**Try it:** with the assistant connected (`docs/ai-assistants.md`), ask it
to import `fixtures/pdf/sizes-layers.pdf` (size 10) and then: *"Check the
names of the pieces against the words printed on them."* It calls
`piece_labels`, which lists for the Front bodice "FRONT BODICE" (the words
that named it), "Cut 1 on fold" (its cutting) and "PLACE ON FOLD" (an
instruction), biggest first. On a big company's pattern it lists each name
in English and, smaller, its translations, and once, at the top, the words
printed on most pieces (the pattern's title). Pieces drawn here, or
imported before this step, say they have no words kept.

**What changed:**

* **The words are kept** with each piece imported from a PDF, as part of
  where it came from (`source.words`: each line's text, where its middle
  was on the piece, its letter height and which way it read). They are
  never printed, and move with the piece when its scale is set again.
  Projects are now schema version 7; earlier ones open as before, their
  pieces with no words kept.
* **`piece_labels`** lists them, marking each as the piece's number (set
  in its biggest type), its cutting, an instruction, a size along its
  lines or words printed on most pieces, and which words named it. Sizes
  and the pattern's own words are left off each piece unless asked for.
  The server's instructions tell an assistant to check names this way
  after importing a PDF.

Over the private test folder, the imports are the same as before (only the
words are kept besides). SVG and DXF imports name pieces from their layers
and blocks and keep no words yet.

---

## Step 72 — The same drape on the graphics card in the browser, every time

Step 66 made the graphics card's own work repeat exactly, but in the
browser two drapes of the same garment could still come out a little
different. Which lining points are kept behind which skirt points, and when
the cloth has settled, were worked out from whichever of the cloth's points
had come back from the card, and that depends on how busy the computer is.

**Try it:** in Chrome or Safari, **Open pattern** and choose
`fixtures/drapewright/lined-skirt.drapewright`, open the avatar (it says
**Drawn and draped with WebGPU**) and **Drape**. Note how the hem and its
folds hang, reload the page and drape again without moving the view: it
hangs exactly as before, fold for fold. It drapes as fast as it did.

**What changed:**

* Every frame's points now come back from the card, and the drape always
  goes on from those of the frame eight before the one it sends: which
  lining points to keep behind which skirt points (looked for again every
  fourth frame at most), when the cloth has settled, and where the body is
  felt as it moves into a pose. Where the computer can wait, as the tests
  do, it waits for them. The browser can't, so the next frame is held back
  until they have come, rather than going on from whatever has. So the
  drape is the same however long they take.
* Measured in Chrome on a Mac (M3 Pro): the lined skirt's 2.2 s of cloth
  takes 0.51 s to drape, as before (0.52 s), and 2.8 s from **Drape** to
  settled, most of it spent placing the garment. Six drapes in a row came
  out the same to the pixel, where before no two did. Outside the browser
  (the tests) a drape is about three times as fast as before, as the card
  no longer stands idle while each frame's points come back: the lined
  skirt settles in 0.5 s, from 1.8 s.
* Looking for the lining's pairs every frame, or keeping the drape only
  four frames behind, made the browser's drape a fifth to two fifths
  slower, so neither is done. With starts nudged by up to 0.3, 1 or 3 mm,
  0 of 384 lined-skirt drapes failed to settle clear of the body, as
  before.
* A new test drapes the lined skirt, moves it into a pose and settles it
  again three ways: waiting for every frame, asking as fast as it can
  whether the next frame can be taken, and asking now and then as a
  page's frames go by. The three must come out the same, bit for bit.
  Before, they settled at different frames.

---

## Step 71 — Instructions drawn as illustrations

A kimono pattern's instructions come as a PDF of their own: six pages of
coloured illustrations (the tools, the marks, the finished measurements,
the cutting plan) with their words in narrow columns beside them. Its
lines were too short to count as prose, so half its pages were read as a
pattern and came in as 25 "pieces": the boxes round the lists and tables,
and parts of the drawings.

**Try it:** import the kimono's instructions PDF from JP's test folder.
It now comes in with no pieces, saying all 6 pages were left out. Its
pattern files come in as before.

**What changed:** besides five lines of prose or more, a page with 40
words or more for each A4 of it, in lines of any length, is a page of
instructions when it is drawn mostly in filled shapes (illustrations)
rather than in lines, as patterns are. A piece's own notes don't count
(step 70).

Over the private test folder, nothing else changes but a magazine's
knickers pattern, which leaves out one more page of its instructions;
its pieces are the same.

---

## Step 70 — One-page templates covered in notes

A face mask's free templates come as two pages, one for the main fabric
and one for the lining, each a single piece covered in notes: what to
cut, a bar to check the print by, the copyright, where the instructions
are. With that many lines of words, both pages were taken for
instructions and left out, and the import came in empty (all four
templates in JP's test folder: "2 pages left out").

**Try it:** import any of the four mask templates. Each comes in as two
pieces, Main Fabric and Lining Fabric, cut 2 each, with their ¼ inch
(or ⅜ inch) seam allowance found inside the cutting line.

**What changed:**

* **A piece's notes are no page of instructions.** Words printed inside
  a piece's outline that says how many to cut ("CUT 2") don't count
  towards a page of prose. Only an outline on the page, no bigger than
  six tenths of it and no box (a lay plan's fabric, a box of notes, a
  frame round the page): a page of instructions with a lay plan on it is
  still left out.
* **Two pages alike are no tiles.** Pages that draw the same piece in
  nearly the same place (the main fabric's and the lining's) were joined
  on top of each other as if they were tiles. Tiles of one sheet are at
  least half a page apart one way; these stay two pages.

Over the private test folder, besides the masks: a bikini's front and
back, which were joined on top of each other, are two pieces; the wrap
skirt's tie page no longer joins the skirt in the wrong place (7 pieces a
size: front, back, two waistband sections and three tie parts, some named
by words printed on them, "Along this line"); two of the indie dress
patterns' tiles join so that
the logo on every tile names no piece; a jacket's back drawn twice is
once, on its fold. A pdfstitcher test file whose two pages draw the same
squares at different scales now comes in as both pages. The instruction
booklets come in as before.

---

## Step 69 — A key to the lines is no list of sizes

A shirt pattern's US Letter file, the same pattern as its A0 and A4
files, prints a key beside the pieces: a sample of each line with what it
is ("1cm seam allowance", "Pink line"). The import took the key for the
sizes, and came in as 23 pieces in 10 "sizes" named after the key; its A0
and A4 files come in as 7 pieces.

**Try it:** import the shirt's US Letter file from JP's test folder. It
comes in as 7 pieces, like the other two files, with its stitching lines
found inside the cutting lines.

**What changed:** a legend names sizes only when its entries read as
sizes ("Size 10", "XS", or "36 (UK 8)" with a note after the size). A key
to the lines names none. And a line starting "See …" ("See the cutting
layout for details.") names no piece.

Over the private test folder, one other pattern changes: a children's
sundress whose key to its lines was read as three sizes named "/", "w"
and "Black line" now comes in without sizes, its yoke named "Yoke".

---

## Step 68 — Tiled sheets without their tiles' furniture

Two graded home-print patterns came in as hundreds of pieces: a summer
jacket in 14 sizes as 489 (35 a size) and a wrap skirt in 8 sizes as 422
(53 a size). Most were not pieces at all but the tiles' furniture: the
frame each page is cut along, the frames of the pages beside it, and the
boxes printed across each join to match the pages by. And the jacket's
pages, numbered "P1" to "P28", were laid in one long line.

**Try it:** import either pattern from JP's test folder. The jacket comes
in as 4 pieces a size (Sleeves, Front bodice, Optional front tie, Back
bodice), like its A0 copy-shop sheet; the wrap skirt as 6 a size: its
front, its back cut on the fold, and the waistband and ties. The
patterns from a sewing book in the folder lose the frame round every
tile, which came in as a piece: the pencil skirt is now Front, Back and Waistband (it was 16
pieces), the pyjamas Fronts, Backs and Waistband (34), the waistcoat
Fronts, Centre back and Side backs (16).

**What changed:**

* **Page numbers place nothing.** Pages labelled "P1", "P2"… with no page
  naming the next at its edges are numbered, not set out in rows and
  columns; they go where the shapes they share say.
* **Tiles' furniture.** Closed shapes that most tiles of a sheet draw at
  the same place on the page (a frame, a neighbour's frame drawn past the
  edge, a box at a join, the designer's logo) are no part of the pattern.
  The pattern's own shapes move from tile to tile. And a layer named for
  the page grid ("Paper Grid", "Tiling") holds no pieces.
* **A piece drawn twice.** A shape the same as two on another page (a piece
  drawn twice on the sheet) counts half for each place it says the page
  goes, so it can't outvote the page's true neighbours.
* **An outline drawn over itself** several times on a page is one piece.

Over the private test folder: besides these, tiles' join marks no longer
count as marks on the pieces of the indie dress patterns, a bucket hat is
Crown, Brim and Top (it was 12 pieces, tiles misplaced), and the logo
printed on every tile no longer names pieces of the indie dress
patterns. The cape coat's size 38 and the corset come in as before.

Known: in the wrap skirt, the long tie's extension page still matches a
page of the skirt at the wrong place (the tie's two parts look alike), so
its waistband and ties are not all right yet (step 70 fixes this). The
circular skirt and the
apron in the sewing book have pieces whose outlines don't close; they come
in as their waistband or pocket only.

---

## Step 67 — Sizes printed alike, told apart by their numbers

The big pattern companies print every size of a piece nested, in one
colour on no layer, each size's number printed along its line; and beside
the pieces, a key to the marks set out as a table in four languages and
boxes of instructions. Such a sheet came in as one muddle: the free blouse
in JP's test folder (two sheets, five sizes each) gave 86 and 106 pieces,
dozens of them the key's boxes, named "Milieu" and "Jareta".

**Try it:** import either of the blouse's sheets, A0 or A4. The import now
asks which of the five sizes (8 to 16, or 18 to 26) to bring in; each size
comes with its own pieces: 10 from the first sheet, and 6 and 4 from the
two of the second set, named in English (Front facing, Collar, Sleeve,
Continuous lap, Cuff, Back facing…). It says "Left out 3 tables and boxes
of notes". A Spanish pattern's back says "Cortar al lomo": it is now cut on
the fold, its centre back a fold, and named "Trasero" rather than by that
instruction.

**What changed:**

* **Sizes by the numbers on their lines.** Sizes nested in line styles
  that come round in turn were told apart before. Now sizes drawn alike
  are too, when each line carries its own size's number over and over,
  more than any other size's (a piece's number by its smallest line
  doesn't count). Nested lines with no sizes on them (a cutting line round
  its stitching line) are still no sizes.
* **Size numbers are no watermark.** Words printed twice or more on most
  pages were taken out as watermarks; a nested pattern's size numbers are,
  on every tile, so the A4 file lost them. Words that read as sizes are
  kept.
* **Tables and boxes of notes.** Boxes set side by side in rows and
  columns, each up to 10 cm (a key, a size chart), and boxes holding five
  lines of prose or more that cover a quarter of the box, are no pieces.
  Pages' frames side by side are bigger, and stay as they were.
* **Names.** By a piece's big number, the pattern's title is the name
  only when nothing else is there, the name in the biggest type is taken
  before its translations set smaller, and lines a little further either
  side of the number's line count. "All sizes" and lines that start in the
  middle of a sentence ("of this piece") name nothing.
* **Five languages.** "Cut" and "on the fold" are read in Spanish, French,
  German and Italian too ("Cortar 1 al lomo", "Couper 2 entoilage",
  "im Stoffbruch"), with lining and interfacing.

Over the private test folder, only these files change: the blouse's six
sheets, and names on five others (a bow tie's two pieces "Short tie
piece" and "Long tie piece" rather than "With interfacing"; a jacket's tie
"Optional front tie" without the pattern's title; a sundress's "Front and
back dress"; a wrap skirt's piece no longer named after the pattern; the
kimono's back "Trasero"), and the kimono's box of notes on which sizes to
print, left out. Nothing changes in the cape coat or the corset.

Known: the second set's two sheets come in separately (6 and 4 pieces per
size). The tiled sheets of a pattern book draw a frame round each page,
and those frames come in as pieces while the pieces across them don't;
that is for another step.

---

## Step 66 — A lined skirt settles on the graphics card every time

On the graphics card (WebGPU), about one drape in five of the test lined
skirt never settled: the cloth kept stirring, and the skirt sat up to a
centimetre into the body round the hips or waist (step 58 noted it).
Whether the GPU test passed was luck, and changed with unrelated code.

**Try it:** in Safari or Chrome, **Open pattern** and choose
`fixtures/drapewright/lined-skirt.drapewright`, open the avatar (it says
**Drawn and draped with WebGPU**) and **Drape**, several times. Each time
it settles in a few seconds ("Draped in poplin. Inside, shown paler: Skirt
front lining and Skirt back lining."), its lining inside and the skirt
clear of the body.

**What was wrong, and what changed:**

* **A folded corner let the lining through.** The lining is kept behind
  the skirt along the skirt's outward side, read from the skirt's
  triangles. As the hem first swings, a corner beside a side seam can
  flip over for a moment; there the skirt's "outward" points inward, and
  the lining was pushed out through it. Once through, each push between
  the two layers undid another, so they tangled and never came to rest,
  and where the skirt's folded side faced the body it was pushed into it.
  Now the lining is kept behind the skirt only where the two face the same
  way; where one has folded over they are left be until it opens. Facings
  and linings are now taken to face the way the cloth they lie on faces,
  rather than by which side is towards the body (a back neck facing away
  from the body could be taken either way).
  The processor drapes the same way. It hardly ever got tangled, because
  it solves the cloth from the top down, but solved in the graphics card's
  order it did too.
* **The graphics card's drape was different every time.** Keeping the
  cloth off the body read neighbouring points while other threads were
  moving them. It now reads them from a copy taken just before, so a
  drape whose every frame is waited for (as the tests do) comes out the
  same, bit for bit. In the browser, which can't wait, which lining
  points are kept behind which skirt points is still worked out from
  points that come back a frame or two late, so two drapes there can
  differ a little.

Measured on a Mac's GPU, each start nudged by up to 0.3, 1 or 3 mm: 0
failures in 384 drapes (it was about 1 in 27, and about 1 in 5 before
the drape was the same every time). Ordinary drapes on the processor come
out as before (the tests' figures move by a tenth of a millimetre at
most). The tests now drape the lined skirt from six nudged starts and
require each to settle clear of the body, check that a drape repeats
exactly, and fold a hem corner over a lining on purpose, on both the
processor and the card.

---

## Step 65 — Tiles wordy enough to look like instructions

Patterns from the big pattern companies print every label in three or
four languages ("LENGTHEN OR SHORTEN HERE / ALLONGER OU RACCOURCIR ICI /
ALARGUE O ACORTE AQUÍ"). Every A4 tile then had as many lines of words as
a page of instructions, so every page was left out as instructions and the
pattern came in empty ("43 pages left out").

**Try it:** import the A4 home-print file of a big pattern company's
pattern, such as the free blouse in JP's test folder (two sheets of 43 and 36 pages). It now
comes in with the same pieces as its A0 copy-shop file: 86 and 106 pieces,
where before there were none. A real page of instructions in the same file
is still left out.

**What changed:** when the pages that aren't wordy make no sheet, the
wordy pages are tried too. Those that share shapes with each other, three
or more, are tiles of one sheet; pages of instructions share none. This
only happens where the import found nothing before, so other patterns come
in as they did.

Known: those two sheets hold five sizes nested, drawn alike with no layers
or colours, and the sizes don't come apart yet. Some pieces are named by
their French or Spanish label ("Milieu", "Jareta").

---

## Step 64 — The same PDF comes in the same every time

Some PDFs came in with their pieces in a different order each time they
were imported. The pieces were the same, but the unnamed ones swapped
numbers: "Piece 3" one time was "Piece 31" the next. An assistant (or a
person) told "Piece 3 is the front" would be wrong after importing again.

**Try it:** import a PDF whose outlines are drawn as many separate strokes,
with unnamed pieces, twice. The pieces get the same numbers both times. In
JP's test folder an underwired bra pattern (40 pieces) changed order on
every import; now it never does. Every file in the folder comes in the
same on every run, Find seams' suggestions included. One sewing book's
dungarees also stop flickering between 190 and 191 small shapes left out.

**What changed:** outlines joined from lines that meet end to middle came
out in whatever order a hash map kept them in. They now come out in the
order they were found.

---

## Step 63 — A pattern that is a picture says so

A PDF of a scanned pattern, or one made from photos of the pages, has no
lines in it: only pictures. It imported with no pieces and no word of why.

**Try it:** import a scanned pattern, or any PDF made from photos (a 1920s
dressmaking book from the Internet Archive, or a free pattern drawn in a
picture program). With no pieces found, the import now explains:

> These 12 pages are pictures (scans or photos), not lines, so Drapewright
> found no pieces in them. If the pattern comes as a PDF, SVG or DXF made
> on a computer, open that instead. Or print it, cut out the pieces, and
> photograph each one beside a sheet of printer paper.

An assistant gets the same note through the MCP server, and is asked to
offer those ways in.

**What changed:**

* **The PDF reader notes where pictures are painted** (images placed on
  the page, and those written into the page itself) without reading them.
* **A page painted mostly with pictures** (a third of it or more) is a
  picture. The note is only given when no pieces were found, so a pattern
  with a photo of the garment beside its pieces says nothing.

---

## Step 62 — A test square 10 cm one way and 4 inches the other

Some patterns print one "test square" to check the print in both units:
10 cm across, 4 inches (101.6 mm) down, with a ruler along each side. It
isn't quite square, so the importer didn't know it. It came in as a piece
called "4 inches", one for each size, with its ruler's marks as notches.

**Try it:** import an A0 copy-shop file whose test square says "10
centimeters" along one side and "4 inches" along the other. The square is
left out ("Found the 10 cm test square; it was left out of the pieces")
and the piece count matches the A4 file's. In JP's test folder:
* a wrap dress gives 80 pieces, not 90;
* a bomber 80, not 90;
* a hoodie 70, not 80;
* a slip dress 30, not 40.

**What changed:** a test square may be metric one way and imperial the
other: 5 cm by 2 in, or 10 cm by 4 in, either way round. As before, it
must be a rectangle with its size or "test square" printed in it. The note
gives its side across.

---

## Step 61 — Pieces named by their own words, not the designer's

Indie patterns print the same words on every piece: the designer's name
or logo (often in large script), the pattern's title, "with seam
allowance", a credit line such as "Project design © …". The importer
named each piece by the largest word inside it, so a whole pattern could
come in with every piece called after its designer.

**Try it:** import a free pattern whose pieces each carry the designer's
logo. In JP's test folder, eight patterns from one designer, about 600
pieces in all, were all named after the logo; now they are named Front,
Back, Sleeve, Front facing, Back neck facing, Pocket… A Spanish kimono's
pieces are Delantero, Manga and Cuello instead of its title, and a
book's patterns lose their "Project design ©" names.

**What changed:**

* **Words on most of the pieces are a last resort for a name.** Words
  found on three pieces or more, and on half the pieces that carry words,
  are the title, the designer or a note. They name a piece only when
  nothing else on it does: three pockets labelled only "Pocket" are still
  pockets.
* **A credit never names a piece** (words with © or "copyright" in them).

---

## Step 60 — Tiles trimmed at a frame inside the page

Many free patterns printed for home draw a frame a few millimetres inside
each A4 or Letter page to trim it at, with scissors on its lines. The
pattern itself runs on past the frame to the page's edges, under the next
tile. Imported, the frames came together into one box round the whole
sheet. The importer took that box for a title block (a key to the lines)
and left out every piece inside it, so these patterns came in with no
pieces at all ("1 title block left out").

**Try it:** import such a pattern's A4/Letter file: one of the free
patterns in your test folder, or any whose tiles say "Page 1A" in the
middle and have a frame with scissors. Each page's frame lines go, the
tiles overlap as printed, and the pieces come in with every size. In JP's
test folder:
* a wrap dress gives 80 pieces in 10 sizes, matching its A0 copy-shop file;
* a jumpsuit gives 50;
* children's joggers give 20 and 42 (before: 4 and none).

Bigger files that half-worked gain the sizes they lost: a hoodie gives 70
pieces (was 32), a bomber 80 (was 30), a haori jacket 7 (was 3).

**What changed:**

* **A frame to trim at is found inside a page** whose drawing is clipped
  to the whole page. Before, a frame was only looked for when the drawing
  stopped at it (plotter tiles to butt together). Each side is a line on
  its own, running on past the frame's corners toward the next tiles. A
  piece's straight edge near the margin is part of its outline, so it
  doesn't count. A grid of squares printed over the page stops at its
  corners, so it doesn't either.
* **Only a frame most tiles draw in the same place** (three at least) is
  taken off, and then from every tile clipped to its page. One page with
  a large rectangle near its margins keeps it.

Every other test pattern imports exactly as before: the bought blouse and
trousers, the corset, the cape coat, the DXF and SVG sets.

---

## Step 59 — An assistant drapes the garment and sees it worn

The MCP server's new `drape` tool drapes the project's garment on the
avatar, as the app's **Drape** does, and gives the assistant a picture of
it worn from the front, the side and the back, with how it fits. An
assistant can now show you the garment on your measurements, in a fabric,
done up or open, and moving.

**Try it:** build the server (`cargo build --release -p drapewright-mcp`)
and add it to your assistant (see `docs/ai-assistants.md`). Then ask, for
example: *"Open fixtures/drapewright/shirt.drapewright, drape it and show
me. Now with the arms raised, as a heatmap of where it pulls."* The shirt
drapes in about 2 seconds; the reply gives at the bust, waist and hip the
shirt's size round the body, the ease (the shirt is about 11 cm bigger
than the figure at the bust), how hard the cloth pulls (typically, and in
its hardest-pulled tenth, in N/m; the heatmap is red at 100) and the room
it leaves. With your measurements kept in the project (`set_measurements`)
it drapes on the avatar fitted to them; `person: "standard"` uses the
standard figure. `fabric` takes the app's fabrics or your own, `open:
true` undoes the buttons, `pose` moves the figure once it has settled,
and `shows: "room"` colours the cloth by the room between it and the body.

**What changed:**

* **One way to drape.** The steps from the pieces to the settled garment
  (placing it round the body, its facings inside, its buttons, measuring
  it again as it settles, then moving into a pose) were in the app's
  WebAssembly side; they are now the simulator's (`drapewright-sim`'s
  `draping`), and the app and the assistant both use them. The app drapes
  as before (its drape tests pass unchanged); so do the heatmap's colours,
  now shared.
* **Pictures of the figure** are drawn by the server itself: the figure and
  the cloth, smoothly lit, nearest in front, drawn twice the size and
  shrunk for smooth edges. The cloth lying on the body is drawn over it.
* **How it fits** is read as a sewist would: the finished garment's size
  round the bust, waist and hip and the ease there (as under the avatar
  in the app), then from the cloth on the body how hard it pulls and how
  much room it leaves near each, and the band of heights where it pulls
  hardest. A garment always pulls most where it hangs from, the shoulders
  for a top or a coat.

Known: a drape on the computer the server runs on is coarser than the
app's on the graphics card (its points 20 mm apart, as in the app without
one).

---

## Step 58 — A cape coat from a layered PDF, sewn and draped as worn

A lined cape coat from an independent designer's PDF (JP's test pattern,
not in the repo: a copy-shop sheet and A4 tiles, each of its eight sizes
on a layer of its own) now imports, sews and drapes as the coat is worn:
an A-line coat in princess panels done up down the front, a stand collar,
and a circular cape over the shoulders, its front and back sewn into the
princess seams.

**Try it:** open the coat's copy-shop PDF (or its A4 file), choose size 38
only, say the seam allowance is included, and **Add 13 pieces**. Then
**Find seams**, **Sew all** (22 seams), open the avatar and **Drape**.
Under the avatar the coat measures about 98 cm at the bust, 8 cm more
than the figure (the pattern's finished bust for size 38 is 100 cm).
**Buttons done up** off shows it open. To see the made-up coat the tests
use: `cargo run --release -p drapewright-sim --example drape_picture --
assets/body/adult.dwbody cape woolSuiting cape pose=arms-down`.

**What changed:**

* **Reading the PDF.** A copy-shop sheet was taken for a page of
  instructions (its copyright notice is a few lines of prose); a big page
  now needs as much prose for its size as an A4 page. Sizes drawn in
  dashes stretched to fit each side (each side stopping a little short,
  a solid stub turning each corner) now join up: five of the eight sizes
  were lost. A piece open along its fold is closed by the fold line drawn
  apart, however long. The box holding the pattern's key (a picture of the
  coat, the layout, the lines) is no longer a piece, nor is a page-sized
  frame beside the A4 tiles. Names set beside their number ("7 Collar") or
  on several lines ("Armhole / Front / Facing") are read whole; a line
  labelled "Center Front … Fabric grain" is the centre front line and the
  grainline; "cut 2 (Inside & Outside)" is two layers, not a left and a
  right.
* **Seam allowance.** The stitching line's corners, drawn as hairs and
  hooks where its dashes meet, no longer make edges of their own or point
  the wrong way. Where a deep hem's corners are shaped (turned back on the
  flare), the allowance is read from most of each edge, and the
  difference is noted. An allowance meeting a fold just past square no
  longer rounds out 4 cm past the fold. A collar's rounded corners now
  split its outline into its edges.
* **Find seams** finds the shoulders though the lines across the panels
  (waist, lengthen-shorten) meet their other seams; leaves out the
  armholes' facings when judging where the coat closes into a point; sews
  a cape's front (back) edge into the princess seam from the neck, with
  its neckline laid on the panel's (**sewn into the seam**), where the
  edges beside them match in length and angle; sews a collar along the
  neckline across the centre back, centred, standing past the fronts to
  button over; never sews a front's edge to its other half where a centre
  front line is drawn inside it, nor a cape's when another piece is the
  centre front, nor treats a front and back sewn at shoulder and side as a
  trouser leg.
* **The drape** sews three layers in one seam (the cape between the side
  front and the centre front) to the same points; hangs the cape outside
  the coat, round the side from its front seam to its back seam and clear
  of the arms; stands the collar up from the neckline; and does up fronts
  that cross past their centre front line where nothing is marked (open
  question 37). Facings sewn but with nothing to lie on are left off, and
  the status line now says so rather than that they aren't sewn (open
  question 38).
* `import_report` takes `--size 38` to list one size.

Known: the pocket's two bags come in as one bag and a 12 mm strip (open
question 39); the GPU drape of the lined skirt sometimes sinks into the
body, before this change too (a separate task).

---

## Step 57 — The sample dress sewn by Find seams, and draped

The sample dress (**A dress from a PDF pattern**) now opens, sews and
drapes as it should: its five pieces, **Find seams** and **Sew all** give
its ten seams, and **Drape** puts a fitted skater dress on the figure.

**Try it:** on an empty table press **A dress from a PDF pattern**, **Add
5 pieces**, **Find seams**, **Sew all**, then open the avatar and
**Drape**. The status line under it reads about 5 cm ease at the bust,
4 cm at the waist and 10 cm at the hip.

**What was wrong, and what changed:**

* **The sample.** FreeSewing's Sasha draws its pockets into the side front
  (a bag behind the panel below it), which can't be sewn or draped yet
  (open question 36), and its back shoulder is 4 cm longer than its front's
  (it widens the front neckline, not the back's). Sasha is now drafted
  without its pockets, its back neckline widened as much as its front's;
  the sample is that, drafted to the example figure and printed on A0.
* **Find seams** no longer sews two hems to each other from the end of a
  seam, nor a front's and a back's piece of an armhole from the side
  seam's top: sewn, either would close the cloth round that point only half
  way, into a spike no garment has. Such seams are found once the likeliest
  are sewn, and the least likely seam meeting there is dropped. Once two
  pieces cut as pairs are sewn half for half (or crossed), they stay so; a
  piece has one centre seam; and a straight back a few millimetres longer
  than its front (a back shoulder's ease) is sewn to it, **the back eased
  in**.
* **The drape** placed every piece of a bodice with its top at the
  shoulders, so a side panel (its top at the armhole) started 13 cm too
  high, and sewing it on pulled the centre front up into a roll across the
  chest. Pieces are now placed where their seams with their neighbours put
  them.
* **The garment's size** under the avatar is now measured where the cloth
  hung from the shoulders has settled (placed flat, it reaches the bust
  lower than it does worn); a skirt is still measured where it is placed,
  as it is worn.

---

## Step 56 — An example person's measurements

**Measurements** always shows an **Example**: the standard figure, the
avatar as it is before anyone's measurements are added, measured as a tape
would measure it (176.7 cm tall, bust 89.5, waist 70.8, hip 98.6). It is
for reference, to see what each measurement is about and roughly how much,
and can't be changed or deleted. It isn't kept in the project: it is the
same for everyone, and stays the standard figure's if that changes (the
body's tests write it).

**Try it:** on a new project, open **Measurements**: the Example is shown.
**Add my measurements** adds you, first, with the Example after. Without
measuring anyone, a FreeSewing design can be drafted to the Example, and
**Which size fits?** in the import dialog finds the Example's size.

---

## Step 55 — Choosing pieces with a box

Drag across the table from where there is no piece, and a box follows the
pointer: every piece it touches is chosen, as files are in a folder, whether
or not the box takes in all of it (it goes by the piece's outline, so a
box past a slanted side chooses nothing). Shift, Command or Ctrl adds the
box (or a tapped piece) to what is already chosen.

**Try it:** open a pattern with several pieces and drag a box over some of
them. The details say how many, with **Turn clockwise**, **Turn
anticlockwise** and **Delete** for all of them; right-click (or hold a
finger on) any one of them for the same in the menu. Delete or Backspace
deletes them too, and arrow keys nudge them. Turning or deleting them is
one undo step. A finger draws the box on an iPad until a Pencil is used
(then the Pencil does, and fingers pan); a second finger calls the box
off, leaving the choice as it was. Holding a finger or right-clicking on
the empty table now offers **Select all**.

---

## Step 54 — The drape on the graphics card (WebGPU)

Where the browser has WebGPU (Safari on iPadOS 26, Chrome, Edge), the
drape is worked out on the graphics card: faster, with cloth more finely
divided (points 12 mm apart instead of 20), so folds and gathers are
softer and the fit truer at curves.

**Try it:** open the avatar in Safari on the iPad and drape the test shirt
or a skirt. Under the view it says **Drawn and draped with WebGPU**. On
this Mac's WebKit a skirt drapes in about 0.8 s, the shirt in 1.3 s and
the lined skirt in about 3 s, each with three times the points the
processor drapes (which takes 3.4, 6 and 6.5 s). Poses move faster too.
Where there is no WebGPU (the view says **Drawn with WebGL2**), the drape
is worked out on the processor as before.

**How it works:** the same small steps as on the processor (XPBD, thirty
a frame), in compute shaders (`crates/sim-gpu/src/cloth.wgsl`). Edges,
bends and stitches are each split into colours, no two constraints of a
colour sharing a point, and a colour is solved at once; the cloth is kept
off the body's distance field (blended between two as the body moves, and
off the seat), with friction, and linings behind the cloth over them. The
drape's own logic (sewing, settling, posing) stays on the processor, which
hands each frame to the graphics card and gets the cloth's points back a
frame or two later, to show and to tell when it has settled. The tests
drape the test garments both ways and compare (on a Mac's graphics card,
and in CI on Mesa's software one).

---

## Step 53 — A paper pattern piece from a photo

A paper pattern can be photographed and its pieces brought in one at a
time, at their true size.

**Try it:**

1. Lay a pattern piece flat on a table or floor of another colour (a
   green cutting mat is ideal), with a sheet of A4 or Letter paper beside
   it, not touching, and photograph them from above (a slant is fine).
2. **Open pattern** and choose the photo (on an iPad you can take it
   there and then). Tap the sheet's four corners; drag one to adjust it.
3. Tap the piece. Its outline appears in orange; if it strays onto the
   table or stops short, move **How alike the paper is**, or tap another
   part of it.
4. Name it and press **Use this piece**: it is imported as a file would
   be. Say whether its seam allowance is included (a printed pattern's
   cutting line usually includes it), and add it.

To try without a camera: `fixtures/photo/piece-and-a4.png`, its sheet's
corners at the top left.

**How it works:** the sheet's corners, tapped, and its size fix how the
table lies in the photo (a homography), so the photo is read as the
table seen from above, a millimetre a pixel. From the point tapped, the
paper is followed as far as its colour goes (in CIE Lab, lightness
counting half, as shade falls unevenly); lines printed on it and holes in
it are filled in, and gaps narrower than 2 mm closed. Its outline is
traced, smoothed, split at its corners into edges, and drawn with
curves within 0.8 mm (Schneider's fitting). Everything happens on the
device; the photo is kept with the project, as a pattern file is.

---

## Step 52 — Facings and linings inside; buttons undone

Facings and linings are draped inside the garment instead of being left
off, and a buttoned garment can be seen worn open.

**Try it:**

1. Open `fixtures/drapewright/lined-skirt.drapewright` and drape it. The
   lining hangs inside the skirt, 3 cm shorter; the status line says
   "Inside, shown paler: Skirt front lining and Skirt back lining." Turn
   the view to look up under the hem to see it.
2. Open `fixtures/drapewright/shirt.drapewright` (a yoked shirt buttoned
   down the front, with a back neck facing) and drape it. Switch off
   **Buttons done up**: it is draped again, its fronts hanging open; on
   again, it is buttoned.
3. Pose either: the lining and facing move with the garment.

**How it works:** the garment as seen from outside is placed round the
figure as before. Each facing or lining is then laid flat on the piece it
was traced from (worked out from where their stitches meet, choosing the
piece it lies on rather than beside), and starts where that piece is on
the figure. While the cloth settles, each inside point near the cloth over
it is kept behind it (along that cloth's outward side) by its thickness
and a millimetre of air, except where the two are sewn together; there is
otherwise no cloth resting on cloth. A garment worn open is the same drape
without the stitches that hold its buttons.

---

## Step 51 — Poses: the figure moves, the garment with it

The avatar can take a pose (arms down, reaching forward, arms raised,
walking, sitting), and a garment draped on it moves with it: to see
whether a sleeve lets the arm reach, a back is wide enough, or a skirt
lets its wearer walk and sit.

**Try it:**

1. Open the avatar and choose a pose from the menu at the top of its pane
   (it starts at **Standing**, arms out and down, as the figure is
   measured). The figure moves into it; for **Sitting** it sits on a plain
   grey seat, its feet on the floor.
2. Drape a garment: it is sewn on the standing figure, settles, then moves
   with the figure into the pose ("Moving into the pose…"). Choose another
   pose and it moves again, from where it is.
3. Show **Pulling** in a pose to see where the garment strains: across the
   back when reaching, at the hips when sitting.

**How it works:** each arm and leg has two bones (an upper arm, and a
forearm with the hand; a thigh, and a shin with the foot), turning about
the figure's own joint markers (shoulder, elbow, hip, knee). How much each
bone moves each point of the skin comes from the body's regions, spread
over the skin a few centimetres either side of where one meets the next,
so that it bends smoothly round the joints; the trunk stays where it is,
and the whole figure is lowered to keep its feet on the floor. Arms hung
down are kept far enough from the hips and sides to leave room for cloth.
While the body moves, the cloth feels it at keyframes a centimetre and a
half apart, blended between, each worked out only round the cloth (without
the hands and feet, which seldom touch it) so that it is quick; still
again, the whole body is felt as when draping. A seat rises under the
thighs as the figure sits. A drape counts as settled once the cloth as a
whole has stopped moving, even where a point caught between an arm and
the side is pushed to and fro in place.

---

## Step 50 — Knits: a fabric that stretches more or less

A knit pattern drawn for a fabric with some stretch can be cut from one
that stretches more or less, with the pieces made to suit; and sizes can
be chosen, and a draped garment judged, for a stretch fit.

**Try it:**

1. Open a pattern, pick a piece and choose **Add a step → Fabric
   stretch**. Set **Made for a fabric that stretches** to what the pattern
   says (20% to start with) and **This fabric stretches** to yours (mark
   10 cm across the grain, stretch it gently as far as it goes: to 13 cm
   is 30%). For 20% and 50% the step reads "Narrowed 20% across the grain
   for the fabric's stretch", and the piece is a fifth narrower; switch on
   **Both ways** for a four-way stretch and it is shorter too.
2. Press **The same on the other pieces of this fabric**: every other piece
   cut from it gets the same step, in one undo.
3. In the size finder, or under a draped garment's fit, choose **Stretch
   (knits, smaller than the body)**: about 4 cm smaller than the body round
   the bust and hip and 3 cm at the waist, as a knit worn close is.

**How it works:** a piece made for a fabric that stretches by p % is
meant to be worn stretched to (1 + p/100) of its width; cut from one
that stretches by f %, it is scaled across the grain by (1 + p/100) /
(1 + f/100), so it stretches as far on the body. It is scaled about its
fold (which stays where it is), or else its grainline, as a step kept on
the piece like the others; switched off, the piece is as drawn. The old
**Close** fit is now for corsets only.

---

## Step 49 — Trousers on the avatar

Trousers drape with a leg round each leg, and Find seams sews their
crotch.

**Try it:** open a trouser pattern (a front and a back, each cut as a
pair), press **Find seams** and drape it. The inseams and outseams are
sewn, and each crotch curve to the other leg's; on the figure each trouser
leg goes round its own leg from just above the crotch down.

**How it works:** the figure has legs now, as it has arms: a line down
each from the hip through the knee to the ankle, and how far the leg
reaches from it. A garment with a panel sewn to another on both its sides
(a front to its back, at the outseam and the inseam) is trousers: round
the hips its panels go round the trunk as a skirt's do; from 4 cm above
the crotch down, each half of it (from the centre front round one side to
the centre back) goes round that leg instead, from the inner side round
the front, each panel wholly on its leg. Find seams, once a front and a
back are sewn into a tube, sews the edge left over from the waist down to
the inseam's top (the crotch curve) to its other half.

---

## Step 48 — Bodysuits: the crotch between the legs

A bodysuit's crotch piece (a corset bunnysuit's, a leotard's gusset) is
now draped with the rest, between the legs, instead of left off.

**Try it:** open a bodysuit pattern with its crotch pieces (a corset
bunnysuit), press **Find seams** and drape it. The crotch's front is sewn
along the bottom of the fronts, its back along the bottom of the backs,
and a crotch in two pieces to itself between the legs; on the figure it
goes under the crotch from the front to the back.

**How it works:** a piece named for the crotch or a gusset starts curved
under the figure's crotch, from just in front of the body to just behind
it (a front piece the front half of the curve, a back piece the back
half), the end sewn to a front at the front; sewing draws it up to the
pieces it is sewn to. Find seams never sews a crotch piece to the top of a
piece drawn upright (its neckline or a corset's top, which can be as long
as the crotch's middle), and sews a crotch's front and back to each other
where they are as long as each other.

---

## Step 47 — FreeSewing designs, drafted to your measurements

FreeSewing's designs are drafted to each person's measurements rather than
graded in sizes. Eight of them can now be drafted in Drapewright, on the
device, and opened as a pattern: the Teagan T-shirt, the Sasha skater
dress, the Penelope pencil skirt, the Sandy circle skirt, the Cathrin
underbust corset, the Bella bodice block, the Simone shirt and the Diana
draped top.

**Try it:**

1. Add someone's measurements (**Measurements**).
2. On an empty table, press **A FreeSewing design, made to measure**
   (or **Draft a FreeSewing design to these measurements** in the
   Measurements dialog). Choose whose measurements and a design.
   It says which of their measurements it drafts to, and which it
   estimates ("Estimated from FreeSewing's standard sizes, in proportion:
   shoulder slope, waist to armpit…": measure those for a closer fit).
3. Press **Draft**. The pieces open as any pattern does, named, with
   their grainlines, folds, notches, hems and how many to cut, and 1 cm
   seam allowance: **Add** them.
4. **Find seams** sews them: a drafted T-shirt's shoulders, sides and
   underarms, and each sleeve's head round its armhole. Drape it.

**How it works:** FreeSewing (MIT, freesewing.org) drafts the design in
the browser, the design loaded only when it is drafted. Drapewright writes
each drafted piece as FreeSewing writes its own files (its stitching line,
grainline, fold, notches and cutting instructions) for the SVG import to
read, saying the allowance to add rather than drawing it, so Drapewright
draws the cutting line itself; and marking its hems, which it finds where
FreeSewing's allowance runs at least twice as deep, as deep all along (not
a dart's leg, which the allowance bridges). Hems are what tell Find seams
which way round a side seam goes. Measurements not taken are estimated
from FreeSewing's standard size nearest the person, in proportion to what
they have measured round the body (or along it, for lengths).

**Not yet:** a design's own options (ease, lengths, necklines) can't be
changed here; Simone's plackets and collar need some seams sewn by hand.
Sasha is drafted without its pockets (see Step 57).

---

## Step 46 — Feedback, and patterns that didn't open right

**Feedback** (the speech bubble at the top right) starts an email to
Drapewright in your own mail, saying which version of Drapewright and
which browser it is, or opens LinkedIn. Nothing is sent by the app
itself.

When a pattern file doesn't open right, **Didn't open right?** in the
import dialog asks what's wrong and then either starts an email or saves
a report to attach. The report says what was read from the file (its
name, size and fingerprint, the scale, pages and sizes, each piece found
with its size and edges, and what the importer noted), never the file
itself: attach the file too only if you may share it.

The header also keeps the project's name in view on a laptop: below
1400 px wide, Print, Layout, Projector and Back up show only their icons
(below 1000 px, every button does).

---

## Step 45 — A picture or a turning video of the drape

Two buttons at the top of the avatar save what it shows, to keep or to
share: **Save a picture of the figure** (a PNG of the view as it is) and
**Save a video turning round the figure** (once round in three seconds,
as WebM, or MP4 in Safari). Both are named for the project ("Test skirt
on the figure.png").

**Try it:** drape the sample skirt, turn the view to the side you want,
and press the camera; then the film button.

**How it works:** the 3D view draws a frame and a picture is taken of it
straight away, in the worker that draws it. For the video it is turned a
72nd of the way round between frames, and the pictures are played at 24
a second into the browser's own video recorder. Nothing leaves the
device.

---

## Step 44 — Projecting the cutting layout, piece by piece

The projector (Step 30) can now project the cutting layout onto the
fabric, and pick out one piece at a time.

**Try it:** open the sample skirt, press **Projector** and calibrate.
Then:

- **Cutting layout** projects the pieces as **Layout** lays them out
  (Step 43), on the fabric last chosen there, with the fabric's fold
  dashed along the top, its selvages along the bottom, and a line across
  where its end goes: lay the fabric along those lines. **As on the
  table** projects the pieces where they lie on the table, as before.
- **Next piece** and **Previous piece** pick out one piece at a time,
  bright, the rest dimmed, and slide it to the middle; **All** shows them
  all again.
- **Thick lines** doubles every line and the names, to see them on dark
  fabric. It is kept for next time, as is the choice of what to project.

---

## Step 43 — Cutting layout, and how much fabric to buy

**Layout**, at the top, lays the pieces out on fabric as they would be
cut, and says how much fabric they take.

**Try it:** open the sample blouse (Step 38) and press **Layout**. On
150 cm fabric folded in half lengthwise: "It takes 1.30 m of main fabric
150 cm wide, folded, 38% of it cut into pieces." The front and back are
on the fold, end to end; the sleeves are cut as a pair through both
layers below them. Change the width (112, 140 or 150 cm, or 45, 54 or 60
inches in inches; or another), open the fabric out, or keep every piece
the same way up for a nap or a one-way print. A project with lining or
interfacing lays out each fabric in turn. The choices are kept for next
time.

**How it works:** each piece's cutting line is turned so that its
grainline runs along the fabric (either way, unless the fabric has a
nap), the biggest first, and slid along the fabric as far back as it
fits, nearest the fold or selvage, with 2 mm between pieces. The fabric
is worked out in rows 2.5 mm deep: in each row, the spans pieces
already take. Folded, a pair is cut at once through both layers, a piece
cut on the fold has its fold edge on the fabric's fold, and a piece cut
once (or the last of an odd number) is cut through the top layer only,
which the layout says. Open, a piece cut on the fold is laid out whole,
and pairs are laid one face down. A piece wider than the fabric is
said, not laid. Lengths are rounded up to the next 5 cm (or eighth of a
yard).

---

## Step 42 — Fit advice, and altering to fit

Once a garment has settled on the figure, the avatar says how it fits
against how it should, and can alter it to fit.

**Try it:**

1. Open the sample skirt (Step 38), open the avatar and press **Drape**.
2. Under the drape controls, **Fit** says how it should fit (**Fitted**,
   **Relaxed**, or **Close** for corsets and stretch fabrics, as in the size
   finder) and how it does: "For fitted: 5.8 cm too tight at the hip; right
   at the waist."
3. Press **Alter to fit**. The side seams are let out or taken in at each
   level by as much as it needs, as steps on each piece ("Hip let out
   14 mm at the side"), all one undo. The garment drapes again by itself
   and says "It fits as fitted should at the waist and hip."

A new step, **Bust at the side**, lets out or takes in a side seam at the
bust, tapering to the waist, alongside the waist and hip steps; add it by
hand from **Add a step** like the others.

**How it works:** the ease wanted at each level is the size finder's
(Step 40). The garment is put on the figure to find where its bust, waist
and hip fall on each seam; the change round the body is shared among the
side seams (seams between a front and a back; on a garment of panels,
every seam), each edge moved by its share divided by how many times it
goes round the body (twice for a piece cut as a pair or on the fold). At
each seam the levels to change and those to keep are joined by steps that
taper from one to the next, so a waist let out leaves the hip as it was.
A level no seam crosses (a bust line at the armhole) is said, not
altered.

---

## Step 41 — Blending between sizes

When the size finder suggests one size at the bust, another at the waist
and another at the hip, the pattern can be cut that way: each piece in the
bust's size at the bust, the waist's at the waist, the hip's at the hip,
and blended smoothly between, as a sewist blends between the lines of a
printed multi-size pattern.

**Try it:**

1. Open a pattern in several sizes and measure every size on someone (Step
   40).
2. Under the table, **Size to cut at each** has the suggested size at each
   level; change any of them.
3. Where they differ, press **Blend between them**. One piece is added for
   each piece of the pattern, named as in the file, its notes saying how it
   was blended ("Blended between sizes: 36 at the waist and 42 at the
   hip."). Where they're all the same, **Choose** that size instead.

Measuring is steadier on real patterns too. A level is left out where the
garment doesn't go round the body there (a bunnysuit, cut up for the legs,
isn't measured round the hips); a side panel standing out under the arm
of a big size is no longer taken for a sleeve; and a waist or bust line a
nested PDF draws once, out to the largest size, now reaches the smallest
sizes too, so they are put on by their waist like the rest.

Some pieces aren't blended, and the app says which and why: a sleeve takes
the size at the top (it goes round the arm), a crotch piece the size at the
bottom, a waistband all at the waist the waist's size, and a piece not worn
round the body (a facing, a modesty panel) the size at the top.

**How it works:** each size between is sewn and placed on the figure
(Step 40), which says how high every point of each piece goes. A piece's
sizes are matched by shape, since a pattern's sizes don't share corners
piece for piece: sharp corners matched to the nearest of the same turn, and
the outline between them in proportion along its length. Sizes drawn
nested stay where they're drawn; sizes laid out apart (a CAD file) are
lined up by how they're worn and by their fold. At each height the size to
cut moves from one level's to the next along a smooth S-curve, through the
sizes between; the piece keeps the edges, notches and darts of the middle
size, its curves kept as curves where they still fit within 0.25 mm, and
its grainline, marks and lines move with the outline round them.

---

## Step 40 — Which size fits?

Opening a pattern in several sizes, the import dialog can now sew every
size and measure it on someone's figure, and suggest the size to cut at the
bust, the waist and the hip.

**Try it:**

1. Add your measurements (**Measurements**, at the top; at least bust,
   waist and hip).
2. Open a pattern in several sizes. Under the sizes, **Which size fits?**:
   choose whose measurements, and how it should fit: **Fitted**,
   **Relaxed**, or **Close** (corsets and stretch fabrics, smaller than the
   body).
3. Press **Sew and measure every size on …**. A table gives each size's
   bust, waist and hip, and the ease against the body, and says which size
   to cut at each ("For Jo: 36 at the waist and 40 at the hip"). **Choose**
   a size to add it.

A size that differs between the waist and the hip is what blending between
sizes is for (Step 41).

**How it works:** each size is sewn as Find seams would sew it (panels
drawn for one side cut as pairs), placed on the person's figure, and
measured as in Step 39. The size suggested at each level is the one whose
ease is nearest to the fit chosen: fitted wants about 6 cm at the bust and
hip and 4 cm at the waist; relaxed 12 cm; close 2 cm smaller at the bust and
hip and 6 cm smaller at the waist.

---

## Step 39 — The garment measured, and its ease

Once a garment is on the figure, the avatar says how big it is round the
bust, the waist and the hip, and the ease there: how much bigger than the
body it is (or smaller, for a corset). It measures the garment as a sewist
measures a finished one, flat, not as stretched round the body, and leaves
the sleeves out.

**Try it:** open the sample blouse (Step 38) and press **Drape**. Over the
figure: "Garment: bust 90.5 cm (1.0 cm ease) · waist 100.7 cm (30.0 cm
ease) · hip 101.9 cm (3.3 cm ease)". Add measurements and fit the avatar to
them: the ease is then against that person's body. A skirt says only its
waist and hip.

**How it works:** the sewn garment, as placed on the figure, is cut across
at the figure's bust, waist and hip. Where each triangle of cloth crosses
that height, the crossing is measured on the flat pattern piece, and the
pieces' crossings are added up. Panels that sit nearer an arm than the
trunk (sleeves) are left out. A tube of two 30 cm rectangles measures 60.0
cm, however much it is stretched round the body.

---

## Step 38 — Sample patterns to try

Someone arriving from the front page used to meet an empty table and need
a pattern file of their own to see anything. The empty table now offers
three samples:

- **A blouse, ready to drape** and **a skirt, ready to drape**: the
  garments the drape tests use, sewn already. Each opens as a project of
  its own ("Sample blouse"), with the figure beside it: press **Drape**.
- **A dress from a PDF pattern**: FreeSewing's Sasha dress as a copy-shop
  PDF (MIT licence, credited, its licence one tap away), read as any PDF
  is, piece by piece.

**Try it:**

1. Open https://drapewright.com/app/ in a private window (an empty
   table), and press **A blouse, ready to drape**. The blouse opens with
   its seams sewn and the figure beside it; press **Drape**.
2. On the front page, **Try it on a sample blouse** opens the app straight
   into the same sample (the address `drapewright.com/app/?sample=blouse`,
   or `skirt`, or `dress`).
3. Opening a sample again opens another copy: changes to one never touch
   another, or anything of your own.

The samples are kept for offline use with the rest of the app.

---

## Step 37 — The avatar sized, or floating as a window

The avatar's pane can now be made bigger or smaller where it sits, or
lifted off to float over the table as a window that moves and resizes.

**Try it:**

1. Open the **Avatar**. Drag the grip on the edge between the table and the
   avatar to give either more room (beside the table, or under it on an
   iPad held upright). Focused, the arrow keys move it too; a double-click
   puts it back halfway.
2. Press the float button in the avatar's title bar (two overlapping
   rectangles). The avatar becomes a window over the table, and the table
   gets the whole width. Drag the window by its title bar, or by the grip
   at its left, to move it; drag its bottom-right corner to resize it.
   Focused, the grip and the corner take the arrow keys.
3. Press the same button (now **Put back beside the table**) to dock it
   again.

Where the edge was and where the window floats are kept on the device, so
the avatar comes back as it was left. A window never leaves the room it
floats in: when the browser gets smaller, it moves and shrinks to stay in
view.

---

## Step 36 — A front page at drapewright.com, the app at /app/

drapewright.com now opens on a page that says what Drapewright is: what it
does, why it was started, what is free and given back, what comes next, how
to help, and the questions people ask (a free alternative to CLO 3D? on an
iPad? which patterns? is anything uploaded?). The app itself is at
drapewright.com/app/.

**Try it:**

1. Open https://drapewright.com. Read down the page; **Open Drapewright**
   (or **Open the studio** at the top) opens the app.
2. Projects saved before are still there: they are kept for the site, not
   for the page.
3. Drapewright added to a Home Screen before still opens straight into the
   app, and so does a link to the old address.
4. Share the address in a message: the preview shows a draped blouse.

**How it works:**

* *One site, two pages.* The front page is plain HTML and CSS built with
  the app; it loads nothing from anywhere else, like the app. The app's
  offline copy covers both: a page of the app opens the app offline.
* *Found by search engines.* A title and description in the words people
  search for, the page's one address, a description of the app as free
  software (structured data), a sitemap, and a picture for link previews.
  The pictures are of the made-up test blouse and the FreeSewing dress, not
  of anyone's own patterns.

---

## Step 35 — Patterns drawn for one side are cut as pairs

Patterns that draw each panel once, for one side of the body, rarely print
how many to cut. **Find seams** now sees such a band of panels, running
from a centre front left open to a centre back left open, and sets each
panel to cut 2 as mirrored pairs (a left and a right) before it looks for
seams. The status line says which pieces it changed, and **Undo** puts them
back to cut 1, all in one step.

**Try it** (with the free corset pattern in 13 sizes from Step 32):

1. **Open pattern**, choose the PDF, pick a size and **Add 10 pieces**.
   Every piece says **Cut 1** in the list.
2. In **Seams**, press **Find seams**. The status line says the seven
   panels "look drawn for one side, so they're now cut 2, a left and a
   right", and names them. The list says **Cut 2** beside each, and the 15
   seams of Step 33 are suggested, the centres and the crotch pieces
   included.
3. **Undo** sets all seven back to cut 1.

In an AI assistant, `get_seams` lists the pieces as `drawnForOneSide`, with
a note to tell the user and set them to cut 2.

A skirt drawn as half a front and half a back is found the same way. Cut as
pairs, each of its centres is now sewn to its own other half, not the
centre front to the centre back, and its waists are left for the
waistband.

**How it works:** pieces are joined into a band by the seams sewn and
suggested along their sides (edges running with the grain, not tops or
hems). A band of two or more pieces, all cut once and none on a fold, with
an open centre at a front piece and another at a back piece, is drawn for
one side. A piece's centre is its outermost edge straight along the grain,
or an edge named for the centre. A suggestion sewing a front's centre to a
back's doesn't count, since that is the two halves joined as if they were
the whole.

---

## Step 34 — Pick a piece, change it: the side panel

The side panel was one long column: the pieces, every seam, then the
chosen piece's details, with **Cutting** below its allowance and grainline.
Changing how many to cut meant scrolling down, then back up for the next
piece. Like the outliner and details panels of Unreal Engine, Unity and
Blender, the lists and the details now scroll on their own, and the most
used settings sit at the top, in the same order for every piece.

**Try it:**

1. Open a pattern and tap a piece. Its name and **Cutting** (how many,
   fabric, mirrored pairs) are at the top of the details. Tap the next
   piece in the list: the list stays where it is, and the details show the
   new piece from the top.
2. **Allowance**, **Grainline** and **Alterations** fold away: tap a
   heading. Folded, each says in a line what it holds ("Added · 1.5 cm",
   "One way", "2 steps"), and it stays folded for every piece, and after a
   reload, until opened again.
3. Drag the bar between the lists and the details to give either more
   room (or focus it and use the arrow keys; double-click puts it back).
4. Each piece in the list says how many are cut ("Cut 2").
5. **Select**, above the pieces, turns tapping into ticking. Tick several
   pieces and the details set them all at once: how many to cut, fabric,
   mirrored pairs and allowance. Where they differ the field says
   **Mixed**, and + and − step each one's own value. **Done** goes back to
   tapping one piece at a time. With a keyboard, Shift and Command choose
   several too.
6. Tap a piece on the table: the list scrolls to it.

On an iPad held upright, the lists and the details sit side by side under
the table.

With a finger, tapping a second piece in the list used to add it to the
pieces chosen, so the details showed "2 pieces selected" instead of the
piece tapped. Now a tap chooses that piece alone, unless **Select** is on.

---

## Step 33 — A corset drawn for one side, draped

Corset patterns often draw each panel once, for one side, with no notches:
the bust, underbust, waist and hip lines drawn across the panels say where
they are sewn. **Find seams** now reads those lines, and a corset goes on
the avatar by its waist line. Before, the corset in the free pattern from
Step 32 got six seams, several of them wrong, hung from its waist like a
skirt, and was dragged out of place by its crotch pieces.

**Try it** (with the free corset pattern in 13 sizes from Step 32):

1. **Open pattern**, choose the PDF, pick a size (C, say) and **Add 10
   pieces**.
2. The pattern doesn't print how many of each piece to cut. Select each of
   the seven panels (Front Center, Front Middle, Front Middle Side, Front
   Side, Back Side, Back Middle and Back Center) and, under **Cutting**, set
   **How many** to 2, as mirrored pairs.
3. In **Seams**, press **Find seams**. Fifteen seams are suggested:
   * the panels side by side, where their bust, waist and hip lines meet
     (**lines across meet**). Front Center runs on below Front Middle,
     which is sewn along its upper part;
   * the centre front and the centre back, each to its other half
     (**centre of a pair**): the busk and the lacing, closed for now;
   * the crotch front to the crotch back (**notches match**), the crotch
     front along both Front Centers' lower edges, and the crotch back along
     the backs' (**fits along the opening**).
4. **Sew all 15**, open the **Avatar** and press **Drape**. The corset goes
   on with its waist line at the avatar's waist and settles round the
   bust, waist and hips. The status line says one piece is left off (the
   modesty panel, sewn to nothing) and that the two crotch pieces are left
   off: the avatar can't wear pieces between the legs yet.

With the panels left cut once, **Find seams** finds only the seven seams
between panels, and **Drape** says the pieces sewn go only half way round
the body, from a front to a back. Set **How many** to 2 on the panels and
press **Find seams** again: it adds the eight that were missing.

A project sewn with the earlier suggestions keeps its seams, and several of
them were wrong. Set the panels to 2, unpick the seams and **Find seams**
again.

Size C is made for a 57.5 cm waist, smaller than the avatar's: a corset
pulls the waist in. The avatar's body doesn't give way yet, so the corset
stretches round it.

**How it works:**

* *Lines that meet.* Where a line across a piece (bust, waist, hip,
  lengthen or shorten here) ends on an edge, it marks the edge. Two edges
  whose marks fall in the same places (two at least, and most of them,
  within 3 mm) are as likely sewn together as two edges as long as each
  other. Where one edge is longer, the shorter is sewn along the part of it
  where the marks meet. Marks at a corner, hatching, and marks evenly
  spaced (a ruffle marked in equal parts) say nothing; nor do a piece's
  copies (a lining, the other sleeve), whose lines lie where its own do.
  Between panels marked this way, a seam whose lines don't meet (one
  panel's top to another's) needs more than a matching length.
* *Centres.* A centre front or back may be shaped a little at the waist:
  up to 2% of its length off straight.
* *Along an opening.* A piece left over is sewn along a run of the
  garment's free edges as long as its own edge, across the centre seam
  there: a crotch piece along both front panels' lower edges. Only where
  just one run fits.
* *Lines named.* Words printed along a line (BUST, WAIST, HIGH HIP) name
  it.
* *Put on by the waist.* A ring of pieces hung from nothing, each with a
  line named for the waist, goes on with those lines at the avatar's
  waist: a corset is put on at the waist, not hung from its top.
* *Between the legs.* Pieces named crotch or gusset are left off the
  drape for now.
* *Half way round.* A ring of panels open from a front to a back goes only
  half way round the body, as a pattern drawn for one side and cut once
  does. **Drape** says so rather than drape half a corset.

---

## Step 32 — PDFs with pages of instructions, and sizes drawn in turn

Many PDF patterns start (or end) with pages of words: a warning, how to
print, a size chart. Those pages are now left out on their own, and you can
see which pages were read and choose others. Tiles go together where the
labels printed on them say ("1A", "1B" and, at each edge, the label of the
page that joins it there), and words printed over every page ("FREE", "Not
for sale") no longer name pieces.

Patterns in many sizes often draw them in a few line styles taken in turn:
size A black, B black dashed, C red, D red dashed, E black again. Each size
is now a size of its own, named by the letter on its line.

**Try it** (with a free corset pattern in 13 sizes, A to M, on 21 A4 pages,
the first of them its size chart and instructions):

1. **Open pattern** and choose the PDF. The dialog says **Found 10 pieces
   in 13 sizes** and **Read 20 of the 21 pages. Left out page 1 (1A):
   instructions, not pattern.**
2. **Choose pages…** lists every page with its label. Untick one and press
   **Read these 19 pages**: the pieces on it are gone. **Read as at first**
   brings them back.
3. The sizes A to M each show their line. Pick one: each piece is named as
   printed (Front Center, Back Side, Crotch Back… from under the piece's
   number), has its grainline, and the lines across it (bust, waist,
   shorten or lengthen here) stop at its own outline, though the pattern
   draws them once, out to the largest size. The modesty panel, one for all
   sizes, comes with every size.
4. In an AI assistant, `inspect_pattern_file` says which pages were read
   and which left out, and why, and `import_pattern` takes `pages`.

In the bought trousers, the belt-loop strip now has its two fold lines and
the marks where it is cut into five loops (they end on its outline, and were
left out before).

**How it works:**

* *Pages of words.* A page with five or more lines of four or more words is
  instructions, unless you chose it. A page that shares no lines with the
  others is no part of the sheet (a cover).
* *Labels.* Each tile prints its own label largest. The labels at its edges
  name its neighbours, which says whether the number counts rows or
  columns. Labelled pages join only their neighbours, on the side the labels
  say, as far apart as the lines they share say (tiles overlap or are
  trimmed); a tile that shares no line goes where its label says.
* *Sizes in turn.* The outlines of a piece nest, one per size, smallest
  first. When the line styles come round again in the nest, each outline is
  a size, named by the letter on its line.
* *Every size draws its whole outline*, the stretches the sizes share too,
  so a line drawn twice on one page is kept twice. A dashed outline's
  corners are marked by solid stubs, which carry its dashes on to the
  corner. Where a size's outline is partly drawn in another style (every
  size's crotch seam in black), that stretch closes it. Where the sizes
  meet at a corner they share, no outline turns back into another size's.

---

## Step 31 — A bought blouse found and draped

A blouse from a bought PDF now goes from import to the avatar with **Find
seams** alone. The pattern is laid out the way pattern CAD fits it on the
sheet: back and fronts lying on their sides, a cuff upside down. The blouse
has a back cut whole, a left and a right front gathered into narrow yokes
over the shoulders, and sleeves set into armholes made of three pieces and
gathered into cuffs. It buttons down the front and has facings.

**Try it** (with the blouse, one of the two real bought patterns):

1. **Open pattern**, choose the PDF and add its pieces.
2. In **Seams**, press **Find seams**. Twenty seams are suggested, each
   saying why. Besides the side seams, shoulders and facings:
   * each front is **gathered to fit** a yoke, the one on its own side;
     the zigzag drawn along the front's top edge marks the gathers;
   * each sleeve is **gathered to fit** its cuff;
   * each sleeve's head is sewn in three parts, each one that **fits round
     the opening**: the back's armhole, the yoke's end and the front's
     armhole. It is split where the sleeve's own notches are, and on the
     table each part's number sits on its own stretch of the sleeve's head;
   * the left front facing's short jog **goes on from a seam**.

   The two fronts' armholes are no longer suggested to each other.
3. **Sew all**, open the **Avatar** and press **Drape**. The blouse hangs
   from the shoulders: V neck, gathers under the yokes, full sleeves
   gathered into the cuffs at the wrists, buttoned down the front. The
   status line says the rouleau strip is left off (it is sewn to nothing)
   and the facings go inside.

A project sewn with the earlier suggestions keeps its seams, and two of
them were wrong: the fronts' armholes sewn to each other, and a yoke sewn to
its own other half. Start a new project from the PDF (or unpick its seams)
and **Find seams** again.

**How it works:**

* *Pieces stood upright.* Each piece is turned, a quarter turn at a time,
  so that its grain runs up and down the body and its hems are at the
  bottom. A hem is an edge marked as one, or with a deeper allowance than
  the seams (20 mm against 10 here). A piece lying on its side with no hem
  to go by is turned so that its grainline's arrow points down, as CAD
  draws it.
* *Yokes, open fronts, bands.* A piece sewn by two of its edges between
  pieces already placed, as a yoke is between a front and a back, is
  placed after them. It spans the gap, each of its points the mean of its
  neighbours, and counts as the shoulder they hang from. A ring of pieces
  open between two fronts meets itself at the centre front. A band sewn by
  one edge, such as a cuff, is turned to hang from what it is sewn to, and
  goes round the arm wherever along it its seam is.
* *Buttons.* Buttons marked along one front's open edge (button marks, or
  crosses) are held to the other front's buttonholes. Where it has none,
  as here (it buttons through loops), they are held to its edge at the
  same height.
* *Facings and linings* are left off the drape. Turned inside, they lie on
  what they face; with no cloth of their own to rest on, they would hang
  loose from their seams.
* *Find seams* tries each likely seam on a model of the garment as sewn so
  far: each piece's copies (a pair as two mirror images), which of their
  corners meet, and which way out each copy is. It passes over a seam that
  would pinch a piece, bringing two of its corners together through other
  pieces (as sewing the fronts' armholes together would, once the side
  seams are sewn), or that would need a copy both right side and wrong
  side out. Of seams as likely as each other, it takes the one joining
  corners nearest each other round the garment (a facing's end to the
  front facing on its own side), then the one closing most openings, then
  fewer seams, then pieces right side out. Gathered edges (a wavy line drawn
  along an edge, or an edge named so) are matched to shorter edges last,
  once the rest is put together: a sleeve to a cuff, or a front or a back to
  a yoke, likeliest. Last, each sleeve's head goes round what is left open
  of an armhole as long as it (within 6%), started where their notches
  agree, or else with the underarm at the side seam.

**Not yet:** "gather to 15.1 cm", written on the pattern, isn't read: the
gathers are matched by the zigzag and the lengths. A cuff's own buttons
aren't done up; it wraps the wrist as a closed band. A collar isn't set
into a neckline the way a sleeve is set into an armhole.

---

## Step 30 — Projector mode (Phase 4, part 9)

Project the pieces true size onto fabric on the cutting table, from a
projector above it, and cut without printing.

**Try it** (any project with pieces; with the iPad mirrored to a
projector, or on a second screen):

1. Press **Projector** in the top bar. The screen goes black, full
   screen, and shows a rectangle with a 10 cm grid and a corner handle at
   each corner.
2. **Calibrate:** mark a rectangle on the mat with tape, or use the mat's
   own lines, and enter its size (40 × 30 cm to start). Drag each corner
   handle until it sits on the marked corner on the table. A projector
   that isn't square on to the table is straightened by this too (the
   rectangle is drawn as the projector sees it). **Done.**
3. The pieces show in bright lines on black, which casts no light on the
   fabric: white cutting lines and notches, dashed cyan stitching lines
   (**Stitching lines** turns them off), yellow grainlines, and each
   piece's name. **Drag** anywhere to slide the pattern across the fabric.
   The pieces are laid out as on the cutting table, so arrange them there
   first.
4. The calibration is kept for this device and screen size. Next time it
   projects straight away; **Calibrate** does it again. **Close** or Esc
   leaves.

**How it works:** the four corners and the rectangle's real size give a
projective map (a homography) from the mat's millimetres to the
projector's pixels, solved from the four pairs of points. Every line is
drawn through it, so the scale and any tilt are taken out exactly.

**Not yet:** pieces can't be turned or moved in projector mode itself
(move them on the table first). There's no mirrored projection for a
projector shining through a mirror; most projectors can flip their own
picture.

---

## Step 29 — Your own fabric, measured at home (Phase 4, part 8)

Drape in the fabric you actually have, from a few measurements anyone can
take with a kitchen scale, a ruler and a table.

**Try it** (open the avatar on the test skirt or blouse):

1. In the drape bar, open **Fabric** and choose **Measure a fabric…**.
2. Give it a name, and choose the ready-made fabric it is **Most like**:
   anything you don't measure is taken from that one.
3. Measure what you can (each is optional):
   * **A 10 cm square weighs**: cut a 10 × 10 cm square and weigh it in
     grams.
   * **Along (or across) the grain, 10 cm stretches to**: cut a strip 5 cm
     wide, mark 10 cm on it, hang 500 g from it (a bag of sugar, pinned
     on) and measure the 10 cm again. A woven barely moves; a jersey
     might reach 13 to 16 cm.
   * **Slides out over a table's edge**: cut a strip 2.5 × 20 cm along
     the grain and slide it slowly over a table's edge until its end
     droops to 45° (a sheet of paper folded corner to corner shows the
     angle). Note how far it sticks out. A stiff denim goes further than a
     soft lawn.
4. **Save**: the fabric is chosen, listed under **Your fabrics**, and kept
   with the project. **Drape** says **Draped in Blue linen.** (or whatever
   you called it). **Change “…”…** in the same list edits or removes it.
   A garment on the avatar is draped again when its fabric changes.

**How it works:** the weight sets the cloth's mass. The pull on the strip
(500 g across 5 cm, 98 N/m) divided by the stretch gives its stiffness
along and across the grain. The overhang gives its bending stiffness by
Peirce's cantilever: half the overhang, cubed, times its weight. The rest
(how it gives on the bias, its thickness and friction) comes from the
fabric it's most like. Projects are now schema version 6; older ones open
with no fabrics of their own.

**Not yet:** a piece can't be given its own fabric; the whole garment is
draped in the one chosen. Fabrics aren't shared between projects.

---

## Step 28 — The drape follows the pattern (Phase 4, part 7)

A garment on the avatar now follows edits to its pattern by itself.

**Try it** (on the test blouse or skirt):

1. Open the avatar and **Drape** the garment.
2. Change the pattern: add an alteration (lengthen the front 10 cm, say),
   change one, switch one off, or sew or unpick a seam. A moment after the
   last change, the drape says **Draping it again to follow the change…**
   while the garment shown stays put. Once the new drape has settled
   (about two seconds for the blouse here), it takes the old one's place.
   Lengthened, the front's extra length is eased into the back's shorter
   side seams, and it shows.
3. Changes that don't alter the garment, such as moving a piece on the
   table or renaming the project, leave the drape alone. **Take off** stops
   it following.

**How it works:** the avatar pane keeps a key of what the garment is made
of: each piece's outline, allowance, cutting, grainline and darts, and the
confirmed seams. When the key changes, it drapes again 0.4 s after the
last change. The worker settles the new drape without showing it, so it
can spend more of each frame on it (40 ms rather than 12) while the view
keeps showing the old drape.

**Not yet:** each follow drapes from scratch, with the same sewing and
settling as a first drape. Starting from the last drape would be quicker,
but an altered piece no longer lines up with the one before, so it needs
more care. The WebGPU drape (step 23) is the other way to speed it up, if
the iPad needs it.

---

## Step 27 — Truing up (Phase 4, part 6)

Alterations leave jogs and corners in the edges they cut across.
Truing them up is now a step of its own.

**Try it** (on the test blouse's front):

1. Make the shoulders 1 cm broader (step 26). The shoulder seam jogs
   where it was slashed.
2. **Add a step → True up an edge**, then tap the shoulder seam. It is
   nearly straight, so it is trued **Straight**: one line from the neck to
   the shoulder's new end.
3. On an edge that curves, such as a hem after **Slash and spread**,
   truing is **Smooth**: each jog and corner is blended into a curve
   running 2.5 cm either side of it, leaving and arriving as the edge does
   there, and the rest of the edge is kept. Either choice can be switched
   in the step.
4. An edge's ends and darts stay as they are. Truing a side seam with a
   bust dart straightens it above and below the dart.

**Walking seams** is in the seams list as before: each seam's two sides
are measured as sewn, darts closed, with how far apart their notches
meet.

**Not yet:** a seam is trued on its own piece, not with the edge it is
sewn to (a hem across a side seam, say). A corner where two edges meet
isn't blended, since it's usually meant.

---

## Step 26 — Swayback, shoulders, waist and hip (Phase 4, part 5)

Four more alterations, each placed by tapping two points. The inspector's
**Alterations** now has a single **Add a step** menu listing every kind,
each with a line saying what it does.

**Try it:**

1. **Shoulders** (on the test blouse's front): tap the shoulder seam
   about halfway along, then the armhole where it hinges, level with its
   notch. The shoulder's end moves 1 cm further out along the seam. The
   piece is slashed down the grain from the shoulder to the hinge's
   level, then across to it, and the part with the shoulder's end turns
   about the hinge. **Narrower** takes it in instead. The shoulder seam
   jogs where it was slashed, for truing later.
2. **Swayback** (on the test skirt's back, `fixtures/drapewright/skirt.drapewright`):
   tap the centre back where the wedge comes out, then the side seam at
   the same height. A 1.5 cm wedge is folded out at the centre back,
   tapering to nothing at the side. A back cut on the fold is refused:
   it needs a centre back seam to take the wedge out of.
3. **Waist** (on the skirt's front): tap the side seam at the waist, then
   at the hip. The side seam moves out 1 cm at the waist, tapering to
   nothing at the hip, and the waist stretches to meet it. **Hip** works
   the other way round: out from the hip down to the hem, tapering to
   nothing at the waist. Each side seam moves this much, so a front and a
   back, each cut on the fold, change the whole skirt by four times as
   much (the step says so).

**Not yet:** seams left with a jog or a corner (shoulders, swayback,
lengthening) aren't trued; that's step 27.

---

## Step 25c — Pivoting a dart (Phase 4, part 4)

Move a dart to another place on the piece: all of it, or half.

**Try it** (on the test blouse, `fixtures/drapewright/blouse.drapewright`):

1. Give **Blouse front** a full bust adjustment, as in 25a: it now has a
   dart in its side seam.
2. Under **Alterations**, press **Move a dart**. The front has only one
   dart, so the bar at the top left asks just where it goes: tap the hem,
   a little in from the side. The side seam closes up straight and the
   dart opens in the hem instead, as wide, its tip where it was. On a piece
   with more than one dart, it first asks which: tap it.
3. **Half** moves half of it: half stays in the side seam, narrower, and
   half opens in the hem.
4. Like any step, it can be switched off or removed. Draped, both darts
   are sewn closed.

**How it works:** the piece is slashed straight from where you tapped to
the dart's tip, and the smaller part turns about the tip until the dart
closes. The slash opens by as much, and that is the new dart. Moved whole,
the dart keeps its name, so a later step can find it and move it again.

**Not yet:** a dart turns about its own tip, not the bust point. A side
dart moved to the waist therefore points where the side dart did (2.5 cm
from the bust point, towards the side). A dart's tip can't be moved or
its length changed yet. After a pivot, the edge the turned part was on
has a corner in it, for truing (step 27).

---

## Step 25b — Darts known as darts: measured and sewn closed (Phase 4, part 3)

The dart a full bust adjustment opens is now known as a dart: seams are
measured with it closed, and the drape sews it closed.

**Try it** (on the test blouse, `fixtures/drapewright/blouse.drapewright`):

1. Give **Blouse front** a full bust adjustment, as in 25a. Its side seam
   still reads **Same length, 38.5 cm** in the seams list: measured as
   sewn, with the dart closed, it is as long as the back's. (In 25a it read
   much longer, its dart's legs counted in.)
2. Open the avatar and **Drape**. The dart is sewn closed, leg to leg from
   the side seam in to its tip, and the side seam is sewn to the back past
   it: the place where the dart closes meets the right place on the back.
   The blouse hangs as evenly as before, with more room over the bust.
3. The avatar view now keeps its size. The drape's progress, what was read
   where you tapped, and a heatmap's key show over the foot of the view.
   The bar under it stays the same height: **Show the cloth as** is always
   there, greyed out until something is draped. Before, a reading made the
   view shorter, so the figure moved under your finger.

**How it works:** a piece keeps its darts in the file (`darts`, §4.14 of
the file format). Each names the edge it opens in and where its legs meet
that edge. Seams are measured along the stitching line with each dart's
span taken out, so notches beyond a dart meet where they should; seam
suggestions measure the same way. In the drape, each dart becomes a seam
of its own, one leg sewn to the other from its mouth to its tip. A seam
that runs past a dart is split there, and its other side is split where
the dart closes.

**Not yet:** darts drawn in imported patterns (as V notches or cut-outs)
aren't recognised as darts. A dart can't yet be added or moved by hand;
pivoting a dart to another place comes next.

---

## Step 25a — Slash and spread; full and small bust adjustments (Phase 4, part 2)

Two more kinds of alteration, each placed by tapping points on the piece.

**Try it** (on the test blouse, `fixtures/drapewright/blouse.drapewright`):

1. **Full bust adjustment:** tap **Blouse front**, then under
   **Alterations** press **Bust adjustment**. The bar at the top left of
   the table says what to tap: the bust point; the armhole where it
   hinges, about a third of the way up from the underarm; the side seam
   where the dart goes. Points on the edge snap to it, and the slashes
   show as you go. The front gains 2 cm across the bust on this half: the
   side turns out about the armhole, a dart opens in the side seam with
   its tip 2.5 cm from the bust point, and the centre front below the bust
   drops to meet the side at the waist. **Small bust** takes room away
   instead (no dart: the parts overlap).
2. **Slash and spread:** on the back, press **Slash and spread** and tap
   the hem where the slash starts, the edge across the piece where it
   hinges (say the shoulder seam), then the part that turns. The hem opens
   2 cm, the side swinging out about the hinge: a flare. **Close** overlaps
   it instead; **Turn the other side instead** swings the other part.
3. As with lengthening, each step can be switched off, changed or removed,
   and **Cancel** stops placing one.

**How it works:** every alteration now shares one way of working. It cuts
the piece's curves where they cross its slashes, moves each part (turned,
moved or folded), and joins the parts where they no longer meet: straight
across a gap; where they cross if they overlap; or with a dart where the
change says so. The bust adjustment works out from the bust point which
part each point of the piece is in.

**Not yet:** the new dart is part of the side seam's edge, so the seams
list counts its legs in the seam's length (showing the front's side seam
much longer than the back's), and the drape would sew its legs to the back
instead of closing it. Darts known as darts, measured and sewn closed,
come next (25b). The bust point and slashes are placed by tapping; they
cannot yet be dragged.

---

## Step 24 — Alterations, kept as steps: lengthen and shorten (Phase 4, part 1)

Change a piece's length without losing it as drawn: each change is a
step that can be switched off, changed or taken away, and undone.

**Try it:**

1. Open the test blouse (**Open pattern**,
   `fixtures/drapewright/blouse.drapewright`) and tap **Blouse front** in
   the pieces list.
2. Under **Alterations**, press **Lengthen or shorten**. The front is
   lengthened 1 cm straight across its middle: the part below moves down,
   an orange dashed line shows where, and the outline as drawn stays
   behind it, finely dashed. The pieces list says 61.0 cm; the seams list
   says the side seam's front is now 1.0 cm longer than its back.
3. In the step: **Lengthen** or **Shorten**, **By how much**, and **From
   the top** (where the line is). A piece with its own lengthen or shorten
   line (from an imported pattern) uses that line, with **Where** to
   choose another.
4. The switch on the step leaves it out (the piece is as drawn) or puts it
   back; the bin takes it away. Undo works as for any change.
5. Lengthen the back alike and the side seam matches again; drape it to see
   the longer blouse.

**How it works:** each piece keeps its alterations in order, and its shape
as drawn beside them; what is shown, printed, sewn and draped is that shape
with the steps that are on applied. Lengthened, the piece is cut along the
line and spread apart, a side crossing the line bridged straight across the
gap. Shortened, a tuck is folded out below the line, and a slanted side
jogs along the line, to be trued up (step 27). Notches stay on their edges'
cloth; marks, labels, internal lines and the grainline move with their
part. Projects are now schema version 5; earlier ones open with no
alterations.

While a piece is altered, changes that say where (a notch's place, a
grainline, splitting an edge) are refused with a plain message: switch the
alterations off to make them on the piece as drawn. Changes that say which
(an edge's kind, a corner, its allowance) are made as drawn and altered
again.

**Found on the way:** with a long project name, the top bar on a 1024-pixel
screen was wider than the window, and the hidden inputs inside the side
panel's switches made the page scroll. The name now gives way, and the
panel keeps its hidden inputs to itself.

**Not yet:** one step for the front and back together (each piece has its
own for now); placing the line by dragging it; altering over the AI
assistants' connection (MCP); full and small bust, swayback, shoulders,
waist and hip (steps 25 and 26).

---

## Step 22 — Where it pulls, and the room it has (Phase 3, part 9)

Heatmaps on the draped garment, and numbers where you tap.

**Try it:**

1. Drape something (the test skirt: **Open pattern**,
   `fixtures/drapewright/skirt.drapewright`, **Avatar**, **Drape**).
2. Under the figure, **Show the cloth as**: **Fabric**, **Pulling** or
   **Room**.
   * **Pulling**: pale where the cloth hangs freely, yellow, orange and
     red where it is pulled hard. On the test skirt, which is cut a
     little small for this figure at the hip, there is a band of red at
     the waist and the hips, easing to pale towards the hem.
   * **Room**: deep blue where the cloth rests on the body, paler where
     it stands away, white from 4 cm.
   A legend beside it says what the colours run from and to.
3. Tap the cloth: the line under it says what it is like there, for
   example "Here it is pulled firmly (48 N/m), stretched 1.6%, and resting
   on the body."
4. Try the same skirt in jersey: much the same stretch, but far less pull.
   A knit gives where a woven pulls.

**How it reads the cloth:** each triangle of cloth is compared with its
shape as cut. Its greatest stretch, and the fabric's stiffness that way
(firm along the grain, giving on the bias), say how hard it pulls, in
newtons per metre of cloth. Each point takes its triangles' mean, evened
out a little with its neighbours'. Room is the body's distance field at
each point, less the cloth's thickness. A tap casts the line of sight
from the eye through the cloth and reads where it first meets it. The
view keeps the cloth as last seen, so it can be shown another way after
the drape has finished.

**Tests:** cloth stretched evenly across the grain, along it and on the
bias reads exactly its stretch and pull; cloth turned or squeezed reads
none. Room over a ball is its distance less the cloth. A line of sight
finds the skirt from in front and from behind, and nothing above it. In
the browser the heatmaps change the view and a tap reads the skirt.

**Not yet:** where the cloth bunches (squeezed: wrinkles); the drape
following each change to the pattern (step 28).

---

## Step 21c — A blouse, its sleeves round the arms (Phase 3, part 8)

The drape now takes a garment hung from the shoulders, with set-in sleeves.

**Try it:**

1. **Open pattern** and choose `fixtures/drapewright/blouse.drapewright`: a
   loose blouse, front and back each cut on the fold, and a short sleeve
   cut as a pair. Its seams are sewn: shoulders, sides, underarms, and each
   sleeve's cap into its armhole, the back half to the back and the front
   half to the front (the seams list shows the cap eased in, 6 mm at the
   back and 10 mm at the front).
2. Press **Avatar**, then **Drape**. It hangs from the shoulders, a sleeve
   round each arm, in about a second on a computer.
3. Turn the figure round to see the sleeves and the back.

**How sleeves go on:** the garment's trunk goes on first. Then a ring of
cloth sewn to it near a shoulder (a sleeve, closed by its underarm seam)
goes round that arm. It is wrapped round a smooth line down the middle of
the arm, from inside the shoulder through the elbow to the wrist, its cap
over the top of the arm. It is turned and slid along the arm until its
seams lie nearest the armhole they are sewn to.

**Found by testing:** the first blouse hung crooked: its hem 11 mm lower
on one side, and a fold at one side of the waist. Mirroring the body, or
the order the cloth was solved in, did not move it. Mirroring the blouse
itself did. A few of the triangles the cloth is made of had their
diagonals the other way on one side of the fold, and that side gave
differently. A piece cut on the fold is now meshed one half at a time,
the other half its mirror image; the second of a pair is the first's
mirror image. Garments cut alike on both sides now hang alike, to a
tenth of a millimetre. Two more things were evened out on the way: the
cloth's points sit evenly about each piece's middle, and the distance
field round the body is centred on it. Triangles of no area, along
straight edges, are left out of the mesh.

**Not yet:** trousers (legs); long sleeves are placed but not yet tried;
a fitted bodice with darts.

---

## Step 21b — Draping in the app (Phase 3, part 7)

Sew the pattern's pieces on the avatar in a chosen fabric, and watch it
settle.

**Try it:**

1. Open a project whose pieces are sewn together (**Seams**). To try it
   straight away, open the test skirt: **Open pattern** and choose
   `fixtures/drapewright/skirt.drapewright` (a front cut on the fold and a
   back cut as a pair, its seams sewn).
2. Press **Avatar**. Under the figure: **Fabric** (poplin to start) and
   **Drape**.
3. Press **Drape**: "Sewing the seams…", then "Letting it settle…", then
   "Draped in poplin." A skirt takes a couple of seconds. The figure can be
   turned and zoomed all the while.
4. Choose another fabric while it is on: it is draped again in that one.
5. **Drape again** after changing the pattern; **Take off** takes it off.
   Choosing someone else's figure takes it off too, as it no longer fits.
6. Pieces sewn to nothing are left off, and it says so; with nothing sewn
   at all it asks for the seams first.

**Found by testing:** on CI's Linux computers the skirt from step 21a never
settled; on a Mac it did, by the luck of rounding. Each row of the cloth
was corrected in the same order, across from one side, at every step. A
garment placed exactly symmetrically was therefore nudged the same way
every time, and it crept round the body for ever. Now the cloth is
corrected from the top down in 1 cm bands, in a scrambled order within
each band, and no point moves faster than 1 m/s: placing the pieces round
the body strains them, and that strain could fling them. The skirt now
settles in about two seconds of cloth time in all 12 fabrics, however its
start is nudged. A test checks that it stays put once settled.

**What's under the hood:** the page sends the project's pieces and seams,
the fabric and the figure to the sim worker. The worker meshes, places and
sews them (`drapewright-sim`) and moves the cloth on for up to 12 ms each
frame before drawing it. So the view stays smooth, and a fast computer
drapes faster than real time. The cloth is a second surface in the
renderer, lit on whichever side faces you, its points rewritten every
frame. The test skirt is written as a project by a Rust test, so it cannot
drift from the one the drape tests use. End-to-end tests drape it in
Chromium (WebGL2) and WebKit (WebGPU), in poplin and then denim, and take
it off. The 3D module is now 2.7 MB (1.1 MB compressed).

**Not yet:** the drape following each change to the pattern (step 28 makes
it live); sleeves and trousers placed round arms and legs; where the
garment is tight or loose (step 22).

---

## Step 21a — Draping: the engine (Phase 3, part 6)

The cloth simulation that drapes a pattern on the avatar, as a library for
now: in the app with step 21b.

**Try it** (from the command line):

```bash
cargo run --release -p drapewright-sim --example drape_picture -- assets/body/adult.dwbody skirt poplin
```

It drapes a skirt cut for the standard figure (the front cut on the fold,
the back as a pair with a centre back seam) and draws it from the front and
the side three times: `skirt-placed.png` (the pieces round the body),
`skirt-sewn.png` and `skirt-settled.png`. Any of the 12 fabrics can be
given: cottonLawn, poplin, linen, chambray, denim, twill, crepe, satin,
jersey, ribKnit, woolSuiting, fleece.

**How it drapes:**
* Each piece is laid out as cut: a pair as two mirror images, a piece on
  the fold unfolded whole. Its edges are sampled so both sides of every
  seam have matching points. It is then meshed in near-equilateral
  triangles 2 cm across (constrained Delaunay).
* The pieces go round the body as a ring, in the order their upright seams
  join them, the front's middle in front. They are wrapped row by row, so
  an A-line skirt goes on as a cone and its seams meet at every height. A
  ring with shoulder seams hangs from the shoulders, another from the
  waist or from the ring it is sewn under.
* The cloth is XPBD in small steps (1/1800 s). Each fabric stretches
  firmly along warp and weft and gives on the bias, and bends (Bergou's
  isometric model). The stitches tighten over the first second as the
  seams close.
* The body keeps the cloth off as a distance field (8 mm cells), its
  points and the middles of its triangles kept 3 mm clear. Friction holds
  cloth where it rests on the body.

The skirt settles in about five seconds of cloth time: hanging from the
waist to well below the hip, seams closed, nothing inside the body. Tests
check a hanging curtain, a cloth falling on a ball, two panels sewn round a
post, and the skirt on the avatar.

**Not yet:** in the app (21b); sleeves and trousers placed round arms and
legs; a pattern's own pieces and seams draped (21b); the drape's speed
(about real time on a computer for a skirt; step 23 moves it to the GPU).

---

## Step 20 — The avatar in 3D (Phase 3, part 5)

See the avatar, in someone's figure, beside the cutting table, and turn it
round.

**Try it:**

1. Press **Avatar** in the top bar. A 3D view opens beside the table
   (under it on an iPad held upright). With no measurements yet it shows
   a standard figure.
2. Add someone's measurements (**Measurements**): the avatar takes their
   figure, and the view's heading says how close it comes ("All 4
   measurements within 1 cm"). With several people, choose whose figure in
   the heading.
3. Turn it: drag with a finger, the Pencil or the mouse. Two fingers slide
   it; pinch, or scroll, to zoom. Double-tap (or **Show the whole figure**,
   or 0) to see it whole from the front again. With the keyboard: the arrow
   keys turn it, + and − zoom.
4. Close it with the cross, or **Avatar** again.
5. In a corner: "Drawn with WebGPU", as on an iPad with iPadOS 26, or
   "Drawn with WebGL2" where there is no WebGPU.

**Found by testing:** with the new button the top bar was wider than a
1024-pixel screen (an iPad held upright), so the page scrolled sideways.
On narrower screens the buttons are tighter now, and below 900 pixels they
show their icons alone.

**What's under the hood:** a `render` crate draws with wgpu: the figure
with smooth normals, lit softly from above with a faint rim, a shadow on
the floor, four samples a pixel where the device allows. `wasm-3d` adds
the view: the page hands its canvas to the sim worker
(`transferControlToOffscreen`), which draws there, through WebGPU where
there is one and WebGL2 where not, once a frame however many changes come.
Turning and zooming go through the same input router as the cutting table
(`orbit-input.ts`). The shader is checked in the tests without a GPU,
translated for WebGL2 too (naga), and an end-to-end test sees the figure
drawn and turned in Chromium and WebKit. The 3D module is now 2.5 MB
(0.9 MB compressed), loaded only when the avatar is first wanted.

**Not yet:** clothes. The drape (step 21) comes next: the pattern's pieces
sewn on the avatar in a chosen fabric.

---

## Step 19b — The avatar in the app (Phase 3, part 4)

The avatar comes into the Measurements panel: fit it to a person's
measurements and see how close it comes, measurement by measurement.

**Try it:**

1. Open **Measurements**, add a person, and enter at least height, bust,
   waist and hip (in centimetres, say 165, 91, 71 and 97).
2. Under **The avatar**, enter the person's age if you like (kept with
   their measurements; 35 if left empty), and press **Fit the avatar**. The first time, the 3D side and the body file load
   (4 MB); after that, fitting takes about a second.
3. A table lists each measurement given: as measured, the avatar's, and
   the difference ("Same", "0.7 cm larger", "1.2 cm shorter"). "The avatar
   matches all 6 measurements to within 1 cm", or how many of them; any
   further off are marked, as close as the avatar's figure can be shaped.
4. Change a measurement: the report says it has changed since the avatar
   was fitted, until **Fit again**.
5. A man's figure leaves out the bust's shape, as the form does. A
   measurement of your own (from an AI assistant, starting "x-") is said
   to be not on the avatar.
6. Offline: once fitted, the body file is kept, and fitting works without
   a connection. Offline before the first fit, it says the avatar's figure
   hasn't been downloaded yet.

AI assistants get `fit_avatar`, the same report for a person's
measurements; `set_measurements` takes their age.

**What changed in the file:** schema version 4: a person's measurements
keep their age (`age`, in years), for the avatar's shape. Earlier files
open with it unknown.

**Not yet:** the fitted avatar isn't kept with the project: it is fitted
again when wanted. Seeing it in 3D comes with step 20.

**What's under the hood:** a second WebAssembly module, `crates/wasm-3d`
(226 KB, built for speed), runs in its own worker (`sim.worker.ts`), which
starts the first time the avatar is wanted, so the 2D app loads exactly as
before. The worker fetches the body file (`?url`; the service worker keeps
it on first use) and fits with `drapewright-body`. In the app,
`AvatarFitter.tsx`; for assistants, `fit_avatar` in `crates/mcp`, with the
body file built into the server. `pnpm wasm` builds both modules.

---

## Step 19a — The avatar: shaped, measured and fitted (Phase 3, part 3)

The body patterns will be draped on, as a library: shaped from
MakeHuman's data, measured as a tape measure would, and fitted to a
person's measurements. It comes into the app with step 19b ("Fit the
avatar" in the Measurements panel), and is seen in 3D with step 20.

**Try it** (from the command line):

1. A picture of a figure, front and side, showing where each measurement
   is taken; it prints them all:

   ```bash
   cargo run --release -p drapewright-body --example picture -- assets/body/adult.dwbody figure.png weight=0.7 cup=0.75
   ```

   Settings run from 0 to 1: `woman`, `weight`, `muscle`, `height`,
   `proportions`, `cup`, `firmness`; `age` is in years.
2. A figure fitted to measurements, in cm:

   ```bash
   cargo run --release -p drapewright-body --example fit -- assets/body/adult.dwbody woman height=165 highBust=86 bust=91 underbust=75 waist=71 hip=97 backWaistLength=41 armLength=58
   ```

   It prints the settings found and how close each measurement comes
   ("wanted 91.0 cm, figure 91.0 cm"), this one in about half a second.
3. `export_obj` writes a figure for any 3D viewer.

**How it measures:** as a tape would. Round measurements go round the
outside of a slice, bridging the hollow between the breasts:
* the bust at the bust point;
* the high bust level across the back under the arms, and rising in front
  over the chest above the breasts, so the gap between bust and high bust
  is the one a full bust adjustment works from (about 3 cm on the average
  figure, 12 cm on the fullest cup);
* the underbust at the fold under the breasts, also where a full breast
  hangs;
* the waist at the narrowest, or level with the elbow on a figure with no
  waist;
* the hip at the seat's fullest.

Lengths follow the body's back and front.

**How it fits:** first weight and muscle on a coarse grid (they shape a
figure together, and unevenly); then height, weight, muscle and cup size
to the main measurements; then all of those with MakeHuman's measure
shapes to every measurement given. A measurement the figure cannot reach
is left short, and said to be, without pulling the others away.

**How close:** a commercial pattern size chart from size 6 to 30W, and
men's chests from 36" to 48", fit to within 2 mm on bust, waist, hip and
height; with eight measurements, usually within half a centimetre. A B
cup's high bust (5 cm under the bust) is reached up to size 22. At 26W and
30W the figure's frame is not broad enough for it: the high bust is left
3 to 10 cm short, and the bust can come up to 3 cm short with it.

**Not yet:** in 3D (step 20); in the app since 19b. The figure stands as
MakeHuman's does, arms held out, so across front and back are
approximate. The model's extremes (the heaviest figures with a flat
chest) measure less well.

**What's under the hood:** `crates/body`: the body file (`asset.rs`),
shaping (`shape.rs`), measuring (`measure.rs`) and fitting (`fit.rs`).
`assets/body/adult.dwbody` (4.4 MB) is built by the `build_body` example
from Anny's copy of MakeHuman's data: CC0 data, and Anny's method under
Apache-2.0 (see `assets/body/README.md` and `docs/licences.md`).

---

## Step 18 — Seams (Phase 3, part 2)

Which edges are sewn together. Seams are useful already in 2D, for walking
a pattern's seams: both sides' lengths on the stitching line, the
difference, and whether the notches meet. The drape (step 21) needs them
to know what to stitch.

**Try it:**

1. Open a pattern with several pieces, such as
   `fixtures/pdf/freesewing-sasha-a0.pdf`, and add its pieces.
2. In the side panel, under **Seams**, press **Find seams**. Drapewright
   lists the edges that look sewn together, how likely each pair is and why
   ("same length · notches match · hems meet"). **Sew** one, **Sew all**, or
   **Not this** to hide one.
3. Each seam gets a number, shown in a badge inside both of its edges on
   the table, with a band along them. In the list: both sides' names, and
   "Same length, 50.5 cm" or "9.8 cm and 10.0 cm: the second side 0.2 cm
   longer", and how the notches meet.
4. Tap a seam in the list to pick it (its edges are drawn stronger), then
   **Turn round** (start to end instead of start to start) or **Unpick**.
5. By hand: tap an edge, press **Sew to another edge…** in the panel, then
   tap its partner. Drapewright turns it the right way (hems meet hems,
   notches meet notches). Tap the same edge again to sew it to its other
   half, for a back cut as a pair with a centre back seam. Escape cancels.
6. The edge's panel says what it is sewn to. Undo takes back anything.

**How suggestions are made:** pairs of edges are scored on how close their
lengths are (pattern CAD trues seams to the millimetre), whether they curve
alike, whether their notches match in kind and place, their names, whether
hems (or folds) meet at the same end, and whether one is a front and the
other a back. Then:
* edges that only run alongside seams already found (a front's and a
  back's armholes, each sewn to the sleeve) are left out;
* a piece's own left and right edges are never paired (they match
  perfectly but are not sewn together), except on a sleeve or a leg, which
  are tubes;
* left and right versions of a piece need notches or names to pair;
* a piece cut as a pair sewn to one drawn whole (a yoke to the back) is
  sewn to both of its shoulders, one by each half.

On the two real bought patterns: the blouse's side seams, shoulders,
sleeve underarm, neck facing and front facings are found (11 seams); the
trousers' three leg seams.

**What changed in the file:** schema version 3, with `seams`. Lengths are
never stored. Adding or removing a corner on a sewn edge is refused until
its seam is unpicked, so what is sewn to what never changes silently.

**What's under the hood:** `crates/seams` (the model, measuring, and
suggesting), `crates/pattern-core/src/stitching.rs` (each edge's own part
of the stitching line, moved in by its allowance where the outline is the
cutting line), three commands in `crates/project`, and in the app
`SeamList.tsx`, the badges in `renderer.ts` and sewing by hand in
`session.ts`. AI assistants get `get_seams`, `sew_edges` and
`unpick_seams`. `import_report -s` lists suggested seams for a folder of
patterns.

**Not yet:** a sleeve cap sewn to the front and back armholes together
(one edge to two) is not suggested; sew each part by hand with parts of
edges once the panel offers ranges. Seams are not yet drawn as lines
between the pieces.

---

## Step 17 — Measurements (Phase 3, part 1)

The first step of Phase 3 ([plan](plan-phase-3-4.md)): people's
measurements, kept with the project. The avatar, the ease checks and the
alterations of the steps to come all start from them.

**Try it:**

1. Press **Measurements** in the top bar, then **Add my measurements**.
2. Fill in what you have; only height, bust, waist and hip are needed to
   start. Each box says how to take that measurement, and boxes are in the
   project's units (cm, mm or inches).
3. Type a waist of 780 while the project is in centimetres: the form says
   it is unusual for a waist and to check the units. Make it 78.
4. Switch the figure to **Man's**: bust becomes chest, and the bust's own
   measurements (high bust, underbust, bust span, shoulder to bust point)
   go away. Values already typed are kept.
5. **Measure again** starts a new measurement dated today; the earlier one
   is kept with the person (shown as "1 earlier measurement kept").
6. **Add a person** for each client. Undo takes back any change. Reload:
   it is all still there, saved with the project.

AI assistants (step 12) get two tools: `get_measurements` and
`set_measurements`.

**What changed in the file:** projects are now schema version 2, with
`measurementSets`. Projects saved before open as before (they have no
measurements); a cached older app refuses version 2 files rather than
dropping the measurements when it saves.

**What's under the hood:** `crates/project/src/measurements.rs` (the 27
measurements, their usual ranges and FreeSewing names, and the sets),
four commands in `command.rs`, the migration in `migrate.rs` with a golden
version 1 file (`crates/project/tests/fixtures/project-v1.json`), and
`web/src/app/MeasurementsDialog.tsx`. The file format is in
[file-format.md §10.2](file-format.md).

**Not yet:** pictures beside the how-to text; importing a FreeSewing or
Seamly2D measurements file.

---

## Step 16 — Reading lettering drawn as strokes (Phase 2, part 4)

PDFs printed through a plotter driver (step 15) draw their words as pen
strokes, so their pieces came in as *Piece 1*, *Piece 2*… The import now
reads that lettering: each piece gets its name and how many to cut from
the label the pattern program printed on it.

**Try it:**

1. The made-up plotter drawing now has names, drawn in a plain hand of its
   own (not the one the importer knows):

   ```bash
   PLOTTED_PDF=plotted.pdf cargo test -p drapewright-io --test pdf_plotter
   ```

   The front comes in as **Left**, cut 4, and the back as **Fly**, cut 2,
   read from lettering turned on its side. The strip, with only a logo on
   it, stays *Piece 3*. The A4 tiles (`PLOTTED_TILES=tiles.pdf`) give the
   same names.
2. On the two real bought patterns (kept out of the repo), every piece is
   named, in the A0 and the A4 files alike:
   * the blouse: Front yoke (cut 4), Sleeve (cut 2), Back, Back neck
     facing, Rouleau button loops (cut 2), Right front, Left front, Cuff
     (cut 4), Right front facing, Left front facing;
   * the trousers: Back leg and Front leg (cut 2 each), Belt loops (5pc),
     Fly shield (cut 1 on the fold, its fold edge marked from the word
     "fold" printed along it), Fly extension (cut 1 in interfacing),
     Waistband, Pocket bag (cut 2).

**How it reads:**
* Letters are strokes that touch, and lines are letters side by side. The
  block is turned whichever of the four ways reads best, and each line
  gets its own baseline, so lettering a hair off straight still reads.
* Each letter is compared with a stroke font of technical lettering, both
  ways round: how far the drawn strokes are from the font's, and the
  font's from the drawn ones. A missing stroke (the side of an O, which a
  C lacks) counts for more than a slightly different hand.
* Lettering in small letters only ("seam") is read at its own size.
* Letters drawn alike but for size (c and C, o and O) take the case of
  their word; O and 0, l and 1 are letters in words and digits in numbers.
  A word one letter off a word patterns use ("RCULEAU") is set right.
* The name is the line above "cut …" in the label, or else a line of
  capitals read closely. No other lettering names a piece: strokes read as
  letters can be anything (a logo, hatching, a page's footer).
* A "fold line" (folded in sewing, as on a cut-on fly) no longer marks the
  edge near it to go on the fabric fold. "Fold" and "on fold" still do.

**Found by testing:**
* Notch cuts beside lettering were taken into it and lost. A stroke from
  one line to another, or straight and long from one, is now a notch cut,
  so the blouse's buttonhole ticks all come in. But a stroke lying along a
  line (an asterisk's, at a dart's point) or a dot sitting on one (an i's)
  stays lettering.
* A single stroke too long to be a letter, scaled up for matching, ran out
  of memory. Strokes are now capped in length for matching.
* The word "foldline" beside the trousers' cut-on fly marked the front
  leg's edge as a fold.

**Not yet:**
* A word crossed by a line drawn dash by dash ("seam" under a centre line)
  reads as nonsense, because the dashes are taken for letters. It names
  nothing. Joining dashed lines is a later step.
* Measurements printed on the pattern (the finished sizes table) are read
  but not used.

**What's under the hood:** `crates/io/src/pdf/strokefont.rs` (the font),
`reading.rs` (letters, lines, which way round, matching, words) and
`drawn_labels` in `import.rs` (which line is the name). Tests are in
`reading.rs` and `crates/io/tests/pdf_plotter.rs`.

---

## Step 15 — PDFs printed from pattern CAD through a plotter driver (Phase 2, part 3)

Some pattern companies draw in pattern CAD (Grafis, for example) and make
their PDFs by "printing" to a plotter driver. Such a PDF has no text and no
shapes, only thousands of short pen strokes: every line, every word and
every logo is strokes. These now import.

**Try it:**

1. Write the made-up plotter drawing the tests use, then open it:

   ```bash
   PLOTTED_PDF=plotted.pdf cargo test -p drapewright-io --test pdf_plotter
   ```

   It has three pieces, each drawn the way such files draw things:
   * **A front with a cut-on fly.** The allowance is 4 cm along the top of
     the fly edge and 1 cm below. The import breaks that edge where the
     allowance changes, so each part keeps its own allowance, and the
     cutting line comes out exactly as drawn.
   * **A back whose stitching line forks** round an extension above the
     hem. The outer way round becomes the stitching line, and the seam's
     way on to the hem stays as a line. Its dart ends in a circle: the
     circle is a mark, and the dart is not mistaken for the grainline.
   * **A bias strip** whose stitching line is drawn on three sides only. The
     fourth edge gets no allowance, as drawn, and the grainline is at 45°.
   * The words and the brand logo on each piece are left out, and the
     import says so. The logo is drawn at three sizes. The round "+"
     outlines become button marks.
2. The same drawing printed on A4 the way plotter drivers tile it:

   ```bash
   PLOTTED_TILES=tiles.pdf cargo test -p drapewright-io --test pdf_plotter
   ```

   Eight pages, each with a frame and only its own part of the drawing,
   to trim at the frame and butt together. The import says **8 pages
   joined** and gives the same three pieces as the one sheet.
3. On two real bought patterns (kept out of the repo), each sold as an A0
   copy-shop file and as A4 pages (36 and 24):
   * the blouse gives 10 pieces, 9 with their stitching lines (the 10th has
     none drawn), all with grainlines, and the 12 buttons;
   * the trousers give 7 pieces, 5 with stitching lines (the other two have
     none), all with grainlines, the fly's end, the dart's point and the
     waistband button;
   * the A4 files give the same pieces as the A0 ones.

**How the strokes become outlines:**
* Strokes join end to end. Where several lines meet, the outline goes on
  the way that turns least, as a pencil would. A notch cut stops where it
  meets the outline it marks.
* Hairline gaps a plotter leaves between strokes (under half a millimetre)
  are bridged.
* Lines that meet end to middle are seen as a network: a seam line running
  on past a corner into the allowance, a fork, notches joining the cutting
  line to the stitching line. Loose ends are trimmed. The outline is the
  network's outer edge, peeled off one after another: the cutting line,
  then the stitching line inside it.
* Lettering is clusters of tiny strokes, including short words a space
  apart ("to") and a dash between words. Logos are the same intricate
  letter shapes repeated on the pieces at any size, with the dots beside
  them.
* **Pages to trim and butt:** each page has a frame drawn round its part
  of the drawing, and no shape is on two pages. A line cut off at one
  page's frame carries on from the next page's frame, so the page grid
  (how many across, rows from the top or the bottom) is the one under
  which the most lines carry on. Then each line's two ends are stitched
  together across the join. Footers touching every frame are ignored.
* Arrowheads drawn as triangle outlines (as big as 3 cm) make grainlines,
  even when "grainline" is written right beside them. A piece's grainline
  is its longest arrow. Shorter ones show which way to press a dart.

**Not yet:**
* **Piece names and cutting counts** (step 16): they are drawn as strokes
  too, so pieces are named *Piece 1*, *Piece 2*… The import says so.
* An allowance that curves from one width into another (the end of the
  real trousers' fly) becomes a square step, and the import gives the
  largest difference from the drawn cutting line (9 mm there).

**Found by testing:** a rectangular piece bigger than 80% of an A4 page,
printed across several tiles, was taken for a page's frame and left out.
A frame must now lie on its page.

**What's under the hood:** `crates/io/src/pdf/butt.rs` (frames, the page
grid, stitches), `loops.rs` (the network of lines and its outer edges),
`lettering.rs` (words, logos, round "+" marks), the stepped allowances and
three-sided stitching lines in `assemble.rs`, and triangle arrowheads in
`marks.rs`. The test `crates/io/tests/pdf_plotter.rs` draws each case as a
plotter would, on one sheet and on butted A4 tiles.

---

## Step 14 — PDF patterns in several sizes (Phase 2, part 2)

**Try it:**

1. Open `fixtures/pdf/sizes-colours.pdf`, a front bodice and a sleeve in
   sizes 8, 10 and 12, nested as printed patterns are, each size in its
   own colour. The import says **Found 2 pieces in 3 sizes** and asks
   **Which size?**. Each choice shows its line as printed (red 8, green
   10, blue 12), named from the pattern's legend, with the middle size
   offered first. Only the chosen size's pieces are added, each with its
   name, grainline, cutting and fold.
2. Open `fixtures/pdf/sizes-layers.pdf`: the same pattern with each size
   on a layer ("Size 8"…), as layered indie patterns are. The sizes come
   from the layer names.
3. Choose two sizes to compare them: each piece says its size, *Front
   bodice (10)*.

**Why it matters:** without this, nested size lines look like a cutting
line with its stitching line inside, and the grade between sizes would
have been "measured" as seam allowance. Now each size is assembled on its
own, with what the sizes share (grainlines, names, cutting, notches).

**How sizes are told apart:**
* Layers named as sizes: "Size 10", "XS", "38", "UK 12", "Taille 38",
  "10/12"…
* Otherwise, line styles (colour, dash, width) that each draw outlines.
  This needs three or more styles, or a legend naming them, because a
  solid line and a dashed one are usually a cutting line and its
  stitching line. The styles' outlines must also nest, so pieces merely
  drawn in different colours stay one size.

**Every piece found is a suggestion:** the list of pieces in the import
now has a tick box for each, with its size. Untick anything that isn't a
pattern piece (a diagram from an instructions page, a title box) and it
isn't added.

**Protected PDFs, as Acrobat makes them:** `fixtures/pdf/sizes-layers-aes128.pdf`
and `…-aes256.pdf` open like the plain file. They use AES-128 and AES-256,
which is what Acrobat and InDesign write. `…-password.pdf` needs a password
to open, and the import says so and what to do.

**Not yet:** sizes whose lines share a stretch (a common centre front
drawn once for all sizes). Each size's line must go all the way round.

---

## Step 13 — PDF patterns (Phase 2, part 1)

**Try it:** *Open pattern* now takes `.pdf`.

1. Open `fixtures/pdf/freesewing-sasha-a0-protected.pdf`. It is the
   FreeSewing dress printed by Drapewright for a copy shop: a cover page
   and the sheet split over two A0 pages. It is also "protected" the way
   many bought patterns are: it opens without a password but says "no
   copying". The import:
   * Puts the 2 pages back together into one sheet, and leaves out the
     cover (test squares and page map).
   * Knows the size exactly: PDF measures in points.
   * Finds the 7 pieces and names them from the words printed on them. It
     reads the cutting from "Cut 2 · Main fabric" and "Cut 1 on the fold",
     and finds the fold edge from the "Place on fold" bracket.
   * Measures the 1 cm allowance from the printed stitching line, so
     nothing needs asking.
   * Turns the notches cut into the cutting line into notches (3 on the
     front, 5 on the front side panel…) and the arrows into grainlines.
     Their arrowheads, and the marks for joining pages, are left out.
2. Print the dress yourself from Drapewright (A4, trim and butt: 31
   pages), then open that PDF. The 31 pages are put back together into
   the same 7 pieces. Letter pages that overlap work too.

**How pages are put back together:** most tiling tools, Drapewright's
included, draw each page as the whole sheet's drawing, moved and clipped
to that page. A shape that appears on two pages, identical but moved,
says exactly where those pages sit on the sheet. So no page grid or
labels need guessing. Furniture that stays put on every page (frames,
footers, page letters) is ignored.

**Not yet:**
* PDFs whose tiles each contain only their own piece of the drawing,
  with no shapes in common. These need the page grid; they come after
  sizes.
* A list to untick pieces you don't want (instruction pages can still add
  a diagram or two).

**Found by testing:**
* The printed piece names sat at the middle of each piece's bounding box,
  which can be outside a curved piece (and printed over another one). The
  PDF now prints names where the piece is widest, as the AI previews do.
* Pages that share no visible shape (a blank band between pattern rows)
  are still joined, through the shapes they draw beyond their edges.

**What's under the hood:** a new crate, `crates/pdf-import`, reads PDFs.
It covers the file structure (damaged files included), streams, fonts and
text, colours, layers, and the standard encryption with an empty user
password. It is pure Rust with no randomness, so it runs in the browser.
`crates/io/src/pdf/import.rs` turns its drawing into pieces, with
`tiles.rs` (putting pages back together) and `marks.rs` (grainlines,
notches, fold brackets, test squares). Round-trip tests print the dress
in each layout and check that importing the PDF gives back the same
pieces.

---

## Step 12 — Working with an AI assistant (MCP server)

**Try it** with Claude Code (Claude Desktop works too; see
[ai-assistants.md](ai-assistants.md)):

```bash
cargo build --release -p drapewright-mcp
claude mcp add drapewright -- "$PWD/target/release/drapewright-mcp"
```

Then start `claude` in the repository folder and ask:

1. *"Import fixtures/dxf/aama-graded.dxf."* The assistant can't import it
   until it asks you three things, because the server refuses to guess
   them: which size (S, M or L, made in M), how big the file is (it only
   says "metric"), and whether seam allowance is included.
2. *"Import fixtures/svg/freesewing-sasha-sa10.svg and show me the
   pieces."* No questions this time. The assistant gets a picture: each
   piece numbered and filled, with folds in blue, the stitching line
   dashed, notches and grainlines.
3. *"Show the front with its edge ids, call the fold edge centre front,
   rename the pocket to Patch pocket, cut 2 in lining, and add a
   70 × 8 cm waistband."* Each request is one undo step (*"undo that"*
   works).
4. *"Lay everything out for 150 cm fabric and save it to
   ~/Desktop/sasha.drapewright."* Open that file in the app with
   **Projects → Restore a backup…**. The project is all there, with the
   original pattern files kept inside it.

**How it behaves:** it reads and writes only the files it is given, makes
no network connections, never overwrites a file or drops unsaved changes
without being told to, and changes projects only through the app's own
commands.

**Found by testing:** piece numbers placed at the middle of a piece's
bounding box fell outside curved pieces, such as the dress front. They now
go at the point farthest inside the piece.

**What's under the hood:** `crates/mcp` speaks the protocol (JSON-RPC over
stdio) in `lib.rs`, and its 25 tools are in `tools.rs`. Each tool's
arguments are a Rust type, so its JSON Schema is generated and can't drift.
Several commands now apply as one all-or-nothing undo step
(`Engine::dispatch_all`). Previews are drawn with tiny-skia, labelled with
a small built-in stroke font so they need no font files (`render.rs`,
`font.rs`).

---

## Step 11 — Real patterns: every size in one file, and FreeSewing (after Phase 1)

**Try it:**

1. **A file with every size.** Garment CAD files (Gerber, CLO, Browzwear)
   carry each piece once per size. Open `fixtures/dxf/aama-graded.dxf`: the
   import says **Found 2 pieces in 3 sizes** and asks **Which size?**, with
   the size the pattern was made in (M) chosen. Choose L as well to compare
   two sizes. Each piece then says which it is, *Front (M)* and *Front (L)*.
   Only the chosen sizes are added.
2. **A FreeSewing pattern.** Open `fixtures/svg/freesewing-sasha-sa10.svg`,
   FreeSewing's Sasha dress in size 36 with 1 cm allowance:
   * The 7 pieces have FreeSewing's names: *Front inside*, *Pocket*,
     *Sleeve*…
   * Both lines are in the file, so the 1 cm allowance is measured, not
     asked.
   * *Front inside* is cut on the fold. FreeSewing leaves its cutting line
     open along the fold, and that edge becomes a **fold** with no
     allowance.
   * The 14 notches land on the seams (the back one is double). Every piece
     has its grainline, and "Cut 2 mirrored" or "Cut 1 on fold" comes
     across.
   * Left out, and listed in the import: FreeSewing's logo on the sleeve,
     and the second copies it draws of the front neckline and back armhole.
     A copy left in would stay behind as a ghost line when you reshape that
     seam.
   * The import says once, for all 7 pieces, that FreeSewing's cutting
     lines differ from Drapewright's by up to 1 cm. That is only at
     corners: FreeSewing cuts its corners off at 45°, while Drapewright
     points them (open question 3). Notes that say the same about several
     pieces are now always said once.
3. `fixtures/svg/freesewing-sasha-no-allowance.svg` is the same dress
   without allowance. Its outlines could be either line, so you are asked
   about allowance. The fold still comes from FreeSewing's cut-on-fold
   bracket.

**Checking against real patterns:** the collection in
`~/Source/drapewright-test-patterns` can't be committed (its README says
where each file comes from), but you can see how all of it imports:

```bash
cargo run --release -p drapewright-io --example import_report -- -v ~/Source/drapewright-test-patterns
```

Every file imports. That covers 44 garment CAD exports from Gerber, Lectra,
Assyst, CLO and Browzwear, museum patterns from MoMu Antwerp, and the
FreeSewing dress.

**Not yet:** Lectra and Assyst exports carry only the size the pattern was
made in. CLO and Browzwear files that come with a `.rul` file hold the base
size plus rules for the others, and those rules are not applied yet.

**Found by testing:** a line inside two overlapping pieces went to either
piece, at random, from one import to the next. It now goes to the smaller
piece, so a file always imports the same way.

**What's under the hood:** FreeSewing's conventions are in
`crates/io/src/svg/freesewing.rs`. Cutting lines left open (at a fold or
a corner) are closed in `close_open_outlines` in `crates/io/src/assemble.rs`.
Sizes are in `ImportResult.sizes`, and `choose_sizes` keeps the chosen ones.

---

## Step 10 — Projects that stay put: autosave, backup, offline (Phase 1, day 10)

**Try it:**

1. Open a pattern and change something. Next to the project name (top
   left), **Saving…** turns into **Saved** a moment later. Close the tab or
   reload: the same project opens, where you left it.
2. **Tap the project name** to see **Projects**:
   * Rename the project.
   * Start a **New project**.
   * Open another project from the list.
   * **Delete** one. You are asked first, and the project that is open
     cannot be deleted.
3. **Back up** (top bar) saves a `.drapewright` file holding the project and the
   original SVG/DXF its pieces came from. Keep it in Files or a cloud
   drive.
4. **Restore a backup…** (in Projects), or just *Open pattern* and pick a
   `.drapewright`. If that project is already on the device, the backup opens as
   a copy named "… (restored)". Nothing is overwritten.
5. **Offline:** after one visit the app loads and works with no internet,
   import and printing included. Nothing is ever uploaded.
6. **This device** (ⓘ) now says whether the browser has promised to keep
   saved projects ("Kept safe"). Safari on iPad does this for apps added
   to the Home Screen; otherwise, back up the projects you care about.

**Where it is kept:** each project is a folder in the browser's private
file system (OPFS), with a list in IndexedDB. Where OPFS cannot be written,
as in Safari's private browsing, the same files go into IndexedDB. Each
save writes a complete copy before replacing the old one, so a crash
mid-save never loses the project.

**The format is published:** `docs/schema/project.schema.json` is generated
from the same Rust types that read and write projects. A test checks real
saved projects against it, and CI fails if it changes without being
committed.

**Checked on every push:**
* A walk-through test: open a pattern, change the allowance, mark a hem, a
  notch and a grainline, print the PDF, reload, and check everything is
  still there.
* An offline reload.
* Edit speed: changing a piece's allowance and redrawing its lines takes a
  median of 0.1 ms (worst under 1 ms) on Chromium and WebKit. The budget is
  one frame, 16 ms.
* The engine is now shrunk with `wasm-opt`: 1.8 MB (690 KB compressed),
  down from 2.5 MB.

**Not yet:** a thumbnail picture for each project in the list. Nudges to
back up after many edits come later, with the Home Screen install prompt.

---

## Step 9 — Printing at true size, and measuring the screen (Phase 1, day 9)

**Try it:** open `fixtures/dxf/aama-skirt.dxf` and
`fixtures/svg/flared-skirt.svg`, then press **Print** (top bar).

1. **The print dialog** shows the pages as they will come out of the
   printer, with the pieces laid out on them and the page letters (A1, A2…
   B1…), plus the page count.
   * Choose **A4, US Letter, A3, Tabloid or A0** (copy shop) and portrait
     or landscape. The preview and count update as you go.
   * Choose how pages join: **Trim and butt** (cut the margin off two sides
     and lay pages edge to edge) or **Overlap** (lay pages over each other
     by a strip, 1.5 cm by default).
   * You can also set the margin, the notch style (slit or V), whether the
     dashed stitching line prints, the test square, the page map, and
     whether empty pages are left out.
2. **Save PDF.** The file is made on your device and nothing is uploaded.
   * Page 1 has the instructions and two test squares, **10 cm** and
     **4 in**, plus a map of the pages.
   * Every other page has a frame to trim along and its letter in the
     margin, in faint large type and next to each neighbour's edge.
   * Trimmed pages have half-diamonds that make a whole diamond when butted
     together. Overlapping pages have a dashed line where the next page's
     edge goes and circles to line up.
   * Pieces have their cutting line (solid), stitching line (dashed),
     notches, grainline, "Place on fold" brackets, drill holes and other
     marks, plus their name and "Cut 1 · Main fabric".
3. **Print it at 100%** ("Actual size"). The PDF asks readers not to
   scale it and to print one-sided, but check the print dialog anyway.
   **Measure the test square: it must be 10 cm.** Print settings are part
   of the project and can be undone.
4. **Measure your screen:** the ruler button (bottom left, with an orange
   dot until it is done). Hold a bank card against the screen and slide
   until the outline matches it, or check the 5 cm line with a ruler.
   After that, **100%** shows pieces at their real size on this device. It
   is remembered on this device only and is not part of any project.

Drill holes and lines inside pieces now show on the table too, as they
print.

**How we know it is exact:** the tests read the PDF back and measure it:
* Every page has the exact paper size, and print scaling is off.
* The test squares measure 100.000 mm and 101.600 mm.
* Known edges keep their lengths to 0.001 mm: 200 and 300 mm stitching
  lines, and 230 and 330 mm cutting lines.
* Every point of every cutting line falls on a printed page.
* The same project always gives the same file.

**Not yet:**
* Pieces keep their table rotation and are laid out on shelves, without
  rotating or nesting to save paper. Moving pieces on the print sheet by
  hand comes later; the file format already holds such a layout.
* A piece's name can be split across two pages.
* The PDF uses the standard Helvetica font, which has Western European
  letters only. The dialog lists any name that would print with "?".
* New projects start on A4. Choosing Letter is one tap and is saved with
  the project.

---

## Step 8b — DXF import, and asking about seam allowance (Phase 1, day 8)

**Try it:** *Open pattern* now takes `.dxf` as well as `.svg`.

1. Open `fixtures/dxf/aama-skirt.dxf`, a garment CAD (AAMA/ASTM) file.
   * **Size:** the file says *metric*, which nearly always means
     millimetres, but not always, so you confirm it. Each reading shows how
     big the skirt front would be.
   * **Skirt front:** the file has both its cutting line and its stitching
     line, so Drapewright keeps the stitching line as the outline and measures the
     allowance edge by edge: 1.5 cm seams, 4 cm hem, and nothing on the
     centre front, which the file marks as a fold. The notch, grainline,
     drill hole, "Quantity: 1" and "Size: 38" come across too.
   * **Waistband:** only its cutting line is in the file. That is what
     garment CAD files mean by their cutting lines, so *Already included*
     is chosen for you. Set the width it should show (try 1 cm). The
     cutting line is kept exactly as drawn, with the stitching line dashed
     inside it.
2. Open `fixtures/svg/inkscape-mm.svg` (or any SVG with one outline per
   piece). The new question **"Is seam allowance included in these
   pieces?"** has **nothing chosen**, and *Add* waits until you answer:
   * *Not included*: the lines are stitching lines, and the allowance is
     added around them (as before).
   * *Already included*: the lines are cutting lines, kept as they are.

   Getting this wrong makes a garment too big or too small by two
   allowances at every seam, and the file cannot say which it is, so Drapewright
   asks rather than guessing. Files with both lines (like
   `fixtures/svg/cut-and-seam.svg`) are not asked about: their allowance is
   measured.
3. Open `fixtures/dxf/generic-mm.dxf`, an ordinary CAD drawing. The pocket's
   rounded corners stay curved, within 0.001 mm of true arcs. The flap was drawn as four separate
   lines and still becomes one piece, named by the text inside it. The
   hatch Drapewright cannot read yet is reported, not silently dropped.

**What it reads:**
* ASCII DXF from any version: lines, polylines (with bulges), arcs,
  circles, splines, points, text and blocks.
* Units come from `$INSUNITS` when the drawing has them, and from the AAMA
  units text otherwise. With neither, you are asked.
* Curves stay curves. Splines of degree 3 or less become exact Béziers,
  and arcs are within 0.001 mm. A spline saved only as the points it passes
  through is joined with straight lines, and the import says so.
* **Not yet:** binary DXF (you are asked to save it as ASCII), ellipses,
  hatches and 3D entities (reported).

**Research-grade, with a fallback:**
* AAMA/ASTM files from real CAD systems vary between vendors. The reader
  follows the published layer numbers and is tested only on files drawn for
  these tests. The fallback is that anything it cannot place is reported,
  and a file that is not recognised as AAMA is read as an ordinary drawing.
  A few real exports would show what needs adjusting (open question 17).

**Found by testing:** measuring the allowance at the nearest point of the
cutting line gave a 4 cm hem as 1.5 cm. Near the fold corner, the fold's
cutting line is closer than the hem's. The allowance is now measured
straight out from the stitching line, as you would with a ruler.

---

## Step 8a — Notches, grainline and cutting instructions (Phase 1, day 8)

**Try it** with `fixtures/svg/flared-skirt.svg`:

* **Toolbar** (top left): *Select* (V), *Notches* (N), *Grainline* (G), with
  a one-line hint for the active tool.
* **Notches:** with the notch tool, tap an edge. A notch appears exactly
  there, drawn as a slit into the allowance with a faint line across it. It
  is measured from the nearer corner, so it stays put if the far end
  changes. **Tap the notch** (on the slit itself, out at the cutting line)
  to cycle single → double → triple. **Drag it** to slide it along the
  edge; the distance from the corner shows as you go, and the slide is one
  undo step. The panel shows the type and the exact distance, which corner
  it is measured from, and *Delete notch*. Right-click an edge for *Add a
  notch here*.
* **Grainline:** with the grainline tool, drag inside a piece. It snaps to
  15° steps of the piece's own axes; hold Shift for any angle. Or use
  *Add a straight grainline* in the panel, and switch on *One way* for
  napped fabric (single arrowhead).
* **Cutting:** *How many* and which fabric; *As mirrored pairs* appears from
  2 up, *On the fold* when the piece has a fold edge; *Also cut in another
  fabric* adds lining or interfacing. The first line shows on the piece
  ("Cut 1 on the fold · Main fabric").

**Found by testing:** you could only pick a notch at its point on the
stitching line, 1.5 cm inside the slit you can see. Now the whole mark
(slit and line) can be tapped.

---

## Step 7 — Seam allowance, edge by edge and corner by corner (Phase 1, day 7)

**Try it:** open `fixtures/svg/flared-skirt.svg` (new: a panel flaring 20°
with a straight hem).

1. **Tap the bottom edge.** The panel now has an *Edge* section: its length
   along the stitching line (also shown on the table next to the edge), a
   name, and **This edge is a Seam / Hem / Fold**. Choose **Hem**: the hem
   gets the piece's hem allowance, and the corners next to it fold back.
2. Under *Piece*, set **Hem allowance** to 4 cm. Watch the bottom-right
   corner: the hem allowance angles back in, exactly as it must to lie flat
   when turned up along a flared side seam.
3. **Tap that corner** (the point where the side seam meets the hem). The
   *Corner* section shows **six live pictures of this very corner**:
   automatic, pointed, squared and folded back, on either side. Each one is
   computed from the real piece, so what you see is what you will cut. Where
   two shapes come out the same (common at right angles) it says so; where
   one is impossible it says that too. **These pictures are what to show the
   seamstress for open questions 3 and 4.**
4. Tap the left edge and choose **Fold**: no allowance, and the cutting line
   meets the fold squarely. Try **Fold** on a curved edge (the hem of
   `inkscape-mm.svg`): Drapewright refuses and explains that a fold must be
   straight.
5. Any edge can have **its own allowance**; *Use the piece's allowance*
   puts it back.
6. Right-click (long-press) an edge for **Add a corner here**, a corner for
   **Remove this corner**, and either for changing the edge type.
7. **Allowance is already included**: for patterns printed with allowance,
   the outline becomes the cutting line and the dashed stitching line is
   worked out inside it.

Undo (⌘Z, two-finger tap) covers all of it; stepping an allowance up and
down is a single undo step.

**Fixed along the way** (found by the new end-to-end tests): ⌘Z did nothing
right after picking a corner shape, because focus on a radio button counted
as typing; and tapping an edge of a piece that was part of a group selection
kept the whole group selected, hiding the edge's settings.

---

## Step 6 — Input layer: Pencil, fingers, mouse and keys (Phase 1, day 6)

**Try it** (`pnpm dev`, open a fixture as in step 5):

* **Move pieces:** drag a piece. While you drag it follows the pointer; on
  release it is one undo step ("Undo move"). Arrow keys nudge the selection
  by 1 mm, Shift+arrow by 1 cm; a run of nudges undoes in one go.
* **Select:** tap or click a piece; Shift-click adds to the selection. Tap
  an *edge* (on the dashed stitching line) and it lights up; the panel shows
  its length along the stitching line.
* **Inspector:** rename the piece, and step the seam allowance up and down.
  The cutting line follows immediately. Stepping is one undo step.
* **Menu:** right-click (or **long-press** on iPad) a piece for *Turn 90°*
  and *Delete piece*; on the empty table for *Open pattern* and *Show all*.
* **Undo/redo gestures on iPad:** two-finger tap undoes, three-finger tap
  redoes, with a short "Undo move" confirmation at the top.
* **Navigate:** two fingers pan and pinch; space + drag pans with a mouse.
* **Keys:** ⌘Z / ⇧⌘Z (Ctrl+Z / Ctrl+Y), ⌫ delete, 0 show all, 1 actual
  size, + and − zoom, Esc clear selection, ⌘A select all.
* **Pencil:** once the Pencil has been used, fingers only navigate and
  select (palm rejection), as in Procreate. Holding the Pencil still on an
  edge shows its length.

**Worth trying on a real iPad** (please report back): palm resting on the
glass while drawing with the Pencil; two-finger tap undo; long-press menu.
These are unit-tested with recorded event sequences, but real hardware is
the real test.

**Under the hood:** `@drapewright/input` is framework-free (it does not even see
the DOM). It turns raw pen, touch and mouse events into pan/zoom, undo/redo,
context menus and a single stream of tool events, with palm rejection and
gesture recognition, all unit-tested by replaying event sequences against a
fake clock. Tools are small state machines that only ever preview or
dispatch commands, so a cancelled gesture can never leave half an edit.

**Fixed along the way:** if the app starts while its window is hidden, the
browser gives it no size, and "show all" used to zoom out to 2%. The canvas
is now measured when it appears, and "show all" waits until the table has a
size.

---

## Step 5 — Open an SVG and see it on the table (Phase 1, day 5)

**Try it:** `pnpm wasm && pnpm dev`, open http://localhost:5173.

1. **Open pattern** and pick `fixtures/svg/inkscape-mm.svg`. Drapewright reads it in
   the background and tells you what it found: two pieces, plus a small
   shape and a stray line it left out. Choose **Add 2 pieces**.
2. The pieces lie on a green cutting mat with a real 1 cm grid, bolder every
   5 cm. Each piece is drawn as pattern paper: solid **cutting line**,
   dashed **stitching line** 1.5 cm inside (the provisional default
   allowance), the **grainline** arrow, and the name.
3. Open `fixtures/svg/illustrator-pt.svg`. This file does not say how big it
   is, so Drapewright **asks**, showing what each reading would mean ("Pocket would
   be 10.0 cm × 5.0 cm"). Pick one; new pieces land beside the existing ones.
4. `fixtures/svg/cut-and-seam.svg` has both lines; Drapewright measures the
   allowance from the file (1.5 cm, with a 3 cm hem) instead of adding its
   own.
5. Navigate: two-finger scroll pans, pinch (or ⌘/Ctrl + scroll) zooms
   around the pointer, drag the empty mat to pan. The zoom buttons are
   bottom left; the percentage button jumps to **actual size**. It is
   approximate until the screen is measured in step 9.
6. Click a piece (on the table or in the list) to select it. **Undo/Redo**
   (toolbar, ⌘Z / ⇧⌘Z) step back and forth through what you added.
7. Switch your system between light and dark: the app and the table follow.

**Not yet:** nothing is saved between reloads (step 10), pieces cannot be
moved or edited yet (steps 6–8), and touch gestures on iPad come with the
input layer (step 6).

**Under the hood:** the `project` crate is the document engine. Every change
is a command, the engine refuses any edit that would leave a piece broken,
and unlimited undo shares unchanged pieces between versions. Random
editing sessions (property test) never produce duplicate ids or broken
pieces, and undoing everything restores the start exactly. Imports run in a
worker on a second copy of the WebAssembly engine. Rust types generate the
TypeScript types, so the two sides cannot drift.

---

## Step 4 — SVG import and export (Phase 1, day 4)

Still nothing new in the browser: the importer gets its screen in step 5.

**Try it:** `cargo test -p drapewright-io`. The fixtures in `fixtures/svg/` cover
what real files throw at an importer:

* **Inkscape, in millimetres** (`inkscape-mm.svg`): two pieces found and
  named from their labels ("Skirt front", "Waistband"), a layer transform
  undone, the grainline recognised from its id, a drill hole kept as a
  mark. A notch drawn as a triangle, a title and a stray line are counted
  and reported, never silently dropped.
* **Illustrator** (`illustrator-pt.svg`): sizes in points written as
  pixels. The importer reads it as points (Illustrator's habit) but marks
  the scale **unconfirmed**; choosing another reading rescales everything.
* **No units** (`unitless.svg`): the scale is a guess and must be
  confirmed or measured. Imports never pretend to know.
* **Cutting and stitching lines both present** (`cut-and-seam.svg`): paired
  automatically. The stitching line becomes the outline and the allowance
  is measured edge by edge (15 mm, with a 30 mm hem). The importer then
  checks its own cutting line against the file's and would report any
  difference.
* **Transforms and `<use>`** (`transforms.svg`): rotated and scaled shapes
  come out at the right size.
* **Round trip**: pieces exported to SVG and imported again come back
  within 0.001 mm, with the same allowances.

**Scale rules:** a file is only trusted when its width and height state
physical units. Otherwise the importer suggests 96, 72 or 90 per inch from
clues (Illustrator's generator comment, old Inkscape versions, a drawing
exactly the size of A4 or Letter) and the app will ask.

---

## Step 3 — Seam allowance engine (Phase 1, day 3)

**Try it:** open `crates/pattern-core/tests/gallery/index.html` in your
browser (double-click it in Finder). It shows eleven hard cases, each
with the stitching line dashed and the cutting line solid:

* a half bodice with curved neckline and armhole, 15 mm seams, a 30 mm
  waist hem folded back at the side seam, and centre front on the fold;
* a flared skirt whose 40 mm hem angles back in at the side seam, so it
  lies flat when turned up;
* a trouser leg narrowing to the hem, where the hem allowance flares out
  instead;
* a 20° flare with a 50 mm hem, where the folded-back end is narrower than
  the hem and the corner is trimmed;
* a 10° point cut square at 40 mm, a squared corner, a circle, and an 8 mm
  curve under a 15 mm allowance that the cutting line bridges cleanly;
* a pattern with allowance *included*, with the stitching line derived
  inside it.

These are the pictures to show the seamstress for open questions 3 and 4
(corner shapes).

`cargo test -p drapewright-pattern-core` also runs the exact cases and four
properties over hundreds of random pieces: the cut piece always contains
the whole stitched piece and never crosses itself, away from corners the
cutting line is exactly the allowance away (within 0.005 mm), and so on.

**Speed:** under 1 ms per piece for everything in the gallery (release
build). Curves tighter than the allowance first took 43 ms; they are now
thinned where they cannot affect the result (0.4 ms, identical output).

**How it works:** the allowance region is a union of simple pieces: a
rectangle per flattened segment, pointed joins at kinks, and one polygon
per corner shaped by its corner style. The union uses i_overlay. This
avoids the classic failure of offsetting a curve and repairing its loops.
The trade-off is a cutting line that is a polyline within 0.002 mm of the
true curve.

---

## Step 2 — Geometry kernel (Phase 1, day 2)

Nothing new in the browser yet; this step is the foundation the next ones
draw with.

**Try it:** `cargo test -p drapewright-pattern-core`. Seven property tests run
256 random pieces each:

* edge lengths add up to the outline length;
* a point found by distance along an edge is found again by "nearest
  point" (what tapping an edge will use);
* splitting an edge and joining it back changes nothing, and notches stay
  exactly where they were (to 0.000001 mm);
* reversing an outline twice gives back the identical piece;
* every piece survives a save/load round trip **bit for bit**. This test
  caught that the JSON library rounds the last digit of some numbers by
  default, which would have been a silent geometry change on every save. It
  now uses exact round-tripping.

**What's under the hood:** the piece model in
`crates/pattern-core/src/model.rs` *is* the file format (serde types, with
TypeScript types generated into `packages/engine/src/generated`), plus
measuring (`geom.rs`), edge split/join/reverse that keep notches and corner
styles in place (`ops.rs`), and the validation rules from the format spec
(`validate.rs`).

---

## Step 1 — Scaffold (Phase 1, day 1)

**Try it:**

1. `pnpm dev` and open http://localhost:5173. You should see the green
   cutting mat and a **This device** card.
2. All four checks should be green in current Chrome and Safari 26:
   *Saves on this device*, *Works without internet*, *3D draping*, *Fast
   mode*. Firefox shows *3D draping* as unavailable unless WebGPU is enabled.
3. The card ends with "Pattern engine 0.1.0". That text comes from the Rust
   engine compiled to WebAssembly, which proves the Rust ↔ browser pipeline.
4. Offline check: `pnpm build && pnpm preview`, open
   http://localhost:4173, reload once, then stop the server and reload
   again. The app still opens.

**What's under the hood:** Cargo workspace (`pattern-core`, `wasm-core`),
pnpm workspace (`web`, `packages/engine`), COOP/COEP headers in dev, preview
and production (`web/public/_headers`, which also sets a CSP that blocks
requests to other sites), PWA manifest and service worker, CI with Rust,
licence, type, lint, unit and Playwright (Chromium + iPad WebKit) jobs.
