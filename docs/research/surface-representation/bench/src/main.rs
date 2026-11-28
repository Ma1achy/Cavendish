//! The surface-representation study's CPU bench (`docs/research/surface-representation.md`).
//!
//!     srbench check   <work>                 C1: the soup kernel against cav_core's Faceted
//!     srbench contrib <work>/<shape> <level> every representation of every boundary cell, read
//!                                            at the three towers: L<k>/contrib.f64, L<k>/stats.f64
//!     srbench time    <work>/<shape> <level> ns per element per point, FLOPs per element (JSON)
//!     srbench whole   <work>/<shape>         the whole mesh read at the towers, and C2
//!
//! Inputs are `prep.py`'s little-endian f64 files. Four threads at most; the caller lowers the
//! process's priority.

mod geom;
mod kappa;
mod kern;
mod real;

use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::Instant;

use geom::{add, cross, cut_cell, dot, norm, scale, sub, unit, Mom, Polys, V};
use kern::{Bx, Mp, Poly, STri};
use real::{take_counts, Ct, Real, KINDS};

const RHO: f64 = 2700.0;
const THREADS: usize = 4;

fn read_f64(p: &Path) -> Vec<f64> {
    let b = fs::read(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    b.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect()
}
fn write_f64(p: &Path, x: &[f64]) {
    let mut b = Vec::with_capacity(8 * x.len());
    for v in x {
        b.extend_from_slice(&v.to_le_bytes());
    }
    fs::write(p, b).unwrap();
}
fn tris_of(f: &[f64]) -> Vec<[V; 3]> {
    f.chunks_exact(9).map(|t| [[t[0], t[1], t[2]], [t[3], t[4], t[5]], [t[6], t[7], t[8]]]).collect()
}

/// The towers: per tower, points and coefficients (phase = Î£ c V/G).
fn towers(dir: &Path) -> Vec<(Vec<V>, Vec<f64>)> {
    let t = read_f64(&dir.join("towers.f64"));
    let rows: Vec<&[f64]> = t.chunks_exact(4).collect();
    let count: usize = fs::read_to_string(dir.join("towers.count")).unwrap().trim().parse().unwrap();
    let n = rows.len() / count;
    (0..count).map(|k| rows[k * n..(k + 1) * n].iter().map(|r| ([r[0], r[1], r[2]], r[3])).unzip()).collect()
}

/// An element of a representation: something with a potential.
enum Elem {
    Soup(Vec<STri>),
    Poly(Poly),
    Mp(Mp, u8),
    Sum(Vec<(f64, Elem)>),
}

impl Elem {
    fn u(&self, p: V, r: &mut Vec<([f64; 3], f64)>, l: &mut Vec<f64>) -> f64 {
        match self {
            Elem::Soup(t) => t.iter().map(|x| x.u::<f64>(p)).sum(),
            Elem::Poly(q) => q.u::<f64>(p, r, l),
            Elem::Mp(m, o) => m.u::<f64>(p, *o),
            Elem::Sum(v) => v.iter().map(|(s, e)| s * e.u(p, r, l)).sum(),
        }
    }
}

fn mp_of(m: &Mom) -> Mp {
    Mp { m: m.vol, dp: [0.0; 3], c: m.cen, i: m.m2, t: m.m3 }
}

fn poly(p: &Polys, h: f64) -> Poly {
    Poly::from_polys(p, 1e-12 * h)
}

/// The piece's surface triangles (not on a wall of the cell).
fn surface(tris: &[[V; 3]], lo: V, h: f64) -> Vec<[V; 3]> {
    let tol = 1e-10 * h;
    tris.iter()
        .copied()
        .filter(|t| {
            !(0..3).any(|a| t.iter().all(|v| (v[a] - lo[a]).abs() < tol) || t.iter().all(|v| (v[a] - lo[a] - h).abs() < tol))
        })
        .collect()
}

const REPS: [&str; 22] = [
    "exact", "fill", "centroid", "second", "third", "plic_centroid_normal", "plic_mesh_normal", "plic_mof", "mof_store32",
    "mof_store16", "mof_store8", "mof_fill32", "mof_fill16", "mof_fill8", "mof_pack32", "two_plane", "mof_residual",
    "two_plane_residual", "plic_mesh_turned", "mof_fill8_residual", "exact_welded", "mof_fill8_residual_common",
];
const STATS: [&str; 50] = [
    "triangles", "surface_triangles", "welded_vertices", "welded_edges", "welded_faces", "plic_vertices", "plic_faces",
    "two_plane", "concave", "mof_resid_over_h", "cen_normal_resid_over_h", "mesh_normal_resid_over_h", "store8_mass_err",
    "store16_mass_err", "store32_mass_err", "flat", "poly_vs_soup", "fill",
    "mof_nx", "mof_ny", "mof_nz", "mof_alpha", "mof_symdiff_over_h3", "lo_x", "lo_y", "lo_z", "hi_x", "hi_y", "hi_z",
    "vol", "cx", "cy", "cz", "i_xx", "i_yy", "i_zz", "i_xy", "i_xz", "i_yz", "t_xxx", "t_yyy", "t_zzz", "t_xxy", "t_xxz",
    "t_xyy", "t_yyz", "t_xzz", "t_yzz", "t_xyz", "f8_symdiff_over_h3",
];

/// Every representation of one cell, and its statistics.
fn represent(lo: V, h: f64, tris: &[[V; 3]]) -> (Vec<Elem>, [f64; 50]) {
    let centre = add(lo, [h / 2.0; 3]);
    let mom = geom::moments(tris, centre);
    let target = mom.vol;
    let surf = surface(tris, lo, h);
    let mut st = [0.0; 50];
    st[0] = tris.len() as f64;
    st[1] = surf.len() as f64;
    st[17] = mom.vol / (h * h * h);
    let welded = Poly::from_soup(tris, 1e-11 * h);
    let (v, e, f, _) = welded.counts();
    st[2] = v as f64;
    st[3] = e as f64;
    st[4] = f as f64;
    // Normals: from the centroid's offset, and the mesh's area-weighted normal.
    let mut nm = [0.0; 3];
    for t in &surf {
        let c = cross(sub(t[1], t[0]), sub(t[2], t[0]));
        nm = add(nm, scale(c, 0.5));
    }
    let nm = if norm(nm) > 0.0 { unit(nm) } else { [0.0, 0.0, 1.0] };
    let off = sub(centre, mom.cen);
    let nc = if norm(off) > 1e-9 * h { unit(off) } else { nm };
    let piece = |n: V| -> (Polys, f64) {
        let a = geom::plic_offset(lo, h, n, target);
        (cut_cell(lo, h, &[(n, a)]), a)
    };
    let resid = |p: &Polys| norm(sub(p.vol_cen(lo).1, mom.cen)) / h;
    let (pc, _) = piece(nc);
    let (pm, _) = piece(nm);
    let (n1, e1) = geom::mof(lo, h, target, mom.cen, nm);
    let (n2, e2) = geom::mof(lo, h, target, mom.cen, nc);
    let nmof = if e1 <= e2 { n1 } else { n2 };
    let (pmof, amof) = piece(nmof);
    st[5] = pmof.vertex_count(1e-12 * h) as f64;
    // For the walk's bound: the plane, |P Δ Q| (exact piece P, PLIC piece Q), the piece's box and moments.
    st[18..21].copy_from_slice(&nmof);
    st[21] = amof;
    let below = vol_below(tris, nmof, amof + dot(nmof, centre), centre);
    st[22] = (mom.vol + pmof.vol_cen(lo).0 - 2.0 * below) / (h * h * h);
    for a in 0..3 {
        st[23 + a] = tris.iter().flatten().map(|v| v[a]).fold(f64::INFINITY, f64::min);
        st[26 + a] = tris.iter().flatten().map(|v| v[a]).fold(f64::NEG_INFINITY, f64::max);
    }
    st[29] = mom.vol;
    st[30..33].copy_from_slice(&mom.cen);
    st[33..39].copy_from_slice(&mom.m2);
    st[39..49].copy_from_slice(&mom.m3);
    st[6] = pmof.faces.len() as f64;
    st[9] = resid(&pmof);
    st[10] = resid(&pc);
    st[11] = resid(&pm);
    let stored = |bits: u32| -> Polys {
        let (nq, aq) = geom::store_plane(nmof, amof, h, bits, bits);
        cut_cell(lo, h, &[(nq, aq)])
    };
    let from_fill = |bits: u32| -> Polys {
        let (nq, _) = geom::store_plane(nmof, amof, h, bits, bits);
        piece(nq).0
    };
    let s32 = stored(32);
    let s16 = stored(16);
    let s8 = stored(8);
    for (k, p) in [(12, &s8), (13, &s16), (14, &s32)] {
        st[k] = if p.faces.is_empty() { -1.0 } else { p.vol_cen(lo).0 / target - 1.0 };
    }
    let (nq, aq) = geom::store_plane(nmof, amof, h, 11, 10);
    let pack = cut_cell(lo, h, &[(nq, aq)]);
    let f8 = from_fill(8);
    // Two planes where the surface has two directions, else the mesh-normal PLIC piece.
    let cl = geom::two_clusters(&surf);
    let (two, two_pieces): (Elem, Vec<(f64, Polys)>) = if cl.len() == 2 {
        let (pieces, concave) = geom::two_plane(lo, h, &cl, target);
        st[7] = 1.0;
        st[8] = f64::from(u8::from(concave));
        let e = Elem::Sum(pieces.iter().filter(|(_, p)| !p.faces.is_empty()).map(|(s, p)| (*s, Elem::Poly(poly(p, h)))).collect());
        (e, pieces)
    } else {
        (Elem::Poly(poly(&pm, h)), vec![(1.0, pm.clone())])
    };
    // A cell cut by one flat face only (C3).
    let flat = !surf.is_empty() && {
        let n0 = STri::new(surf[0]).n;
        surf.iter().all(|t| {
            let s = STri::new(*t);
            dot(s.n, n0) > 1.0 - 1e-12 && t.iter().all(|v| dot(n0, sub(*v, surf[0][0])).abs() < 1e-12 * h)
        })
    };
    st[15] = f64::from(u8::from(flat));
    let perp = unit(cross(nm, if nm[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] }));
    let (pturn, _) = piece(unit(add(nm, scale(perp, 1e-3))));
    let mom_of = |p: &Polys| geom::moments(&p.tris(), centre);
    let resid_elem = |p: &Polys| -> Elem {
        Elem::Sum(vec![(1.0, Elem::Poly(poly(p, h))), (1.0, Elem::Mp(mp_of(&mom), 3)), (-1.0, Elem::Mp(mp_of(&mom_of(p)), 3))])
    };
    let two_mom = {
        let mut m = Mom::default();
        let mut tot = [0.0; 3];
        // Signed moments of the two-plane piece: sum the signed pieces' moments about the centre.
        let ms: Vec<(f64, Mom)> = two_pieces.iter().filter(|(_, p)| !p.faces.is_empty()).map(|(s, p)| (*s, mom_of(p))).collect();
        for (s, x) in &ms {
            m.vol += s * x.vol;
            tot = add(tot, scale(x.cen, s * x.vol));
        }
        m.cen = scale(tot, 1.0 / m.vol);
        for (s, x) in &ms {
            let d = sub(x.cen, m.cen);
            for (n, &(i, j)) in geom::I2.iter().enumerate() {
                m.m2[n] += s * (x.m2[n] + x.vol * d[i] * d[j]);
            }
            for (n, &(i, j, k)) in geom::I3.iter().enumerate() {
                m.m3[n] += s
                    * (x.m3[n]
                        + d[i] * x.m2[geom::i2(j, k)]
                        + d[j] * x.m2[geom::i2(i, k)]
                        + d[k] * x.m2[geom::i2(i, j)]
                        + x.vol * d[i] * d[j] * d[k]);
            }
        }
        m
    };
    let soup: Vec<STri> = tris.iter().map(|t| STri::new(*t)).collect();
    let el = vec![
        Elem::Soup(soup),
        Elem::Mp(Mp { m: mom.vol, dp: [0.0; 3], c: centre, i: [0.0; 6], t: [0.0; 10] }, 0),
        Elem::Mp(mp_of(&mom), 0),
        Elem::Mp(mp_of(&mom), 2),
        Elem::Mp(mp_of(&mom), 3),
        Elem::Poly(poly(&pc, h)),
        Elem::Poly(poly(&pm, h)),
        Elem::Poly(poly(&pmof, h)),
        Elem::Poly(poly(&s32, h)),
        Elem::Poly(poly(&s16, h)),
        Elem::Poly(poly(&s8, h)),
        Elem::Poly(poly(&from_fill(32), h)),
        Elem::Poly(poly(&from_fill(16), h)),
        Elem::Poly(poly(&f8, h)),
        Elem::Poly(poly(&pack, h)),
        two,
        resid_elem(&pmof),
        Elem::Sum(vec![
            (1.0, Elem::Sum(two_pieces.iter().filter(|(_, p)| !p.faces.is_empty()).map(|(s, p)| (*s, Elem::Poly(poly(p, h)))).collect())),
            (1.0, Elem::Mp(mp_of(&mom), 3)),
            (-1.0, Elem::Mp(mp_of(&two_mom), 3)),
        ]),
        Elem::Poly(poly(&pturn, h)),
        resid_elem(&f8),
        Elem::Poly(welded),
        common_residual(&f8, &mom, centre, h),
    ];
    let (nq8, _) = geom::store_plane(nmof, amof, h, 8, 8);
    let a8 = geom::plic_offset(lo, h, nq8, target);
    st[49] = (mom.vol + f8.vol_cen(lo).0 - 2.0 * vol_below(tris, nq8, a8 + dot(nq8, centre), centre)) / (h * h * h);
    (el, st)
}

