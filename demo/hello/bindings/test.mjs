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

if (failures) {
  console.log(`\n${failures} failure(s)`);
  process.exit(1);
}
console.log("\nall checks passed");
