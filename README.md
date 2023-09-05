# dstvec
[![GitHub actions](https://github.com/SOF3/dstvec/workflows/CI/badge.svg)](https://github.com/SOF3/dstvec/actions?query=workflow%3ACI)
[![crates.io](https://img.shields.io/crates/v/dstvec.svg)](https://crates.io/crates/dstvec)
[![crates.io](https://img.shields.io/crates/d/dstvec.svg)](https://crates.io/crates/dstvec)
[![docs.rs](https://docs.rs/dstvec/badge.svg)](https://docs.rs/dstvec)
[![GitHub](https://img.shields.io/github/last-commit/SOF3/dstvec)](https://github.com/SOF3/dstvec)
[![GitHub](https://img.shields.io/github/stars/SOF3/dstvec?style=social)](https://github.com/SOF3/dstvec)

Compact contiguous storage for dynamic dispatch types.

`DstVec<dyn Trait>` works like `Vec<Box<dyn Trait>>`, with more compact memory but potentially slower access.

This is just a proof of concept. Not recommended for production unless justified by benchmarks.
USE WITH CAUTION :warning:
