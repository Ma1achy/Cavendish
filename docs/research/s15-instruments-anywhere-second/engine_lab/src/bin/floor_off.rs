use cav_core::forward::{Instrument, Mass, Reader};
use cav_core::Vec3;
fn main() {
    let mut a = Instrument::aion10(); a.floor = false;
    for (d, z) in [(5.0, 5.0), (10.0, 5.0), (50.0, 5.0), (200.0, 5.0), (3.0, 12.0)] {
        let r = Reader::new(a);
        let p = Vec3::new(d, 0.0, z);
        let s = Mass::point(1.0, move |_t| p);
        println!("floor-off d={d} z={z}: {:.12e}", r.phase(Vec3::ZERO, &s, 1.46));
    }
    let mut c = Instrument::aice(); c.floor = false;
    let r = Reader::new(c); let p = Vec3::new(200.0, 0.0, 35.0);
    let s = Mass::point(1.0, move |_t| p);
    println!("AICE floor-off d=200 z=35: {:.12e}", r.phase(Vec3::ZERO, &s, 5.0));
}