/// Raw moments of `m` about `o`: volume, dipole, second and third (the Mp orders).
fn raw_about(m: &Mom, o: V) -> (f64, V, [f64; 6], [f64; 10]) {
    let c = sub(m.cen, o);
    let mut m2 = m.m2;
    for (n, &(i, j)) in geom::I2.iter().enumerate() {
        m2[n] += m.vol * c[i] * c[j];
    }
    let mut m3 = m.m3;
    for (n, &(i, j, k)) in geom::I3.iter().enumerate() {
        m3[n] += c[i] * m.m2[geom::i2(j, k)] + c[j] * m.m2[geom::i2(i, k)] + c[k] * m.m2[geom::i2(i, j)] + m.vol * c[i] * c[j] * c[k];
    }
    (m.vol, scale(c, m.vol), m2, m3)
}

/// The PLIC piece Q plus the multipole (to l = 3) of P − Q about the cell centre: its error is the
/// l ≥ 4 part of P − Q, bounded by ρ|P Δ Q| (b/d)⁴/(d − b) with b the cell's half-diagonal.
fn common_residual(q: &Polys, p: &Mom, centre: V, h: f64) -> Elem {
    let qm = geom::moments(&q.tris(), centre);
    let (pv, pd, p2, p3) = raw_about(p, centre);
    let (qv, qd, q2, q3) = raw_about(&qm, centre);
    let mut i = [0.0; 6];
    let mut t = [0.0; 10];
    for n in 0..6 {
        i[n] = p2[n] - q2[n];
    }
    for n in 0..10 {
        t[n] = p3[n] - q3[n];
    }
    Elem::Sum(vec![(1.0, Elem::Poly(poly(q, h))), (1.0, Elem::Mp(Mp { m: pv - qv, dp: sub(pd, qd), c: centre, i, t }, 3))])
}

