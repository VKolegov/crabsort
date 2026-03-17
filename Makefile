all:
	cargo build && ./target/debug/crabsort ~/Music
dev:
	cargo build && ./target/debug/crabsort ~/dev
home:
	cargo build && ./target/debug/crabsort ~/
