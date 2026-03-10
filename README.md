# Purpose
Tool to identify regions of identical code in multiple files. Intended to be used to support refactoring to reduce redundancy.

Drops use of `O(n * n)` patience diff in favor of rolling hash windows composed with buzhash.

Supersedes previous repo `samesame-patience` (also known as `samesame`).
