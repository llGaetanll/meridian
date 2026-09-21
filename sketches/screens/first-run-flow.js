// The wires on a flow chart, for the desktop board and the phone one.
//
// A path is measured from where its two nodes landed rather than written down
// beside them, so moving a screen is one number in the page and no path to
// redraw here.

// How far out from a column the dashed tie to its other states runs.
const TIE = 34;

function drawFlow(edges) {
  const canvas = document.querySelector('.canvas');
  const svg = canvas.querySelector('.wires');
  const base = canvas.getBoundingClientRect();

  const box = id => {
    const r = document.getElementById(id).querySelector('.frame').getBoundingClientRect();
    return {
      l: r.left - base.left, t: r.top - base.top,
      r: r.right - base.left, b: r.bottom - base.top,
      cx: (r.left + r.right) / 2 - base.left,
      cy: (r.top + r.bottom) / 2 - base.top,
    };
  };

  for (const e of edges) {
    const a = box(e.from), b = box(e.to);
    let d, lx, ly;

    // Along the road: out of one screen's right side, into the next one's left.
    if (e.kind === 'fwd') {
      // `bend` moves the turn off centre, for a screen that forks into several
      // and would otherwise send every wire down the same line. `dy` moves the
      // exit off the middle of the side, for the same reason.
      const mid = (a.r + b.l) / 2 + (e.bend ?? 0);
      d = `M${a.r} ${a.cy + (e.dy ?? 0)} H${mid} V${b.cy} H${b.l}`;
      lx = mid; ly = b.cy;

    // Down into a screen that is a place of its own, not a state of the one above.
    } else if (e.kind === 'down') {
      const mid = (a.b + b.t) / 2;
      d = `M${a.cx} ${a.b} V${mid} H${b.cx} V${b.t}`;
      lx = (a.cx + b.cx) / 2; ly = mid;

    // The same screen, in another state: down the column's own gutter.
    // `side: 'right'` runs down the right-hand gutter instead, for a state
    // whose column sits to the right of its screen's.
    } else if (e.kind === 'tie') {
      const right = e.side === 'right';
      const gx = right ? a.r + TIE : a.l - TIE;
      d = `M${right ? a.r : a.l} ${a.b - 70} H${gx} V${b.cy} H${b.l}`;

    // Back the way it came, below the arrow that came: out of the left side and
    // into the right side of the screen before it. `dy` is how far under the
    // forward arrow it runs.
    } else if (e.kind === 'backhop') {
      d = `M${a.l} ${a.cy + e.dy} H${e.via} V${b.cy + e.dy} H${b.r}`;
      lx = (a.l + b.r) / 2; ly = (a.cy + b.cy) / 2 + e.dy;

    // The chevron: a short hop back under the pair.
    } else if (e.kind === 'under') {
      d = `M${a.cx} ${a.b} V${e.lane} H${e.gap} V${b.b}`;
      lx = (a.cx + e.gap) / 2; ly = e.lane;

    // Round the outside and up into the bank screen: the ways of arriving
    // there that are not the road through payment.
    } else if (e.kind === 'rise') {
      const bx = b.l + e.in;
      const head = e.out === 'left'  ? `M${a.l} ${a.cy} H${e.via}`
                 : e.out === 'right' ? `M${a.r} ${a.cy + (e.dy ?? 0)} H${e.via}`
                 : e.via ? `M${a.cx} ${a.b} V${a.b + 44} H${e.via}`
                         : `M${a.cx} ${a.b}`;
      d = `${head} V${e.lane} H${bx} V${b.b}`;
      lx = ((e.via ?? a.cx) + bx) / 2; ly = e.lane;
    }

    const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    path.setAttribute('d', d);
    path.setAttribute('class', 'wire ' + e.kind);
    if (e.kind !== 'tie') path.setAttribute('marker-end', 'url(#head)');
    svg.appendChild(path);

    if (!e.label) continue;
    const label = document.createElement('div');
    label.className = 'wire-label ' + e.kind;
    label.textContent = e.label;
    label.style.left = lx + 'px';
    label.style.top = ly + 'px';
    canvas.appendChild(label);
  }
}
