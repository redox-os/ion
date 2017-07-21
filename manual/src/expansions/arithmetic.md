# Arithmetic Expansions

We've exported our arithmetic logic into a separate crate
[calculate](https://crates.io/crates/calculate). We use this library for both our `calc` builtin,
and for parsing arithmetic expansions. Use `calc` if you want a REPL for arithmetic, else use
arithmetic expansions (`$((a + b))`) if you want the result inlined. Variables may be passed into
arithmetic expansions without the **$** sigil, as it is automatically inferred that text references
string variables. Supported operators are as below:

- Add (`$((a + b))`)
- Subtract(`$((a - b))`)
- Divide(`$((a / b))`)
- Multiply(`$((a * b))`)
- Powers(`$((a ** b))`)
- Square(`$((a²))`)
- Cube(`$((a³))`)
- Modulus(`$((a % b))`)
- Bitwise XOR(`$((a ^ b))`)
- Bitwise AND(`$((a & b))`)
- Bitwise OR(`$((a | b)))`)
- Bitwise NOT(`$(a ~ b))`)
- Left Shift(`$((a << b))`)
- Right Shift(`$((a >> b))`)
- Parenthesis(`$((4 * (pi * r²)))`)
