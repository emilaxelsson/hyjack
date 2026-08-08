After making changes, run

  * `cargo build`
  * `cargo +nightly fmt`
  * `cargo clippy`
  * `cargo test`

and fix any warnings or errors.


## Testing

When writing property tests, prefer using existing methods to set up the state in the "arrange" phase rather than using elaborate value strategies.
