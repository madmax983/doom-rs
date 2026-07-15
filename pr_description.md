🧨 **The Trigger:**
Passing an extremely large or infinitely streaming `-file` argument via a response file (`@massive.rsp`) crashes the application natively with an Out-of-Memory (OOM) error because `tokenize_response_file` eagerly buffers the entire string into a `Vec<char>`.

📉 **The Stack Trace:**
```
memory allocation of X bytes failed
fatal runtime error: Rust panics must be aborted when out of memory.
```

🧪 **Reproduction:**
Run a fuzzer or write a script generating a 1GB string containing response file tokens and execute `tokenize_response_file(&s)`.

😈 **Comment:**
"You assumed the buffer would never be larger than RAM. You were wrong."

*Assumption:* Existing clippy warnings such as `clippy::precedence` and `unused_unsafe` on `Bam::init_trig_tables()` were left intact as they pertain to out-of-scope files for this patch and `doom_types::Bam::init_trig_tables()` requires `unsafe` in other crates but is technically marked safe here due to test scaffolding context.
