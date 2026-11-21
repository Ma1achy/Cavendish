//! Reading and writing fixtures: NumPy `.npy` files of little-endian `f64`.
//!
//! Enough of the format (versions 1.0 and 2.0, C order, `<f8`) to read what `numpy.save` writes
//! for a float64 array and to write a one-dimensional one. Anything else is refused by name:
//! a fixture read as the wrong type would be a wrong reference, quietly.

use std::path::Path;

const MAGIC: &[u8; 6] = b"\x93NUMPY";

/// The array in an `.npy` file: its shape and its values in C order.
///
/// # Errors
/// When the bytes are not an `.npy` file of little-endian `f64` in C order, or the data does not
/// fill the shape exactly.
pub fn parse(bytes: &[u8]) -> Result<(Vec<usize>, Vec<f64>), String> {
    if bytes.len() < 10 || &bytes[..6] != MAGIC {
        return Err("not an .npy file: the magic bytes are missing".into());
    }
    let (header_at, header_len) = match bytes[6] {
        1 => (10, usize::from(u16::from_le_bytes([bytes[8], bytes[9]]))),
        2 | 3 if bytes.len() >= 12 => {
            (12, u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize)
        }
        other => return Err(format!(".npy version {other} is not read here")),
    };
    let data_at = header_at + header_len;
    let header = bytes.get(header_at..data_at).ok_or("the .npy header runs past the end of the file")?;
    let header = std::str::from_utf8(header).map_err(|_| "the .npy header is not text")?;
    // The header is a Python dict literal: {'descr': '<f8', 'fortran_order': False, 'shape': (3,), }
    let value = |key: &str| -> Result<&str, String> {
        let at = header.find(&format!("'{key}'")).ok_or_else(|| format!("the .npy header has no `{key}`"))?;
        let rest = header[at + key.len() + 2..].trim_start().strip_prefix(':').unwrap_or("").trim_start();
        Ok(rest)
    };
    let descr = value("descr")?;
    if !(descr.starts_with("'<f8'") || descr.starts_with("'|f8'") || descr.starts_with("'=f8'")) {
        let found = descr.split(',').next().unwrap_or(descr);
        return Err(format!("the array is {found}, not little-endian float64 ('<f8')"));
    }
    if !value("fortran_order")?.starts_with("False") {
        return Err("the array is in Fortran order; fixtures are saved in C order".into());
    }
    let shape = value("shape")?;
    let inner = shape
        .strip_prefix('(')
        .and_then(|s| s.split(')').next())
        .ok_or("the .npy header's shape is not a tuple")?;
    let shape: Vec<usize> = inner
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().map_err(|_| format!("`{s}` in the .npy shape is not a size")))
        .collect::<Result<_, _>>()?;
    let count: usize = shape.iter().product();
    let data = &bytes[data_at..];
    if data.len() != count * 8 {
        return Err(format!(
            "the shape {shape:?} needs {} bytes of data and the file has {}",
            count * 8,
            data.len()
        ));
    }
    // The length was checked above, so there is no remainder to lose.
    let (eights, _) = data.as_chunks::<8>();
    let values = eights.iter().map(|c| f64::from_le_bytes(*c)).collect();
    Ok((shape, values))
}

/// A one-dimensional `f64` array as `.npy` bytes (version 1.0), as `numpy.save` would write it.
#[must_use]
pub fn encode(values: &[f64]) -> Vec<u8> {
    let mut header = format!("{{'descr': '<f8', 'fortran_order': False, 'shape': ({},), }}", values.len());
    // The data starts on a multiple of 64 bytes; the header ends with a line break.
    while (10 + header.len() + 1) % 64 != 0 {
        header.push(' ');
    }
    header.push('\n');
    let mut bytes = Vec::with_capacity(10 + header.len() + 8 * values.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&[1, 0]);
    bytes.extend_from_slice(&u16::try_from(header.len()).expect("a short header").to_le_bytes());
    bytes.extend_from_slice(header.as_bytes());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

/// Reads a one-dimensional `f64` array from an `.npy` file.
///
/// # Errors
/// When the file cannot be read, is not such an array, or has more than one dimension.
pub fn read(path: &Path) -> Result<Vec<f64>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (shape, values) = parse(&bytes).map_err(|why| format!("{}: {why}", path.display()))?;
    if shape.len() != 1 {
        return Err(format!(
            "{}: the array has shape {shape:?}; a one-dimensional one was expected",
            path.display()
        ));
    }
    Ok(values)
}

