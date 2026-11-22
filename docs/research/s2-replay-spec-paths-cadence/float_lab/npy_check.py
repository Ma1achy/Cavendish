"""Reads what float_lab's `npy` mode wrote, with numpy, and compares bits and bytes. Run: python3 -I npy_check.py <dir>"""
import io, sys
import numpy as np

d = sys.argv[1]
bits = [int(line, 16) for line in open(f"{d}/bits.txt")]
for name, shape in [("rust_2d", (5, 7)), ("rust_1d", (35,)), ("rust_0d", ()), ("rust_empty", (0, 3))]:
    a = np.load(f"{d}/{name}.npy")
    n = int(np.prod(shape))
    buf = io.BytesIO()
    np.save(buf, a)
    same_bytes = buf.getvalue() == open(f"{d}/{name}.npy", "rb").read()
    print(name, "shape ok:", a.shape == shape, "dtype:", a.dtype, "bits equal:", a.view(np.uint64).ravel().tolist() == bits[:n], "byte-identical to np.save:", same_bytes)

# What numpy writes for the other cases the reader must refuse or understand.
for arr in [np.zeros((3, 4)), np.asfortranarray(np.arange(12.0).reshape(3, 4)), np.arange(3, dtype=">f8"), np.arange(3, dtype="<f4")]:
    buf = io.BytesIO()
    np.save(buf, arr)
    b = buf.getvalue()
    header_len = int.from_bytes(b[8:10], "little")
    print("version", tuple(b[6:8]), "header", header_len, "bytes; data starts at", 10 + header_len, "(multiple of 64:", (10 + header_len) % 64 == 0, ")", b[10:10 + header_len].decode().strip())
