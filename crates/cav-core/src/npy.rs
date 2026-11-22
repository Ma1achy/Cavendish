//! NumPy `.npy` files of little-endian `f64` and of `bool`: the one reader and the one writer.
//!
//! Fixtures are read with it and a bundle's arrays are written with it. The writer produces, byte
//! for byte, what `numpy.save` writes for a C-ordered array of `float64` or `bool`: format version
//! 1.0, the header padded as numpy pads it, the values little-endian on every host. The reader
//! takes versions 1.0 to 3.0 of those two types in C order and refuses anything else by name: an
//! array read as the wrong type would be a wrong reference, quietly.
//!
//! Every bit of a value survives, a NaN's sign and payload included. Whether a NaN may be stored
//! at all is for the caller to decide (a bundle refuses one); this module only keeps bits.
//!
//! It must not be used for anything else `numpy.save` can write: other dtypes (`'<f4'`, integers,
//! strings, structured or pickled arrays), big-endian data, Fortran order, or `.npz` archives.

use std::io::{self, Write};
use std::path::Path;

const MAGIC: &[u8; 6] = b"\x93NUMPY";

/// The dtype of an `f64` array as the header names it: little-endian float64.
pub const F64: &str = "<f8";
/// The dtype of a `bool` array as the header names it: one byte each, 0 or 1.
pub const BOOL: &str = "|b1";

/// numpy's `ARRAY_ALIGN`: the data starts on a multiple of this many bytes.
const ALIGN: usize = 64;
/// numpy's `GROWTH_AXIS_MAX_DIGITS`: room it leaves for the first axis to grow in place.
const GROWTH_DIGITS: usize = 21;
/// Magic, version and the `u16` header length of a version 1.0 file.
const PREFIX: usize = 10;

/// The values of an array, in C order (the last index runs fastest).
#[derive(Clone, Debug, PartialEq)]
pub enum Data {
    /// `float64` values, every bit as stored. The file carries no unit: the caller knows it.
    F64(Vec<f64>),
    /// `bool` values, one for each byte of the file.
    Bool(Vec<bool>),
}

/// An array as read from an `.npy` file.
#[derive(Clone, Debug, PartialEq)]
pub struct Array {
    /// The length of each axis; empty for a scalar.
    pub shape: Vec<usize>,
    /// The values in C order: as many as the product of `shape`.
    pub data: Data,
}

impl Array {
    /// The dtype as the header names it: [`F64`] or [`BOOL`].
    #[must_use]
    pub const fn descr(&self) -> &'static str {
        match self.data {
            Data::F64(_) => F64,
            Data::Bool(_) => BOOL,
        }
    }

    /// `(shape, values)` of an `f64` array: the length of each axis, and the values in C order.
    ///
    /// # Errors
    /// When the array is of `bool`.
    pub fn into_f64(self) -> Result<(Vec<usize>, Vec<f64>), String> {
        match self.data {
            Data::F64(values) => Ok((self.shape, values)),
            Data::Bool(_) => Err("the array is of bool ('|b1'); float64 ('<f8') was expected".into()),
        }
    }

    /// `(shape, values)` of a `bool` array: the length of each axis, and the values in C order.
    ///
    /// # Errors
    /// When the array is of `f64`.
    pub fn into_bool(self) -> Result<(Vec<usize>, Vec<bool>), String> {
        match self.data {
            Data::Bool(values) => Ok((self.shape, values)),
            Data::F64(_) => Err("the array is of float64 ('<f8'); bool ('|b1') was expected".into()),
        }
    }
}

/// The number of values an array of this shape holds (1 for a scalar), or `None` when that is
/// more than a `usize` counts.
#[must_use]
pub fn count(shape: &[usize]) -> Option<usize> {
    shape.iter().try_fold(1usize, |all, &axis| all.checked_mul(axis))
}

