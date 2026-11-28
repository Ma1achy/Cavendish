//! The study's GPU kernels on wgpu (Vulkan), timed and checked against the CPU f64 kernels.
//!
//!     gpu <work>/<shape> <level> <out.json>
//!
//! Three kernels (`shaders.wgsl`): a soup triangle of the exact pieces, the PLIC piece built from its
//! plane in the shader, and the moments sum (point + second + third). Each in f32, float-float
//! (`vec2<f32>`, Joldes et al. 2017's algorithms with Dekker's split, own log1p and atan2) and,
//! where the adapter has SHADER_F64, f64 (own log1p and atan2: Vulkan's GLSL.std.450 has none for
//! 64-bit). Dispatches are capped at ~8 ms each and waited for one at a time, so the desktop stays
//! usable. The GTX 1660 SUPER is a correctness-only card (the owner's ruling): times are indicative.

#[path = "../geom.rs"]
mod geom;
#[path = "../kappa.rs"]
mod kappa;
#[path = "../kern.rs"]
mod kern;
#[path = "../real.rs"]
mod real;

use std::time::Instant;

use geom::{add, cross, cut_cell, norm, sub, unit, V};
use kern::{Mp, Poly, STri};

const RHO: f64 = 2700.0;
const SRC: &str = include_str!("../shaders.wgsl");

fn read_f64(p: &std::path::Path) -> Vec<f64> {
    std::fs::read(p).unwrap().chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect()
}

#[derive(Clone, Copy, PartialEq)]
enum Prec {
    F32,
    Ff,
    F64,
}

fn pair(x: f64) -> (f32, f32) {
    let h = x as f32;
    (h, (x - f64::from(h)) as f32)
}

fn lit(x: f32) -> String {
    let s = format!("{x:e}");
    if s.contains('.') || s.contains('e') { s } else { format!("{s}.0") }
}

fn constants() -> String {
    let c2 = |x: f64| {
        let (h, l) = pair(x);
        format!("nc2({}, {})", lit(h), lit(l))
    };
    let inv_odd: Vec<String> = (0..14).map(|j| c2(1.0 / (2 * j + 1) as f64)).collect();
    let mut fact = vec![1.0f64];
    for k in 1..=20 {
        fact.push(fact[k - 1] * k as f64);
    }
    let sin_c: Vec<String> = (0..10).map(|j| c2(if j % 2 == 0 { 1.0 } else { -1.0 } / fact[2 * j + 1])).collect();
    let cos_c: Vec<String> = (0..10).map(|j| c2(if j % 2 == 0 { 1.0 } else { -1.0 } / fact[2 * j])).collect();
    format!(
        "fn inv_odd() -> array<N, 14> {{ return array<N, 14>({}); }}\n\
         fn sin_c() -> array<N, 10> {{ return array<N, 10>({}); }}\n\
         fn cos_c() -> array<N, 10> {{ return array<N, 10>({}); }}\n\
         fn ln2() -> N {{ return {}; }}\nfn pio2() -> N {{ return {}; }}\nfn third() -> N {{ return {}; }}\n",
        inv_odd.join(", "),
        sin_c.join(", "),
        cos_c.join(", "),
        c2(std::f64::consts::LN_2),
        c2(std::f64::consts::FRAC_PI_2),
        c2(1.0 / 3.0)
    )
}

