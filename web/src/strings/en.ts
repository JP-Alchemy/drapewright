// Everything the app says, in plain sewing language. One place, so wording
// stays consistent and can be translated later.

import type {
  AllowanceNote,
  AvatarPose,
  CornerStyle,
  DisplayUnits,
  ClothShows,
  EdgeKind,
  FabricPreset,
  ImportNote,
  LayNote,
  MeasurementGroup,
  OneSize,
  Reason,
  MeasurementKey,
  NotchFrom,
  NotchKind,
  PageLeftOut,
  PageUse,
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

/** A test square's side as printed on it: whole centimetres, or whole inches. */
const squareSize = (mm: number) => {
  const inches = mm / 25.4
  return mm % 10 !== 0 && Math.abs(inches - Math.round(inches)) < 0.01
    ? `${Math.round(inches)} in`
    : `${Math.round(mm / 10)} cm`
}

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

  sizing: {
    heading: 'Which size fits?',
    noPeople:
      'To see which size fits, add someone’s measurements first (Measurements, at the top).',
    person: 'Whose measurements',
    fit: 'How it should fit',
    fits: {
      fitted: 'Fitted',
      relaxed: 'Relaxed',
      close: 'Close (corsets)',
      stretch: 'Stretch (knits, smaller than the body)',
    } as Record<string, string>,
    measure: (name: string) => `Sew and measure every size on ${name}`,
    measuring: 'Sewing and measuring every size…',
    failed: (why: string) => `The sizes couldn’t be measured. ${why}`,
    suggested: (name: string, parts: string[]) =>
      `For ${name}: ${new Intl.ListFormat('en', { type: 'conjunction' }).format(parts)}.`,
    at: (level: string, size: string) => `${size} at the ${level}`,
    size: 'Size',
    columns: { bust: 'Bust', waist: 'Waist', hip: 'Hip' } as Record<string, string>,
    chooseColumn: 'Choose',
    choose: (size: string) => `Choose ${size}`,
    notSewn: 'Couldn’t be sewn round the figure',
    suggestedHere: 'Suggested',
    ease: (sign: string, by: string) => `${sign}${by}`,
    how: 'Each size is sewn as Find seams would sew it, and measured flat round the figure’s bust, waist and hip. The ease is how much bigger than the body it is there: the size suggested at each has the ease nearest to the fit chosen.',
    cutAt: 'Size to cut at each',
    blend: 'Blend between them',
    blending: 'Blending the sizes…',
    blendFailed: (why: string) => `The sizes couldn’t be blended. ${why}`,
    blendHow:
      'Each piece is cut in the size chosen at each level, and blended smoothly between them up and down the body, as you would blend between the lines of a printed pattern.',
    /** A blended piece's notes. */
    blendedNote: (parts: string[]) =>
      `Blended between sizes: ${new Intl.ListFormat('en', { type: 'conjunction' }).format(parts)}.`,
    blended: (count: number, parts: string[]) =>
      `${plural(count, 'One piece', '# pieces')} blended between sizes: ${new Intl.ListFormat('en', { type: 'conjunction' }).format(parts)}.`,
    oneSize: (piece: string, size: string, why: OneSize, top: string, bottom: string) =>
      ({
        oneLevel: `“${piece}” is cut in ${size} all over: it sits where that size is chosen.`,
        roundAnArm: `“${piece}” goes round the arm, so it is cut in ${size}, the ${top}’s size.`,
        betweenLegs: `“${piece}” goes between the legs, so it is cut in ${size}, the ${bottom}’s size.`,
        notWorn: `“${piece}” isn’t worn round the body, so it is cut in ${size}, the ${top}’s size.`,
        notInEverySize: `“${piece}” isn’t in every size between, so it is cut in ${size}.`,
      })[why],
  },

  drafting: {
    heading: 'Draft a FreeSewing design',
    open: 'Draft a FreeSewing design to these measurements',
    intro:
      'FreeSewing’s designs are drafted to each person’s measurements, not cut from standard sizes. Choose one: it is drafted here, on this device, and opened as a pattern to add.',
    noPeople:
      'Add someone’s measurements first (Measurements, at the top): the design is drafted to them.',
    whose: 'Whose measurements',
    which: 'Design',
    designs: {
      teagan: ['Teagan T-shirt', 'A T-shirt for knit fabrics'],
      sasha: ['Sasha dress', 'A skater dress with princess seams and pockets'],
      penelope: ['Penelope pencil skirt', 'A fitted pencil skirt with darts'],
      sandy: ['Sandy circle skirt', 'A circle skirt with a waistband'],
      cathrin: ['Cathrin corset', 'An underbust corset or waist trainer'],
      bella: ['Bella bodice block', 'A womenswear bodice block, to design from'],
      simone: ['Simone shirt', 'A button-down shirt for people with breasts'],
      diana: ['Diana draped top', 'A top with a draped neck'],
    } satisfies Record<string, [string, string]>,
    uses: (name: string, used: string[]) =>
      used.length > 0
        ? `Drafted to ${name}’s ${new Intl.ListFormat('en', { type: 'conjunction' }).format(used)}.`
        : `None of ${name}’s measurements are ones it uses.`,
    guessed: (names: string[]) =>
      `Estimated from FreeSewing’s standard sizes, in proportion: ${new Intl.ListFormat('en', { type: 'conjunction' }).format(names)}. Measure them for a closer fit.`,
    forWhom: (name: string) => `for ${name}`,
    credit: 'Designs by FreeSewing (Joost De Cock and contributors), shared under the MIT licence.',
    cancel: 'Cancel',
    draft: (design: string) => `Draft ${design}`,
    drafting: 'Drafting…',
    failed: (why: string) => `It couldn’t be drafted. ${why}`,
  },

  feedback: {
    heading: 'Feedback',
    intro:
      'Drapewright is made by one person, for and with the people who sew. What works, what doesn’t, what you wish it did: all of it helps.',
    email: 'Email feedback',
    linkedin: 'Connect on LinkedIn',
    subject: 'Drapewright feedback',
    body: (about: string[]) => `\n\n\n—\n${about.join('\n')}`,
    version: (build: string, engine: string) => `Drapewright ${build}, engine ${engine}`,
    browser: (agent: string) => `Browser: ${agent}`,
    notRight: 'Didn’t open right?',
    notRightHow:
      'Say what’s wrong, then email the report. It says what was read from the file, not the file itself: attach the file too only if you may share it.',
    whatsWrong: 'What’s wrong',
    whatsWrongPlaceholder: 'The back is missing; the sleeve came in twice…',
    emailReport: 'Email the report',
    saveReport: 'Save the report',
    reportSubject: (file: string) => `Drapewright: “${file}” didn’t open right`,
    reportBody: (said: string, report: string) =>
      `${said || '(What’s wrong:)'}\n\n— What Drapewright read —\n${report}`,
    report: {
      file: (name: string, kb: number, fingerprint: string) =>
        `File: ${name} (${kb} KB, ${fingerprint}…)`,
      scale: (basis: string, certain: boolean) => `Scale: ${basis}${certain ? '' : ' (asked)'}`,
      pages: (read: number, all: number, chosen: number[] | null) =>
        `Pages: ${read} of ${all} read${chosen ? ` (chosen: ${chosen.join(', ')})` : ''}`,
      sizes: (names: string[]) => `Sizes: ${names.join(', ')}`,
      pieces: (n: number) => `Pieces found: ${n}`,
      piece: (name: string, w: number, h: number, edges: number, cut: number) =>
        `  ${name}: ${w} × ${h} mm, ${edges} edges, cut ${cut}`,
      notes: 'Notes:',
    },
  },

  layout: {
    open: 'Layout',
    openHint: 'How the pieces lie on the fabric to cut, and how much fabric they take.',
    heading: 'Cutting layout',
    material: 'Fabric',
    width: 'Fabric width',
    otherWidth: 'Another width…',
    ownWidth: 'Width',
    folded: 'Folded in half lengthwise',
    oneWay: 'Nap or one-way print',
    takes: (length: string, width: string, material: string, folded: boolean, used: number) =>
      `It takes ${length} of ${material} ${width} wide${folded ? ', folded' : ''}, ${used}% of it cut into pieces. Buy a little more to straighten its ends.`,
    nothing: 'No piece is cut from this fabric.',
    note: (kind: LayNote['kind'], piece: string) =>
      ({
        tooWide: `“${piece}” is wider than the fabric: it isn’t laid out. Open the fabric out, or choose a wider one.`,
        oneLayer: `“${piece}” is cut through the top layer only: fold the bottom one back out of the way.`,
        noGrain: `“${piece}” has no grainline: it is laid along the fabric as it is drawn.`,
      })[kind],
    how: 'Each piece is laid with its grainline along the fabric, the biggest first, each as far along as it fits. Folded, a pair is cut at once through both layers, and a piece cut on the fold goes on the fabric’s fold. Pieces marked face down are the other of a pair, cut with the pattern turned over.',
    picture: (n: number) =>
      plural(n, 'One piece laid out on the fabric', '# pieces laid out on the fabric'),
    fold: 'Fold',
    selvage: 'Selvages',
    faceDown: '(face down)',
    failed: (why: string) => `The pieces couldn’t be laid out. ${why}`,
  },

  fitting: {
    heading: 'Fit',
    /** How each fit is named in a sentence. */
    fitNames: {
      fitted: 'fitted',
      relaxed: 'relaxed',
      close: 'close-fitting',
      stretch: 'a stretch fit',
    } as Record<string, string>,
    fits: (fit: string, levels: string[]) =>
      `It fits as ${fit} should at the ${new Intl.ListFormat('en', { type: 'conjunction' }).format(levels)}.`,
    tight: (by: string, level: string) => `${by} too tight at the ${level}`,
    loose: (by: string, level: string) => `${by} too loose at the ${level}`,
    off: (fit: string, wrong: string[], right: string[]) =>
      `For ${fit}: ${new Intl.ListFormat('en', { type: 'conjunction' }).format(wrong)}${
        right.length > 0
          ? `; right at the ${new Intl.ListFormat('en', { type: 'conjunction' }).format(right)}`
          : ''
      }.`,
    alter: 'Alter to fit',
    altering: 'Altering…',
    letOut: (by: string, level: string) => `let out ${by} at the ${level}`,
    takenIn: (by: string, level: string) => `taken in ${by} at the ${level}`,
    altered: (done: string[], pieces: string[]) => {
      const said = new Intl.ListFormat('en', { type: 'conjunction' }).format(done)
      return `${said.charAt(0).toUpperCase()}${said.slice(1)}, at the side seams of ${namesList(pieces)}: each a step on its piece, to change or switch off. It drapes again to show.`
    },
    nothingAltered: 'No side seam could be let out or taken in.',
    missed: (level: string) => `No seam crosses the ${level} to let out or take in there.`,
    failed: (why: string) => `It couldn’t be altered to fit. ${why}`,
  },

  samples: {
    heading: 'No pattern to hand? Try one of ours:',
    titles: {
      blouse: 'A blouse, ready to drape',
      skirt: 'A skirt, ready to drape',
      dress: 'A dress from a PDF pattern',
      freesewing: 'A FreeSewing design, made to measure',
    },
    about: {
      blouse: 'Sewn already: see it on the figure, read where it pulls, alter it.',
      skirt: 'A front and a back, sewn at the sides, to drape in any fabric.',
      dress: 'FreeSewing’s Sasha dress as a copy-shop PDF: see it read, piece by piece.',
      freesewing: 'A T-shirt, a dress, skirts, a corset: drafted to fit someone you have measured.',
    },
    names: { blouse: 'Sample blouse', skirt: 'Sample skirt', dress: 'Sample dress' },
    opened: (name: string) =>
      `“${name}” is open as a project of its own: press Drape beside the figure to see it on.`,
    failed: (why: string) => `The sample couldn’t be opened. ${why}`,
    unavailable: 'It isn’t on this device yet; open it once while online.',
    credit: 'The Sasha dress is by FreeSewing (Joost De Cock), shared under the MIT licence.',
    licence: 'Licence',
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
    nothing:
      'Tap a piece to see its details. Tap an edge or a corner to adjust it. To set several pieces at once, press Select above the list.',
    manyHeading: (n: number) => `${n} pieces`,
    manyHint: (names: string[]) =>
      `Changes here apply to all of them: ${namesList(names)}. Drag one on the table to move them together.`,
    mixed: 'Mixed',
    increase: (label: string) => `Increase ${label}`,
    decrease: (label: string) => `Decrease ${label}`,
    name: 'Name',
    allowance: 'Allowance',
    allowanceSummary: (included: boolean, seam: string, hem: string | null) =>
      `${included ? 'Included' : 'Added'} · ${seam}${hem ? `, hems ${hem}` : ''}`,
    someIncluded: 'Some have their allowance included and some don’t: choose for all of them.',
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
    oneWayShort: 'One way',
    bothWays: 'Both ways',
    noneShort: 'None',
  },

  pick: {
    prompts: {
      spread: [
        'Tap the edge where the slash starts.',
        'Tap the edge across the piece where it hinges.',
        'Tap the part that turns.',
        '',
      ],
      bust: [
        'Tap the bust point.',
        'Tap the armhole where it hinges, about a third of the way up from the underarm.',
        'Tap the side seam where the dart goes.',
        '',
      ],
      pivot: ['Tap the edge where the dart goes.', ''],
      swayback: [
        'Tap the centre back where the wedge is taken out.',
        'Tap the side seam where it tapers to nothing.',
        '',
      ],
      shoulder: [
        'Tap the shoulder seam about halfway along.',
        'Tap the armhole where it hinges, about level with its notch.',
        '',
      ],
      bustSide: [
        'Tap the side seam at the bust.',
        'Tap the side seam at the waist, where it tapers to nothing.',
        '',
      ],
      waist: [
        'Tap the side seam at the waist.',
        'Tap the side seam at the hip, where it tapers to nothing.',
        '',
      ],
      hip: [
        'Tap the side seam at the hip.',
        'Tap the side seam at the waist, where it tapers to nothing.',
        '',
      ],
      trueUp: ['Tap the edge to true.', ''],
    },
    whichDart: 'Tap the dart to move.',
    cancel: 'Cancel',
  },

  projector: {
    open: 'Projector',
    openHint: 'Project the pieces true size onto fabric on the cutting table.',
    heading: 'Projector',
    calibrate: 'Calibrate the projector',
    calibrateHow:
      'Mark a rectangle on the mat (with tape, or along the mat’s own lines) and say how big it is. Then drag the four corners onto its corners.',
    width: 'Rectangle across',
    height: 'Rectangle down',
    done: 'Done',
    close: 'Close',
    recalibrate: 'Calibrate',
    slide: 'Drag to slide the pattern across the fabric.',
    stitching: 'Stitching lines',
    thick: 'Thick lines (for dark fabric)',
    shows: 'What to project',
    asOnTable: 'As on the table',
    asLaidOut: 'Cutting layout',
    layFabric: (folded: boolean, width: string) =>
      folded
        ? `Fabric ${width} wide, folded: lay its fold along the dashed line, its selvages along the other, its end at the line across.`
        : `Fabric ${width} wide: lay its selvages along the two lines, its end at the line across.`,
    pieces: 'Piece by piece',
    previous: 'Previous piece',
    next: 'Next piece',
    allPieces: 'All pieces',
    onePiece: (name: string, k: number, n: number) => `${name} (${k} of ${n})`,
    showAll: 'All',
    calibrationLabel: 'The rectangle to calibrate by, with its corners to drag.',
    projectionLabel: 'The pieces, true size.',
  },

  fabric: {
    heading: 'Measure a fabric',
    change: (name: string) => `Change “${name}”`,
    intro:
      'Measure what you can of your own fabric; what you leave empty is taken from the ready-made fabric it is most like.',
    name: 'Name',
    namePlaceholder: 'Blue linen from the market',
    like: 'Most like',
    likeHint: 'Woven or knitted, light or heavy: the closest of the ready-made fabrics.',
    weight: 'A 10 cm square weighs',
    weightHow: 'Cut a square 10 cm by 10 cm and weigh it on a kitchen scale.',
    along: 'Along the grain, 10 cm stretches to',
    across: 'Across the grain, 10 cm stretches to',
    stretchHow: (along: boolean) =>
      `Cut a strip 5 cm wide ${along ? 'along' : 'across'} the grain and mark 10 cm on it. Hang 500 g from it (a bag of sugar, pinned on) and measure the 10 cm again.`,
    overhang: 'Slides out over a table’s edge',
    overhangHow:
      'Cut a strip 2.5 cm wide and 20 cm long along the grain. Lay it on a table and slide it slowly out over the edge until its end droops to 45° (a sheet of paper folded corner to corner shows the angle). Measure how far it sticks out.',
    remove: 'Remove',
    cancel: 'Cancel',
    save: 'Save',
  },

  alter: {
    heading: 'Alterations',
    summary: (n: number) => (n === 0 ? 'None' : plural(n, '1 step', '# steps')),
    add: 'Add a step',
    kinds: {
      lengthen: ['Lengthen or shorten', 'Across a line'],
      spread: ['Slash and spread', 'Open or close a slash'],
      bust: ['Bust adjustment', 'Full or small bust, on a front'],
      shoulder: ['Shoulders', 'Broader or narrower'],
      swayback: ['Swayback', 'A wedge out of the centre back'],
      bustSide: ['Bust at the side', 'At a side seam, tapering to the waist'],
      waist: ['Waist', 'At a side seam, tapering to the hip'],
      hip: ['Hip', 'At a side seam, from the hip down'],
      pivot: ['Move a dart', 'Closed where it is, opened elsewhere'],
      trueUp: ['True up an edge', 'Its jogs and corners smoothed, or made straight'],
      stretch: ['Fabric stretch', 'For a fabric that stretches more or less than the pattern’s'],
    } as Record<string, [string, string]>,
    none: 'None yet. Each change is kept as a step that can be switched off: the piece as drawn is kept too.',
    lengthen: 'Lengthen or shorten',
    step: (longer: boolean, by: string) => `${longer ? 'Lengthened' : 'Shortened'} by ${by}`,
    longer: 'Lengthen',
    shorter: 'Shorten',
    way: 'Lengthen or shorten',
    spread: 'Slash and spread',
    spreadStep: (open: boolean, by: string) => `Slashed and ${open ? 'spread' : 'closed'} ${by}`,
    open: 'Spread',
    close: 'Close',
    spreadWay: 'Spread or close',
    otherSide: 'Turn the other side instead',
    bust: 'Bust adjustment',
    bustStep: (full: boolean, by: string) => `${full ? 'Full' : 'Small'} bust adjustment, ${by}`,
    full: 'Full bust',
    small: 'Small bust',
    bustWay: 'Full or small bust',
    bustHint: 'Measured across the bust on this half: the whole front changes by twice as much.',
    swaybackStep: (out: boolean, by: string) =>
      out ? `Swayback, ${by} taken out` : `${by} let out at the centre back`,
    swaybackWay: 'Take out or let out',
    takeOut: 'Take out',
    letOut: 'Let out',
    swaybackHint: 'At the centre back, tapering to nothing at the side seam.',
    shoulderStep: (broader: boolean, by: string) =>
      `Shoulders ${by} ${broader ? 'broader' : 'narrower'}`,
    shoulderWay: 'Broader or narrower',
    broader: 'Broader',
    narrower: 'Narrower',
    shoulderHint: 'How far the end of the shoulder seam moves along it.',
    stretchStep: (percent: number, bothWays: boolean) =>
      percent === 0
        ? 'As drawn: the fabric stretches as the pattern’s does'
        : `${percent < 0 ? 'Narrowed' : 'Widened'} ${Math.abs(percent)}% ${bothWays ? 'both ways' : 'across the grain'} for the fabric’s stretch`,
    patternStretch: 'Made for a fabric that stretches',
    patternStretchHint: 'As the pattern says: “for fabrics with 20% stretch”.',
    fabricStretch: 'This fabric stretches',
    fabricStretchHint:
      'Mark 10 cm across the grain and stretch it gently as far as it goes: to 13 cm is 30%.',
    bothWays: 'Both ways (four-way stretch)',
    everyPiece: (n: number) =>
      plural(
        n,
        'The same on the other piece of this fabric',
        'The same on the # other pieces of this fabric',
      ),
    everyPieceDone: (n: number) => plural(n, 'Added to one more piece.', 'Added to # more pieces.'),
    bustSideStep: (larger: boolean, by: string) =>
      `Bust ${larger ? 'let out' : 'taken in'} ${by} at the side`,
    waistStep: (larger: boolean, by: string) =>
      `Waist ${larger ? 'let out' : 'taken in'} ${by} at the side`,
    hipStep: (larger: boolean, by: string) =>
      `Hip ${larger ? 'let out' : 'taken in'} ${by} at the side`,
    sideWay: 'Let out or take in',
    sideHint:
      'At this side seam. A front and a back, each cut on the fold, change the whole garment by four times as much.',
    trueStep: (edge: string, straight: boolean) =>
      `${edge.charAt(0).toUpperCase()}${edge.slice(1)} trued ${straight ? 'straight' : 'smooth'}`,
    trueWay: 'Straight or smooth',
    straight: 'Straight',
    smooth: 'Smooth',
    trueHint:
      'Straight: a straight line between its ends (and its darts). Smooth: each jog and corner along it blended into a curve.',
    pivot: 'Move a dart',
    pivotStep: (share: number) =>
      share >= 1
        ? 'Dart moved'
        : share === 0.5
          ? 'Half a dart moved'
          : `${Math.round(share * 100)}% of a dart moved`,
    pivotHint: 'Turned about its tip: the dart closes where it was and opens where it goes.',
    share: 'How much of it',
    all: 'All of it',
    half: 'Half',
    by: 'By how much',
    where: 'Where',
    across: 'Straight across the piece',
    ownLine: (n: number, of: number) =>
      of === 1 ? 'Its lengthen or shorten line' : `Its lengthen or shorten line ${n}`,
    fromTop: 'From the top',
    remove: (step: string) => `Remove the step “${step}”`,
    asDrawn: 'Dashed: the piece as drawn.',
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
    someMirrored: 'Some are cut as mirrored pairs and some aren’t.',
    onFold: 'On the fold',
    add: 'Also cut in another fabric',
    remove: 'Remove this line',
    summary: (quantity: number, material: string, onFold: boolean) =>
      `Cut ${quantity}${onFold ? ' on the fold' : ''} · ${material}`,
    short: (quantity: number, onFold: boolean) => `Cut ${quantity}${onFold ? ' on the fold' : ''}`,
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

  sidePanel: {
    resize: 'Resize the lists and the details',
  },

  pieces: {
    heading: 'Pieces',
    select: 'Select',
    selectSeveral: 'Select several pieces',
    selectAll: 'Select all',
    done: 'Done',
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
    /** A PDF's pages: how many were read, and those left out (by number). */
    pagesRead: (read: number, total: number) => `Read ${read} of the ${total} pages.`,
    page: (p: PageUse) => (p.label ? `Page ${p.number} (${p.label})` : `Page ${p.number}`),
    pagesLeftOut: (pages: PageUse[]): string => {
      if (pages.length > 3) {
        return `Left out ${pages.length} pages that are not part of the pattern sheet.`
      }
      const which = new Intl.ListFormat('en', { type: 'conjunction' }).format(
        pages.map((p) => (p.label ? `${p.number} (${p.label})` : String(p.number))),
      )
      const why = pages.every((p) => p.leftOut === 'instructions')
        ? 'instructions, not pattern'
        : 'not part of the pattern sheet'
      return `Left out ${plural(pages.length, 'page', 'pages')} ${which}: ${why}.`
    },
    pageLeftOut: (why: PageLeftOut): string =>
      ({
        instructions: 'instructions',
        notJoined: 'not part of the sheet',
        notChosen: 'not chosen',
      })[why],
    choosePages: 'Choose pages…',
    pagesToRead: 'Pages to read',
    pagesHint:
      'Tick the pages that hold the pattern. Pages of instructions, and pages that are not part of the pattern sheet, are left out on their own.',
    readPages: (n: number) => plural(n, 'Read this page', 'Read these # pages'),
    readAllPages: 'Read as at first',
    piecesToAdd: 'Pieces to add',
    untickHint: 'Untick anything that is not a pattern piece, such as a diagram or a title box.',
    sizeHint:
      'The file has several sizes of this pattern. Pick yours, or more than one to compare them.',
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
        case 'letteringIgnored':
          return 'The words on this pattern are drawn as lines, not written as text. Drapewright read the pieces’ names and how many to cut as best it could, so check them after adding. A piece it couldn’t name is called “Piece” and a number.'
        case 'testSquareFound':
          return `Found the ${squareSize(n.size)} test square and left it out of the pieces. Measure it on your print before cutting.`
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

  seams: {
    heading: 'Seams',
    none: 'No seams yet. Find them, or pick an edge and choose Sew to another edge.',
    find: 'Find seams',
    finding: 'Edges that look sewn together',
    noneFound: 'No more likely seams found. Pick an edge to sew it by hand.',
    sew: 'Sew',
    sewAll: (n: number) => plural(n, 'Sew it', 'Sew all #'),
    cutAsPairs: (names: string[]) =>
      plural(
        names.length,
        `${namesList(names)} looks drawn for one side, so it’s now cut 2, a left and a right. Undo to cut it once.`,
        `${namesList(names)} look drawn for one side, so they’re now cut 2, a left and a right. Undo to cut them once.`,
      ),
    notThis: 'Not this',
    sewn: 'Sewn. Its number shows on both edges.',
    seam: (n: number, name: string) => (name ? `${n} · ${name}` : `Seam ${n}`),
    between: (a: string, b: string) => `${a} ↔ ${b}`,
    otherHalf: (piece: string) => `${piece}, other half`,
    partOf: (edge: string) => `part of ${edge}`,
    sameLength: 'Same length',
    lengths: (a: string, b: string) => `${a} and ${b}`,
    longer: (which: 'first' | 'second', by: string) =>
      which === 'first' ? `the first side ${by} longer` : `the second side ${by} longer`,
    notchesMeet: 'notches meet',
    notchesApart: (by: string) => `notches up to ${by} apart`,
    notchAlone: (n: number) => plural(n, 'a notch without a partner', '# notches without partners'),
    problem: 'An edge of this seam is gone: unpick it.',
    turn: 'Turn round',
    turnHint: 'Sews the first side’s start to the second side’s end instead.',
    unpick: 'Unpick',
    likely: (confidence: number) =>
      confidence >= 0.75 ? 'Very likely' : confidence >= 0.5 ? 'Likely' : 'Possible',
    reasons: {
      sameLength: 'same length',
      sameShape: 'same curve',
      notchesMatch: 'notches match',
      linesMeet: 'lines across meet',
      sameName: 'same name',
      endsMatch: 'hems meet',
      frontAndBack: 'front to back',
      centreOfPair: 'centre of a pair',
      gathered: 'gathered to fit',
      continues: 'goes on from a seam',
      fitsOpening: 'fits round the opening',
      fitsAlong: 'fits along the opening',
    } satisfies Record<Reason, string>,
    sewTo: 'Sew to another edge…',
    sewingHint: (edge: string) =>
      `Tap the edge to sew to ${edge}. Tap it again to sew it to the other half of a pair.`,
    cancel: 'Cancel',
    sewnTo: (other: string) => `Sewn to ${other}`,
  },

  measure: {
    open: 'Measurements',
    heading: 'Measurements',
    intro:
      'Measure over light clothing, with the tape snug but not tight. Leave out anything you don’t have: only height, bust, waist and hip are needed to start.',
    empty: 'No measurements yet.',
    addFirst: 'Add my measurements',
    add: 'Add a person',
    defaultName: 'Me',
    newName: (n: number) => `Person ${n}`,
    people: 'People',
    name: 'Name',
    figure: 'Figure',
    figures: { woman: 'Woman’s', man: 'Man’s' },
    takenOn: 'Measured on',
    revision: (n: number, date: string | null) =>
      date ? `Measurement ${n}, taken ${date}` : `Measurement ${n}`,
    again: 'Measure again',
    againHint:
      'Starts a new measurement from today’s date. These values are kept as the earlier one.',
    earlier: (n: number) => plural(n, '1 earlier measurement kept', '# earlier measurements kept'),
    remove: 'Delete',
    removeConfirm: (name: string) => `Delete ${name}’s measurements?`,
    clear: (name: string) => `Clear ${name}`,
    howTo: 'How to measure',
    unusual: (name: string) => `Unusual for ${name.toLowerCase()}: check it is in the right units.`,
    done: 'Done',
    groups: {
      height: 'Height',
      around: 'Around the body',
      torso: 'Torso lengths',
      arms: 'Arms',
      legs: 'Legs',
    } satisfies Record<MeasurementGroup, string>,
    names: {
      height: [
        'Height',
        'Barefoot, standing against a wall: from the floor to the top of the head.',
      ],
      bust: ['Bust', 'Around the fullest part of the bust, the tape level across the back.'],
      highBust: [
        'High bust',
        'Around the chest just under the arms, above the bust, level across the back.',
      ],
      underbust: ['Underbust', 'Around the ribcage just below the bust, level.'],
      waist: [
        'Waist',
        'Around the natural waist, the narrowest part. Bend to one side: the crease is the waist.',
      ],
      highHip: ['High hip', 'Around the top of the hip bones, about 10 cm below the waist.'],
      hip: ['Hip', 'Around the fullest part of the hips and seat, feet together, the tape level.'],
      neck: ['Neck', 'Around the base of the neck, where a collar sits, over the nape bone.'],
      shoulderWidth: [
        'Shoulder width',
        'Across the back from one shoulder point (the bony tip at the top of the arm) to the other.',
      ],
      shoulderLength: ['Shoulder length', 'From the side of the neck to the shoulder point.'],
      acrossFront: [
        'Across front',
        'Across the front from armpit crease to armpit crease, halfway between shoulder and underarm.',
      ],
      acrossBack: [
        'Across back',
        'Across the back from armpit crease to armpit crease, halfway between nape and underarm.',
      ],
      backWaistLength: [
        'Back waist length',
        'From the nape bone (the bump at the base of the neck) straight down to the waist.',
      ],
      frontWaistLength: [
        'Front waist length',
        'From the side of the neck at the shoulder, over the bust point, down to the waist.',
      ],
      bustSpan: ['Bust span', 'From one bust point to the other.'],
      shoulderToBust: [
        'Shoulder to bust point',
        'From the side of the neck at the shoulder down to the bust point.',
      ],
      waistToHip: ['Waist to hip', 'Down the side from the waist to the fullest part of the hips.'],
      armLength: [
        'Arm length',
        'From the shoulder point, over a slightly bent elbow, to the wrist bone.',
      ],
      upperArm: ['Upper arm', 'Around the fullest part of the upper arm, relaxed.'],
      wrist: ['Wrist', 'Around the wrist, over the wrist bone.'],
      inseam: ['Inside leg', 'Barefoot: from the crotch down the inside of the leg to the floor.'],
      outseam: ['Outside leg', 'Down the side from the waist to the floor, barefoot.'],
      crotchDepth: [
        'Crotch depth',
        'Sitting on a hard chair: down the side from the waist to the seat.',
      ],
      thigh: ['Thigh', 'Around the fullest part of the thigh.'],
      knee: ['Knee', 'Around the knee, slightly bent.'],
      calf: ['Calf', 'Around the fullest part of the calf.'],
      ankle: ['Ankle', 'Around the ankle, over the ankle bone.'],
    } satisfies Record<MeasurementKey, readonly [string, string]>,
    /** Men's figures say chest, not bust. */
    chest: ['Chest', 'Around the fullest part of the chest, under the arms, the tape level.'],
    avatar: {
      heading: 'The avatar',
      intro:
        'A figure shaped to these measurements, to drape patterns on. Fit it to see how close it comes.',
      age: 'Age',
      ageHint: 'The avatar is an adult’s, 25 to 90; 35 if left empty.',
      fit: 'Fit the avatar',
      again: 'Fit again',
      fitting: 'Fitting…',
      need: 'Enter some measurements first: height, bust, waist and hip are enough to start.',
      changed: 'These measurements have changed since the avatar was fitted.',
      failed: (why: string) => `The avatar could not be fitted: ${why}.`,
      noBodyFile:
        'The avatar’s figure hasn’t been downloaded yet: connect to the internet once and fit it again. After that it works offline too.',
      report: 'How close the avatar comes',
      columns: ['Measurement', 'Measured', 'Avatar', 'Difference'],
      same: 'Same',
      larger: (by: string) => `${by} larger`,
      smaller: (by: string) => `${by} smaller`,
      longer: (by: string) => `${by} longer`,
      shorter: (by: string) => `${by} shorter`,
      summary: (close: number, of: number) =>
        close === of
          ? plural(
              of,
              'The avatar matches the measurement to within 1 cm.',
              'The avatar matches all # measurements to within 1 cm.',
            )
          : `The avatar matches ${close} of ${of} measurements to within 1 cm; the others are as close as its figure can be shaped.`,
      notTaken: (names: string) => `Not on the avatar: ${names}.`,
    },
  },

  avatar: {
    savePicture: 'Save a picture of the figure',
    saveVideo: 'Save a video turning round the figure',
    makingVideo: 'Making the video…',
    onTheFigure: (project: string) => `${project} on the figure`,
    saved: (file: string) => `Saved “${file}”.`,
    notSaved: (why: string) => `It couldn’t be saved. ${why}`,
    open: 'Avatar',
    openHint: 'The avatar in 3D, fitted to someone’s measurements.',
    heading: 'Avatar',
    garmentSize: (parts: string[]) => `Garment: ${parts.join(' · ')}`,
    girthAt: (level: string, size: string, ease: string) => `${level} ${size} (${ease})`,
    easeMore: (by: string) => `${by} ease`,
    easeLess: (by: string) => `${by} smaller than the body`,
    noEase: 'no ease',
    levels: { bust: 'bust', waist: 'waist', hip: 'hip' } as Record<string, string>,
    float: 'Float over the table, to move and resize it',
    dockBeside: 'Put back beside the table',
    dockBelow: 'Put back under the table',
    resizeDocked: 'Resize the avatar',
    moveWindow: 'Move the avatar window',
    resizeWindow: 'Resize the avatar window',
    person: 'Whose figure',
    starting: 'Starting…',
    fitting: 'Fitting…',
    standard: 'A standard figure: add measurements to see someone’s.',
    fitted: (close: number, of: number) =>
      close === of
        ? plural(of, 'The measurement within 1 cm', 'All # measurements within 1 cm')
        : `${close} of ${of} measurements within 1 cm`,
    noView: 'This browser can’t draw the avatar in 3D.',
    reset: 'Show the whole figure',
    close: 'Close the avatar',
    viewLabel: (who: string | null) =>
      who
        ? `The avatar in ${who}’s figure. Drag to turn it; pinch or scroll to zoom.`
        : 'A standard figure. Drag to turn it; pinch or scroll to zoom.',
    drawnWith: (backend: string) => `Drawn with ${backend}`,
    drapeGroup: 'Drape the garment',
    fabric: 'Fabric',
    yourFabrics: 'Your fabrics',
    measureFabric: 'Measure a fabric…',
    changeFabric: (name: string) => `Change “${name}”…`,
    fabrics: {
      cottonLawn: 'Cotton lawn',
      poplin: 'Poplin',
      linen: 'Linen',
      chambray: 'Chambray',
      denim: 'Denim',
      twill: 'Twill',
      crepe: 'Crêpe',
      satin: 'Satin',
      jersey: 'Jersey',
      ribKnit: 'Rib knit',
      woolSuiting: 'Wool suiting',
      fleece: 'Fleece',
    } satisfies Record<FabricPreset, string>,
    drape: 'Drape',
    drapeAgain: 'Drape again',
    stop: 'Stop',
    takeOff: 'Take off',
    sewing: 'Sewing the seams…',
    settling: 'Letting it settle…',
    posing: 'Moving into the pose…',
    pose: 'Pose',
    poses: {
      standing: 'Standing',
      armsDown: 'Arms down',
      reaching: 'Reaching forward',
      armsUp: 'Arms raised',
      stepping: 'Walking',
      sitting: 'Sitting',
    } satisfies Record<AvatarPose, string>,
    following: 'Draping it again to follow the change…',
    draped: (fabric: string) => `Draped in ${fabric}.`,
    stillMoving: (fabric: string) => `Draped in ${fabric}; still moving a little.`,
    nothingSewn: 'Nothing is sewn yet: sew the pieces’ edges together first.',
    leftOut: (n: number) =>
      plural(
        n,
        'One piece isn’t sewn to anything, so it’s left off.',
        '# pieces aren’t sewn to anything, so they’re left off.',
      ),
    inside: (n: number) =>
      plural(
        n,
        'A facing or lining is left off: it goes inside.',
        '# facings and linings are left off: they go inside.',
      ),
    cannotDrape: (why: string) => `This can’t be draped yet: ${why}.`,
    shows: 'Show the cloth as',
    showsAs: { fabric: 'Fabric', pull: 'Pulling', room: 'Room' } satisfies Record<
      ClothShows,
      string
    >,
    pullLegend: ['None', 'Hard'],
    roomLegend: (far: string) => ['Touching', `${far} away`],
    legend: (shows: string, from: string, to: string) => `${shows}: from ${from} to ${to}`,
    pulled: (newtons: number) =>
      newtons < 5
        ? 'hardly pulled'
        : newtons < 30
          ? 'pulled lightly'
          : newtons < 80
            ? 'pulled firmly'
            : 'pulled hard',
    reading: (pulled: string, newtons: number, stretch: string, room: string | null) =>
      `Here it is ${pulled} (${newtons} N/m), stretched ${stretch}, and ${
        room === null ? 'resting on the body' : `${room} from the body`
      }.`,
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