/// The volume of the closed solid `tris` below the plane n·x = d: each triangle clipped, the cap
/// closed by its projected area (the clipped surface's vector area is minus the cap's).
fn vol_below(tris: &[[V; 3]], n: V, d: f64, o: V) -> f64 {
    let (mut vol, mut area) = (0.0, [0.0; 3]);
    for t in tris {
        let p = Polys { faces: vec![t.to_vec()] };
        let mut out = Vec::new();
        let f = &p.faces[0];
        for i in 0..3 {
            let (a, b) = (f[i], f[(i + 1) % 3]);
            let (da, db) = (dot(n, a) - d, dot(n, b) - d);
            if da <= 0.0 {
                out.push(a);
            }
            if (da < 0.0 && db > 0.0) || (da > 0.0 && db < 0.0) {
                out.push(add(a, scale(sub(b, a), da / (da - db))));
            }
        }
        for k in 1..out.len().saturating_sub(1) {
            let (a, b, c) = (sub(out[0], o), sub(out[k], o), sub(out[k + 1], o));
            vol += dot(a, cross(b, c)) / 6.0;
            area = add(area, scale(cross(sub(b, a), sub(c, a)), 0.5));
        }
    }
    vol + (d - dot(n, o)) * (-dot(area, n)) / 3.0
}

