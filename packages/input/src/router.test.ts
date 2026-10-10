import { beforeEach, describe, expect, it } from 'vitest'
import {
  type InputHandlers,
  InputRouter,
  type PointerKind,
  type RawPointer,
  type Scheduler,
  type ToolPointer,
} from './router.ts'

/** A scheduler driven by the test, sharing the clock with the event script. */
class FakeClock implements Scheduler {
  now = 0
  #timers: { at: number; fn: () => void; live: boolean }[] = []
  after(ms: number, fn: () => void) {
    const timer = { at: this.now + ms, fn, live: true }
    this.#timers.push(timer)
    return () => {
      timer.live = false
    }
  }
  advance(ms: number) {
    this.now += ms
    for (const t of this.#timers) {
      if (t.live && t.at <= this.now) {
        t.live = false
        t.fn()
      }
    }
  }
}

function setup(fingerEditing: 'auto' | 'always' | 'never' = 'auto') {
  const log: string[] = []
  const pt = (p: ToolPointer) => `${p.kind} ${p.x},${p.y}`
  const handlers: InputHandlers = {
    pan: (dx, dy) => log.push(`pan ${dx.toFixed(1)},${dy.toFixed(1)}`),
    zoom: (cx, cy, f) => log.push(`zoom ${cx.toFixed(1)},${cy.toFixed(1)} x${f.toFixed(3)}`),
    undo: () => log.push('undo'),
    redo: () => log.push('redo'),
    toolDown: (p) => log.push(`down ${pt(p)}`),
    toolMove: (p) => log.push(`move ${pt(p)}`),
    toolUp: (p) => log.push(`up ${pt(p)}`),
    toolCancel: () => log.push('cancel'),
    hover: (p) => log.push(p ? `hover ${pt(p)}` : 'hover none'),
    contextMenu: (x, y, kind) => log.push(`menu ${kind} ${x},${y}`),
    hold: (p) => log.push(`hold ${pt(p)}`),
  }
  const clock = new FakeClock()
  const router = new InputRouter(handlers, clock, fingerEditing)
  const send = (
    phase: RawPointer['phase'],
    kind: PointerKind,
    id: number,
    x: number,
    y: number,
    extra: Partial<RawPointer> = {},
  ) => router.handle({ phase, kind, id, x, y, time: clock.now, ...extra })
  return { log, clock, router, send }
}

describe('two- and three-finger taps', () => {
  it('two fingers tapped together undo', () => {
    const { log, clock, send } = setup()
    send('down', 'touch', 1, 100, 100)
    send('down', 'touch', 2, 160, 100)
    clock.advance(120)
    send('up', 'touch', 1, 101, 100)
    send('up', 'touch', 2, 160, 101)
    expect(log).toEqual(['undo'])
  })

  it('three fingers tapped together redo', () => {
    const { log, clock, send } = setup()
    send('down', 'touch', 1, 100, 100)
    send('down', 'touch', 2, 160, 100)
    send('down', 'touch', 3, 220, 100)
    clock.advance(150)
    send('up', 'touch', 1, 100, 100)
    send('up', 'touch', 2, 160, 100)
    send('up', 'touch', 3, 220, 100)
    expect(log).toEqual(['redo'])
  })

  it('a slow two-finger touch is not a tap', () => {
    const { log, clock, send } = setup()
    send('down', 'touch', 1, 100, 100)
    send('down', 'touch', 2, 160, 100)
    clock.advance(600)
    send('up', 'touch', 1, 100, 100)
    send('up', 'touch', 2, 160, 100)
    expect(log).toEqual([])
  })
})

describe('two-finger navigation', () => {
  it('spreading two fingers zooms about their centre', () => {
    const { log, clock, send } = setup()
    send('down', 'touch', 1, 100, 100)
    send('down', 'touch', 2, 200, 100)
    clock.advance(16)
    send('move', 'touch', 1, 50, 100)
    send('move', 'touch', 2, 250, 100)
    // Finger 1 moving first shifts the centre left by 25, spread 50 → 75.
    expect(log).toEqual([
      'pan -25.0,0.0',
      'zoom 125.0,100.0 x1.500',
      'pan 25.0,0.0',
      'zoom 150.0,100.0 x1.333',
    ])
    send('up', 'touch', 1, 50, 100)
    send('up', 'touch', 2, 250, 100)
    expect(log).not.toContain('undo')
  })

  it('moving two fingers together pans', () => {
    const { log, send } = setup()
    send('down', 'touch', 1, 100, 100)
    send('down', 'touch', 2, 200, 100)
    send('move', 'touch', 1, 100, 140)
    send('move', 'touch', 2, 200, 140)
    expect(log.filter((l) => l.startsWith('pan'))).toEqual(['pan 0.0,20.0', 'pan 0.0,20.0'])
  })
})

