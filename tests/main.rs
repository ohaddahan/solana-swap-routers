#![allow(
    clippy::unwrap_used,
    reason = "test code — panicking on failure is expected"
)]
#![allow(
    clippy::expect_used,
    reason = "test code — panicking on failure is expected"
)]
#![allow(clippy::panic, reason = "test code — panicking on failure is expected")]
#![allow(clippy::map_unwrap_or, reason = "readability in test setup helpers")]
#![allow(clippy::print_stdout, reason = "tests print tx signatures to stdout")]

pub mod common;

mod dflow;
mod jupiter;
mod titan;
