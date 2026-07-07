//! Geometric entity layout, mirroring `anxwritter/layouts/_geometric.py` and
//! the alias table in `anxwritter/layouts/__init__.py`.
//!
//! Each function takes node keys plus graph data and returns NEW `(x, y)`
//! integer positions. Geometric modes (grid/circle/radial/random) and the tidy
//! `tree` layout are faithful ports — `radial` and `tree` reproduce upstream
//! coordinates exactly. The force-directed engines (`fr`, `forceatlas2`) use a
//! local PRNG for initial placement, so they produce valid but not
//! byte-identical layouts (upstream coordinate parity is not required).

use std::collections::BTreeMap;

use indexmap::IndexMap;

/// A node -> `(x, y)` position map, preserving insertion order.
pub type Positions = IndexMap<String, (i64, i64)>;

/// Floor division matching Python's `//` for a positive divisor.
fn fdiv(a: i64, b: i64) -> i64 {
    a.div_euclid(b)
}

/// Arrange-mode **alias table**: `(alias, canonical)`, mirroring the `_ALIASES`
/// dict in `anxwritter/layouts/__init__.py`. Aliases are already canonicalized
/// keys (lowercase, `-`/space collapsed to `_`); [`normalize_arrange`]
/// canonicalizes its input the same way before looking it up here.
///
/// This is the single source of truth for both [`normalize_arrange`] and the
/// discovery/`meta` payload — see [`crate::discovery`].
pub const ARRANGE_ALIASES: &[(&str, &str)] = &[
    // Force-directed (Fruchterman-Reingold)
    ("fr", "fr"),
    ("fruchterman_reingold", "fr"),
    // ForceAtlas2
    ("forceatlas2", "forceatlas2"),
    ("force_atlas_2", "forceatlas2"),
    ("force_atlas2", "forceatlas2"),
    ("fa2", "forceatlas2"),
    // Tidy tree (Reingold-Tilford family)
    ("tree", "tree"),
    ("reingold_tilford", "tree"),
    ("tidy_tree", "tree"),
    // Geometric modes (passthrough)
    ("radial", "radial"),
    ("circle", "circle"),
    ("grid", "grid"),
    ("random", "random"),
];

/// The canonical arrange algorithm keys (the deduped values of
/// [`ARRANGE_ALIASES`]), in table order.
pub const ARRANGE_ALGORITHMS: &[&str] = &[
    "fr",
    "forceatlas2",
    "tree",
    "radial",
    "circle",
    "grid",
    "random",
];

/// Canonicalize an arrange string (lowercase, spaces/dashes -> underscores)
/// then resolve aliases via [`ARRANGE_ALIASES`]. Unknown values pass through
/// unchanged (so [`place`] falls them through to `random`, matching upstream).
pub fn normalize_arrange(mode: &str) -> String {
    let canon = mode.trim().to_lowercase().replace(['-', ' '], "_");
    ARRANGE_ALIASES
        .iter()
        .find(|(alias, _)| *alias == canon)
        .map(|(_, canonical)| canonical.to_string())
        .unwrap_or_else(|| mode.to_string())
}

/// Grid placement: `ceil(sqrt(n))` columns, 200·scale spacing.
pub fn place_grid(auto_keys: &[String], cx: i64, cy: i64, scale: f64) -> Positions {
    let n = auto_keys.len() as i64;
    let mut out = Positions::new();
    if n == 0 {
        return out;
    }
    let cols = (n as f64).sqrt().ceil() as i64;
    let spacing = (200.0 * scale).round() as i64;
    for (i, key) in auto_keys.iter().enumerate() {
        let i = i as i64;
        let row_i = i / cols;
        let col_i = i % cols;
        let x = cx + col_i * spacing - fdiv((cols - 1) * spacing, 2);
        let y = cy + row_i * spacing - fdiv((fdiv(n, cols) - 1) * spacing, 2);
        out.insert(key.clone(), (x, y));
    }
    out
}

