// The end-to-end proof for TypeScript/Node: a real Node process calling
// generated bindings.
//
// Nothing here knows about Rust. Every assertion is about whether the binding
// behaves the way a JS developer would expect -- camelCase names, native
// types, thrown errors, no gratuitous promises.

import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const hello = require("./.jedem/hello.node");

let failures = 0;
const check = (label, got, want) => {
  const ok = JSON.stringify(got) === JSON.stringify(want);
  if (!ok) {
    console.log(`  FAIL ${label}: got ${JSON.stringify(got)}, want ${JSON.stringify(want)}`);
    failures++;
  } else {
    console.log(`  ok   ${label}: ${JSON.stringify(got)}`);
  }
};

console.log("names are camelCase, not snake_case");
check("greet", hello.greet("world"), "Hello, world!");
check("reverseBytes exists", typeof hello.reverseBytes, "function");
check("reverse_bytes does not", hello.reverse_bytes, undefined);
check("greetChecked exists", typeof hello.greetChecked, "function");

console.log("plain values cross natively");
check("add", hello.add(2, 40), 42);
check("add is a number", typeof hello.add(1, 1), "number");

console.log("Option<T> becomes null");
check("repeat(2)", hello.repeat("ab", 2), "abab");
check("repeat(0)", hello.repeat("ab", 0), null);

console.log("Vec<T> becomes an array");
check("split", hello.split("a,b,c", ","), ["a", "b", "c"]);
check("split is an array", Array.isArray(hello.split("a", ",")), true);

console.log("bytes are position-aware: Uint8Array in, Buffer out");
const reversed = hello.reverseBytes(new Uint8Array([1, 2, 3]));
check("reverseBytes value", Array.from(reversed), [3, 2, 1]);
check("return is a Buffer", Buffer.isBuffer(reversed), true);
// A Buffer is a Uint8Array, so the caller can treat it as either.
check("Buffer is also a Uint8Array", reversed instanceof Uint8Array, true);

console.log("a pinned export name is respected");
check("shout", hello.shout("hi"), "HI");
check("the Rust name is not exported", hello.shoutIt, undefined);

console.log("Result becomes a thrown Error, not a returned value");
check("greetChecked ok", hello.greetChecked("x"), "Hello, x!");
try {
  hello.greetChecked("");
  console.log("  FAIL greetChecked('') should have thrown");
  failures++;
} catch (e) {
  check("greetChecked throws", e.message, "name must not be empty");
}

console.log("a synchronous function stays synchronous");
const r = hello.greet("sync");
check("no promise", r instanceof Promise, false);
check("result is available immediately", r, "Hello, sync!");

console.log("wrong types are rejected by the binding");
try {
  hello.add("not", "numbers");
  console.log("  FAIL add('not','numbers') should have thrown");
  failures++;
} catch {
  console.log("  ok   add rejects non-numbers");
}

console.log("enums arrive as TypeScript string literals, not numbers");
check("classify -> string union", hello.classify(20), "Done");
check("partial", hello.classify(5), "Partial");
check("missing", hello.classify(0), "Missing");
check("pinned boundary name", hello.classify(-1), "NotApplicable");
check("enum as a parameter", hello.isSettled("Done"), true);
check("enum as a parameter, false", hello.isSettled("Partial"), false);
check("Option<enum> present", hello.maybe(true), "Done");
check("Option<enum> absent", hello.maybe(false), null);
try {
  hello.isSettled("NotAVariant");
  console.log("  FAIL an unknown variant should be rejected");
  failures++;
} catch {
  console.log("  ok   an unknown variant is rejected");
}

console.log("a Rust move-builder chains the same way here");
// `withTotal` takes `self` and returns `Self` in Rust. Nothing about it was
// annotated or reshaped -- the binding mutates in place and returns `this`, so
// the chain reads the way the Rust one does, and the way JS builders do.
const chained = new hello.Counter().withTotal(10);
failures += check("builder set the value", chained.total(), 10);
failures += check("and it is one object", chained.withTotal(20).total(), 20);
const built = new hello.Counter();
const same = built.withTotal(5);
failures += check("returns the same handle, not a copy", same === built, true);
built.add(1);
failures += check("so mutations land on both names", same.total(), 6);

console.log("handles: state that lives across calls");
const c = new hello.Counter();
check("starts empty", c.total(), 0);
c.add(10);
c.add(6);
check("state persisted across calls", c.total(), 16);
check("and was counted", c.steps(), 2);
check("halve", c.halve(), 8);

console.log("two handles are independent objects");
const a = new hello.Counter(), b = new hello.Counter();
a.add(1);
check("a moved", a.total(), 1);
check("b did not", b.total(), 0);

console.log("an alternate constructor is a factory");
const d = hello.Counter.startingAt(100);
check("started at 100", d.total(), 100);
try {
  hello.Counter.startingAt(-1);
  console.log("  FAIL negative should have thrown");
  failures++;
} catch (e) {
  check("negative throws", e.message, "-1 is negative");
}

if (failures) {
  console.log(`\n${failures} failure(s)`);
  process.exit(1);
}
console.log("\nall checks passed");
