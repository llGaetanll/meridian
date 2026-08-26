/* Enough script to make a sketch answer a tap.

   None of this belongs in the app. The core decides which chips are on, which
   day is picked and which range is live, and it hands the chip order over
   already packed — the same way it hands over every other finished value.
   This file exists so the states can be looked at rather than described, and
   so a row of chips is laid out by the rule the app uses rather than by hand.

   The packing below is a transcription of `ChipRows.swift`, which is the
   implementation the others follow. It used to be its own thing here — an
   exact partition costing every line, which laid the chips out in a different
   order and spread the leftover room over all of them. Two rules for one
   control is how three platforms end up with three design languages, so this
   follows the app, and every sketch now loads this file rather than carrying
   its own copy. */

(() => {
  const GAP = 8;

  const gaps = (count) => GAP * Math.max(0, count - 1);

  /* What a line takes up: the chips on it, and the gaps between them. */
  const fill = (line, widths) =>
    line.reduce((total, i) => total + widths[i], 0) + gaps(line.length);

  /* Which line has the most room to spare, and so goes last. */
  function loosest(lines, widths) {
    let pick = -1;
    let least = Infinity;

    lines.forEach((line, index) => {
      const width = fill(line, widths);
      if (width < least) {
        least = width;
        pick = index;
      }
    });

    return pick;
  }

  /* What a row costs: the square of what each line wastes, and out of reach if
     any line overflows.

     The loosest line is left out of the sum. That line is the one set last,
     where the room left over is not stretched across the chips and so costs
     nothing — and leaving it out is what pushes the remainder onto it instead
     of spreading it over every line. */
  function cost(lines, widths, measure) {
    const last = loosest(lines, widths);
    let total = 0;

    for (let index = 0; index < lines.length; index++) {
      const width = fill(lines[index], widths);
      if (width > measure) return Infinity;
      if (index === last) continue;
      total += (measure - width) * (measure - width);
    }

    return total;
  }

  /* Largest first into the line it leaves least room on. Null if the chips do
     not fit in this many lines at all.

     Tightest-fit and not most-room-first: spreading the wide chips evenly
     leaves every line with a hole too small for anything left, and the row then
     needs a line it should not have. */
  function seed(order, widths, count, measure) {
    const lines = Array.from({ length: count }, () => []);
    const filled = new Array(count).fill(0);

    for (const index of order) {
      let target = null;

      for (let line = 0; line < count; line++) {
        const width = filled[line] + widths[index] + gaps(lines[line].length + 1);
        if (width > measure) continue;
        if (target !== null && filled[target] >= filled[line]) continue;
        target = line;
      }

      if (target === null) return null;

      lines[target].push(index);
      filled[target] += widths[index];
    }

    return lines.some((line) => !line.length) ? null : lines;
  }

  /* Move and swap chips between lines for as long as that makes the row
     cheaper. It settles in a handful of passes at these sizes; the cap is there
     so a pathological measure cannot spin. */
  function refine(lines, widths, measure) {
    let price = cost(lines, widths, measure);

    for (let pass = 0; pass < 8; pass++) {
      let improved = false;

      for (let from = 0; from < lines.length; from++) {
        for (let to = 0; to < lines.length; to++) {
          if (to === from) continue;

          /* High slot first, so a move that lands shrinks only the slots
             already gone by. */
          for (let slot = lines[from].length - 1; slot >= 0; slot--) {
            /* A line emptied is a line fewer, which is a different line count
               and not this pass's business. */
            if (lines[from].length < 2) break;

            const moved = lines.map((line) => [...line]);
            moved[to].push(moved[from].splice(slot, 1)[0]);

            const after = cost(moved, widths, measure);

            if (after < price) {
              lines.splice(0, lines.length, ...moved);
              price = after;
              improved = true;
            }
          }

          for (let here = 0; here < lines[from].length; here++) {
            for (let there = 0; there < lines[to].length; there++) {
              const swapped = lines.map((line) => [...line]);
              swapped[from][here] = lines[to][there];
              swapped[to][there] = lines[from][here];

              const after = cost(swapped, widths, measure);

              if (after < price) {
                lines.splice(0, lines.length, ...swapped);
                price = after;
                improved = true;
              }
            }
          }
        }
      }

      if (!improved) return;
    }
  }

  /* The cheapest arrangement this can find, at whichever line count wins.

     Squaring the waste is what makes four lines wasting a little beat three
     wasting a lot on one of them, so the fewest lines that fit is a floor to
     start from rather than the answer. */
  function pack(widths, measure) {
    if (!widths.length || measure <= 0) return [];

    const order = widths.map((_, i) => i).sort((a, b) => widths[b] - widths[a]);
    const content = widths.reduce((a, b) => a + b, 0) + GAP * (widths.length - 1);
    const fewest = Math.max(1, Math.ceil(content / measure));

    let best = [];
    let cheapest = Infinity;

    for (let count = fewest; count <= fewest + 2; count++) {
      const lines = seed(order, widths, count, measure);
      if (!lines) continue;

      refine(lines, widths, measure);

      const price = cost(lines, widths, measure);

      if (price < cheapest) {
        cheapest = price;
        best = lines;
      }
    }

    /* Nothing fits: a chip wider than the measure. One per line, which at least
       leaves each of them readable. */
    if (!best.length) best = order.map((i) => [i]);

    /* Widest chip first within a line, and the fullest lines first: the line
       with the most room to spare is the one that sits last and ragged, so it
       is the one that has to be at the bottom. */
    for (const line of best) line.sort((a, b) => widths[b] - widths[a]);

    return best.sort((a, b) => fill(b, widths) - fill(a, widths));
  }

  function lay(grid) {
    const measure = grid.clientWidth;
    if (!measure) return;

    const chips = [...grid.querySelectorAll(":scope > .chip")];
    if (!chips.length) return;
    const widths = chips.map((el) => Math.ceil(el.getBoundingClientRect().width));

    const lines = pack(widths, measure);
    const packed = document.createElement("div");
    packed.className = "chip-rows";

    lines.forEach((line, index) => {
      const row = document.createElement("div");

      /* The chips keep their own widths; the space between them takes up the
         remainder, so the line is flush at both edges. The last line keeps its
         natural spacing and sits left, and so does a line with one chip on it —
         a lone chip stretched across the measure is not a row. */
      const flush = index < lines.length - 1 && line.length > 1;

      row.className = flush ? "chip-row flush" : "chip-row";
      for (const i of line) row.appendChild(chips[i]);
      packed.appendChild(row);
    });

    grid.replaceChildren(packed);
  }

  /* Measure once the webfont has landed: Inter is wider than the fallback, so
     packing before it arrives lays out lines that then overflow. */
  const layout = () => document.querySelectorAll("[data-pack]").forEach(lay);
  document.fonts ? document.fonts.ready.then(layout) : addEventListener("load", layout);

  /* A window drawn part-way down the page. Set on load rather than faked with
     a negative margin, because these frames scroll for real. */
  const scroll = () =>
    document.querySelectorAll("[data-scrolled]").forEach((el) => {
      el.scrollTop = Number(el.dataset.scrolled);
    });
  addEventListener("load", scroll);

  document.addEventListener("click", (event) => {
    const hit = event.target.closest(".chip, .range .opt, .btn-group .btn, .cal-day");
    if (!hit || hit.classList.contains("dead") || hit.classList.contains("outside")) return;

    /* A day belongs to the calendar it is drawn in, which is a strip on the
       phone and a grid on the web, and only sometimes carries `data-select`. */
    if (hit.classList.contains("cal-day")) {
      const grid = hit.closest(".cal-strip, .cal-grid, [data-select]");
      if (!grid) return;

      grid.querySelectorAll(".picked").forEach((day) => day.classList.remove("picked"));
      hit.classList.add("picked");
      return;
    }

    const group = hit.closest("[data-select]");
    if (!group) return;

    if (group.dataset.select === "one") {
      group.querySelectorAll(".on").forEach((one) => one.classList.remove("on"));
      hit.classList.add("on");
    } else {
      hit.classList.toggle("on");
    }
  });
})();
