// Everything the app says, in plain sewing language. One place, so wording
// stays consistent and can be translated later.

import type {
  AllowanceNote,
  CornerStyle,
  DisplayUnits,
  EdgeKind,
  ImportNote,
  ScaleBasis,
  ScaleReason,
} from '@soom/engine'

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

export const t = {
  appName: 'Soom',
  untitled: 'Untitled pattern',

  actions: {
    open: 'Open pattern',
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
      'SVG files from Inkscape, Illustrator, FreeSewing and most pattern apps work. DXF comes next.',
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
  },

  pieces: {
    heading: 'Pieces',
    none: 'No pieces yet.',
    size: (w: string, h: string) => `${w} × ${h}`,
  },

  importing: {
    reading: (name: string) => `Reading ${name}…`,
    unsupported: (name: string) =>
      `Soom can open SVG files for now; “${name}” is not one. DXF and PDF are coming.`,
    failed: (message: string) => `This file could not be opened. ${message}`,
    found: (n: number) => plural(n, 'Found 1 piece', 'Found # pieces'),
    nothingFound: 'No pieces were found in this file.',
    sizeQuestion: 'How big is this pattern?',
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
      })[r],
    basis: (b: ScaleBasis): string =>
      ({
        declaredUnits: 'As the file says',
        assumed96dpi: '96 to the inch (most apps)',
        assumed72dpi: '72 to the inch (Adobe Illustrator)',
        assumed90dpi: '90 to the inch (Inkscape before 2017)',
        measured: 'As measured',
      })[b],
    wouldBe: (name: string, size: string) => `${name} would be ${size}`,
    add: (n: number) => plural(n, 'Add the piece', 'Add # pieces'),
    cancel: 'Cancel',
    note: (n: ImportNote, pieceName: (id: string) => string, units: DisplayUnits): string => {
      switch (n.kind) {
        case 'seamLineFound':
          return `${pieceName(n.piece)}: found both the cutting line and the stitching line, so the allowance (${formatLength(n.allowance, units)}) was measured from the file.`
        case 'cuttingLineDiffers':
          return `${pieceName(n.piece)}: the file’s cutting line differs from Soom’s by up to ${formatLength(n.maxMm, 'mm')}, usually at corners. You can adjust the corners after adding.`
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
        case 'noClosedShapes':
          return 'The file has lines but no closed shapes. Soom needs each piece’s outline drawn as one closed shape.'
        case 'outlineCrossesItself':
          return `${pieceName(n.piece)}: the outline crosses itself. Check that piece after adding.`
      }
    },
  },

  crash: {
    heading: 'Something went wrong',
    body: 'Soom hit a problem showing this screen. Reloading usually fixes it.',
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