/// log1p and atan2 in N arithmetic (float-float and f64): reduction + atanh series; an f32 seed
/// corrected once with sin and cos of the seed by Taylor series.
const GENERIC: &str = r#"
fn nlog1p(x: N) -> N {
    var t: N;
    var k = 0.0;
    if (nhi(x) < 0.41) {
        t = ndiv(x, nadd(nc(2.0), x));
    } else {
        let y = nadd(nc(1.0), x);
        k = round(log2(nhi(y)));
        let f = nmul(y, nc(exp2(-k)));
        t = ndiv(nsub(f, nc(1.0)), nadd(f, nc(1.0)));
    }
    let t2 = nmul(t, t);
    let c = inv_odd();
    var term = t;
    var s = t;
    for (var j = 1u; j < 14u; j++) {
        term = nmul(term, t2);
        if (abs(nhi(term)) <= 1e-17 * abs(nhi(s))) { break; }
        s = nadd(s, nmul(term, c[j]));
    }
    return nadd(nmul(nc(2.0), s), nmul(ln2(), nc(k)));
}
struct SC { s: N, c: N }
fn nsincos(th: f32) -> SC {
    let k = round(th / 1.5707964);
    let r = nsub(nc(th), nmul(pio2(), nc(k)));
    let r2 = nmul(r, r);
    let cs = sin_c();
    let cc = cos_c();
    var ps = cs[9];
    var pc = cc[9];
    for (var j = 8i; j >= 0i; j--) {
        ps = nadd(cs[j], nmul(r2, ps));
        pc = nadd(cc[j], nmul(r2, pc));
    }
    let s = nmul(r, ps);
    let q = ((i32(k) % 4) + 4) % 4;
    if (q == 0) { return SC(s, pc); }
    if (q == 1) { return SC(pc, nsub(nc(0.0), s)); }
    if (q == 2) { return SC(nsub(nc(0.0), s), nsub(nc(0.0), pc)); }
    return SC(nsub(nc(0.0), pc), s);
}
fn natan2(y: N, x: N) -> N {
    let th0 = atan2(nhi(y), nhi(x));
    let sc = nsincos(th0);
    let q = ndiv(nsub(nmul(y, sc.c), nmul(x, sc.s)), nadd(nmul(x, sc.c), nmul(y, sc.s)));
    let q3 = nmul(nmul(q, q), q);
    return nadd(nc(th0), nsub(q, nmul(q3, third())));
}
"#;

