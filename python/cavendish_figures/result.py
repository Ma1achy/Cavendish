"""
Errors as values: ``Result[T, E] = Ok[T] | Err[E]``, with Rust's method names and meanings.

    fallible call -> Ok(value)  -> .unwrap_or(default), .map(f), isinstance(result, Ok)
                  -> Err(error) -> .map_err(f), isinstance(result, Err), match

Anything in this package that can fail for a reason outside the program (a file that is not
there, a figure written by a later version) returns a ``Result``; nothing raises for such a
reason. ``unwrap``, ``expect`` and ``unwrap_err`` raise ``UnwrapError`` and are for tests and for
states proven impossible.

    result = load("fig.json")
    if isinstance(result, Err):
        print(result.error.code)
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Callable, Generic, NoReturn, TypeVar, final

T = TypeVar("T", covariant = True)
E = TypeVar("E", covariant = True)
U = TypeVar("U")
F = TypeVar("F")


class UnwrapError(Exception):
    """
    Raised by ``unwrap``, ``expect`` and ``unwrap_err`` on the wrong variant.

    Attributes:
        result (Ok[object] | Err[object]): The result that was unwrapped.
    """

    def __init__(self, result: Ok[object] | Err[object], message: str) -> None:
        super().__init__(message)
        self.result = result


@final
@dataclass(frozen = True)
class Ok(Generic[T]):
    """
    The success variant: it holds the value.

    Attributes:
        value (T): The value.
    """

    value: T

    def is_ok(self) -> bool:
        """True: this is ``Ok``."""
        return True

    def is_err(self) -> bool:
        """False: this is ``Ok``."""
        return False

    def ok(self) -> T | None:
        """The value."""
        return self.value

    def err(self) -> None:
        """``None``: there is no error."""
        return None

    def map(self, f: Callable[[T], U]) -> Ok[U]:
        """``Ok(f(value))``."""
        return Ok(f(self.value))

    def map_err(self, f: Callable[..., object]) -> Ok[T]:
        """Itself: there is no error to map."""
        return self

    def and_then(self, f: Callable[[T], Ok[U] | Err[F]]) -> Ok[U] | Err[F]:
        """``f(value)``: the next fallible step."""
        return f(self.value)

    def or_else(self, f: Callable[..., object]) -> Ok[T]:
        """Itself: there is nothing to recover from."""
        return self

    def unwrap_or(self, default: object) -> T:
        """The value."""
        return self.value

    def unwrap_or_else(self, f: Callable[..., object]) -> T:
        """The value."""
        return self.value

    def unwrap(self) -> T:
        """The value."""
        return self.value

    def expect(self, message: str) -> T:
        """The value."""
        return self.value

    def unwrap_err(self) -> NoReturn:
        """Raises ``UnwrapError``: there is no error."""
        raise UnwrapError(self, f"called unwrap_err on an Ok: {self.value!r}")


@final
@dataclass(frozen = True)
class Err(Generic[E]):
    """
    The failure variant: it holds the error.

    Attributes:
        error (E): The error.
    """

    error: E

    def is_ok(self) -> bool:
        """False: this is ``Err``."""
        return False

    def is_err(self) -> bool:
        """True: this is ``Err``."""
        return True

    def ok(self) -> None:
        """``None``: there is no value."""
        return None

    def err(self) -> E | None:
        """The error."""
        return self.error

    def map(self, f: Callable[..., object]) -> Err[E]:
        """Itself: there is no value to map."""
        return self

    def map_err(self, f: Callable[[E], F]) -> Err[F]:
        """``Err(f(error))``."""
        return Err(f(self.error))

    def and_then(self, f: Callable[..., object]) -> Err[E]:
        """Itself: the chain stops at the first error."""
        return self

    def or_else(self, f: Callable[[E], Ok[U] | Err[F]]) -> Ok[U] | Err[F]:
        """``f(error)``: a recovery."""
        return f(self.error)

    def unwrap_or(self, default: U) -> U:
        """The default."""
        return default

    def unwrap_or_else(self, f: Callable[[E], U]) -> U:
        """``f(error)``."""
        return f(self.error)

    def unwrap(self) -> NoReturn:
        """Raises ``UnwrapError``: there is no value."""
        raise UnwrapError(self, f"called unwrap on an Err: {self.error!r}")

    def expect(self, message: str) -> NoReturn:
        """Raises ``UnwrapError`` with ``message``."""
        raise UnwrapError(self, f"{message}: {self.error!r}")

    def unwrap_err(self) -> E:
        """The error."""
        return self.error


Result = Ok[T] | Err[E]
