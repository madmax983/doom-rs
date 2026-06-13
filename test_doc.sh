#!/bin/bash
RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc --no-deps --document-private-items
