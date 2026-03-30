#!/bin/bash
find crates -name "*.rs" | xargs grep -E "HashMap<String, Vec<u8>>"