/// Circle placement: radius `max(150, n*35) * scale`.
pub fn place_circle(auto_keys: &[String], cx: i64, cy: i64, scale: f64) -> Positions {
    let n = auto_keys.len();
    let mut out = Positions::new();
    if n == 0 {
        return out;
    }
    let radius = (150.0_f64).max(n as f64 * 35.0) * scale;
    for (i, key) in auto_keys.iter().enumerate() {
        let angle = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
        let x = cx + (radius * angle.cos()) as i64;
        let y = cy + (radius * angle.sin()) as i64;
        out.insert(key.clone(), (x, y));
    }
    out
}

/// Deterministic "random" spread.
///
/// **Intentional divergence from upstream:** Python seeds `random.Random(42)`
/// (a Mersenne-Twister MT19937 generator); this uses a small local LCG instead.
/// Coordinates are therefore *not* byte-equivalent to the Python reference for
/// the `random` arrange mode. This is deliberate — reproducing MT19937 is not
/// worth the weight, and `random` layouts have no fixed geometry to match. The
/// output is valid, deterministic, and stable across runs. Downstream
/// byte-parity batteries should exclude the `random` mode (all other geometric
/// modes — grid/circle/radial/tree — match upstream exactly).
pub fn place_random(auto_keys: &[String], cx: i64, cy: i64, scale: f64) -> Positions {
    let mut out = Positions::new();
    let extent = (400.0 * scale).round() as i64;
    // Simple stable LCG seeded at 42; range-mapped into [-extent, extent].
    let mut state: u64 = 42;
    let mut next = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((state >> 33) as i64).rem_euclid(2 * extent + 1) - extent
    };
    for key in auto_keys {
        let x = cx + next();
        let y = cy + next();
        out.insert(key.clone(), (x, y));
    }
    out
}

