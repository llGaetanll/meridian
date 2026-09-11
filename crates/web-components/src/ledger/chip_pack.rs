//! Cutting a row of chips into lines that fill the measure.
//!
//! `swift/Sources/MeridianApp/Ui/ChipRows.swift` is where the rule is written
//! down, and this is the same rule: every line but the last as full as it can
//! be, costed by the square of what each line wastes, with the loosest line left
//! out of the cost because it is the one set last and ragged.

/// How many passes of moves and swaps before the packing settles for what it
/// has. It settles in a handful at these sizes; the cap is there so a
/// pathological measure cannot spin.
const PASSES: usize = 8;

/// The lines, each a list of indices into `widths`, fullest line first and
/// widest chip first within a line.
pub fn pack(widths: &[f64], measure: f64, spacing: f64) -> Vec<Vec<usize>> {
    if widths.is_empty() || measure <= 0.0 {
        return Vec::new();
    }

    let mut order: Vec<usize> = (0..widths.len()).collect();
    order.sort_by(|a, b| widths[*b].total_cmp(&widths[*a]));

    let content = widths.iter().sum::<f64>() + spacing * (widths.len() - 1) as f64;
    let fewest = ((content / measure).ceil() as usize).max(1);

    let mut best: Vec<Vec<usize>> = Vec::new();
    let mut cheapest = f64::INFINITY;

    for count in fewest..=fewest + 2 {
        let Some(mut lines) = seed(&order, widths, count, measure, spacing) else {
            continue;
        };

        refine(&mut lines, widths, measure, spacing);

        let price = cost(&lines, widths, measure, spacing);

        if price < cheapest {
            cheapest = price;
            best = lines;
        }
    }

    // A chip wider than the measure: one per line, which at least leaves each
    // of them readable.
    if best.is_empty() {
        best = order.iter().map(|index| vec![*index]).collect();
    }

    for line in &mut best {
        line.sort_by(|a, b| widths[*b].total_cmp(&widths[*a]));
    }

    best.sort_by(|a, b| fill(b, widths, spacing).total_cmp(&fill(a, widths, spacing)));

    best
}

/// Largest first into the line it leaves least room on. `None` if the chips do
/// not fit in this many lines at all.
fn seed(
    order: &[usize],
    widths: &[f64],
    count: usize,
    measure: f64,
    spacing: f64,
) -> Option<Vec<Vec<usize>>> {
    let mut lines: Vec<Vec<usize>> = vec![Vec::new(); count];
    let mut filled = vec![0.0; count];

    for &index in order {
        let mut target: Option<usize> = None;

        for line in 0..count {
            let width = filled[line] + widths[index] + gaps(lines[line].len() + 1, spacing);

            if width > measure {
                continue;
            }

            if let Some(best) = target
                && filled[best] >= filled[line]
            {
                continue;
            }

            target = Some(line);
        }

        let target = target?;

        lines[target].push(index);
        filled[target] += widths[index];
    }

    if lines.iter().any(Vec::is_empty) {
        return None;
    }

    Some(lines)
}

/// Move and swap chips between lines for as long as that makes the row cheaper.
fn refine(lines: &mut Vec<Vec<usize>>, widths: &[f64], measure: f64, spacing: f64) {
    let mut price = cost(lines, widths, measure, spacing);

    for _ in 0..PASSES {
        let mut improved = false;

        for from in 0..lines.len() {
            for to in 0..lines.len() {
                if to == from {
                    continue;
                }

                for slot in (0..lines[from].len()).rev() {
                    let mut moved = lines.clone();
                    let chip = moved[from].remove(slot);
                    moved[to].push(chip);

                    // A line emptied is a line fewer, which is a different
                    // line count and not this pass's business.
                    if moved[from].is_empty() {
                        continue;
                    }

                    let after = cost(&moved, widths, measure, spacing);

                    if after < price {
                        *lines = moved;
                        price = after;
                        improved = true;
                    }
                }

                for here in 0..lines[from].len() {
                    for there in 0..lines[to].len() {
                        let mut swapped = lines.clone();
                        swapped[from][here] = lines[to][there];
                        swapped[to][there] = lines[from][here];

                        let after = cost(&swapped, widths, measure, spacing);

                        if after < price {
                            *lines = swapped;
                            price = after;
                            improved = true;
                        }
                    }
                }
            }
        }

        if !improved {
            return;
        }
    }
}

/// The square of what each line wastes, and out of reach if any line
/// overflows. The loosest line is left out: it is set last, where its slack is
/// not stretched across the chips and so costs nothing.
fn cost(lines: &[Vec<usize>], widths: &[f64], measure: f64, spacing: f64) -> f64 {
    let last = loosest(lines, widths, spacing);

    let mut total = 0.0;

    for (index, line) in lines.iter().enumerate() {
        let width = fill(line, widths, spacing);

        if width > measure {
            return f64::INFINITY;
        }

        if Some(index) == last {
            continue;
        }

        total += (measure - width) * (measure - width);
    }

    total
}

/// Which line has the most room to spare, and so goes last.
fn loosest(lines: &[Vec<usize>], widths: &[f64], spacing: f64) -> Option<usize> {
    (0..lines.len()).min_by(|a, b| {
        fill(&lines[*a], widths, spacing).total_cmp(&fill(&lines[*b], widths, spacing))
    })
}

/// How much of the measure a line takes, the gaps between its chips included.
fn fill(line: &[usize], widths: &[f64], spacing: f64) -> f64 {
    line.iter().map(|index| widths[*index]).sum::<f64>() + gaps(line.len(), spacing)
}

fn gaps(count: usize, spacing: f64) -> f64 {
    spacing * count.saturating_sub(1) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_on_one_line_when_it_fits() {
        assert_eq!(pack(&[40.0, 60.0], 200.0, 8.0), vec![vec![1, 0]]);
    }

    #[test]
    fn nothing_to_pack_is_no_lines() {
        assert!(pack(&[], 200.0, 8.0).is_empty());
        assert!(pack(&[40.0], 0.0, 8.0).is_empty());
    }

    #[test]
    fn a_chip_wider_than_the_measure_gets_a_line_of_its_own() {
        assert_eq!(pack(&[300.0, 20.0], 100.0, 8.0), vec![vec![0], vec![1]]);
    }

    #[test]
    fn no_line_overflows() {
        let widths = [120.0, 96.0, 70.0, 110.0, 104.0, 92.0, 104.0, 128.0, 112.0];
        let lines = pack(&widths, 300.0, 8.0);

        assert!(lines.iter().all(|line| fill(line, &widths, 8.0) <= 300.0));
        assert_eq!(lines.iter().map(Vec::len).sum::<usize>(), widths.len());
    }

    /// The line with the most room to spare is the one that sits last and
    /// ragged, so the fuller lines come first.
    #[test]
    fn the_loosest_line_goes_last() {
        let widths = [120.0, 96.0, 70.0, 110.0, 104.0, 92.0, 104.0, 128.0, 112.0];
        let lines = pack(&widths, 300.0, 8.0);
        let fills: Vec<f64> = lines.iter().map(|line| fill(line, &widths, 8.0)).collect();

        assert!(fills.windows(2).all(|pair| pair[0] >= pair[1]));
    }
}