fn prelude(p: Prec, slots: u32) -> String {
    let s = slots;
    match p {
        Prec::F32 => format!(
            "alias N = f32;\nfn nc(x: f32) -> N {{ return x; }}\nfn nadd(a: N, b: N) -> N {{ return a + b; }}\n\
             fn nsub(a: N, b: N) -> N {{ return a - b; }}\nfn nmul(a: N, b: N) -> N {{ return a * b; }}\n\
             fn ndiv(a: N, b: N) -> N {{ return a / b; }}\nfn nsqrt(a: N) -> N {{ return sqrt(a); }}\n\
             fn nhi(a: N) -> f32 {{ return a; }}\nfn natan2(y: N, x: N) -> N {{ return atan2(y, x); }}\n\
             fn nlog1p(x: N) -> N {{ let u = 1.0 + x; if (u == 1.0) {{ return x; }} return log(u) * x / (u - 1.0); }}\n\
             fn ld(e: u32, k: u32) -> N {{ return data[e * {s}u + k]; }}\nfn ldf(e: u32, k: u32) -> f32 {{ return data[e * {s}u + k]; }}\n\
             fn pt(h: f32, l: f32) -> N {{ return h; }}\nfn load(v: vec2<f32>) -> N {{ return v.x; }}\n\
             fn store(a: N) -> vec2<f32> {{ return vec2<f32>(a, 0.0); }}\n"
        ),
        Prec::Ff => format!(
            "alias N = vec2<f32>;\n\
             fn g1(x: f32) -> f32 {{ return x * bitcast<f32>(prm.pad); }}
\n             fn two_sum(a: f32, b: f32) -> vec2<f32> {{ let s = g1(a + b); let bb = g1(s - a); return vec2<f32>(s, (a - g1(s - bb)) + (b - bb)); }}\n\
             fn qts(a: f32, b: f32) -> vec2<f32> {{ let s = g1(a + b); return vec2<f32>(s, b - g1(s - a)); }}\n\
             fn split(a: f32) -> vec2<f32> {{ let c = g1(4097.0 * a); let ah = g1(c - g1(c - a)); return vec2<f32>(ah, a - ah); }}\n\
             fn two_prod(a: f32, b: f32) -> vec2<f32> {{ let p = g1(a * b); let sa = split(a); let sb = split(b); \
                return vec2<f32>(p, ((sa.x * sb.x - p) + sa.x * sb.y + sa.y * sb.x) + sa.y * sb.y); }}\n\
             fn nc(x: f32) -> N {{ return vec2<f32>(x, 0.0); }}\nfn nc2(h: f32, l: f32) -> N {{ return vec2<f32>(h, l); }}\n\
             fn nadd(a: N, b: N) -> N {{ let s = two_sum(a.x, b.x); let t = two_sum(a.y, b.y); let c = s.y + t.x; \
                let v = qts(s.x, c); let w = t.y + v.y; return qts(v.x, w); }}\n\
             fn nsub(a: N, b: N) -> N {{ return nadd(a, -b); }}\n\
             fn nmul(a: N, b: N) -> N {{ let p = two_prod(a.x, b.x); return qts(p.x, p.y + (a.x * b.y + a.y * b.x)); }}\n\
             fn ndiv(a: N, b: N) -> N {{ let q1 = a.x / b.x; let r = nsub(a, nmul(b, nc(q1))); let q2 = r.x / b.x; \
                let r2 = nsub(r, nmul(b, nc(q2))); return nadd(qts(q1, q2), nc(r2.x / b.x)); }}\n\
             fn nsqrt(a: N) -> N {{ if (a.x <= 0.0) {{ return nc(0.0); }} let s = sqrt(a.x); let p = two_prod(s, s); \
                return qts(s, ((a.x - p.x) - p.y + a.y) / (2.0 * s)); }}\n\
             fn nhi(a: N) -> f32 {{ return a.x; }}\n\
             fn ld(e: u32, k: u32) -> N {{ let i = 2u * (e * {s}u + k); return vec2<f32>(data[i], data[i + 1u]); }}\n\
             fn ldf(e: u32, k: u32) -> f32 {{ return data[2u * (e * {s}u + k)]; }}\n\
             fn pt(h: f32, l: f32) -> N {{ return vec2<f32>(h, l); }}\nfn load(v: vec2<f32>) -> N {{ return v; }}\n\
             fn store(a: N) -> vec2<f32> {{ return a; }}\n{}{GENERIC}",
            constants()
        ),
        Prec::F64 => format!(
            "alias N = f64;\nfn nc(x: f32) -> N {{ return f64(x); }}\nfn nc2(h: f32, l: f32) -> N {{ return f64(h) + f64(l); }}\n\
             fn nadd(a: N, b: N) -> N {{ return a + b; }}\nfn nsub(a: N, b: N) -> N {{ return a - b; }}\n\
             fn nmul(a: N, b: N) -> N {{ return a * b; }}\nfn ndiv(a: N, b: N) -> N {{ return a / b; }}\n\
             fn nsqrt(a: N) -> N {{ return sqrt(a); }}\nfn nhi(a: N) -> f32 {{ return f32(a); }}\n\
             fn ld(e: u32, k: u32) -> N {{ let i = 2u * (e * {s}u + k); return f64(data[i]) + f64(data[i + 1u]); }}\n\
             fn ldf(e: u32, k: u32) -> f32 {{ return data[2u * (e * {s}u + k)]; }}\n\
             fn pt(h: f32, l: f32) -> N {{ return f64(h) + f64(l); }}\nfn load(v: vec2<f32>) -> N {{ return f64(v.x) + f64(v.y); }}\n\
             fn store(a: N) -> vec2<f32> {{ let h = f32(a); return vec2<f32>(h, f32(a - f64(h))); }}\n{}{GENERIC}",
            constants()
        ),
    }
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    info: String,
    f64: bool,
}