/// Radial hub-and-spokes layout with compaction (faithful port of
/// `place_radial`).
pub fn place_radial(
    auto_keys: &[String],
    all_keys: &[String],
    edges: &[(String, String)],
    cx: i64,
    cy: i64,
    scale: f64,
) -> Positions {
    let mut out = Positions::new();

    // Undirected adjacency spanning all_keys.
    let mut adj: BTreeMap<&str, std::collections::BTreeSet<&str>> = BTreeMap::new();
    for k in all_keys {
        adj.entry(k.as_str()).or_default();
    }
    for (a, b) in edges {
        if adj.contains_key(a.as_str()) && adj.contains_key(b.as_str()) {
            adj.get_mut(a.as_str()).unwrap().insert(b.as_str());
            adj.get_mut(b.as_str()).unwrap().insert(a.as_str());
        }
    }
    let degree = |k: &str| adj.get(k).map(|s| s.len() as i64).unwrap_or(0);

    // Hubs: degree >= 2 among auto_keys, sorted by (-degree, key).
    let mut hubs: Vec<&str> = auto_keys
        .iter()
        .map(|s| s.as_str())
        .filter(|k| degree(k) >= 2)
        .collect();
    hubs.sort_by(|a, b| degree(b).cmp(&degree(a)).then_with(|| a.cmp(b)));
    let hub_set: std::collections::BTreeSet<&str> = hubs.iter().copied().collect();

    // Attach leaves to their highest-degree hub neighbour; collect isolated.
    let mut leaf_to_hub: IndexMap<&str, &str> = IndexMap::new();
    let mut isolated: Vec<&str> = Vec::new();
    for k in auto_keys.iter().map(|s| s.as_str()) {
        if hub_set.contains(k) {
            continue;
        }
        let mut hub_neighbours: Vec<&str> = adj
            .get(k)
            .map(|s| s.iter().copied().filter(|n| hub_set.contains(n)).collect())
            .unwrap_or_default();
        if hub_neighbours.is_empty() {
            isolated.push(k);
        } else {
            // max by (degree, key).
            hub_neighbours.sort_by(|a, b| degree(a).cmp(&degree(b)).then_with(|| a.cmp(b)));
            leaf_to_hub.insert(k, *hub_neighbours.last().unwrap());
        }
    }

    // Group leaves per hub, preserving hub order.
    let mut hub_leaves: IndexMap<&str, Vec<&str>> = IndexMap::new();
    for h in &hubs {
        hub_leaves.insert(*h, Vec::new());
    }
    for (leaf, hub) in &leaf_to_hub {
        hub_leaves.get_mut(*hub).unwrap().push(*leaf);
    }

    // Place hubs.
    let n_hubs = hubs.len();
    let hub_ring_radius = if n_hubs <= 1 {
        0.0
    } else {
        (260.0_f64).max(n_hubs as f64 * 70.0) * scale
    };
    let mut hub_pos: IndexMap<&str, (i64, i64)> = IndexMap::new();
    if n_hubs == 1 {
        hub_pos.insert(hubs[0], (cx, cy));
        out.insert(hubs[0].to_string(), (cx, cy));
    } else {
        for (i, h) in hubs.iter().enumerate() {
            let a = 2.0 * std::f64::consts::PI * i as f64 / n_hubs as f64;
            let hx = cx + (hub_ring_radius * a.cos()) as i64;
            let hy = cy + (hub_ring_radius * a.sin()) as i64;
            hub_pos.insert(*h, (hx, hy));
            out.insert(h.to_string(), (hx, hy));
        }
    }

    // Place leaves on an outward-facing arc per hub.
    for (h, leaves) in &hub_leaves {
        if leaves.is_empty() {
            continue;
        }
        let (hx, hy) = hub_pos[*h];
        let n_leaves = leaves.len();
        let leaf_radius = (110.0_f64).max(25.0 + 14.0 * n_leaves as f64) * scale;
        if n_hubs == 1 {
            for (j, leaf) in leaves.iter().enumerate() {
                let a = 2.0 * std::f64::consts::PI * j as f64 / n_leaves as f64;
                let lx = hx + (leaf_radius * a.cos()) as i64;
                let ly = hy + (leaf_radius * a.sin()) as i64;
                out.insert(leaf.to_string(), (lx, ly));
            }
        } else {
            let base_angle = ((hy - cy) as f64).atan2((hx - cx) as f64);
            let arc_span = std::f64::consts::PI;
            for (j, leaf) in leaves.iter().enumerate() {
                let a = if n_leaves == 1 {
                    base_angle
                } else {
                    base_angle - arc_span / 2.0 + arc_span * j as f64 / (n_leaves as f64 - 1.0)
                };
                let lx = hx + (leaf_radius * a.cos()) as i64;
                let ly = hy + (leaf_radius * a.sin()) as i64;
                out.insert(leaf.to_string(), (lx, ly));
            }
        }
    }

    // Isolated entities go in a grid below the layout.
    if !isolated.is_empty() {
        let cols = (isolated.len() as f64).sqrt().ceil() as i64;
        let spacing = (160.0 * scale).round() as i64;
        let y_offset = (hub_ring_radius + 320.0 * scale).round() as i64;
        for (i, key) in isolated.iter().enumerate() {
            let i = i as i64;
            let row_i = i / cols;
            let col_i = i % cols;
            let x = cx + col_i * spacing - fdiv((cols - 1) * spacing, 2);
            let y = cy + y_offset + row_i * spacing;
            out.insert(key.to_string(), (x, y));
        }
    }

    out
}

// ── Force-directed + tree layouts ───────────────────────────────────────────

/// Pinned node positions (fixed anchors) for the topology layouts.
pub type Pinned = IndexMap<String, (f64, f64)>;

/// Small deterministic PRNG for layout initialisation. Does NOT reproduce
/// numpy's PCG64 sequence — force layouts need not match upstream coordinates,
/// only be valid and stable.
struct Lcg(u64);
impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed
            .wrapping_mul(2862933555777941757)
            .wrapping_add(3037000493))
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn f01(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + self.f01() * (b - a)
    }
}

/// Unique undirected edges as index pairs into `nodes` (self-loops dropped).
fn edge_index_pairs(nodes: &[String], edges: &[(String, String)]) -> Vec<(usize, usize)> {
    let idx: std::collections::HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (a, b) in edges {
        if a == b {
            continue;
        }
        let (ia, ib) = match (idx.get(a.as_str()), idx.get(b.as_str())) {
            (Some(&ia), Some(&ib)) => (ia, ib),
            _ => continue,
        };
        let key = if ia < ib { (ia, ib) } else { (ib, ia) };
        if seen.insert(key) {
            out.push((ia, ib));
        }
    }
    out
}