describe('one finger', () => {
  it('a tap reaches the tool as a press and release', () => {
    const { log, clock, send } = setup()
    send('down', 'touch', 1, 30, 40)
    clock.advance(80)
    send('up', 'touch', 1, 31, 40)
    expect(log).toEqual(['down touch 30,40', 'up touch 30,40'])
  })

  it('edits before a Pencil has been seen', () => {
    const { log, send } = setup()
    send('down', 'touch', 1, 0, 0)
    send('move', 'touch', 1, 5, 0)
    expect(log).toEqual([])
    send('move', 'touch', 1, 20, 0)
    send('up', 'touch', 1, 30, 0)
    expect(log).toEqual(['down touch 0,0', 'move touch 20,0', 'up touch 30,0'])
  })

  it('pans once a Pencil has been used', () => {
    const { log, clock, send } = setup()
    send('down', 'pen', 9, 0, 0)
    send('up', 'pen', 9, 0, 0)
    log.length = 0
    clock.advance(1000)
    send('down', 'touch', 1, 0, 0)
    send('move', 'touch', 1, 20, 0)
    send('move', 'touch', 1, 30, 5)
    expect(log).toEqual(['pan 20.0,0.0', 'pan 10.0,5.0'])
  })

  it('a second finger turns an edit into a gesture', () => {
    const { log, send } = setup()
    send('down', 'touch', 1, 0, 0)
    send('move', 'touch', 1, 30, 0)
    send('down', 'touch', 2, 100, 0)
    expect(log).toEqual(['down touch 0,0', 'move touch 30,0', 'cancel'])
  })

  it('holding still opens the context menu, and lifting then does nothing', () => {
    const { log, clock, send } = setup()
    send('down', 'touch', 1, 50, 60)
    clock.advance(520)
    send('up', 'touch', 1, 50, 60)
    expect(log).toEqual(['menu touch 50,60'])
  })

  it('can be told to always or never edit', () => {
    const never = setup('never')
    never.send('down', 'touch', 1, 0, 0)
    never.send('move', 'touch', 1, 20, 0)
    expect(never.log).toEqual(['pan 20.0,0.0'])

    const always = setup('always')
    always.send('down', 'pen', 9, 0, 0)
    always.send('up', 'pen', 9, 0, 0)
    always.clock.advance(1000)
    always.log.length = 0
    always.send('down', 'touch', 1, 0, 0)
    always.send('move', 'touch', 1, 20, 0)
    expect(always.log).toEqual(['down touch 0,0', 'move touch 20,0'])
  })
})

describe('palm rejection', () => {
  let s: ReturnType<typeof setup>
  beforeEach(() => {
    s = setup()
  })

  it('a pen landing cancels a finger edit and ignores the hand', () => {
    s.send('down', 'touch', 1, 0, 0)
    s.send('move', 'touch', 1, 30, 0)
    s.send('down', 'pen', 9, 200, 200)
    s.send('move', 'touch', 1, 60, 0)
    s.send('down', 'touch', 2, 300, 300)
    s.send('move', 'pen', 9, 210, 200)
    s.send('up', 'pen', 9, 210, 200)
    s.send('up', 'touch', 1, 60, 0)
    expect(s.log).toEqual([
      'down touch 0,0',
      'move touch 30,0',
      'cancel',
      'down pen 200,200',
      'move pen 210,200',
      'up pen 210,200',
    ])
  })

  it('touches right after the pen lifts are ignored, later ones are not', () => {
    s.send('down', 'pen', 9, 0, 0)
    s.send('up', 'pen', 9, 0, 0)
    s.log.length = 0
    s.clock.advance(100)
    s.send('down', 'touch', 1, 50, 50)
    s.send('up', 'touch', 1, 50, 50)
    expect(s.log).toEqual([])
    s.clock.advance(400)
    s.send('down', 'touch', 2, 50, 50)
    s.send('up', 'touch', 2, 50, 50)
    expect(s.log).toEqual(['down touch 50,50', 'up touch 50,50'])
  })

  it('large contacts are palms once a pen is known', () => {
    s.send('down', 'pen', 9, 0, 0)
    s.send('up', 'pen', 9, 0, 0)
    s.clock.advance(1000)
    s.log.length = 0
    s.send('down', 'touch', 1, 50, 50, { width: 60, height: 80 })
    s.send('up', 'touch', 1, 50, 50)
    expect(s.log).toEqual([])
  })
})

describe('pen and mouse', () => {
  it('holding the pen still shows measurements; moving cancels that', () => {
    const { log, clock, send } = setup()
    send('down', 'pen', 1, 10, 10)
    clock.advance(400)
    send('up', 'pen', 1, 10, 10)
    expect(log).toEqual(['down pen 10,10', 'hold pen 10,10', 'up pen 10,10'])

    log.length = 0
    send('down', 'pen', 1, 10, 10)
    send('move', 'pen', 1, 30, 10)
    clock.advance(400)
    send('up', 'pen', 1, 30, 10)
    expect(log).toEqual(['down pen 10,10', 'move pen 30,10', 'up pen 30,10'])
  })

  it('right-click opens the menu and middle-drag pans', () => {
    const { log, send } = setup()
    send('down', 'mouse', 1, 5, 6, { button: 2 })
    expect(log).toEqual(['menu mouse 5,6'])
    log.length = 0
    send('down', 'mouse', 1, 0, 0, { button: 1 })
    send('move', 'mouse', 1, 7, 3)
    send('up', 'mouse', 1, 7, 3)
    expect(log).toEqual(['pan 7.0,3.0'])
  })

  it('space plus drag pans', () => {
    const { log, router, send } = setup()
    router.setPanModifier(true)
    send('down', 'mouse', 1, 0, 0, { button: 0 })
    send('move', 'mouse', 1, 4, 4)
    expect(log).toEqual(['pan 4.0,4.0'])
  })

  it('hovering reaches the tool when nothing is pressed', () => {
    const { log, send } = setup()
    send('hover', 'mouse', 1, 3, 4)
    send('hover', 'pen', 2, 5, 6)
    expect(log).toEqual(['hover mouse 3,4', 'hover pen 5,6'])
  })
})