fn contrib(dir: &Path, level: usize) {
    let tw = towers(dir);
    let ld = dir.join(format!("L{level}"));
    let cells = read_f64(&ld.join("cells.f64"));
    let tris = tris_of(&read_f64(&ld.join("tris.f64")));
    let cells: Vec<&[f64]> = cells.chunks_exact(6).collect();
    let nrep = REPS.len();
    let nt = tw.len();
    let mut out = vec![0.0; cells.len() * nrep * nt];
    let mut stats = vec![0.0; cells.len() * STATS.len()];
    let t0 = Instant::now();
    let chunk = cells.len().div_ceil(THREADS).max(1);
    std::thread::scope(|s| {
        for ((oc, sc), cs) in out.chunks_mut(chunk * nrep * nt).zip(stats.chunks_mut(chunk * STATS.len())).zip(cells.chunks(chunk)) {
            let tw = &tw;
            let tris = &tris;
            s.spawn(move || {
                let (mut r, mut l) = (Vec::new(), Vec::new());
                for (k, c) in cs.iter().enumerate() {
                    let lo = [c[0], c[1], c[2]];
                    let h = c[3];
                    let (a, n) = (c[4] as usize, c[5] as usize);
                    let (el, st) = represent(lo, h, &tris[a..a + n]);
                    sc[k * STATS.len()..(k + 1) * STATS.len()].copy_from_slice(&st);
                    for (j, e) in el.iter().enumerate() {
                        for (t, (pts, co)) in tw.iter().enumerate() {
                            let mut ph = 0.0;
                            for (p, c) in pts.iter().zip(co) {
                                ph += c * (-RHO * e.u(*p, &mut r, &mut l));
                            }
                            oc[(k * nrep + j) * nt + t] = ph;
                        }
                    }
                }
            });
        }
    });
    write_f64(&ld.join("contrib.f64"), &out);
    write_f64(&ld.join("stats.f64"), &stats);
    println!(
        "{{\"cells\": {}, \"towers\": {nt}, \"reps\": {:?}, \"stats\": {:?}, \"seconds\": {:.2}}}",
        cells.len(),
        REPS,
        STATS,
        t0.elapsed().as_secs_f64()
    );
}

