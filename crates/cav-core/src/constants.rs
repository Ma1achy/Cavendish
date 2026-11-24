//! Physical constants, each from a cited source, and the constants the references use instead.
//!
//! The engine's defaults are the CODATA 2022 values and the NIST mass of ⁸⁷Sr (George's answer of
//! 21 Nov 2026: the atom is ⁸⁷Sr). v1 and the reference scripts use rounded values; they are kept
//! here by name so that the oracle ties can set an instrument to exactly what the reference used.

/// The Newtonian constant of gravitation, m³ kg⁻¹ s⁻². CODATA 2022: 6.674 30(15) × 10⁻¹¹
/// (<https://physics.nist.gov/cgi-bin/cuu/Value?bg>).
pub const BIG_G: f64 = 6.674_30e-11;

/// The reduced Planck constant, J s: h / 2π with h = 6.626 070 15 × 10⁻³⁴ J s exactly (SI 2019).
pub const HBAR: f64 = 6.626_070_15e-34 / std::f64::consts::TAU;

/// The atomic mass constant, kg. CODATA 2022: 1.660 539 068 92(52) × 10⁻²⁷
/// (<https://physics.nist.gov/cgi-bin/cuu/Value?ukg>, read 21 Nov 2026).
pub const ATOMIC_MASS_UNIT: f64 = 1.660_539_068_92e-27;

/// The relative atomic mass of ⁸⁷Sr: 86.908 877 5(12), NIST Atomic Weights and Isotopic Compositions
/// (<https://physics.nist.gov/cgi-bin/Compositions/stand_alone.pl?ele=Sr>, read 21 Nov 2026).
pub const SR87_RELATIVE_MASS: f64 = 86.908_877_5;

/// The mass of one ⁸⁷Sr atom, kg: 1.443 155 87 × 10⁻²⁵.
pub const SR87_MASS: f64 = SR87_RELATIVE_MASS * ATOMIC_MASS_UNIT;

/// The clock transition's wavelength, metres (698 nm, ¹S₀–³P₀ of strontium).
pub const CLOCK_WAVELENGTH: f64 = 698e-9;

/// The kick number n: how many photon momenta the large-momentum-transfer sequence gives an
/// arm. AION-10's, and the default of a scene's `array.instrument.kicks`; AICE would support
/// more (George, 23 Nov 2026).
pub const LMT_KICKS: f64 = 1000.0;

/// Local gravity, m s⁻². The references use 9.81, and so does the site (George, 23 Nov 2026).
pub const LOCAL_GRAVITY: f64 = 9.81;

/// What v1 (tag `v1-final`) and the reference scripts use where they differ from the above.
pub mod reference {
    /// The atom's mass in v1 and in the scripts, kg. It is the mass of ⁸⁸Sr.
    pub const ATOM_MASS: f64 = 1.46e-25;
    /// ħ in v1 and in the scripts, J s.
    pub const HBAR: f64 = 1.055e-34;
    /// G in the scripts (v1 uses the CODATA value, [`super::BIG_G`]).
    pub const SCRIPT_BIG_G: f64 = 6.67e-11;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_atom_is_strontium_87_and_not_88() {
        // George: ⁸⁷Sr, about 1.443e-25 kg; v1 and the scripts carry 1.46e-25, which is ⁸⁸Sr.
        assert!((SR87_MASS - 1.443_155_87e-25).abs() < 1e-33, "{SR87_MASS:e}");
        assert!((SR87_MASS / 1.443e-25 - 1.0).abs() < 2e-4);
        // Control: the old value is 1.2% away, far outside that.
        assert!((reference::ATOM_MASS / SR87_MASS - 1.0).abs() > 0.011);
        assert!((HBAR - 1.054_571_817e-34).abs() < 1e-43);
        assert!((reference::HBAR / HBAR - 1.0).abs() > 4e-4, "the references' ħ is rounded");
    }
}