/// Everything in a version 1.0 `.npy` file before the data: magic, version, header length and the
/// header, exactly as `numpy.save` writes them for a C-ordered array of dtype `descr` ([`F64`] or
/// [`BOOL`]). Its length is a multiple of 64 bytes.
///
/// It must not be used for a dtype this module does not write: `descr` is copied as given.
///
/// # Panics
/// When the header is longer than version 1.0 allows (65 535 bytes: a shape of thousands of axes).
#[must_use]
pub fn header(descr: &str, shape: &[usize]) -> Vec<u8> {
    // A Python tuple: `()`, `(5,)`, `(5, 7)`.
    let tuple = match shape {
        [] => "()".to_owned(),
        [only] => format!("({only},)"),
        axes => format!("({})", axes.iter().map(usize::to_string).collect::<Vec<_>>().join(", ")),
    };
    let mut text = format!("{{'descr': '{descr}', 'fortran_order': False, 'shape': {tuple}, }}");
    let spaces = |text: &mut String, n: usize| text.extend(std::iter::repeat_n(' ', n));
    // numpy leaves room for the first axis to grow to 21 digits without moving the data.
    if let Some(first) = shape.first() {
        spaces(&mut text, GROWTH_DIGITS - first.to_string().len());
    }
    // Then spaces and a line break up to the next multiple of 64 bytes. As in numpy, a header
    // that already ends on one is given 64 more.
    let unpadded = PREFIX + text.len() + 1;
    spaces(&mut text, ALIGN - unpadded % ALIGN);
    text.push('\n');
    let length = u16::try_from(text.len()).expect("a header within version 1.0's 65 535 bytes");
    let mut bytes = Vec::with_capacity(PREFIX + text.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&[1, 0]);
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(text.as_bytes());
    bytes
}