/// The whole mesh and the interior cubes at each tower (phase, rad), and C2 on level `level`.
fn whole(dir: &Path) {
    let tw = towers(dir);
    let mesh: Vec<STri> = tris_of(&read_f64(&dir.join("mesh.f64"))).into_iter().map(STri::new).collect();
    let phase_of = |tr: &[STri]| -> Vec<f64> {
        tw.iter()
            .map(|(pts, co)| {
                let chunk = pts.len().div_ceil(THREADS);
                let parts: Vec<f64> = std::thread::scope(|s| {
                    let hs: Vec<_> = pts
                        .chunks(chunk)
                        .zip(co.chunks(chunk))
                        .map(|(pp, cc)| {
                            s.spawn(move || pp.iter().zip(cc).map(|(p, c)| c * (-RHO * tr.iter().map(|t| t.u::<f64>(*p)).sum::<f64>())).sum::<f64>())
                        })
                        .collect();
                    hs.into_iter().map(|h| h.join().unwrap()).collect()
                });
                parts.iter().sum()
            })
            .collect()
    };
    let t0 = Instant::now();
    let m = phase_of(&mesh);
    println!("{{\"mesh_triangles\": {}, \"mesh_phase\": {:?}, \"seconds\": {:.1}}}", mesh.len(), m, t0.elapsed().as_secs_f64());
}

/// C2: exact pieces + interior cubes against the whole mesh, on one level; control drops a piece.
fn partition(dir: &Path, level: usize) {
    let tw = towers(dir);
    let (pts, co) = &tw[0];
    let mesh: Vec<STri> = tris_of(&read_f64(&dir.join("mesh.f64"))).into_iter().map(STri::new).collect();
    let ld = dir.join(format!("L{level}"));
    let pieces: Vec<STri> = tris_of(&read_f64(&ld.join("tris.f64"))).into_iter().map(STri::new).collect();
    let cells = read_f64(&ld.join("cells.f64"));
    let h = cells[3];
    let interior = read_f64(&ld.join("interior.f64"));
    let mut cubes = Vec::new();
    for c in interior.chunks_exact(3) {
        let lo = [c[0] - h / 2.0, c[1] - h / 2.0, c[2] - h / 2.0];
        cubes.extend(Polys::cube(lo, h).tris().into_iter().map(STri::new));
    }
    let first_piece = cells[5] as usize;
    let sum = |tr: &[STri]| -> f64 {
        let chunk = pts.len().div_ceil(THREADS);
        std::thread::scope(|s| {
            let hs: Vec<_> = pts
                .chunks(chunk)
                .zip(co.chunks(chunk))
                .map(|(pp, cc)| s.spawn(move || pp.iter().zip(cc).map(|(p, c)| c * (-RHO * tr.iter().map(|t| t.u::<f64>(*p)).sum::<f64>())).sum::<f64>()))
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).sum()
        })
    };
    let pm = sum(&mesh);
    let pp = sum(&pieces);
    let pc = sum(&cubes);
    let dropped = sum(&pieces[first_piece..]);
    println!(
        "{{\"level\": {level}, \"mesh_phase\": {pm:e}, \"pieces_phase\": {pp:e}, \"interior_phase\": {pc:e}, \"difference\": {:e}, \"control_one_piece_dropped\": {:e}}}",
        pp + pc - pm,
        dropped + pc - pm
    );
}

/// C1: the soup kernel (and the indexed Poly kernel) against cav_core's `Faceted::potential`.
fn check(work: &Path) {
    use cav_core::polyhedral::Faceted;
    use cav_core::Vec3;
    let sphere = tris_of(&read_f64(&work.join("sphere").join("mesh.f64")));
    let cube = Polys::cube([0.1234, -0.0567, 0.0789], 0.37).tris();
    let mut out = Vec::new();
    for (name, tris) in [("icosphere", sphere), ("cube", cube)] {
        let fa = Faceted::new(&tris.iter().map(|t| t.map(|v| Vec3::new(v[0], v[1], v[2]))).collect::<Vec<_>>()).unwrap();
        let soup: Vec<STri> = tris.iter().map(|t| STri::new(*t)).collect();
        let pl = Poly::from_soup(&tris, 1e-12);
        let mut moved = tris.clone();
        moved[0][0][0] += 1e-6;
        let ctrl: Vec<STri> = moved.iter().map(|t| STri::new(*t)).collect();
        let (mut ws, mut wp, mut wc) = (0f64, 0f64, 0f64);
        let (mut r, mut l) = (Vec::new(), Vec::new());
        for d in [1.5, 3.0, 5.0, 10.0, 20.0] {
            for dir in [[1.0, 0.2, -0.3], [-0.3, 1.0, 0.2], [0.2, -0.3, 1.0]] {
                let p = scale(unit(dir), d + 1.0);
                let f = -fa.potential(1.0, Vec3::new(p[0], p[1], p[2]));
                let s: f64 = soup.iter().map(|t| t.u::<f64>(p)).sum();
                let q = pl.u::<f64>(p, &mut r, &mut l);
                let c: f64 = ctrl.iter().map(|t| t.u::<f64>(p)).sum();
                ws = ws.max(((s - f) / f).abs());
                wp = wp.max(((q - f) / f).abs());
                wc = wc.max(((c - f) / f).abs());
            }
        }
        out.push(format!(
            "\"{name}\": {{\"triangles\": {}, \"soup_vs_faceted\": {ws:e}, \"poly_vs_faceted\": {wp:e}, \"control_vertex_moved\": {wc:e}}}",
            tris.len()
        ));
    }
    println!("{{{}}}", out.join(", "));
}

