"""Writes four copies of a synthetic scene spec (60 structs and 60 enums, 8 fields each) differing only in derives.
Run: python3 -I gen.py   then   sh time.sh"""
N = 60

def gen(derive: str, repr_u8: bool = False) -> str:
    out = ["use serde::{Serialize, Deserialize};", "pub type V3 = [f64; 3];"]
    for i in range(N):
        out.append(f"""
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize{derive})]
#[serde(deny_unknown_fields)]
{"#[repr(u8)]" if repr_u8 else ""}
pub enum K{i} {{ A, B {{ x: f64, y: V3 }}, C(f64) }}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize{derive})]
#[serde(deny_unknown_fields)]
pub struct S{i} {{
    /// a label
    pub label: String,
    /// metres
    pub a: f64,
    pub b: V3,
    pub c: Option<f64>,
    pub d: Vec<f64>,
    pub k: K{i},
    pub n: u32,
    pub flag: bool,
}}""")
    return "\n".join(out)

for name, derive, repr_u8 in [("serde", "", False), ("schemars", ", schemars::JsonSchema", False),
                              ("bevy", ", bevy_reflect::Reflect", False), ("facet", ", facet::Facet", True)]:
    open(f"gen_{name}.rs", "w").write(gen(derive, repr_u8))