fn gpu() -> Gpu {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        flags: wgpu::InstanceFlags::empty(),
        backend_options: wgpu::BackendOptions::default(),
        memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
        display: None,
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("a Vulkan adapter");
    let ai = adapter.get_info();
    let has64 = adapter.features().contains(wgpu::Features::SHADER_F64);
    let features = if has64 { wgpu::Features::SHADER_F64 } else { wgpu::Features::empty() };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("srbench"),
        required_features: features,
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .expect("a device");
    let info = format!("{} ({:?}, driver {} {})", ai.name, ai.backend, ai.driver, ai.driver_info);
    Gpu { device, queue, info, f64: has64 }
}

struct Run {
    u: Vec<f64>,
    seconds: f64,
    dispatches: usize,
}

impl Gpu {
    fn buffer(&self, bytes: &[u8], usage: wgpu::BufferUsages) -> wgpu::Buffer {
        use wgpu::util::DeviceExt;
        self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytes, usage })
    }

    /// Run `kernel` over all elements at all points; dispatches of a few ms each, waited for in turn.
    fn run(&self, prec: Prec, kernel: &str, slots: u32, elems: &[f64], points: &[V]) -> Result<Run, String> {
        let src = format!("{}\n{}", prelude(prec, slots), SRC.trim_start_matches('\u{feff}').replace("KERNEL(", &format!("{kernel}(")));
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(kernel),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });
        let pipeline = self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(kernel),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        if let Some(e) = pollster::block_on(scope.pop()) {
            return Err(format!("{e}"));
        }
        let n = elems.len() / slots as usize;
        let data: Vec<f32> = if prec == Prec::F32 {
            elems.iter().map(|&x| x as f32).collect()
        } else {
            elems.iter().flat_map(|&x| {
                let (h, l) = pair(x);
                [h, l]
            }).collect()
        };
        let pts: Vec<f32> = points.iter().flat_map(|p| p.iter().flat_map(|&x| {
            let (h, l) = pair(x);
            [h, l]
        }).collect::<Vec<_>>()).collect();
        let npts = points.len() as u32;
        let dbuf = self.buffer(bytemuck::cast_slice(&data), wgpu::BufferUsages::STORAGE);
        let pbuf = self.buffer(bytemuck::cast_slice(&pts), wgpu::BufferUsages::STORAGE);
        let obuf = self.buffer(&vec![0u8; 8 * points.len()], wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC);
        let ubuf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: dbuf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: pbuf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: obuf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: ubuf.as_entire_binding() },
            ],
        });
        let dispatch = |e0: usize, e1: usize| -> Result<f64, String> {
            self.queue.write_buffer(&ubuf, 0, bytemuck::cast_slice(&[e0 as u32, e1 as u32, npts, 1.0f32.to_bits()]));
            let mut enc = self.device.create_command_encoder(&Default::default());
            {
                let mut pass = enc.begin_compute_pass(&Default::default());
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &bind, &[]);
                pass.dispatch_workgroups(npts.div_ceil(64), 1, 1);
            }
            self.queue.submit([enc.finish()]);
            let t0 = Instant::now();
            self.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| format!("{e}"))?;
            Ok(t0.elapsed().as_secs_f64())
        };
        dispatch(0, 0)?; // first use of the pipeline, an empty range: not timed
        let mut chunk = 4usize;
        let mut e0 = 0usize;
        let mut seconds = 0.0;
        let mut dispatches = 0;
        let mut slept = 0.0;
        let wall = Instant::now();
        while e0 < n {
            let e1 = (e0 + chunk).min(n);
            let dt = dispatch(e0, e1)?;
            seconds += dt;
            dispatches += 1;
            // Aim at ~8 ms per dispatch, so the desktop stays usable.
            chunk = ((0.008 / (dt / (e1 - e0) as f64).max(1e-12)) as usize).clamp(1, 1 << 20);
            e0 = e1;
            let z = Instant::now();
            std::thread::sleep(std::time::Duration::from_millis(2));
            slept += z.elapsed().as_secs_f64();
        }
        // Wall time of the whole loop less the sleeps: the per-dispatch waits alone can return
        // before the work is done on this driver (seen: 0.0 s for 748 x 16320 pairs).
        let raw_back = self.read(&obuf);
        let wall_s = wall.elapsed().as_secs_f64() - slept;
        let _ = seconds;
        let seconds = wall_s;
        let raw = raw_back;
        let out: Vec<f32> = bytemuck::cast_slice(&raw).to_vec();
        let u = out.chunks_exact(2).map(|c| f64::from(c[0]) + f64::from(c[1])).collect();
        Ok(Run { u, seconds, dispatches })
    }

    fn read(&self, buf: &wgpu::Buffer) -> Vec<u8> {
        let copy = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: buf.size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_buffer_to_buffer(buf, 0, &copy, 0, buf.size());
        self.queue.submit([enc.finish()]);
        let slice = copy.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        slice.get_mapped_range().unwrap().to_vec()
    }
}

