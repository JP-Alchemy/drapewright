// Everything the app says, in plain sewing language. One place, so wording
// stays consistent and can be translated later.

import type {
  AllowanceNote,
  CornerStyle,
  DisplayUnits,
  EdgeKind,
  ImportNote,
  NotchFrom,
  NotchKind,
  PaperName,
  ScaleBasis,
  ScaleReason,
} from '@drapewright/engine'

/** A length in the user's units: "45.2 cm", "452 mm" or "17.8 in". */
export function formatLength(mm: number, units: DisplayUnits): string {
  switch (units) {
    case 'mm':
      return `${Math.round(mm)} mm`
    case 'in':
      return `${(mm / 25.4).toFixed(1)} in`
    case 'cm':
      return `${(mm / 10).toFixed(1)} cm`
  }
}

const plural = (n: number, one: string, many: string) =>
  n === 1 ? one : many.replace('#', String(n))

/** “Front”, “Back” and “Collar” */
const namesList = (names: string[]) =>
  new Intl.ListFormat('en', { type: 'conjunction' }).format(names.map((n) => `“${n}”`))

/** Who a note is about: “Front”, “Front and Back”, or “All 7 pieces”. */
const piecesLabel = (names: string[], total: number) => {
  if (names.length > 1 && names.length === total) {
    return names.length === 2 ? 'Both pieces' : `All ${names.length} pieces`
  }
  return new Intl.ListFormat('en', { type: 'conjunction' }).format(names)
}