/// All-pairs repulsion: force on `i` from `j` is `(x_i - x_j) * strength * w_i *
/// w_j / dist²`. `weight = None` for uniform (Fruchterman-Reingold).
fn repulsion_forces(
    pos: &[(f64, f64)],
    weight: Option<&[f64]>,
    strength: f64,
    eps_sq: f64,
) -> Vec<(f64, f64)> {
    let n = pos.len();
    let mut f = vec![(0.0, 0.0); n];
    for i in 0..n {
        let (xi, yi) = pos[i];
        let (mut fx, mut fy) = (0.0, 0.0);
        for j in 0..n {
            if i == j {
                continue;
            }
            let dx = xi - pos[j].0;
            let dy = yi - pos[j].1;
            let d2 = (dx * dx + dy * dy).max(eps_sq);
            let c = match weight {
                Some(w) => strength * w[i] * w[j] / d2,
                None => strength / d2,
            };
            fx += dx * c;
            fy += dy * c;
        }
        f[i] = (fx, fy);
    }
    f
}

/// Fruchterman-Reingold force-directed layout.
pub fn apply_fr(
    nodes: &[String],
    edges: &[(String, String)],
    pinned: &Pinned,
    iterations: usize,
    scale: f64,
    center: (i64, i64),
) -> Positions {
    let n = nodes.len();
    let mut out = Positions::new();
    if n == 0 {
        return out;
    }
    let idx: std::collections::HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i))
        .collect();
    let mut rng = Lcg::new(42);
    let mut pos: Vec<(f64, f64)> = (0..n)
        .map(|_| (rng.uniform(-scale, scale), rng.uniform(-scale, scale)))
        .collect();
    let mut pinned_mask = vec![false; n];
    for (nm, (px, py)) in pinned {
        if let Some(&i) = idx.get(nm.as_str()) {
            pos[i] = (*px, *py);
            pinned_mask[i] = true;
        }
    }
    let ep = edge_index_pairs(nodes, edges);
    let area = (2.0 * scale).powi(2);
    let k = (area / n.max(1) as f64).sqrt();
    let k_sq = k * k;
    let t_init = scale / 10.0;
    let eps = 1e-9;
    let eps_sq = eps * eps;

    for it in 0..iterations {
        let mut disp = repulsion_forces(&pos, None, k_sq, eps_sq);
        for &(a, b) in &ep {
            let d = (pos[b].0 - pos[a].0, pos[b].1 - pos[a].1);
            let dn = (d.0 * d.0 + d.1 * d.1).sqrt();
            let dns = dn.max(eps);
            let att = dns * dns / k;
            let av = (d.0 / dns * att, d.1 / dns * att);
            disp[a].0 += av.0;
            disp[a].1 += av.1;
            disp[b].0 -= av.0;
            disp[b].1 -= av.1;
        }
        let t = t_init * (1.0 - it as f64 / iterations.max(1) as f64);
        let mut max_move = 0.0_f64;
        for i in 0..n {
            if pinned_mask[i] {
                continue;
            }
            let dmag = (disp[i].0 * disp[i].0 + disp[i].1 * disp[i].1).sqrt();
            let dms = dmag.max(eps);
            let s = dmag.min(t) / dms;
            let (cx, cy) = (disp[i].0 * s, disp[i].1 * s);
            pos[i].0 += cx;
            pos[i].1 += cy;
            max_move = max_move.max(cx.abs()).max(cy.abs());
        }
        if max_move < 0.5 {
            break;
        }
    }
    finalize(nodes, &pos, &idx, pinned, center, 1.0, &mut out);
    out
}

