//! Times the fluid step on a room-sized grid with a warm block on the floor.
use physics::{Fluid, FluidParams, Grid3, Heat};
use std::time::Instant;

fn main() {
    let grid = Grid3::new([0.0; 3], 0.1, [40, 40, 30]);
    let mut f = Fluid::new(grid, FluidParams::air(293.15));
    for k in 0..6 {
        for j in 18..22 {
            for i in 18..22 {
                let id = grid.idx(i, j, k);
                f.solid[id] = true;
                f.solid_heat[id] = Heat::Film {
                    t_surface: 320.0,
                    h_over_rho_cp: 5.0 / (1.2 * 1005.0),
                };
            }
        }
    }
    let steps = 100;
    let start = Instant::now();
    let mut iters = 0;
    for _ in 0..steps {
        f.step(0.05);
        iters += f.last_iterations;
    }
    let per = start.elapsed().as_secs_f64() / steps as f64;
    let vmax = f.w.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    let tmax = f.t.iter().fold(0.0f64, |m, &x| m.max(x));
    println!("profile [advect, diffuse, project] s: {:.2?}", f.timing);
    println!(
        "{} cells: {:.1} ms/step, {:.1} CG iterations/step, w_max {:.3} m/s, T_max {:.2} K after {} s",
        grid.len(),
        per * 1e3,
        iters as f64 / steps as f64,
        vmax,
        tmax,
        steps as f64 * 0.05
    );
}
