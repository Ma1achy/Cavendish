"""Run a script at below-normal priority: python lowrun.py <script> <args...> (the study's helper)."""

import ctypes
import runpy
import sys

if sys.platform == "win32":
    k32 = ctypes.windll.kernel32  # type: ignore[attr-defined]
    k32.SetPriorityClass(k32.GetCurrentProcess(), 0x4000)
sys.argv = sys.argv[1:]
runpy.run_path(sys.argv[0], run_name = "__main__")