/// Time `f` over every (point, element): ns per element per point, best of three.
fn time_it<F: Fn(V) -> f64>(pts: &[V], n: usize, f: F) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..3 {
        let t0 = Instant::now();
        let mut s = 0.0;
        for p in pts {
            s += f(*p);
        }
        black_box(s);
        best = best.min(t0.elapsed().as_secs_f64());
    }
    best / (pts.len() * n) as f64 * 1e9
}

fn counts_json(c: [u64; 6], n: usize) -> String {
    let per: Vec<String> = KINDS.iter().zip(c).map(|(k, x)| format!("\"{k}\": {:.2}", x as f64 / n as f64)).collect();
    format!("{{{}}}", per.join(", "))
}

fn time(dir: &Path, level: usize) {
    let tw = towers(dir);
    let pts: Vec<V> = tw[0].0.iter().step_by(4).copied().collect(); // 102 of the 408 points
    let ld = dir.join(format!("L{level}"));
    let cells = read_f64(&ld.join("cells.f64"));
    let tris = tris_of(&read_f64(&ld.join("tris.f64")));
    let cells: Vec<&[f64]> = cells.chunks_exact(6).collect();
    let stride = (cells.len() / 400).max(1);
    let sample: Vec<&&[f64]> = cells.iter().step_by(stride).collect();
    let mut soups = Vec::new();
    let mut welded = Vec::new();
    let mut plics = Vec::new();
    let mut planes = Vec::new();
    let mut twos = Vec::new();
    let mut mps = Vec::new();
    let mut boxes = Vec::new();
    let mut build_ns = 0.0;
    let h = cells[0][3];
    for c in &sample {
        let lo = [c[0], c[1], c[2]];
        let (a, n) = (c[4] as usize, c[5] as usize);
        let t = &tris[a..a + n];
        let centre = add(lo, [h / 2.0; 3]);
        let mom = geom::moments(t, centre);
        soups.push(t.iter().map(|x| STri::new(*x)).collect::<Vec<_>>());
        welded.push(Poly::from_soup(t, 1e-11 * h));
        let surf = surface(t, lo, h);
        let mut nm = [0.0; 3];
        for s in &surf {
            nm = add(nm, cross(sub(s[1], s[0]), sub(s[2], s[0])));
        }
        let nm = if norm(nm) > 0.0 { unit(nm) } else { [0.0, 0.0, 1.0] };
        let al = geom::plic_offset(lo, h, nm, mom.vol);
        let t0 = Instant::now();
        let pp = poly(&cut_cell(lo, h, &[(nm, al)]), h);
        build_ns += t0.elapsed().as_secs_f64() * 1e9;
        plics.push(pp);
        planes.push((lo, nm, al));
        let cl = geom::two_clusters(&surf);
        if cl.len() == 2 {
            let (pieces, _) = geom::two_plane(lo, h, &cl, mom.vol);
            twos.push(pieces.iter().filter(|(_, p)| !p.faces.is_empty()).map(|(s, p)| (*s, poly(p, h))).collect::<Vec<_>>());
        }
        mps.push(mp_of(&mom));
        boxes.push(Bx::new(mom.vol, centre, [h / 2.0 * 0.9, h / 2.0, h / 2.0 * 1.1]));
    }
    let n = sample.len();
    let ntri: usize = soups.iter().map(Vec::len).sum();
    let (mut r, mut l) = (Vec::new(), Vec::new());
    let (mut rf, mut lf) = (Vec::<([f32; 3], f32)>::new(), Vec::<f32>::new());
    let mut rows: Vec<String> = Vec::new();
    let mut row = |name: &str, ns: f64, per: &str, flops: String, bytes: f64| {
        rows.push(format!("\"{name}\": {{\"ns_per_element_point\": {ns:.2}, \"per\": \"{per}\", \"ops\": {flops}, \"kernel_bytes\": {bytes:.0}}}"));
    };
    // Op counts at one point.
    let p0 = pts[0];
    let pc = [Ct(p0[0]), Ct(p0[1]), Ct(p0[2])];
    let count = |f: &dyn Fn()| {
        take_counts();
        f();
        take_counts()
    };
    let soup_ns = time_it(&pts, n, |p| soups.iter().map(|s| s.iter().map(|t| t.u::<f64>(p)).sum::<f64>()).sum());
    let c = count(&|| {
        for s in &soups {
            for t in s {
                black_box(t.u::<Ct>(pc));
            }
        }
    });
    row("exact_soup", soup_ns, "piece", counts_json(c, n), (ntri * std::mem::size_of::<STri>()) as f64 / n as f64);
    let c1 = count(&|| {
        black_box(soups[0][0].u::<Ct>(pc));
    });
    row("soup_triangle", soup_ns * n as f64 / ntri as f64, "triangle", counts_json(c1, 1), std::mem::size_of::<STri>() as f64);
    let wb: f64 = welded.iter().map(|w| (w.v.len() * 24 + w.e.len() * 12 + w.f.iter().map(|f| 24 + f.edges.len() * 28 + f.fan.len() * 14).sum::<usize>()) as f64).sum::<f64>() / n as f64;
    let ns = time_it(&pts, n, |p| welded.iter().map(|w| w.u::<f64>(p, &mut Vec::new(), &mut Vec::new())).sum());
    let ns_reuse = {
        let mut best = f64::INFINITY;
        for _ in 0..3 {
            let t0 = Instant::now();
            let mut s = 0.0;
            for p in &pts {
                for w in &welded {
                    s += w.u::<f64>(*p, &mut r, &mut l);
                }
            }
            black_box(s);
            best = best.min(t0.elapsed().as_secs_f64());
        }
        best / (pts.len() * n) as f64 * 1e9
    };
    let c = count(&|| {
        let (mut r, mut l) = (Vec::new(), Vec::new());
        for w in &welded {
            black_box(w.u::<Ct>(pc, &mut r, &mut l));
        }
    });
    let _ = ns;
    row("exact_welded", ns_reuse, "piece", counts_json(c, n), wb);
    let pb: f64 = plics.iter().map(|w| (w.v.len() * 24 + w.e.len() * 12 + w.f.iter().map(|f| 24 + f.edges.len() * 28 + f.fan.len() * 14).sum::<usize>()) as f64).sum::<f64>() / n as f64;
    let mut best = f64::INFINITY;
    for _ in 0..3 {
        let t0 = Instant::now();
        let mut s = 0.0;
        for p in &pts {
            for w in &plics {
                s += w.u::<f64>(*p, &mut r, &mut l);
            }
        }
        black_box(s);
        best = best.min(t0.elapsed().as_secs_f64());
    }
    let c = count(&|| {
        let (mut r, mut l) = (Vec::new(), Vec::new());
        for w in &plics {
            black_box(w.u::<Ct>(pc, &mut r, &mut l));
        }
    });
    row("plic_prebuilt", best / (pts.len() * n) as f64 * 1e9, "piece", counts_json(c, n), pb);
    // f32 PLIC on the CPU, for reference against the GPU.
    let mut best = f64::INFINITY;
    for _ in 0..3 {
        let t0 = Instant::now();
        let mut s = 0.0f32;
        for p in &pts {
            let pf = [p[0] as f32, p[1] as f32, p[2] as f32];
            for w in &plics {
                s += w.u::<f32>(pf, &mut rf, &mut lf);
            }
        }
        black_box(s);
        best = best.min(t0.elapsed().as_secs_f64());
    }
    row("plic_prebuilt_f32", best / (pts.len() * n) as f64 * 1e9, "piece", "{}".into(), pb);
    // Built from the stored plane at every evaluation.
    let few = &pts[..8];
    let ns = time_it(few, n, |p| {
        planes.iter().map(|&(lo, nm, al)| poly(&cut_cell(lo, h, &[(nm, al)]), h).u::<f64>(p, &mut Vec::new(), &mut Vec::new())).sum()
    });
    row("plic_rebuilt_each_point", ns, "piece", "{}".into(), 16.0);
    row("plic_build_once", build_ns / n as f64, "piece (not per point)", "{}".into(), 0.0);
    if !twos.is_empty() {
        let m = twos.len();
        let mut best = f64::INFINITY;
        for _ in 0..3 {
            let t0 = Instant::now();
            let mut s = 0.0;
            for p in &pts {
                for w in &twos {
                    for (sg, q) in w {
                        s += sg * q.u::<f64>(*p, &mut r, &mut l);
                    }
                }
            }
            black_box(s);
            best = best.min(t0.elapsed().as_secs_f64());
        }
        row("two_plane", best / (pts.len() * m) as f64 * 1e9, &format!("element ({m} of {n} cells)"), "{}".into(), 0.0);
    }
    for (o, name) in [(0u8, "point"), (2, "point_second"), (3, "point_second_third")] {
        let ns = time_it(&pts, n, |p| mps.iter().map(|m| m.u::<f64>(p, o)).sum());
        let c = count(&|| {
            black_box(mps[0].u::<Ct>(pc, o));
        });
        // Stored as f32: mass and centroid 16 B, + six second moments 40 B, + ten third 80 B.
        row(name, ns, "element", counts_json(c, 1), match o { 0 => 16.0, 2 => 40.0, _ => 80.0 });
    }
    for lmax in [0u8, 2, 4, 6] {
        let ns = time_it(&pts, n, |p| boxes.iter().map(|b| b.u::<f64>(p, lmax)).sum());
        let c = count(&|| {
            black_box(boxes[0].u::<Ct>(pc, lmax));
        });
        row(&format!("box_series_l{lmax}"), ns, "element", counts_json(c, 1), (32 + 8 * [0, 3, 9, 19][usize::from(lmax / 2)]) as f64);
    }
    {
        use cav_core::voxel::kernel::cube_potential;
        use cav_core::Vec3;
        let ns = time_it(&pts, n, |p| {
            mps.iter().map(|m| cube_potential(m.m, h, Vec3::new(p[0] - m.c[0], p[1] - m.c[1], p[2] - m.c[2]))).sum()
        });
        row("engine_cube_potential", ns, "element", "{}".into(), 32.0);
    }
    let cube: Vec<STri> = Polys::cube([0.0; 3], h).tris().into_iter().map(STri::new).collect();
    let ns = time_it(&pts, n, |p| mps.iter().map(|m| cube.iter().map(|t| t.u::<f64>(sub(p, m.c))).sum::<f64>()).sum());
    row("cube_closed_form_12_triangles", ns, "element", "{}".into(), 12.0 * 200.0);
    println!(
        "{{\"level\": {level}, \"h\": {h}, \"sample_cells\": {n}, \"points\": {}, \"mean_piece_triangles\": {:.1}, \"kernels\": {{{}}}}}",
        pts.len(),
        ntri as f64 / n as f64,
        rows.join(", ")
    );
}