/// An `f64` array as `.npy` bytes, as `numpy.save` would write it. `values` are in C order.
///
/// For an array already in memory and small enough to copy; a large one is streamed with
/// [`write_f64`].
///
/// # Panics
/// When the number of values is not the product of the shape.
#[must_use]
pub fn encode_f64(shape: &[usize], values: &[f64]) -> Vec<u8> {
    assert_eq!(count(shape), Some(values.len()), "the shape {shape:?} does not hold {} values", values.len());
    let mut bytes = header(F64, shape);
    bytes.reserve_exact(8 * values.len());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

/// A `bool` array as `.npy` bytes, as `numpy.save` would write it. `values` are in C order.
///
/// # Panics
/// When the number of values is not the product of the shape.
#[must_use]
pub fn encode_bool(shape: &[usize], values: &[bool]) -> Vec<u8> {
    assert_eq!(count(shape), Some(values.len()), "the shape {shape:?} does not hold {} values", values.len());
    let mut bytes = header(BOOL, shape);
    bytes.extend(values.iter().map(|&value| u8::from(value)));
    bytes
}

/// Writes an `f64` array as `.npy` through `writer`: the header, then each value as it comes from
/// `values` (C order). No copy of the array is made, so hand it a buffered writer.
///
/// # Errors
/// When the writer fails, or when `values` does not yield exactly the product of the shape; the
/// error then gives both counts. By then the header and values have gone to the writer: what it
/// holds is not an array and must be discarded.
pub fn write_f64(
    writer: &mut impl Write,
    shape: &[usize],
    values: impl IntoIterator<Item = f64>,
) -> io::Result<()> {
    stream(writer, F64, shape, values.into_iter().map(f64::to_le_bytes))
}

/// Writes a `bool` array as `.npy` through `writer`, as [`write_f64`] does for `f64`.
///
/// # Errors
/// As [`write_f64`].
pub fn write_bool(
    writer: &mut impl Write,
    shape: &[usize],
    values: impl IntoIterator<Item = bool>,
) -> io::Result<()> {
    stream(writer, BOOL, shape, values.into_iter().map(|value| [u8::from(value)]))
}

fn stream<const N: usize>(
    writer: &mut impl Write,
    descr: &str,
    shape: &[usize],
    values: impl Iterator<Item = [u8; N]>,
) -> io::Result<()> {
    let refusal = |why: String| io::Error::new(io::ErrorKind::InvalidInput, why);
    let expected = count(shape)
        .ok_or_else(|| refusal(format!("the shape {shape:?} holds more values than can be counted")))?;
    writer.write_all(&header(descr, shape))?;
    let mut values = values.fuse();
    let mut given = 0usize;
    for bytes in values.by_ref().take(expected) {
        writer.write_all(&bytes)?;
        given += 1;
    }
    // Whatever is left over is counted, not written.
    given += values.count();
    if given != expected {
        return Err(refusal(format!(
            "the shape {shape:?} holds {expected} values and {given} were given; what was written is not an array"
        )));
    }
    Ok(())
}

/// The array in an `.npy` file: its type, its shape and its values in C order.
///
/// # Errors
/// When the bytes are not an `.npy` file of little-endian `f64` or of `bool` in C order, or the
/// data does not fill the shape exactly. The error names what was found.
pub fn parse(bytes: &[u8]) -> Result<Array, String> {
    if bytes.len() < PREFIX || &bytes[..6] != MAGIC {
        return Err("not an .npy file: the magic bytes are missing".into());
    }
    let (header_at, header_len) = match bytes[6] {
        1 => (10usize, usize::from(u16::from_le_bytes([bytes[8], bytes[9]]))),
        2 | 3 if bytes.len() >= 12 => {
            (12, u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize)
        }
        other => return Err(format!(".npy version {other} is not read here")),
    };
    let header = header_at
        .checked_add(header_len)
        .and_then(|data_at| bytes.get(header_at..data_at))
        .ok_or("the .npy header runs past the end of the file")?;
    let data = &bytes[header_at + header_len..];
    let header = std::str::from_utf8(header).map_err(|_| "the .npy header is not text")?;
    // The header is a Python dict literal: {'descr': '<f8', 'fortran_order': False, 'shape': (3,), }
    let value = |key: &str| -> Result<&str, String> {
        let at = header.find(&format!("'{key}'")).ok_or_else(|| format!("the .npy header has no `{key}`"))?;
        let rest = header[at + key.len() + 2..].trim_start().strip_prefix(':').unwrap_or("").trim_start();
        Ok(rest)
    };
    let descr = value("descr")?;
    let is = |names: &[&str]| names.iter().any(|name| descr.starts_with(&format!("'{name}'")));
    let (is_f64, is_bool) = (is(&[F64, "|f8", "=f8"]), is(&[BOOL]));
    if !(is_f64 || is_bool) {
        let found = descr.split(',').next().unwrap_or(descr);
        return Err(format!("the array is {found}, not little-endian float64 ('<f8') or bool ('|b1')"));
    }
    if !value("fortran_order")?.starts_with("False") {
        return Err("the array is in Fortran order; arrays are saved in C order".into());
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
    let width = if is_f64 { 8 } else { 1 };
    let needed = count(&shape)
        .and_then(|values| values.checked_mul(width))
        .ok_or_else(|| format!("the shape {shape:?} holds more values than can be counted"))?;
    if data.len() != needed {
        return Err(format!(
            "the shape {shape:?} needs {needed} bytes of data and the file has {}",
            data.len()
        ));
    }
    let data = if is_f64 {
        // The length was checked above, so there is no remainder to lose.
        let (eights, _) = data.as_chunks::<8>();
        Data::F64(eights.iter().map(|c| f64::from_le_bytes(*c)).collect())
    } else {
        // numpy writes 0 and 1; any other byte is not a bool, and is not guessed at.
        if let Some(at) = data.iter().position(|&byte| byte > 1) {
            return Err(format!("byte {at} of the bool data is {}, neither 0 nor 1", data[at]));
        }
        Data::Bool(data.iter().map(|&byte| byte == 1).collect())
    };
    Ok(Array { shape, data })
}

/// Reads a one-dimensional `f64` array from an `.npy` file.
///
/// # Errors
/// When the file cannot be read, is not an `f64` array, or has more or fewer dimensions than one.
pub fn read(path: &Path) -> Result<Vec<f64>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (shape, values) =
        parse(&bytes).and_then(Array::into_f64).map_err(|why| format!("{}: {why}", path.display()))?;
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
    std::fs::write(path, encode_f64(&[values.len()], values)).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The values a text format loses: -0.0, the smallest and the largest subnormal, `f64::MAX`,
    /// both infinities, a quiet NaN with a payload and a negative signalling NaN.
    const AWKWARD: [u64; 8] = [
        0x8000_0000_0000_0000,
        0x0000_0000_0000_0001,
        0x000f_ffff_ffff_ffff,
        0x7fef_ffff_ffff_ffff,
        0x7ff0_0000_0000_0000,
        0xfff0_0000_0000_0000,
        0x7ff8_dead_beef_0001,
        0xfff0_0000_0000_0001,
    ];

    fn bits(values: &[f64]) -> Vec<u64> {
        values.iter().map(|x| x.to_bits()).collect()
    }

    fn floats(bits: impl IntoIterator<Item = u64>) -> Vec<f64> {
        bits.into_iter().map(f64::from_bits).collect()
    }

    /// The `f64` array in a file, or why there is none.
    fn f64s(bytes: &[u8]) -> Result<(Vec<usize>, Vec<f64>), String> {
        parse(bytes).and_then(Array::into_f64)
    }

    /// The 35 values of `numpy_5x7.npy`, by the rule `scripts/npy_fixture.py` states: the awkward
    /// ones, then ordinary values of either sign between 1 and 2 in size.
    fn grid() -> Vec<f64> {
        let ordinary = |i: u64| (0x3ff0_0000_0000_0000 + i * 0x0003_1415_9265_3589) | ((i & 1) << 63);
        floats(AWKWARD.into_iter().chain((8..35).map(ordinary)))
    }

    /// Each file `scripts/npy_fixture.py` had numpy write, with the array it was told to hold.
    fn fixtures() -> Vec<(&'static str, &'static [u8], Array)> {
        let array = |shape: &[usize], data| Array { shape: shape.to_vec(), data };
        vec![
            (
                "numpy_5x7",
                include_bytes!("../tests/fixtures/numpy_5x7.npy"),
                array(&[5, 7], Data::F64(grid())),
            ),
            (
                "numpy_scalar",
                include_bytes!("../tests/fixtures/numpy_scalar.npy"),
                array(&[], Data::F64(vec![std::f64::consts::PI])),
            ),
            (
                "numpy_empty",
                include_bytes!("../tests/fixtures/numpy_empty.npy"),
                array(&[0, 3], Data::F64(vec![])),
            ),
            (
                "numpy_bool_2x3",
                include_bytes!("../tests/fixtures/numpy_bool_2x3.npy"),
                array(&[2, 3], Data::Bool(vec![true, false, true, false, false, true])),
            ),
            // Two shapes where numpy's padding is not "spaces to the next 64": fifteen axes, where
            // the room left for the first axis to grow tips the header into another block; and a
            // header that ends exactly on a block, which numpy follows with a whole block of spaces.
            (
                "numpy_15_axes",
                include_bytes!("../tests/fixtures/numpy_15_axes.npy"),
                array(&[1; 15], Data::F64(vec![-1.5])),
            ),
            (
                "numpy_full_block",
                include_bytes!("../tests/fixtures/numpy_full_block.npy"),
                array(
                    &[1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 100],
                    Data::F64((0..100).map(f64::from).collect()),
                ),
            ),
        ]
    }

    fn encode(array: &Array) -> Vec<u8> {
        match &array.data {
            Data::F64(values) => encode_f64(&array.shape, values),
            Data::Bool(values) => encode_bool(&array.shape, values),
        }
    }

    /// Whether two arrays are the same to the bit (`==` would call a NaN different from itself).
    fn identical(a: &Array, b: &Array) -> bool {
        a.shape == b.shape
            && match (&a.data, &b.data) {
                (Data::F64(a), Data::F64(b)) => bits(a) == bits(b),
                (Data::Bool(a), Data::Bool(b)) => a == b,
                _ => false,
            }
    }

    #[test]
    fn the_writer_reproduces_a_file_numpy_wrote() {
        for (name, file, array) in fixtures() {
            let ours = encode(&array);
            assert!(ours == file, "{name}: the writer's bytes are not numpy's");
            assert_eq!(
                file.len() % 64,
                array.data_len() % 64,
                "{name}: the data starts on a 64-byte boundary"
            );
            let read = parse(file).unwrap_or_else(|why| panic!("{name}: {why}"));
            assert!(identical(&read, &array), "{name}: read back as {read:?}");
            assert_eq!(read.descr(), array.descr());

            // Control, the header: one bit of the byte-order mark ('<' becomes '>', '|' becomes '~').
            // The comparison sees it, and the reader refuses the type it now names.
            let mark = file.windows(10).position(|w| w == b"'descr': '").expect("a descr") + 10;
            let mut other = file.to_vec();
            other[mark] ^= 2;
            assert!(ours != other, "{name}: a changed header byte went unseen");
            let why = parse(&other).unwrap_err();
            assert!(why.contains("not little-endian float64"), "{name}: {why}");
            // A changed byte of padding is seen by the comparison too (the reader does not mind it).
            let mut other = file.to_vec();
            let last_space = file.len() - array.data_len() - 2;
            assert_eq!(file[last_space], b' ', "{name}: the header ends in padding and a line break");
            other[last_space] ^= 1;
            assert!(ours != other && parse(&other).is_ok_and(|read| identical(&read, &array)), "{name}");

            // Control, the data: one bit of the last byte, or one byte more where there is no data.
            let mut other = file.to_vec();
            if array.data_len() == 0 {
                other.push(0);
                assert!(parse(&other).unwrap_err().contains("needs 0 bytes"), "{name}");
            } else {
                *other.last_mut().unwrap() ^= 2;
                match parse(&other) {
                    Ok(read) => {
                        assert!(!identical(&read, &array), "{name}: a changed value read as the same")
                    }
                    Err(why) => assert!(why.contains("neither 0 nor 1"), "{name}: {why}"),
                }
                assert_eq!(
                    parse(&other).is_err(),
                    array.descr() == BOOL,
                    "{name}: 3 is not a bool; any f64 is"
                );
            }
            assert!(ours != other, "{name}: a changed data byte went unseen");
        }
        // The fixtures are the cases they are said to be: a scalar, an empty array, and the two
        // paddings (one block of header for most, two for the last two).
        let starts: Vec<usize> =
            fixtures().iter().map(|(_, file, array)| file.len() - array.data_len()).collect();
        assert_eq!(starts, [128, 128, 128, 128, 192, 192]);
    }

    impl Array {
        fn data_len(&self) -> usize {
            match &self.data {
                Data::F64(values) => 8 * values.len(),
                Data::Bool(values) => values.len(),
            }
        }
    }

    #[test]
    fn headers_are_numpys() {
        let text = |descr, shape: &[usize]| String::from_utf8(header(descr, shape)[10..].to_vec()).unwrap();
        assert!(text(F64, &[]).starts_with("{'descr': '<f8', 'fortran_order': False, 'shape': (), } "));
        assert!(text(F64, &[5]).starts_with("{'descr': '<f8', 'fortran_order': False, 'shape': (5,), } "));
        assert!(
            text(BOOL, &[5, 7]).starts_with("{'descr': '|b1', 'fortran_order': False, 'shape': (5, 7), } ")
        );
        for shape in [&[][..], &[5], &[5, 7], &[0, 3], &[usize::MAX, 3], &[1; 40]] {
            let bytes = header(F64, shape);
            assert_eq!(&bytes[..8], b"\x93NUMPY\x01\x00");
            assert_eq!(bytes.len() % 64, 0, "{shape:?}");
            assert_eq!(usize::from(u16::from_le_bytes([bytes[8], bytes[9]])), bytes.len() - 10);
            assert_eq!(bytes.last(), Some(&b'\n'));
            assert_eq!(bytes.iter().filter(|&&b| b == b'\n').count(), 1, "one line break, at the end");
            // A header alone is a whole file only for an array with nothing in it; the reader
            // finds the shape that was written and says how much data it is owed.
            match count(shape) {
                Some(0) => assert_eq!(f64s(&bytes).unwrap(), (shape.to_vec(), vec![])),
                Some(n) => assert!(f64s(&bytes).unwrap_err().contains(&format!("needs {} bytes", 8 * n))),
                None => assert!(f64s(&bytes).unwrap_err().contains("more values than can be counted")),
            }
        }
        assert_eq!(count(&[]), Some(1));
        assert_eq!(count(&[5, 7]), Some(35));
        assert_eq!(count(&[0, usize::MAX]), Some(0));
        // Control: a shape too large to count is said to be, not wrapped round.
        assert_eq!(count(&[usize::MAX, 3]), None);
    }

    #[test]
    fn arrays_round_trip_to_the_bit() {
        let values = [0.0, -0.0, 1.0 / 3.0, 5e-324, f64::MAX, -1.46e-25, f64::INFINITY];
        let bytes = encode_f64(&[values.len()], &values);
        assert_eq!(bytes.len() % 8, 0);
        assert_eq!((bytes.len() - 8 * values.len()) % 64, 0, "the data starts on a 64-byte boundary");
        let (shape, back) = f64s(&bytes).unwrap();
        assert_eq!(shape, [values.len()]);
        assert_eq!(bits(&back), bits(&values));
        assert_eq!(f64s(&encode_f64(&[0], &[])).unwrap(), (vec![0], vec![]));
        // Control: one changed byte of data is one changed value.
        let mut nudged = bytes.clone();
        *nudged.last_mut().unwrap() ^= 1;
        assert_ne!(bits(&f64s(&nudged).unwrap().1), bits(&values));

        // In any number of dimensions, with every awkward value and its negative.
        let awkward = floats(AWKWARD.into_iter().chain(AWKWARD.map(|b| b ^ (1 << 63))).chain([0, 1 << 63]));
        assert_eq!(awkward.len(), 18);
        for shape in [&[18][..], &[2, 9], &[3, 1, 6], &[1, 2, 1, 3, 3], &[18, 1]] {
            let (read, back) = f64s(&encode_f64(shape, &awkward)).unwrap();
            assert_eq!(read, shape);
            assert_eq!(bits(&back), bits(&awkward), "{shape:?}");
        }
        assert_eq!(bits(&f64s(&encode_f64(&[], &awkward[6..7])).unwrap().1), [0x7ff8_dead_beef_0001]);
        assert_eq!(f64s(&encode_f64(&[4, 0, 2], &[])).unwrap(), (vec![4, 0, 2], vec![]));
        // Control: the comparison is of bits, which tells the NaNs apart and -0.0 from 0.0.
        assert_ne!(bits(&awkward[6..7]), bits(&[f64::NAN]));
        assert_ne!(bits(&[-0.0]), bits(&[0.0]));

        // Bools, the same way.
        let flags = [true, false, false, true, true, false];
        for shape in [&[6][..], &[2, 3], &[3, 2, 1]] {
            let read = parse(&encode_bool(shape, &flags)).unwrap();
            assert_eq!((read.descr(), &read.shape[..]), (BOOL, shape));
            assert_eq!(read.into_bool().unwrap().1, flags);
        }
        // Control: an array is of one type, and asking for the other is an error that says which.
        assert!(parse(&encode_bool(&[6], &flags)).unwrap().into_f64().unwrap_err().contains("is of bool"));
        assert!(parse(&bytes).unwrap().into_bool().unwrap_err().contains("is of float64"));
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
        assert_eq!(f64s(&file(&header, &data, 1)).unwrap(), (vec![3], data.to_vec()));
        assert_eq!(f64s(&file(&header, &data, 2)).unwrap(), (vec![3], data.to_vec()), "version 2 as well");
        assert_eq!(file(&header, &data, 1), encode_f64(&[3], &data), "and it is what the writer writes");
        // Two dimensions parse, in C order; `read` is the one that insists on one.
        let grid = header.replace("(3,)", "(3, 1)");
        assert_eq!(f64s(&file(&grid, &data, 1)).unwrap().0, [3, 1]);
        // Each thing that would make a wrong reference is refused, and says what it is.
        let refused: [(String, &[f64], &str); 9] = [
            (header.replace("<f8", "<f4"), &data, "the array is '<f4', not little-endian float64"),
            (header.replace("<f8", ">f8"), &data, "the array is '>f8', not little-endian float64"),
            (header.replace("<f8", "<i8"), &data, "the array is '<i8', not little-endian float64"),
            (header.replace("False", "True"), &data, "Fortran order"),
            (header.replace("(3,)", "(4,)"), &data, "needs 32 bytes"),
            (header.clone(), &data[..2], "needs 24 bytes of data and the file has 16"),
            (header.clone(), &[1.5, -2.0, 3.25, 4.0], "needs 24 bytes of data and the file has 32"),
            (header.replace("'shape'", "'size'"), &data, "no `shape`"),
            (header.replace("(3,)", "(18446744073709551615, 3)"), &data, "more values than can be counted"),
        ];
        for (header, data, expected) in refused {
            let why = parse(&file(&header, data, 1)).unwrap_err();
            assert!(why.contains(expected), "`{why}` should mention `{expected}`");
        }
        assert!(parse(b"PK\x03\x04 a zip file").unwrap_err().contains("magic"));
        assert!(parse(&file(&header, &data, 9)).unwrap_err().contains("version 9"));
        // A file cut short in its data, by a byte, or in its header.
        let whole = file(&header, &data, 1);
        assert!(parse(&whole[..whole.len() - 1]).unwrap_err().contains("the file has 23"));
        assert!(parse(&whole[..100]).unwrap_err().contains("runs past the end"));
        // Bools: a byte that is neither 0 nor 1 is not read as one, and a short file is short.
        let flags = encode_bool(&[3], &[true, false, true]);
        let mut odd = flags.clone();
        *odd.last_mut().unwrap() = 255;
        assert!(parse(&odd).unwrap_err().contains("byte 2 of the bool data is 255"));
        assert!(
            parse(&flags[..flags.len() - 1])
                .unwrap_err()
                .contains("needs 3 bytes of data and the file has 2")
        );
        // Control: the same bytes untouched are read.
        assert_eq!(parse(&flags).unwrap().into_bool().unwrap(), (vec![3], vec![true, false, true]));
    }

    #[test]
    fn a_streamed_array_is_the_encoded_one() {
        let values = grid();
        let mut streamed = Vec::new();
        write_f64(&mut streamed, &[5, 7], values.iter().copied()).unwrap();
        assert!(streamed == encode_f64(&[5, 7], &values));
        assert!(streamed == include_bytes!("../tests/fixtures/numpy_5x7.npy"), "and numpy's");
        let flags = [true, false, true, false, false, true];
        let mut streamed = Vec::new();
        write_bool(&mut streamed, &[2, 3], flags).unwrap();
        assert!(streamed == encode_bool(&[2, 3], &flags));
        let mut streamed = Vec::new();
        write_f64(&mut streamed, &[0, 3], []).unwrap();
        assert!(streamed == encode_f64(&[0, 3], &[]));

        // Control: one value too few or too many is an error that gives both counts, whichever
        // type; and so is a shape that cannot be counted.
        let mut sink = Vec::new();
        let few = write_f64(&mut sink, &[5, 7], values[..34].iter().copied()).unwrap_err();
        assert_eq!(few.kind(), io::ErrorKind::InvalidInput);
        assert!(few.to_string().contains("holds 35 values and 34 were given"), "{few}");
        let many = write_f64(&mut sink, &[5, 7], values.iter().copied().chain([1.0, 2.0])).unwrap_err();
        assert!(many.to_string().contains("holds 35 values and 37 were given"), "{many}");
        let flag = write_bool(&mut sink, &[2, 2], flags).unwrap_err();
        assert!(flag.to_string().contains("holds 4 values and 6 were given"), "{flag}");
        let scalar = write_f64(&mut sink, &[], []).unwrap_err();
        assert!(scalar.to_string().contains("holds 1 values and 0 were given"), "{scalar}");
        let vast = write_f64(&mut sink, &[usize::MAX, 2], []).unwrap_err();
        assert!(vast.to_string().contains("more values than can be counted"), "{vast}");

        // The writer's own failure is passed on, not swallowed.
        struct Full;
        impl Write for Full {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::StorageFull, "the disc is full"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        assert_eq!(write_f64(&mut Full, &[1], [1.0]).unwrap_err().kind(), io::ErrorKind::StorageFull);
    }

    #[test]
    #[should_panic(expected = "does not hold 2 values")]
    fn encoding_the_wrong_number_of_values_is_a_bug() {
        let _ = encode_f64(&[3], &[1.0, 2.0]);
    }

    #[test]
    fn files_are_written_and_read_back() {
        let dir = std::env::temp_dir().join(format!("cav-core-npy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("series.npy");
        write(&path, &[1.0, 2.5]).unwrap();
        assert_eq!(read(&path).unwrap(), [1.0, 2.5]);
        // Control: a file that is not there is an error that names it.
        let missing = read(&dir.join("absent.npy")).unwrap_err();
        assert!(missing.contains("absent.npy"), "{missing}");
        // `read` is for a series: a grid and an array of bools are refused, each by name.
        std::fs::write(&path, encode_f64(&[2, 1], &[1.0, 2.5])).unwrap();
        assert!(read(&path).unwrap_err().contains("has shape [2, 1]"));
        std::fs::write(&path, encode_bool(&[2], &[true, false])).unwrap();
        assert!(read(&path).unwrap_err().contains("is of bool"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// SplitMix64 (Steele, Lea and Flood 2014): the next of a fixed sequence of 64-bit patterns.
    fn splitmix64(state: &mut u64) -> u64 {
        *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    cav_validate::case! {
        test: validation_npy_round_trip_is_exact,
        name: "bundle.npy_round_trip",
        subsystem: "bundle",
        slice: "S2",
        reference: Exact,
        against: "10^6 random f64 bit patterns and the awkward values written as .npy and read back; values whose bits differ",
        unit: Count,
        tolerance: 0.0,
        measure: {
            let mut state = 0x00ca_7e9d_15b0_0001_u64;
            let values = floats(AWKWARD.into_iter().chain((0..1_000_000).map(|_| splitmix64(&mut state))));
            // Every kind of value is among them: the test is of all bit patterns, not of finite numbers.
            assert!(values.iter().any(|x| x.is_nan()) && values.iter().any(|x| x.is_subnormal()));
            // Streamed as a bundle's arrays are, in two dimensions.
            let shape = [values.len() / 8, 8];
            let mut file = Vec::with_capacity(128 + 8 * values.len());
            write_f64(&mut file, &shape, values.iter().copied()).expect("a Vec takes every byte");
            let (read, back) = f64s(&file).expect("what was written is read");
            assert_eq!(read, shape);
            assert_eq!(back.len(), values.len());
            let differing = values.iter().zip(&back).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
            // Control: the same count finds the damage that sixteen digits of text do. More than a
            // tenth of the first 50 000 must come back changed (criterion set before measuring).
            let through_text = |x: &f64| format!("{x:.15e}").parse::<f64>().expect("a number");
            let lost = values[..50_000].iter().filter(|x| through_text(x).to_bits() != x.to_bits()).count();
            assert!(lost > 5_000, "only {lost} of 50 000 values changed through text: the count sees too little");
            cav_validate::Measured::Error(differing as f64)
        },
    }
}