/// ForceAtlas2 force-directed layout (linear attraction, weak gravity defaults).
pub fn apply_forceatlas2(
    nodes: &[String],
    edges: &[(String, String)],
    pinned: &Pinned,
    iterations: usize,
    scale: f64,
    center: (i64, i64),
) -> Positions {
    let n = nodes.len();
    let mut out = Positions::new();
    if n == 0 {
        return out;
    }
    let (scaling_ratio, gravity, base_speed) = (2.0, 1.0, 0.1);
    let idx: std::collections::HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i))
        .collect();
    let mut rng = Lcg::new(42);
    let mut pos: Vec<(f64, f64)> = (0..n)
        .map(|_| (rng.uniform(-1.0, 1.0), rng.uniform(-1.0, 1.0)))
        .collect();
    let inv_scale = 1.0 / scale.max(1e-9);
    let mut pinned_mask = vec![false; n];
    for (nm, (px, py)) in pinned {
        if let Some(&i) = idx.get(nm.as_str()) {
            pos[i] = (px * inv_scale, py * inv_scale);
            pinned_mask[i] = true;
        }
    }
    let ep = edge_index_pairs(nodes, edges);
    let mut mass = vec![1.0_f64; n];
    for &(a, b) in &ep {
        mass[a] += 1.0;
        mass[b] += 1.0;
    }
    let eps = 1e-9;
    let eps_sq = eps * eps;

    for it in 0..iterations {
        let mut f = repulsion_forces(&pos, Some(&mass), scaling_ratio, eps_sq);
        for &(a, b) in &ep {
            let d = (pos[b].0 - pos[a].0, pos[b].1 - pos[a].1);
            let dn = (d.0 * d.0 + d.1 * d.1).sqrt();
            let dns = dn.max(eps);
            let av = (d.0 / dns * dn, d.1 / dns * dn);
            f[a].0 += av.0;
            f[a].1 += av.1;
            f[b].0 -= av.0;
            f[b].1 -= av.1;
        }
        for i in 0..n {
            let dorg = (pos[i].0 * pos[i].0 + pos[i].1 * pos[i].1).sqrt();
            let dos = dorg.max(eps);
            let grav = gravity * mass[i] / dos;
            f[i].0 -= pos[i].0 / dos * grav;
            f[i].1 -= pos[i].1 / dos * grav;
        }
        let step = base_speed * (1.0 - it as f64 / iterations.max(1) as f64);
        let mut max_move = 0.0_f64;
        for i in 0..n {
            if pinned_mask[i] {
                continue;
            }
            let dp = (f[i].0 * step / mass[i], f[i].1 * step / mass[i]);
            pos[i].0 += dp.0;
            pos[i].1 += dp.1;
            max_move = max_move.max((dp.0 * scale).abs()).max((dp.1 * scale).abs());
        }
        if max_move < 0.5 {
            break;
        }
    }
    finalize(nodes, &pos, &idx, pinned, center, scale, &mut out);
    out
}

/// Round simulation positions to integers (with scale + centre) for non-pinned
/// nodes.
#[allow(clippy::too_many_arguments)]
fn finalize(
    nodes: &[String],
    pos: &[(f64, f64)],
    idx: &std::collections::HashMap<&str, usize>,
    pinned: &Pinned,
    center: (i64, i64),
    scale: f64,
    out: &mut Positions,
) {
    let (cx, cy) = (center.0 as f64, center.1 as f64);
    for nm in nodes {
        if pinned.contains_key(nm) {
            continue;
        }
        let i = idx[nm.as_str()];
        out.insert(
            nm.clone(),
            (
                (pos[i].0 * scale + cx).round() as i64,
                (pos[i].1 * scale + cy).round() as i64,
            ),
        );
    }
}

