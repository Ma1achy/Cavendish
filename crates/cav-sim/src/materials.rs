//! Materials: what matter is made of, as data (S19, `docs/slices/S19.md` §2).
//!
//! A [`MaterialSpec`] is a label, a density and a colour, with the elastic and thermal properties
//! where the material has them. The **library** is `scenes/materials.ron`, compiled in; a scene's
//! own `materials` list is looked in first ([`find`]), so a scene can add a material or change a
//! library one for itself. The world volume (S4) indexes its own list of the same kind; a voxel
//! body names its material, and its parts' materials, by label ([`crate::shape::ShapeSpec`]).
//!
//! Only the density is read by the forward model today; the elastic properties are for rigid
//! dynamics (S7) and the thermal ones for heat (S13). Units: kg/m³, Pa, 1, W/(m K), J/(kg K), 1/K.

use std::sync::OnceLock;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::spec::{SpecError, limits};

/// The library's file, as compiled in.
pub const MATERIALS_RON: &str = include_str!("../../../scenes/materials.ron");
/// Where that file is, relative to the repository's root.
pub const MATERIALS_PATH: &str = "scenes/materials.ron";
/// The most materials a scene may define of its own.
pub const MAX_SCENE_MATERIALS: usize = 255;

/// One material.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterialSpec {
    /// Its name, as a body names it and legends and the paper drawing show it. At most 200
    /// characters.
    pub label: String,
    /// Kilograms per cubic metre, 0 to 1e5 (0 for matter that is drawn but weighs nothing).
    #[schemars(range(min = 0.0, max = 1.0e5))]
    #[schemars(extend("x-unit" = "kg/m^3"))]
    pub density: f64,
    /// Its colour in the picture, sRGB, as `#rrggbb`.
    pub colour: String,
    /// Young's modulus, pascals, where it has one (above 0, at most 1e13).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "Pa"), title = "Young's modulus")]
    pub young: Option<f64>,
    /// Poisson's ratio, from -1 to 0.5, where it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "1"), title = "Poisson's ratio")]
    pub poisson: Option<f64>,
    /// Thermal conductivity, watts per metre kelvin (above 0, at most 1e4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "W/(m K)"))]
    pub conductivity: Option<f64>,
    /// Specific heat capacity, joules per kilogram kelvin (above 0, at most 1e5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "J/(kg K)"), title = "Specific heat")]
    pub heat_capacity: Option<f64>,
    /// Linear thermal expansion, per kelvin (from -1e-3 to 1e-3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "1/K"), title = "Thermal expansion")]
    pub expansion: Option<f64>,
    /// Where the numbers come from. At most 200 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl MaterialSpec {
    /// A material of a label, a density (kg/m³) and a colour (`#rrggbb`), its other properties
    /// unknown.
    #[must_use]
    pub fn plain(label: &str, density: f64, colour: &str) -> Self {
        Self {
            label: label.into(),
            density,
            colour: colour.into(),
            young: None,
            poisson: None,
            conductivity: None,
            heat_capacity: None,
            expansion: None,
            source: None,
        }
    }

    /// Checks every number and the colour, under `at` (`materials[2]`, `world.materials[0]`).
    ///
    /// # Errors
    /// The first field out of its range, by its path.
    pub fn validate(&self, at: &str) -> Result<(), SpecError> {
        if self.label.trim().is_empty() {
            return Err(SpecError(format!("{at}.label: must not be empty")));
        }
        crate::spec::short(&format!("{at}.label"), &self.label, limits::LABEL)?;
        crate::spec::within(&format!("{at}.density"), self.density, (0.0, limits::DENSITY))?;
        if crate::world::parse_colour(&self.colour).is_none() {
            return Err(SpecError(format!("{at}.colour: `{}` is not a colour `#rrggbb`", self.colour)));
        }
        let open = |name: &str, value: Option<f64>, most: f64| match value {
            Some(v) if !(v.is_finite() && v > 0.0 && v <= most) => {
                Err(SpecError(format!("{at}.{name}: {v} must be above 0 and at most {most:e}")))
            }
            _ => Ok(()),
        };
        open("young", self.young, 1.0e13)?;
        open("conductivity", self.conductivity, 1.0e4)?;
        open("heat_capacity", self.heat_capacity, 1.0e5)?;
        if let Some(nu) = self.poisson {
            crate::spec::within(&format!("{at}.poisson"), nu, (-1.0, 0.5))?;
        }
        if let Some(alpha) = self.expansion {
            crate::spec::within(&format!("{at}.expansion"), alpha, (-1.0e-3, 1.0e-3))?;
        }
        if let Some(source) = &self.source {
            crate::spec::short(&format!("{at}.source"), source, limits::LABEL)?;
        }
        Ok(())
    }

    /// Its colour as sRGB bytes (mid grey if the text is not a colour, which validation refuses).
    #[must_use]
    pub fn rgb(&self) -> [u8; 3] {
        crate::world::parse_colour(&self.colour).unwrap_or([128; 3])
    }
}

