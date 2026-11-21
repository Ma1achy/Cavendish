---
name: python-style
description: The owner's Python style and API conventions, taken from the Four project (python/four). Use whenever writing or reviewing Python in Cavendish — the `cavendish` package, its tests, examples and scripts.
---

# Python, the way the owner writes it (after `Ma1achy/Four`, `python/four`)

## Typing and layout
- Python ≥ 3.10, `from __future__ import annotations` at the top of every module.
- **pyright strict** over the package and the tests (`typeCheckingMode = "strict"` in `pyproject.toml`);
  ship `py.typed` and a `.pyi` stub for the Rust extension (`_cavendish.pyi`). No `Any` in public
  signatures except where a framework forces it (torch `Dataset[dict[str, Any]]`).
- The Rust extension is private (`_cavendish`); the public package wraps it in typed Python.
- Lines up to about 105 characters.

## Formatting quirks to copy exactly
- **Spaces around `=` in keyword arguments and defaults**: `split(source, by = "player", test = 0.1)`,
  `@dataclass(frozen = True)`, `TypeVar("T", covariant = True)`.
- **Aligned columns** in multi-line signatures and dataclass fields:
  ```python
  def batches(
      source:     Source,
      table:      str                  = "shots",
      fields:     Sequence[str] | None = None,
      window:     int                  = 64,
  ) -> Result[Batches, SourceError | DatasetError]:
  ```
  ```python
  @dataclass(frozen = True)
  class Options:
      table:      str
      window:     int
      batch_size: int
  ```
- **Section banners** splitting a module, centred title, fixed width:
  `# ======================================= UNWRAPPING =======================================`

## Docstrings
- Every module opens with a docstring that says what it is for, often with a small ASCII flow
  (`fallible call -> Ok(value) -> ok_value`) and a short usage example.
- Functions and classes: a one-line summary, a paragraph on behaviour and edge cases, then Google-style
  sections with types in parentheses and aligned descriptions:
  ```
  Args:
      source  (Source):     The scenes.
      window  (int):        Shots per window, ``T``.
  Returns:
      Result[Batches, DatasetError]: ...
  Attributes:
      result (Ok[object] | Err[object]): The result that was unwrapped.
  ```
- Shapes are written into the docstring: ``{field: [B, T, D, ...], "mask": [B, T, D], "meta": [...]}``.
- Examples start with a docstring showing the command lines that run them.

## Errors are values
- Anything that can fail for a reason outside the program returns the package's own
  `Result[T, E]` = `Ok[T] | Err[E]` (frozen, `@final`), with Rust's method names and meanings
  (`map`, `map_err`, `and_then`, `or_else`, `unwrap_or`, `unwrap_or_else`, `is_ok`, `ok`, `err`).
  `unwrap`, `expect`, `unwrap_err` raise `UnwrapError` and are for tests and proven-impossible states.
- Narrow with `isinstance` or `match`; never parse error text.
- Errors are **frozen dataclasses in families** under one base `Error` (e.g. `SourceError`,
  `DatasetError`), each variant carrying its evidence as fields and a stable `code`
  (`source.io`, `dataset.unknown_field`). A signature names the narrowest family it can return.
- The Rust side's errors are mirrored variant for variant; the extension raises one `RustError` with a
  structured payload that Python turns into the dataclass; an `error_catalog()` and a test keep the
  mirror and the Rust enum identical.
- Per-item failures inside a long run are **not** `Err`: they are typed entries in a `Ledger` and
  the run continues. `Err` is for a request that cannot start.
- Check every argument up front, before any work starts, and return the typed refusal
  (`UnknownField(name, table, nearest, others)` with the nearest valid names).

## Names and structure
- Short verbs for the public API: `fields`, `filter_source`, `split`, `windows`, `batches`,
  `manifest`, `write_manifest`, `Source.scenes`, `Source.files`.
- Module-level helpers are `_private`; **private methods in classes use double-underscore
  name mangling** (`__decode`), not a single underscore.
- Frozen dataclasses for options, records and errors; small classes; no inheritance beyond the error
  families.
- Parallelism lives in Rust with the GIL released; examples say so (`DataLoader(num_workers = 0)`).

## Tests
- pytest; a synthetic data module for tests that need no real data; tests for the error catalogue,
  for `Result` itself, and for every public function's refusals as well as its results.