/// Tidy tree layout (Reingold-Tilford family) — deterministic; faithful port.
pub fn apply_tree(
    nodes: &[String],
    edges: &[(String, String)],
    pinned: &Pinned,
    x_spacing: f64,
    y_spacing: f64,
    center: (i64, i64),
) -> Positions {
    let mut out = Positions::new();
    if nodes.is_empty() {
        return out;
    }
    // Undirected adjacency (sorted for determinism).
    let mut adj: BTreeMap<String, std::collections::BTreeSet<String>> = BTreeMap::new();
    for n in nodes {
        adj.entry(n.clone()).or_default();
    }
    for (a, b) in edges {
        if a != b && adj.contains_key(a) && adj.contains_key(b) {
            adj.get_mut(a).unwrap().insert(b.clone());
            adj.get_mut(b).unwrap().insert(a.clone());
        }
    }
    let degree = |n: &str| adj.get(n).map(|s| s.len()).unwrap_or(0);

    // Roots: pinned first, then highest degree, then name.
    let mut candidates: Vec<&String> = nodes.iter().collect();
    candidates.sort_by(|a, b| {
        let ka = (
            pinned.contains_key(*a) as i32,
            -(degree(a) as i64),
            (*a).clone(),
        );
        let kb = (
            pinned.contains_key(*b) as i32,
            -(degree(b) as i64),
            (*b).clone(),
        );
        ka.cmp(&kb)
    });

    // BFS spanning forest.
    let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for n in nodes {
        children.insert(n.clone(), Vec::new());
    }
    let mut visited = std::collections::HashSet::new();
    let mut roots: Vec<String> = Vec::new();
    let mut depth: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for r in &candidates {
        if visited.contains(*r) {
            continue;
        }
        roots.push((*r).clone());
        visited.insert((*r).clone());
        depth.insert((*r).clone(), 0);
        let mut q = std::collections::VecDeque::new();
        q.push_back((*r).clone());
        while let Some(u) = q.pop_front() {
            let neigh: Vec<String> = adj[&u].iter().cloned().collect();
            for v in neigh {
                if !visited.contains(&v) {
                    visited.insert(v.clone());
                    depth.insert(v.clone(), depth[&u] + 1);
                    children.get_mut(&u).unwrap().push(v.clone());
                    q.push_back(v);
                }
            }
        }
    }

    // Subtree widths (post-order).
    let mut width: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    fn compute_width(
        node: &str,
        children: &BTreeMap<String, Vec<String>>,
        width: &mut std::collections::HashMap<String, i64>,
    ) -> i64 {
        let kids = &children[node];
        let w = if kids.is_empty() {
            1
        } else {
            kids.iter()
                .map(|c| compute_width(c, children, width))
                .sum::<i64>()
                .max(1)
        };
        width.insert(node.to_string(), w);
        w
    }
    for r in &roots {
        compute_width(r, &children, &mut width);
    }

    // Top-down x assignment; parents centred over children.
    let mut raw: std::collections::HashMap<String, (f64, f64)> = std::collections::HashMap::new();
    #[allow(clippy::too_many_arguments)]
    fn assign(
        node: &str,
        left: i64,
        children: &BTreeMap<String, Vec<String>>,
        width: &std::collections::HashMap<String, i64>,
        depth: &std::collections::HashMap<String, i64>,
        x_spacing: f64,
        y_spacing: f64,
        raw: &mut std::collections::HashMap<String, (f64, f64)>,
    ) {
        let kids = &children[node];
        let mut child_left = left;
        for c in kids {
            assign(
                c, child_left, children, width, depth, x_spacing, y_spacing, raw,
            );
            child_left += width[c];
        }
        let x = if kids.is_empty() {
            (left as f64 + 0.5) * x_spacing
        } else {
            let xs: Vec<f64> = kids.iter().map(|c| raw[c].0).collect();
            (xs.iter().cloned().fold(f64::INFINITY, f64::min)
                + xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
                / 2.0
        };
        raw.insert(node.to_string(), (x, depth[node] as f64 * y_spacing));
    }
    let mut cursor = 0;
    for r in &roots {
        assign(
            r, cursor, &children, &width, &depth, x_spacing, y_spacing, &mut raw,
        );
        cursor += width[r];
    }
    if raw.is_empty() {
        return out;
    }
    let xs: Vec<f64> = raw.values().map(|p| p.0).collect();
    let ys: Vec<f64> = raw.values().map(|p| p.1).collect();
    let mid_x = (xs.iter().cloned().fold(f64::INFINITY, f64::min)
        + xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
        / 2.0;
    let mid_y = (ys.iter().cloned().fold(f64::INFINITY, f64::min)
        + ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
        / 2.0;
    let (cx, cy) = (center.0 as f64, center.1 as f64);
    for nm in nodes {
        if pinned.contains_key(nm) {
            continue;
        }
        let (x, y) = raw[nm];
        out.insert(
            nm.clone(),
            (
                (x - mid_x + cx).round() as i64,
                (y - mid_y + cy).round() as i64,
            ),
        );
    }
    out
}

/// Compute NEW positions for `auto_keys` under `mode`.
///
/// `mode` is resolved through [`normalize_arrange`] first. Geometric modes
/// (`grid`/`circle`/`radial`/`random`) position only `auto_keys`; the topology
/// modes (`fr`/`forceatlas2`/`tree`) run over `all_keys`. Unknown modes fall
/// through to `random`.
#[allow(clippy::too_many_arguments)]
pub fn place(
    mode: &str,
    all_keys: &[String],
    auto_keys: &[String],
    edges: &[(String, String)],
    center: (i64, i64),
    scale: f64,
) -> Positions {
    let mode = normalize_arrange(mode);
    let (cx, cy) = center;
    // Topology layouts treat manually-placed nodes (all_keys not in auto_keys)
    // as pinned anchors.
    let pinned: Pinned = Pinned::new();
    match mode.as_str() {
        "grid" => place_grid(auto_keys, cx, cy, scale),
        "circle" => place_circle(auto_keys, cx, cy, scale),
        "radial" => place_radial(auto_keys, all_keys, edges, cx, cy, scale),
        "fr" => apply_fr(all_keys, edges, &pinned, 50, 800.0 * scale, center),
        "forceatlas2" => apply_forceatlas2(all_keys, edges, &pinned, 200, 60.0 * scale, center),
        "tree" => apply_tree(
            all_keys,
            edges,
            &pinned,
            160.0 * scale,
            200.0 * scale,
            center,
        ),
        _ => place_random(auto_keys, cx, cy, scale),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn radial_two_unlinked_hubs_matches_upstream_minimal() {
        // alice<->bob: both degree 1 => no hubs => isolated grid below.
        // With center (0,0), scale 1.0 this reproduces the minimal .anx:
        // alice(-80, 320), bob(80, 320).
        let all = keys(&["alice", "bob"]);
        let edges = vec![("alice".to_string(), "bob".to_string())];
        let pos = place_radial(&all, &all, &edges, 0, 0, 1.0);
        assert_eq!(pos["alice"], (-80, 320));
        assert_eq!(pos["bob"], (80, 320));
    }

    #[test]
    fn grid_is_centered() {
        let k = keys(&["a", "b", "c", "d"]);
        let pos = place_grid(&k, 0, 0, 1.0);
        assert_eq!(pos.len(), 4);
    }

    #[test]
    fn tree_matches_upstream_exactly() {
        // Deterministic Reingold-Tilford — identical to the Python layout.
        let nodes = keys(&["root", "a", "b", "c", "d"]);
        let edges: Vec<(String, String)> = [("root", "a"), ("root", "b"), ("a", "c"), ("a", "d")]
            .iter()
            .map(|(x, y)| (x.to_string(), y.to_string()))
            .collect();
        let p = apply_tree(&nodes, &edges, &Pinned::new(), 160.0, 200.0, (0, 0));
        assert_eq!(p["root"], (160, 0));
        assert_eq!(p["a"], (0, -200));
        assert_eq!(p["b"], (160, 200));
        assert_eq!(p["c"], (-160, 0));
        assert_eq!(p["d"], (0, 0));
    }

    #[test]
    fn force_layouts_produce_finite_positions_for_all_nodes() {
        let nodes = keys(&["a", "b", "c", "d"]);
        let edges: Vec<(String, String)> = [("a", "b"), ("b", "c"), ("c", "d")]
            .iter()
            .map(|(x, y)| (x.to_string(), y.to_string()))
            .collect();
        for pos in [
            apply_fr(&nodes, &edges, &Pinned::new(), 50, 800.0, (0, 0)),
            apply_forceatlas2(&nodes, &edges, &Pinned::new(), 200, 60.0, (0, 0)),
        ] {
            assert_eq!(pos.len(), 4);
            assert!(pos
                .values()
                .all(|(x, y)| x.abs() < 1_000_000 && y.abs() < 1_000_000));
        }
    }

    #[test]
    fn normalize_aliases() {
        assert_eq!(normalize_arrange("Force-Atlas 2"), "forceatlas2");
        assert_eq!(normalize_arrange("FA2"), "forceatlas2");
        assert_eq!(normalize_arrange("Reingold-Tilford"), "tree");
        assert_eq!(normalize_arrange("weird"), "weird");
    }
}
