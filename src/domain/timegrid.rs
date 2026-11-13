//! Calendar time grid: overlapping timed items sit side by side, as in Google Calendar.

/// `(start, end)` minutes per item, any order → `(col, cols)` per item, in input order.
/// Items that overlap directly or through a chain form one cluster and share its column count,
/// so every item in a cluster has the same width; a free column is reused by a later item.
pub fn columns(spans: &[(u32, u32)]) -> Vec<(usize, usize)> {
    let mut order: Vec<usize> = (0..spans.len()).collect();
    // Longer first on a tie, so the long item takes the left column.
    order.sort_by_key(|&i| (spans[i].0, std::cmp::Reverse(spans[i].1)));
    let mut out = vec![(0, 1); spans.len()];
    let mut cluster: Vec<usize> = Vec::new();
    // Per column: where its last item ends.
    let mut ends: Vec<u32> = Vec::new();
    let mut cluster_end = 0;
    for i in order {
        let (s, e) = spans[i];
        // A zero-length item still takes a slot.
        let e = e.max(s + 1);
        if !cluster.is_empty() && s >= cluster_end {
            for &j in &cluster {
                out[j].1 = ends.len();
            }
            cluster.clear();
            ends.clear();
            cluster_end = 0;
        }
        let col = match ends.iter().position(|&end| end <= s) {
            Some(c) => {
                ends[c] = e;
                c
            }
            None => {
                ends.push(e);
                ends.len() - 1
            }
        };
        out[i].0 = col;
        cluster.push(i);
        cluster_end = cluster_end.max(e);
    }
    for &j in &cluster {
        out[j].1 = ends.len();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::columns;

    #[test]
    fn overlaps_side_by_side_and_clusters_share_width() {
        assert_eq!(columns(&[]), vec![]);
        // Apart: each takes the full width.
        assert_eq!(columns(&[(600, 635), (900, 960)]), vec![(0, 1), (0, 1)]);
        // Touching ends don't overlap.
        assert_eq!(columns(&[(600, 660), (660, 720)]), vec![(0, 1), (0, 1)]);
        // The board's Thursday: 10:00–10:35 and 10:15–11:00 split, 15:00 stands alone.
        assert_eq!(
            columns(&[(600, 635), (615, 660), (900, 960)]),
            vec![(0, 2), (1, 2), (0, 1)]
        );
        // A chain: c overlaps only b, yet all three share one cluster; c reuses a's column.
        assert_eq!(
            columns(&[(600, 660), (630, 720), (690, 750)]),
            vec![(0, 2), (1, 2), (0, 2)]
        );
        // c starts exactly when a ends: it takes a's column back, the cluster stays two wide.
        assert_eq!(
            columns(&[(600, 660), (610, 720), (660, 700)]),
            vec![(0, 2), (1, 2), (0, 2)]
        );
        // Input order is kept; the longer of two same-start items goes left.
        assert_eq!(columns(&[(600, 630), (600, 700)]), vec![(1, 2), (0, 2)]);
        // Three at once.
        assert_eq!(
            columns(&[(600, 700), (610, 700), (620, 700)]),
            vec![(0, 3), (1, 3), (2, 3)]
        );
        // Zero-length still counts.
        assert_eq!(columns(&[(600, 600), (600, 660)]), vec![(1, 2), (0, 2)]);
    }
}