export const t = {
  appName: 'Drapewright',
  untitled: 'Untitled pattern',

  actions: {
    open: 'Open pattern',
    print: 'Print',
    undo: 'Undo',
    redo: 'Redo',
    undoThing: (label: string) => `Undo ${label}`,
    redoThing: (label: string) => `Redo ${label}`,
    zoomIn: 'Zoom in',
    zoomOut: 'Zoom out',
    fitAll: 'Show all pieces',
    actualSize: 'Actual size',
    device: 'About this device',
    close: 'Close',
  },

  table: {
    empty: 'Open a sewing pattern to lay it out here.',
    emptyHint:
      'PDF patterns work, both copy-shop sheets and home-print pages (the pages are put back together for you), as do SVG files from Inkscape, Illustrator, FreeSewing and most pattern apps, and DXF files from CAD and garment pattern systems.',
    zoomLabel: (zoom: number) => `${Math.round(zoom * 100)}%`,
    notCalibrated: 'Screen not measured yet: 100% is approximate.',
  },

  inspector: {
    heading: 'Piece',
    nothing: 'Tap a piece to see its details. Tap an edge or a corner to adjust it.',
    many: (n: number) => `${n} pieces selected. Drag to move them together.`,
    name: 'Name',
    included: 'Allowance is already included',
    includedHint:
      'The outline is the cutting line, as printed on the pattern. The dashed stitching line is worked out from the allowance below.',
    addedHint: 'The outline is the stitching line. Allowance is added outside it for cutting.',
    seamAllowance: 'Seam allowance',
    hemAllowance: 'Hem allowance',
    edge: 'Edge',
    edgeName: 'Edge name',
    edgeNamePlaceholder: 'For example: side seam',
    edgeNumber: (n: number) => `edge ${n}`,
    theHem: 'the hem',
    theFold: 'the fold',
    edgeLength: (length: string) => `Length along the stitching line: ${length}`,
    kind: 'This edge is a',
    kinds: { seam: 'Seam', hem: 'Hem', fold: 'Fold' } satisfies Record<EdgeKind, string>,
    kindHints: {
      seam: 'Sewn to another piece.',
      hem: 'Turned up and finished. The corners next to it fold back to match.',
      fold: 'Placed on the fold of the fabric. It has no allowance and must be straight.',
    } satisfies Record<EdgeKind, string>,
    edgeAllowance: 'Allowance on this edge',
    usePieceAllowance: 'Use the piece’s allowance',
    foldNoAllowance: 'No allowance: this edge is cut on the fold.',
  },

  toolbar: {
    label: 'Tools',
    select: 'Select and move (V)',
    notch: 'Notches (N)',
    grainline: 'Grainline (G)',
    hints: {
      select: 'Tap a piece, edge or corner. Drag to move a piece.',
      notch: 'Tap an edge to add a notch. Drag a notch to slide it; tap it for double or triple.',
      grainline: 'Drag inside a piece to draw its grainline. Hold Shift for any angle.',
    },
  },

  notch: {
    fromCorner: (d: string) => `${d} from the corner`,
    heading: 'Notch',
    kind: 'Notch',
    kinds: { single: 'Single', double: 'Double', triple: 'Triple' } satisfies Record<
      NotchKind,
      string
    >,
    distance: 'Distance from the corner',
    measuredFrom: 'Measured from the',
    from: { start: 'Start of the edge', end: 'End of the edge' } satisfies Record<
      NotchFrom,
      string
    >,
    delete: 'Delete notch',
  },

  grain: {
    heading: 'Grainline',
    none: 'No grainline yet. Draw one with the grainline tool, or:',
    add: 'Add a straight grainline',
    remove: 'Remove grainline',
    oneWay: 'One way (napped or one-way fabric)',
  },

  cut: {
    heading: 'Cutting',
    quantity: 'How many',
    material: 'Fabric',
    materials: {
      main: 'Main fabric',
      lining: 'Lining',
      interfacing: 'Interfacing',
      contrast: 'Contrast',
    } as Record<string, string>,
    mirrored: 'As mirrored pairs',
    onFold: 'On the fold',
    add: 'Also cut in another fabric',
    remove: 'Remove this line',
    summary: (quantity: number, material: string, onFold: boolean) =>
      `Cut ${quantity}${onFold ? ' on the fold' : ''} · ${material}`,
  },

  corner: {
    sectionHeading: 'Corner',
    heading: 'Shape of the allowance here',
    between: (a: string, b: string) => `Where ${a} meets ${b}.`,
    sides: (left: string, right: string) =>
      `In the pictures: ${left} on the left, ${right} on the right.`,
    style: (s: CornerStyle): string =>
      ({
        auto: 'Automatic',
        mitre: 'Pointed',
        squareThis: 'Squared on the left',
        squareNext: 'Squared on the right',
        mirrorThis: 'Folded back on the left',
        mirrorNext: 'Folded back on the right',
      })[s],
    notHere: 'Not possible here',
    likePointed: 'Same as pointed here',
    remove: 'Remove this corner',
  },

  allowanceNote: (n: AllowanceNote, edgeName: (id: string) => string): string => {
    switch (n.kind) {
      case 'cornerFallback':
        return `At the end of ${edgeName(n.edge)}, that corner shape is not possible, so it is pointed.`
      case 'enclosedGap':
        return 'Allowances from different edges meet and enclose a gap; it is cut as part of the piece.'
      case 'nothingLeft':
        return 'The allowance is wider than the piece, so no stitching line remains.'
    }
  },

  menu: {
    label: 'Actions',
    turn: 'Turn 90° clockwise',
    turnBack: 'Turn 90° anticlockwise',
    deletePiece: 'Delete piece',
    addCorner: 'Add a corner here',
    removeCorner: 'Remove this corner',
    makeSeam: 'Make this edge a seam',
    makeHem: 'Make this edge a hem',
    makeFold: 'Place this edge on the fold',
    addNotch: 'Add a notch here',
    deleteNotch: 'Delete notch',
  },

  pieces: {
    heading: 'Pieces',
    none: 'No pieces yet.',
    size: (w: string, h: string) => `${w} × ${h}`,
  },

  importing: {
    reading: (name: string) => `Reading ${name}…`,
    unsupported: (name: string) =>
      `Drapewright opens SVG, DXF and PDF patterns; “${name}” is none of these.`,
    failed: (message: string) => `This file could not be opened. ${message}`,
    found: (n: number) => plural(n, 'Found 1 piece', 'Found # pieces'),
    foundSized: (pieces: number, sizes: number) =>
      `${plural(pieces, 'Found 1 piece', 'Found # pieces')} in ${sizes} sizes`,
    sizeQuestion: 'Which size?',
    sizeHint:
      'The file has every size of this pattern. Pick yours, or more than one to compare them.',
    sampleSize: 'sample',
    sampleSizeLabel: (size: string) => `${size}, the size the pattern was made in`,
    nothingFound: 'No pieces were found in this file.',
    scaleQuestion: 'How big is this pattern?',
    sizeCertain: 'The file says how big it is, so the size is exact.',
    reason: (r: ScaleReason): string =>
      ({
        declaredUnits: 'The file says how big it is.',
        illustratorFile:
          'This file was made with Adobe Illustrator, which usually measures in points (72 to the inch). Please check the size below.',
        oldInkscapeFile:
          'This file was made with an older Inkscape, which used 90 dots to the inch. Please check the size below.',
        paperSize:
          'The drawing is exactly a paper size at this reading. Please check the size below.',
        noUnits:
          'The file does not say how big it is. Pick the reading where a piece has the size you expect.',
        noDrawingUnits:
          'The drawing does not say which unit it was drawn in. Pick the reading where a piece has the size you expect.',
        aamaMetric:
          'This garment CAD file says it is metric, which usually means millimetres. Please check the size below.',
      })[r],
    basis: (b: ScaleBasis): string =>
      ({
        declaredUnits: 'As the file says',
        assumed96dpi: '96 to the inch (most apps)',
        assumed72dpi: '72 to the inch (Adobe Illustrator)',
        assumed90dpi: '90 to the inch (Inkscape before 2017)',
        assumedMm: 'Millimetres',
        assumedCm: 'Centimetres',
        assumedInches: 'Inches',
        measured: 'As measured',
      })[b],
    wouldBe: (name: string, size: string) => `${name} would be ${size}`,
    /** `asked`: names of the pieces drawn with one line; `total`: pieces found. */
    allowanceQuestion: (asked: string[], total: number) =>
      asked.length === total
        ? plural(
            total,
            'Is seam allowance included in this piece?',
            'Is seam allowance included in these pieces?',
          )
        : asked.length <= 3
          ? `Is seam allowance included in ${namesList(asked)}?`
          : `Is seam allowance included in the ${asked.length} pieces drawn with one line?`,
    allowanceWhy: (n: number) =>
      plural(
        n,
        'It has a single outline, which could be the cutting line or the stitching line. The file does not say which.',
        'Each has a single outline, which could be the cutting line or the stitching line. The file does not say which.',
      ),
    allowanceSuggested: (n: number) =>
      plural(
        n,
        'The file marks this outline as the cutting line, so the allowance is already included. Check its width.',
        'The file marks these outlines as cutting lines, so the allowance is already included. Check its width.',
      ),
    allowanceAdd: 'Not included',
    allowanceAddDetail:
      'The lines are stitching lines. Drapewright adds the allowance around them.',
    allowanceIncluded: 'Already included',
    allowanceIncludedDetail:
      'The lines are cutting lines, kept exactly as drawn. The stitching line is shown inside.',
    allowanceWidth: 'Seam allowance',
    allowanceNeeded: 'Say whether seam allowance is included to add the pieces.',
    add: (n: number) => plural(n, 'Add the piece', 'Add # pieces'),
    cancel: 'Cancel',
    /** `pieces` are the names of the pieces a note is about, of `total` found. */
    note: (n: ImportNote, pieces: string[], total: number, units: DisplayUnits): string => {
      const who = piecesLabel(pieces, total)
      const one = pieces.length === 1
      switch (n.kind) {
        case 'seamLineFound':
          return one
            ? `${who}: found both the cutting line and the stitching line, so the allowance (${formatLength(n.allowance, units)}) was measured from the file.`
            : `${who}: found both the cutting and the stitching lines, so the allowance (${formatLength(n.allowance, units)}) was measured from the file.`
        case 'cuttingLineDiffers':
          return one
            ? `${who}: the file’s cutting line differs from Drapewright’s by up to ${formatLength(n.maxMm, 'mm')}, usually at corners. You can adjust the corners after adding.`
            : `${who}: the file’s cutting lines differ from Drapewright’s by up to ${formatLength(n.maxMm, 'mm')}, usually at corners. You can adjust the corners after adding.`
        case 'smallShapesIgnored':
          return plural(
            n.count,
            'Left out 1 small shape (an arrowhead, notch or logo).',
            'Left out # small shapes (arrowheads, notches or logos).',
          )
        case 'looseLinesIgnored':
          return plural(
            n.count,
            'Left out 1 line outside the pieces (a title or border).',
            'Left out # lines outside the pieces (titles or borders).',
          )
        case 'linesOnOutlineIgnored':
          return plural(
            n.count,
            'Left out 1 line drawn over a piece’s outline (a second copy of a seam).',
            'Left out # lines drawn over the pieces’ outlines (second copies of seams).',
          )
        case 'noClosedShapes':
          return 'The file has lines but no closed shapes. Drapewright needs each piece’s outline drawn as one closed shape.'
        case 'unsupportedEntities':
          return plural(
            n.count,
            'Left out 1 part of the drawing Drapewright cannot read yet (such as a hatch or a 3D face).',
            'Left out # parts of the drawing Drapewright cannot read yet (such as hatches or 3D faces).',
          )
        case 'curvesTraced':
          return plural(
            n.count,
            '1 curve was stored only as the points it passes through, so Drapewright joined them with straight lines. Check its shape after adding.',
            '# curves were stored only as the points they pass through, so Drapewright joined them with straight lines. Check their shape after adding.',
          )
        case 'pagesJoined':
          return `Put ${n.count} pages back together into one sheet, leaving out the marks for joining them.`
        case 'pagesLeftOut':
          return plural(
            n.count,
            'Left out 1 page that is not part of the pattern sheet (a cover or instructions).',
            'Left out # pages that are not part of the pattern sheet (covers or instructions).',
          )
        case 'testSquareFound':
          return `Found the ${formatLength(n.size, n.size < 50 ? 'mm' : 'cm')} test square and left it out of the pieces. Measure it on your print before cutting.`
        case 'outlineCrossesItself':
          return one
            ? `${who}: the outline crosses itself. Check that piece after adding.`
            : `${who}: the outlines cross themselves. Check those pieces after adding.`
      }
    },
  },

  print: {
    heading: 'Print at true size',
    paper: 'Paper',
    papers: {
      a4: 'A4',
      letter: 'US Letter',
      a3: 'A3',
      tabloid: 'Tabloid (11 × 17 in)',
      a0: 'A0 (copy shop)',
    } satisfies Record<PaperName, string>,
    custom: 'Custom size',
    orientation: 'Orientation',
    portrait: 'Portrait',
    landscape: 'Landscape',
    joining: 'Joining the pages',
    trim: 'Trim and butt',
    trimDetail: 'Cut off the margin on two sides and lay the pages edge to edge.',
    overlap: 'Overlap',
    overlapDetail: 'Lay each page over the next by a strip and match the circles.',
    overlapWidth: 'Overlap strip',
    margin: 'Margin',
    marginHint: 'Most printers cannot print right up to the edge of the paper.',
    notches: 'Notches',
    slit: 'Slit into the allowance',
    v: 'V outside the line',
    stitchLine: 'Stitching line (dashed)',
    testSquare: 'Test square',
    pageMap: 'Page map on the first page',
    skipEmpty: 'Leave out empty pages',
    planning: 'Laying out the pages…',
    summary: (pages: number) => plural(pages, '1 page', '# pages'),
    grid: (columns: number, rows: number) => `${columns} across, ${rows} down`,
    cover: 'including the page with the test square',
    skipped: (n: number) => plural(n, '1 empty page left out', '# empty pages left out'),
    preview: (pages: number) => `Layout of the ${plural(pages, 'page', '# pages')}`,
    unprintable: (names: string[]) =>
      `Some letters in ${namesList(names)} cannot be printed yet and will show as “?”.`,
    save: 'Save PDF',
    saving: 'Making the PDF…',
    saved: (name: string) => `Saved ${name}. Print it at 100% and measure the test square.`,
    failed: (message: string) => `The PDF could not be made. ${message}`,
    hint: 'Print the PDF at 100% (“Actual size”), never “Fit to page”, then measure the test square before you cut.',
    /** Words printed in the PDF. The PDF font has Western European letters only. */
    pdf: {
      subtitle: (pages: number, paper: string, joining: string) =>
        `${plural(pages, '1 page', '# pages')} of ${paper} · ${joining.toLowerCase()}`,
      scale:
        'Print at 100% (“Actual size” or “Scale: 100”), never “Fit to page”, on one side of the paper.',
      measure:
        'Measure the test square before cutting: it must be exactly 10 cm (4 in). If it is not, check the print settings and print again.',
      trim: 'Trim along the frame on the left and top of each page, except in column 1 and row A. Butt each page against its neighbours so the diamonds and page letters match, then tape them together.',
      overlap:
        'Trim the left and top margins of each page, except in column 1 and row A. Lay each page over its neighbour up to the dashed line so the circles match, then tape them together.',
      skipped: 'Pages with nothing on them are left out; they are dashed on the map.',
      squareMetric: '10 cm',
      squareImperial: '4 in',
      pageMap: 'Page map',
      notPrinted: 'Dashed: not printed (nothing on it)',
      placeOnFold: 'Place on fold',
    },
  },

  projects: {
    heading: 'Projects',
    open: (name: string) => `Projects (open now: ${name})`,
    name: 'Project name',
    openNow: 'Open now',
    new: 'New project',
    restore: 'Restore a backup…',
    backup: 'Back up',
    backupHint: 'Save a .drapewright file of this project, with the files it came from.',
    empty: 'No other projects on this device yet.',
    loading: 'Looking for projects…',
    summary: (pieces: number, modified: string) =>
      `${plural(pieces, '1 piece', '# pieces')} · ${new Date(modified).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' })}`,
    delete: (name: string) => `Delete “${name}”`,
    deleteQuestion: (name: string) => `Delete “${name}” from this device?`,
    deleteWarning: 'This cannot be undone. Back it up first if you might need it again.',
    deleteConfirm: 'Delete',
    cancel: 'Cancel',
    deleted: (name: string) => `Deleted “${name}”.`,
    opened: (name: string) => `Opened “${name}”.`,
    openFailed: (message: string) => `This project could not be opened. ${message}`,
    restored: (name: string) => `Restored “${name}”.`,
    restoredName: (name: string) => `${name} (restored)`,
    restoreFailed: (message: string) => `This backup could not be restored. ${message}`,
    backedUp: (file: string) =>
      `Saved ${file}. Keep it somewhere safe, such as Files or a cloud drive.`,
    backupFailed: (message: string) => `The backup could not be made. ${message}`,
    needsChecking: (n: number) =>
      plural(
        n,
        '1 thing in this project needs checking.',
        '# things in this project need checking.',
      ),
    saving: 'Saving…',
    saved: 'Saved',
    savedLong: 'Saved on this device',
    notSaved: 'Not saved',
    notSavedLong: (message: string) =>
      `Not saved on this device: ${message} Use Back up to keep a copy.`,
  },

  screen: {
    open: 'Measure the screen',
    openUnmeasured: 'Measure the screen (not measured yet, so 100% is approximate)',
    heading: 'Measure your screen',
    intro:
      'Hold a bank card, or any card that size, against the screen and slide until the outline matches the card’s edges. Then 100% shows pieces at their real size.',
    card: 'Card outline, 85.6 × 54 mm',
    size: 'Outline size',
    smaller: 'A little smaller',
    larger: 'A little larger',
    ruler: 'Or measure this line with a ruler: it should be exactly 5 cm.',
    rulerLabel: '5 cm line',
    reset: 'Standard size',
    cancel: 'Cancel',
    save: 'Save',
    saved: 'Screen measured: at 100% pieces show at their real size.',
    standard: 'Back to the standard size: 100% is approximate.',
  },

  crash: {
    heading: 'Something went wrong',
    body: 'Drapewright hit a problem showing this screen. Reloading usually fixes it.',
    reload: 'Reload',
  },

  device: {
    heading: 'This device',
    checking: 'Checking…',
    storage: [
      'Saves on this device',
      'Projects stay private, in this browser’s own storage.',
      'This browser cannot store projects. Use Back up often.',
    ],
    offline: [
      'Works without internet',
      'After this first visit the app opens offline.',
      'Offline use is not available in this browser.',
    ],
    gpu: [
      '3D draping',
      'Your graphics chip can run the fabric simulation.',
      '3D will show saved drapes only. Try the latest Chrome or Safari.',
    ],
    kept: [
      'Kept safe',
      'The browser will not clear saved projects to free up space.',
      'The browser may clear saved projects if space runs low. Back up the ones you care about.',
    ],
    isolation: [
      'Fast mode',
      'Cross-origin isolated, for multi-core work.',
      'The page is not served with isolation headers.',
    ],
    engineLoading: 'Loading pattern engine…',
    engineVersion: (v: string) => `Pattern engine ${v}`,
    engineFailed: (e: string) => `Pattern engine failed to load: ${e}`,
  },
} as const
