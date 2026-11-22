"""Writes strs.txt: decimal strings and the bits CPython's correctly rounded float() gives them.
Run: python3 -I std_parse_check.py && rustc -O std_parse_check.rs -o chk && ./chk"""
import random, struct
random.seed(5)
with open("strs.txt", "w") as f:
    for _ in range(1_000_000):
        x = random.random() * 10.0 ** random.randint(-30, 30)
        s = "%.16e" % x if random.random() < 0.5 else repr(x)
        f.write(f"{s} {struct.unpack('<Q', struct.pack('<d', float(s)))[0]:016x}\n")