/// The library, in the file's order.
///
/// # Panics
/// When the built-in file does not read: `the_library_is_valid` fails first.
pub fn library() -> &'static [MaterialSpec] {
    static LIBRARY: OnceLock<Vec<MaterialSpec>> = OnceLock::new();
    LIBRARY
        .get_or_init(|| ron::from_str(MATERIALS_RON).expect("scenes/materials.ron reads (a test holds it)"))
}

/// The material called `label`: the scene's own (`scene`, a scene's `materials`) if it has one of
/// that label, else the library's.
#[must_use]
pub fn find<'a>(scene: &'a [MaterialSpec], label: &str) -> Option<&'a MaterialSpec> {
    scene.iter().find(|m| m.label == label).or_else(|| library().iter().find(|m| m.label == label))
}

/// Every material a scene can name: its own, then the library's it does not shadow.
#[must_use]
pub fn every(scene: &[MaterialSpec]) -> Vec<&MaterialSpec> {
    let mut out: Vec<&MaterialSpec> = scene.iter().collect();
    out.extend(library().iter().filter(|m| !scene.iter().any(|s| s.label == m.label)));
    out
}

/// Checks a scene's own materials: each valid, the labels unique, at most
/// [`MAX_SCENE_MATERIALS`].
///
/// # Errors
/// The first that fails, by its path under `materials`.
pub fn validate(scene: &[MaterialSpec]) -> Result<(), SpecError> {
    if scene.len() > MAX_SCENE_MATERIALS {
        return Err(SpecError(format!(
            "materials: {} materials is more than {MAX_SCENE_MATERIALS}",
            scene.len()
        )));
    }
    for (n, m) in scene.iter().enumerate() {
        m.validate(&format!("materials[{n}]"))?;
        if scene[..n].iter().any(|o| o.label == m.label) {
            return Err(SpecError(format!("materials[{n}].label: `{}` is used twice", m.label)));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_library_is_valid() {
        // S19 M1: every entry reads and is valid, the labels unique, each with a source. Control:
        // the file with a label written twice is refused, naming the second.
        let all = library();
        assert!(all.len() >= 12, "{} materials", all.len());
        assert_eq!(validate(all), Ok(()));
        assert!(all.iter().all(|m| m.density > 0.0 && m.source.is_some()), "each weighs and says whence");
        assert_eq!(find(&[], "lead").map(|m| m.density), Some(11340.0));
        let mut doubled = all.to_vec();
        doubled.push(all[0].clone());
        let why = validate(&doubled).unwrap_err().0;
        assert!(why.starts_with(&format!("materials[{}].label", all.len())), "{why}");
    }

    #[test]
    fn a_scene_s_material_comes_before_the_library_s() {
        // A scene that says lead is lighter (a hollow brick of it) has its own way; the library's
        // lead is untouched, and a label neither has is none. Control: without the scene's, the
        // library's.
        let own = [MaterialSpec { label: "lead".into(), density: 5000.0, ..library()[1].clone() }];
        assert_eq!(find(&own, "lead").map(|m| m.density), Some(5000.0));
        assert_eq!(find(&[], "lead").map(|m| m.density), Some(11340.0));
        assert!(find(&own, "unobtainium").is_none());
        let every = every(&own);
        assert_eq!(every.iter().filter(|m| m.label == "lead").count(), 1);
        assert_eq!(every.len(), library().len());
        let bad = MaterialSpec { colour: "red".into(), ..own[0].clone() };
        assert!(bad.validate("materials[0]").unwrap_err().0.starts_with("materials[0].colour"));
        let soft = MaterialSpec { poisson: Some(0.7), ..own[0].clone() };
        assert!(soft.validate("m").unwrap_err().0.starts_with("m.poisson"));
    }
}
