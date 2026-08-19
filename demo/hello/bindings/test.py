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

print("a Rust move-builder chains the same way here")
# `with_total` takes `self` and returns `Self` in Rust. Nothing about it was
# annotated or reshaped -- the binding mutates in place and hands the same
# object back, so the chain reads the way the Rust one does.
chained = hello.Counter().with_total(10)
failures += check("builder set the value", chained.total(), 10)
failures += check("and it is one object", chained.with_total(20).total(), 20)
built = hello.Counter()
same = built.with_total(5)
failures += check("returns the same handle, not a copy", same is built, True)
built.add(1)
failures += check("so mutations land on both names", same.total(), 6)

print("a width that is not the boundary width")
# `take_steps(usize) -> usize` crosses as i64 both ways, cast at the call site.
stepper = hello.Counter()
failures += check("usize in, usize out", stepper.take_steps(3), 3)

print("handles: state that lives across calls")
c = hello.Counter()
check("starts empty", c.total(), 0)
c.add(10)
c.add(6)
check("state persisted across calls", c.total(), 16)
check("and was counted", c.steps(), 2)
check("halve", c.halve(), 8)

print("two handles are independent objects")
a, b = hello.Counter(), hello.Counter()
a.add(1)
check("a moved", a.total(), 1)
check("b did not", b.total(), 0)

print("an alternate constructor is a static factory")
d = hello.Counter.starting_at(100)
check("started at 100", d.total(), 100)
try:
    hello.Counter.starting_at(-1)
    print("  FAIL negative should raise"); failures += 1
except ValueError as e:
    check("negative raises", str(e), "-1 is negative")

print("a failing method leaves the handle usable")
odd = hello.Counter.starting_at(7)
try:
    odd.halve()
    print("  FAIL odd should raise"); failures += 1
except ValueError:
    check("still usable after a failure", odd.total(), 7)


print("Box<dyn Error>: two failure types, one signature")
failures += check("halve_parsed ok", hello.halve_parsed("10"), 5)
try:
    hello.halve_parsed("9")
    print("  FAIL odd should raise"); failures += 1
except ValueError as e:
    failures += check("odd raises", str(e), "9 is odd")
try:
    hello.halve_parsed("banana")
    print("  FAIL unparseable should raise"); failures += 1
except ValueError as e:
    failures += check("parse error raises", "invalid digit" in str(e), True)

print("enums arrive as a real Python class, not a magic string")
failures += check("classify -> enum", hello.classify(20), hello.Ripeness.Done)
failures += check("is a Ripeness", type(hello.classify(5)).__name__, "Ripeness")
failures += check("partial", hello.classify(5), hello.Ripeness.Partial)
failures += check("missing", hello.classify(0), hello.Ripeness.Missing)
failures += check("pinned boundary name", hello.classify(-1), hello.Ripeness.not_applicable)
failures += check("enum as a parameter", hello.is_settled(hello.Ripeness.Done), True)
failures += check("enum as a parameter, false", hello.is_settled(hello.Ripeness.Partial), False)
failures += check("Option<enum> present", hello.maybe(True), hello.Ripeness.Done)
failures += check("Option<enum> absent", hello.maybe(False), None)
try:
    hello.is_settled("Done")
    print("  FAIL a bare string should not be accepted"); failures += 1
except TypeError:
    print("  ok   a bare string is rejected -- the whole point")

if failures:
    print(f"\n{failures} failure(s)"); sys.exit(1)