/// Does the driver keep the error-free transforms? two_sum(1, 2^-30) and two_prod(1 + 2^-12, 1 + 2^-12)
/// must return their exact error terms; with `guard` the operands pass through a runtime 1.0.
fn ff_probe(g: &Gpu) -> String {
    let mut out = Vec::new();
    for guard in [false, true] {
        let one = if guard { "u.one" } else { "1.0" };
        let src = format!(
            "struct U {{ one: f32, a: f32, b: f32, c: f32 }}
@group(0) @binding(0) var<storage, read_write> o: array<f32>;
             @group(0) @binding(1) var<uniform> u: U;
             fn two_sum(a: f32, b: f32) -> vec2<f32> {{ let s = (a + b) * {one}; let bb = (s - a) * {one}; return vec2<f32>(s, (a - (s - bb)) + (b - bb)); }}
             fn split(a: f32) -> vec2<f32> {{ let c = (4097.0 * a) * {one}; let ah = (c - (c - a)) * {one}; return vec2<f32>(ah, a - ah); }}
             fn two_prod(a: f32, b: f32) -> vec2<f32> {{ let p = (a * b) * {one}; let sa = split(a); let sb = split(b); return vec2<f32>(p, ((sa.x * sb.x - p) + sa.x * sb.y + sa.y * sb.x) + sa.y * sb.y); }}
             @compute @workgroup_size(1) fn main() {{ let t = two_sum(u.a, u.b); let q = two_prod(u.c, u.c); o[0] = t.x; o[1] = t.y; o[2] = q.x; o[3] = q.y; }}"
        );
        let module = g.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(src.into()) });
        let pipeline = g.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None, layout: None, module: &module, entry_point: Some("main"), compilation_options: Default::default(), cache: None,
        });
        let obuf = g.buffer(&[0u8; 16], wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC);
        let c = 1.0f32 + 2f32.powi(-12);
        let ubuf = g.buffer(bytemuck::cast_slice(&[1.0f32, 1.0, 2f32.powi(-30), c]), wgpu::BufferUsages::UNIFORM);
        let bind = g.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: obuf.as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: ubuf.as_entire_binding() }],
        });
        let mut enc = g.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        g.queue.submit([enc.finish()]);
        let r: Vec<f32> = bytemuck::cast_slice(&g.read(&obuf)).to_vec();
        let exact_q = f64::from(c) * f64::from(c) - f64::from(r[2]);
        out.push(format!("{{\"guard\": {guard}, \"two_sum_err\": {:e}, \"expected\": {:e}, \"two_prod_err\": {:e}, \"expected_prod\": {:e}}}", r[1], 2f32.powi(-30), r[3], exact_q));
    }
    format!("[{}]", out.join(", "))
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a[1] == "probe" {
        let g = gpu();
        println!("{}", ff_probe(&g));
        return;
    }
    let dir = std::path::PathBuf::from(&a[1]);
    let level: usize = a[2].parse().unwrap();
    let ld = dir.join(format!("L{level}"));
    let cells = read_f64(&ld.join("cells.f64"));
    let tris = read_f64(&ld.join("tris.f64"));
    let tris: Vec<[V; 3]> = tris.chunks_exact(9).map(|t| [[t[0], t[1], t[2]], [t[3], t[4], t[5]], [t[6], t[7], t[8]]]).collect();
    // Towers: all points, with their coefficients (accuracy); timing on tower 2's points, repeated
    // with small offsets to fill the card.
    let tw = read_f64(&dir.join("towers.f64"));
    let rows: Vec<&[f64]> = tw.chunks_exact(4).collect();
    let ntw: usize = std::fs::read_to_string(dir.join("towers.count")).unwrap().trim().parse().unwrap();
    let per = rows.len() / ntw;
    let pts: Vec<V> = rows.iter().map(|r| [r[0], r[1], r[2]]).collect();
    let co: Vec<f64> = rows.iter().map(|r| r[3]).collect();
    let mut tpts = Vec::new();
    for k in 0..40 {
        let o = [0.01 * (k % 7) as f64, 0.01 * (k % 5) as f64, 0.0];
        tpts.extend(pts[per..2 * per].iter().map(|p| add(*p, o)));
    }
    // Elements from the cells (a stride through them, ~600 cells).
    let cells: Vec<&[f64]> = cells.chunks_exact(6).collect();
    let stride = (cells.len() / 600).max(1);
    let (mut soup, mut plic, mut mom) = (Vec::new(), Vec::new(), Vec::new());
    let (mut soup_c, mut plic_c, mut mom_c) = (Vec::new(), Vec::new(), Vec::new());
    for c in cells.iter().step_by(stride) {
        let lo = [c[0], c[1], c[2]];
        let h = c[3];
        let (s, k) = (c[4] as usize, c[5] as usize);
        let t = &tris[s..s + k];
        let centre = add(lo, [h / 2.0; 3]);
        let m = geom::moments(t, centre);
        for x in t {
            let st = STri::new(*x);
            soup.extend(st.v.iter().flatten());
            soup.extend(st.n);
            soup.extend(st.ne.iter().flatten());
            soup.extend(st.el);
            soup.push(st.a2);
            soup_c.push(st);
        }
        let mut nm = [0.0; 3];
        for x in t {
            let tol = 1e-10 * h;
            let wall = (0..3).any(|a| x.iter().all(|v| (v[a] - lo[a]).abs() < tol) || x.iter().all(|v| (v[a] - lo[a] - h).abs() < tol));
            if !wall {
                nm = add(nm, cross(sub(x[1], x[0]), sub(x[2], x[0])));
            }
        }
        let nm = if norm(nm) > 0.0 { unit(nm) } else { [0.0, 0.0, 1.0] };
        let al = geom::plic_offset(lo, h, nm, m.vol);
        plic.extend(lo);
        plic.push(h);
        plic.extend(nm);
        plic.push(al);
        plic_c.push(Poly::from_polys(&cut_cell(lo, h, &[(nm, al)]), 1e-12 * h));
        mom.extend(m.cen);
        mom.push(m.vol);
        mom.extend(m.m2);
        mom.extend(m.m3);
        mom_c.push(Mp { m: m.vol, dp: [0.0; 3], c: m.cen, i: m.m2, t: m.m3 });
    }
    // CPU f64 references at every tower point, four threads.
    let reference = |f: &(dyn Fn(V) -> f64 + Sync)| -> Vec<f64> {
        let chunk = pts.len().div_ceil(4);
        std::thread::scope(|s| {
            let hs: Vec<_> = pts.chunks(chunk).map(|pp| s.spawn(move || pp.iter().map(|p| f(*p)).collect::<Vec<f64>>())).collect();
            hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
        })
    };
    let t0 = Instant::now();
    let ref_soup = reference(&|p| soup_c.iter().map(|t| t.u::<f64>(p)).sum());
    let ref_plic = reference(&|p| {
        let (mut r, mut l) = (Vec::new(), Vec::new());
        plic_c.iter().map(|q| q.u::<f64>(p, &mut r, &mut l)).sum()
    });
    let ref_mom = reference(&|p| mom_c.iter().map(|m| m.u::<f64>(p, 3)).sum());
    let cpu_ref_s = t0.elapsed().as_secs_f64();
    // CPU single-thread time per element per point on the timing points (f64), for the ratio.
    let cpu_time = |f: &dyn Fn(V) -> f64, n: usize| -> f64 {
        let few = &tpts[..64];
        let t0 = Instant::now();
        let s: f64 = few.iter().map(|p| f(*p)).sum();
        std::hint::black_box(s);
        t0.elapsed().as_secs_f64() / (few.len() * n) as f64 * 1e9
    };
    let cpu_soup = cpu_time(&|p| soup_c.iter().map(|t| t.u::<f64>(p)).sum(), soup_c.len());
    let cpu_plic = cpu_time(
        &|p| {
            let (mut r, mut l) = (Vec::new(), Vec::new());
            plic_c.iter().map(|q| q.u::<f64>(p, &mut r, &mut l)).sum()
        },
        plic_c.len(),
    );
    let cpu_mom = cpu_time(&|p| mom_c.iter().map(|m| m.u::<f64>(p, 3)).sum(), mom_c.len());

    let g = gpu();
    let mut rows_out = Vec::new();
    let phase = |u: &[f64]| -> Vec<f64> {
        (0..ntw).map(|t| (t * per..(t + 1) * per).map(|i| co[i] * (-RHO * u[i])).sum()).collect()
    };
    let jobs: Vec<(&str, &str, u32, &Vec<f64>, &Vec<f64>, f64, usize, Prec)> = {
        let mut j = Vec::new();
        for (p, pn) in [(Prec::F32, "f32"), (Prec::Ff, "ff"), (Prec::F64, "f64")] {
            if p == Prec::F64 && !g.f64 {
                continue;
            }
            j.push(("soup_u", pn, 25, &soup, &ref_soup, cpu_soup, soup_c.len(), p));
            j.push(("plic_u", pn, 8, &plic, &ref_plic, cpu_plic, plic_c.len(), p));
            j.push(("mom_u", pn, 20, &mom, &ref_mom, cpu_mom, mom_c.len(), p));
            if p == Prec::Ff {
                j.push(("mom_mixed_u", pn, 20, &mom, &ref_mom, cpu_mom, mom_c.len(), p));
            }
        }
        j
    };
    for (kernel, pn, slots, elems, reference, cpu_ns, n, p) in jobs {
        let acc = g.run(p, kernel, slots, elems, &pts);
        // Best of three: the card's clocks and the desktop make single runs vary by up to 5x.
        let mut tim = g.run(p, kernel, slots, elems, &tpts);
        for _ in 0..2 {
            if let (Ok(t0), Ok(t1)) = (&tim, g.run(p, kernel, slots, elems, &tpts)) {
                if t1.seconds < t0.seconds {
                    tim = Ok(t1);
                }
            }
        }
        match (acc, tim) {
            (Ok(acc), Ok(tim)) => {
                let rel = acc.u.iter().zip(reference).map(|(a, b)| ((a - b) / b).abs()).fold(0.0, f64::max);
                let pg = phase(&acc.u);
                let pr = phase(reference);
                let dph: Vec<String> = pg.iter().zip(&pr).map(|(a, b)| format!("{:e}", (a - b).abs())).collect();
                let ns = tim.seconds / (n as f64 * tpts.len() as f64) * 1e9;
                let bytes = (slots as usize * if p == Prec::F32 { 4 } else { 8 }) as f64;
                println!("{kernel} {pn}: {ns:.3} ns/elem/pt, rel {rel:.1e}, phase err {dph:?}");
                rows_out.push(format!(
                    "{{\"kernel\": \"{kernel}\", \"precision\": \"{pn}\", \"elements\": {n}, \"timing_points\": {}, \"seconds\": {:.4}, \
                     \"dispatches\": {}, \"ns_per_element_point\": {ns:.4}, \"cpu_f64_ns_per_element_point\": {cpu_ns:.2}, \
                     \"element_bytes\": {bytes}, \"max_rel_err_u\": {rel:e}, \"phase_error_per_tower\": [{}], \"phase_per_tower\": [{}]}}",
                    tpts.len(),
                    tim.seconds,
                    tim.dispatches,
                    dph.join(", "),
                    pr.iter().map(|x| format!("{x:e}")).collect::<Vec<_>>().join(", ")
                ));
            }
            (a, b) => {
                let e = a.err().or(b.err()).unwrap();
                println!("{kernel} {pn}: failed: {e}");
                rows_out.push(format!("{{\"kernel\": \"{kernel}\", \"precision\": \"{pn}\", \"error\": {:?}}}", e));
            }
        }
    }
    let rate = rate_bench(&g);
    let json = format!(
        "{{\"adapter\": {:?}, \"shader_f64\": {}, \"cpu_reference_seconds\": {cpu_ref_s:.2}, \"runs\": [{}], \"fma_rate\": {rate}}}",
        g.info,
        g.f64,
        rows_out.join(",\n ")
    );
    std::fs::write(&a[3], json).unwrap();
}