/// Boxes `<file>` (rows lo, hi; m) read at the towers: per box and tower, the phase of its closed
/// form (12 triangles) and of its series to l = 0, 2, 4, 6 (`<file>.phases.f64`).
fn boxes(dir: &Path, file: &Path) {
    let tw = towers(dir);
    let b = read_f64(file);
    let rows: Vec<&[f64]> = b.chunks_exact(6).collect();
    let nt = tw.len();
    let mut out = vec![0.0; rows.len() * nt * 5];
    let chunk = rows.len().div_ceil(THREADS).max(1);
    let t0 = Instant::now();
    std::thread::scope(|s| {
        for (oc, rs) in out.chunks_mut(chunk * nt * 5).zip(rows.chunks(chunk)) {
            let tw = &tw;
            s.spawn(move || {
                for (k, r) in rs.iter().enumerate() {
                    let (lo, hi) = ([r[0], r[1], r[2]], [r[3], r[4], r[5]]);
                    let half = scale(sub(hi, lo), 0.5);
                    let c = add(lo, half);
                    let tris: Vec<STri> = Polys::cube([0.0; 3], 1.0)
                        .tris()
                        .into_iter()
                        .map(|t| STri::new(t.map(|v| [lo[0] + v[0] * (hi[0] - lo[0]), lo[1] + v[1] * (hi[1] - lo[1]), lo[2] + v[2] * (hi[2] - lo[2])])))
                        .collect();
                    let bx = Bx::new(8.0 * half[0] * half[1] * half[2], c, half);
                    for (t, (pts, co)) in tw.iter().enumerate() {
                        let mut ph = [0.0; 5];
                        for (p, cc) in pts.iter().zip(co) {
                            ph[0] += cc * (-RHO * tris.iter().map(|x| x.u::<f64>(*p)).sum::<f64>());
                            for (j, l) in [0u8, 2, 4, 6].iter().enumerate() {
                                ph[1 + j] += cc * (-RHO * bx.u::<f64>(*p, *l));
                            }
                        }
                        oc[(k * nt + t) * 5..(k * nt + t) * 5 + 5].copy_from_slice(&ph);
                    }
                }
            });
        }
    });
    let mut name = file.as_os_str().to_owned();
    name.push(".phases.f64");
    write_f64(Path::new(&name), &out);
    println!("{{\"boxes\": {}, \"towers\": {nt}, \"seconds\": {:.2}}}", rows.len(), t0.elapsed().as_secs_f64());
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = PathBuf::from(&a[2]);
    match a[1].as_str() {
        "check" => check(&p),
        "contrib" => contrib(&p, a[3].parse().unwrap()),
        "time" => time(&p, a[3].parse().unwrap()),
        "whole" => whole(&p),
        "partition" => partition(&p, a[3].parse().unwrap()),
        "boxes" => boxes(&p, Path::new(&a[3])),
        x => panic!("unknown command {x}"),
    }
}
