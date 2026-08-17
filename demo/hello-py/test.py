"""The end-to-end proof: a real Python process calling generated bindings.

Nothing here knows about Rust. It imports a module and calls functions, and
every assertion is about whether they behave the way a Python developer would
expect -- native types, native exceptions, native names.

Run via ./run.sh, which builds the extension first.
"""

import sys

import hello


def check(label, got, want):
    if got != want:
        print(f"  FAIL {label}: got {got!r}, want {want!r}")
        return 1
    print(f"  ok   {label}: {got!r}")
    return 0


failures = 0

print("plain values cross natively")
failures += check("greet", hello.greet("world"), "Hello, world!")
failures += check("add", hello.add(2, 40), 42)
failures += check("add is a real int", type(hello.add(1, 1)).__name__, "int")

print("Option<T> becomes None, not a sentinel")
failures += check("repeat(2)", hello.repeat("ab", 2), "abab")
failures += check("repeat(0)", hello.repeat("ab", 0), None)

print("Vec<T> becomes a list")
failures += check("split", hello.split("a,b,c", ","), ["a", "b", "c"])
failures += check("split type", type(hello.split("a", ",")).__name__, "list")

print("Vec<u8> becomes bytes, not a list of ints")
failures += check("reverse_bytes", hello.reverse_bytes(b"abc"), b"cba")
failures += check("bytes type", type(hello.reverse_bytes(b"a")).__name__, "bytes")

print("a pinned export name is respected")
failures += check("shout", hello.shout("hi"), "HI")
failures += check(
    "the Rust name is not exported", hasattr(hello, "shout_it"), False
)

print("Result becomes a raised exception, not a returned error")
failures += check("greet_checked ok", hello.greet_checked("x"), "Hello, x!")
try:
    hello.greet_checked("")
    print("  FAIL greet_checked('') should have raised")
    failures += 1
except ValueError as e:
    failures += check("greet_checked raises", str(e), "name must not be empty")

print("an infallible function has no error seam at all")
failures += check(
    "greet never raises", hello.greet(""), "Hello, !"
)

print("doc comments arrive as docstrings")
failures += check(
    "greet.__doc__",
    (hello.greet.__doc__ or "").strip().splitlines()[0],
    "Greet someone by name.",
)

print("wrong types are rejected by the binding, not by us")
try:
    hello.add("not", "ints")
    print("  FAIL add('not','ints') should have raised TypeError")
    failures += 1
except TypeError:
    print("  ok   add rejects non-integers with TypeError")

if failures:
    print(f"\n{failures} failure(s)")
    sys.exit(1)
print("\nall checks passed")