/// Peak arithmetic: 16 independent FMA chains per thread, f32 and (if present) f64; GFLOP/s
/// counting an FMA as two.
fn rate_bench(g: &Gpu) -> String {
    let mut out = Vec::new();
    for (t, name) in [("f32", "f32"), ("f64", "f64")] {
        if t == "f64" && !g.f64 {
            continue;
        }
        let iters = if t == "f32" { 4096 } else { 256 };
        // Sixteen named accumulators, unrolled (an indexed local array would live in memory).
        let decl: String = (0..16).map(|k| format!("var a{k} = {t}(f32({k}) * 0.01);
")).collect();
        let body: String = (0..16).map(|k| format!("a{k} = fma(a{k}, y, x);
")).collect();
        let sum: String = (0..16).map(|k| format!(" + a{k}")).collect();
        let src = format!(
            "@group(0) @binding(0) var<storage, read_write> o: array<f32>;
             @compute @workgroup_size(64) fn main(@builtin(global_invocation_id) gid: vec3<u32>) {{
             let x = {t}(f32(gid.x) * 1e-9); let y = {t}(0.999999);
{decl}             for (var i = 0u; i < {iters}u; i++) {{
{body}}}
             o[gid.x] = f32({t}(0.0){sum}); }}"
        );
        let module = g.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(src.into()) });
        let pipeline = g.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let threads = 64 * 1024u32;
        let obuf = g.buffer(&vec![0u8; 4 * threads as usize], wgpu::BufferUsages::STORAGE);
        let bind = g.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: obuf.as_entire_binding() }],
        });
        let mut best = f64::INFINITY;
        for _ in 0..4 {
            let mut enc = g.device.create_command_encoder(&Default::default());
            {
                let mut pass = enc.begin_compute_pass(&Default::default());
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &bind, &[]);
                pass.dispatch_workgroups(threads / 64, 1, 1);
            }
            g.queue.submit([enc.finish()]);
            let t0 = Instant::now();
            g.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            best = best.min(t0.elapsed().as_secs_f64());
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let flops = 2.0 * 16.0 * f64::from(iters) * f64::from(threads);
        out.push(format!("\"{name}_gflops\": {:.1}, \"{name}_seconds\": {best:.5}", flops / best / 1e9));
    }
    format!("{{{}}}", out.join(", "))
}
