#[cfg(not(any(feature="sc", feature="bv", feature="fc")))] include!("../gen_serde.rs");
#[cfg(feature="sc")] include!("../gen_schemars.rs");
#[cfg(feature="bv")] include!("../gen_bevy.rs");
#[cfg(feature="fc")] include!("../gen_facet.rs");
