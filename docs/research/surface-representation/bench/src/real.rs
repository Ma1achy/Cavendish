//! A number type the kernels are generic over: `f64` (the reference), `f32` (what a GPU does) and
//! [`Ct`], an `f64` that counts every operation, so that FLOPs per element are measured by running
//! the kernel once rather than counted by hand.

use std::cell::Cell;
use std::ops::{Add, Div, Mul, Neg, Sub};

/// The operations a kernel may use.
pub trait Real:
    Copy + Add<Output = Self> + Sub<Output = Self> + Mul<Output = Self> + Div<Output = Self> + Neg<Output = Self> + PartialOrd
{
    fn c(x: f64) -> Self;
    fn f(self) -> f64;
    fn sqrt(self) -> Self;
    fn atan2(self, x: Self) -> Self;
    fn ln_1p(self) -> Self;
    fn abs(self) -> Self;
}

impl Real for f64 {
    #[inline(always)]
    fn c(x: f64) -> Self {
        x
    }
    #[inline(always)]
    fn f(self) -> f64 {
        self
    }
    #[inline(always)]
    fn sqrt(self) -> Self {
        f64::sqrt(self)
    }
    #[inline(always)]
    fn atan2(self, x: Self) -> Self {
        f64::atan2(self, x)
    }
    #[inline(always)]
    fn ln_1p(self) -> Self {
        f64::ln_1p(self)
    }
    #[inline(always)]
    fn abs(self) -> Self {
        f64::abs(self)
    }
}

impl Real for f32 {
    #[inline(always)]
    fn c(x: f64) -> Self {
        x as f32
    }
    #[inline(always)]
    fn f(self) -> f64 {
        f64::from(self)
    }
    #[inline(always)]
    fn sqrt(self) -> Self {
        f32::sqrt(self)
    }
    #[inline(always)]
    fn atan2(self, x: Self) -> Self {
        f32::atan2(self, x)
    }
    #[inline(always)]
    fn ln_1p(self) -> Self {
        f32::ln_1p(self)
    }
    #[inline(always)]
    fn abs(self) -> Self {
        f32::abs(self)
    }
}

/// Operation counts: add/sub, mul, div, sqrt, atan2, log1p.
pub const KINDS: [&str; 6] = ["add", "mul", "div", "sqrt", "atan2", "log1p"];

thread_local! {
    static COUNTS: [Cell<u64>; 6] = const { [const { Cell::new(0) }; 6] };
}

fn bump(k: usize) {
    COUNTS.with(|c| c[k].set(c[k].get() + 1));
}

/// Read and reset the counts.
pub fn take_counts() -> [u64; 6] {
    COUNTS.with(|c| {
        let out = [c[0].get(), c[1].get(), c[2].get(), c[3].get(), c[4].get(), c[5].get()];
        for x in c {
            x.set(0);
        }
        out
    })
}

/// An `f64` that counts its operations (negation and comparisons are free).
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub struct Ct(pub f64);

impl Add for Ct {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        bump(0);
        Ct(self.0 + o.0)
    }
}
impl Sub for Ct {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        bump(0);
        Ct(self.0 - o.0)
    }
}
impl Mul for Ct {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        bump(1);
        Ct(self.0 * o.0)
    }
}
impl Div for Ct {
    type Output = Self;
    fn div(self, o: Self) -> Self {
        bump(2);
        Ct(self.0 / o.0)
    }
}
impl Neg for Ct {
    type Output = Self;
    fn neg(self) -> Self {
        Ct(-self.0)
    }
}
impl Real for Ct {
    fn c(x: f64) -> Self {
        Ct(x)
    }
    fn f(self) -> f64 {
        self.0
    }
    fn sqrt(self) -> Self {
        bump(3);
        Ct(self.0.sqrt())
    }
    fn atan2(self, x: Self) -> Self {
        bump(4);
        Ct(self.0.atan2(x.0))
    }
    fn ln_1p(self) -> Self {
        bump(5);
        Ct(self.0.ln_1p())
    }
    fn abs(self) -> Self {
        Ct(self.0.abs())
    }
}