/// Writes a one-dimensional `f64` array to an `.npy` file.
///
/// # Errors
/// When the file cannot be written.
pub fn write(path: &Path, values: &[f64]) -> Result<(), String> {
    std::fs::write(path, encode(values)).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrays_round_trip_to_the_bit() {
        let values = [0.0, -0.0, 1.0 / 3.0, 5e-324, f64::MAX, -1.46e-25, f64::INFINITY];
        let bytes = encode(&values);
        assert_eq!(bytes.len() % 8, 0);
        assert_eq!((bytes.len() - 8 * values.len()) % 64, 0, "the data starts on a 64-byte boundary");
        let (shape, back) = parse(&bytes).unwrap();
        assert_eq!(shape, [values.len()]);
        let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(&back), bits(&values));
        assert_eq!(parse(&encode(&[])).unwrap(), (vec![0], vec![]));
        // Control: one changed byte of data is one changed value.
        let mut nudged = bytes.clone();
        *nudged.last_mut().unwrap() ^= 1;
        assert_ne!(bits(&parse(&nudged).unwrap().1), bits(&values));
    }

    #[test]
    fn a_file_numpy_wrote_is_read_and_others_are_refused_by_name() {
        // What numpy 2 writes for np.save(f, np.array([1.5, -2.0, 3.25])): version 1.0, a 118-byte header.
        let mut header = String::from("{'descr': '<f8', 'fortran_order': False, 'shape': (3,), }");
        while header.len() < 117 {
            header.push(' ');
        }
        header.push('\n');
        let file = |header: &str, data: &[f64], version: u8| {
            let mut bytes = MAGIC.to_vec();
            bytes.extend_from_slice(&[version, 0]);
            if version == 1 {
                bytes.extend_from_slice(&(header.len() as u16).to_le_bytes());
            } else {
                bytes.extend_from_slice(&(header.len() as u32).to_le_bytes());
            }
            bytes.extend_from_slice(header.as_bytes());
            data.iter().for_each(|v| bytes.extend_from_slice(&v.to_le_bytes()));
            bytes
        };
        let data = [1.5, -2.0, 3.25];
        assert_eq!(parse(&file(&header, &data, 1)).unwrap(), (vec![3], data.to_vec()));
        assert_eq!(parse(&file(&header, &data, 2)).unwrap(), (vec![3], data.to_vec()), "version 2 as well");
        // Two dimensions parse, in C order; `read` is the one that insists on one.
        let grid = header.replace("(3,)", "(3, 1)");
        assert_eq!(parse(&file(&grid, &data, 1)).unwrap().0, [3, 1]);
        // Each thing that would make a wrong reference is refused, and says what it is.
        let refused: [(String, &[f64], &str); 5] = [
            (header.replace("<f8", "<f4"), &data, "not little-endian float64"),
            (header.replace("<f8", ">f8"), &data, "not little-endian float64"),
            (header.replace("False", "True"), &data, "Fortran order"),
            (header.replace("(3,)", "(4,)"), &data, "needs 32 bytes"),
            (header.replace("'shape'", "'size'"), &data, "no `shape`"),
        ];
        for (header, data, expected) in refused {
            let why = parse(&file(&header, data, 1)).unwrap_err();
            assert!(why.contains(expected), "`{why}` should mention `{expected}`");
        }
        assert!(parse(b"PK\x03\x04 a zip file").unwrap_err().contains("magic"));
        assert!(parse(&file(&header, &data, 9)).unwrap_err().contains("version 9"));
    }

    #[test]
    fn files_are_written_and_read_back() {
        let dir = std::env::temp_dir().join(format!("cav-validate-npy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("series.npy");
        write(&path, &[1.0, 2.5]).unwrap();
        assert_eq!(read(&path).unwrap(), [1.0, 2.5]);
        // Control: a file that is not there is an error that names it.
        let missing = read(&dir.join("absent.npy")).unwrap_err();
        assert!(missing.contains("absent.npy"), "{missing}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
