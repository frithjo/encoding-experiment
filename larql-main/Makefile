.PHONY: build release test check clean fmt lint demos product-metadata-check product-role-check dependency-direction-check validation-disclosure-check line-limit-check structure-check target-size-check target-size-warn target-preflight scientific-alignment-snapshot scientific-alignment-summary scientific-alignment-trend scientific-alignment-watch bit-perfect-eraser bit-perfect-eraser-one

# Build
build:
	cargo build --workspace

release:
	cargo build --release -p larql-cli

# Test
test:
	cargo test --workspace

# Check (compile without building)
check:
	cargo check --workspace

# Code quality
fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace --tests -- -D warnings

product-metadata-check:
	python3 scripts/check_product_metadata.py

product-role-check:
	python3 scripts/check_role_consistency.py

dependency-direction-check:
	python3 scripts/check_dependency_direction.py

validation-disclosure-check:
	python3 scripts/check_validation_disclosure.py

line-limit-check:
	python3 scripts/check_source_line_limits.py

structure-check: product-metadata-check product-role-check dependency-direction-check validation-disclosure-check line-limit-check

target-size-check:
	python3 scripts/check_target_size.py --path target --max-gib 30

target-size-warn:
	python3 scripts/check_target_size.py --path target --max-gib 30 --warn-only

target-preflight: target-size-check

scientific-alignment-snapshot:
	python3 scripts/scientific_alignment_snapshot.py

scientific-alignment-summary:
	python3 scripts/scientific_alignment_snapshot.py --print-summary

scientific-alignment-trend:
	python3 scripts/scientific_alignment_trend.py --print-summary

scientific-alignment-watch: scientific-alignment-snapshot scientific-alignment-trend

# All quality checks
ci: fmt-check structure-check lint test

# Clean
clean:
	cargo clean

# Demos
demos:
	cargo run --release -p larql-models --example architecture_demo
	cargo run --release -p larql-core --example graph_demo
	cargo run --release -p larql-core --example edge_demo
	cargo run --release -p larql-core --example serialization_demo
	cargo run --release -p larql-core --example algorithm_demo

demos-inference:
	cargo run --release -p larql-inference --example inference_demo

# Benchmarks
bench: bench-core

bench-core:
	cargo run --release -p larql-core --example bench_graph

bench-inference:
	cargo run --release -p larql-inference --example bench_inference

bench-all: bench-core bench-inference

# Python extension (managed via uv)
python-setup:
	cd crates/larql-python && uv sync --no-install-project --group dev --extra ui

python-build: python-setup
	cd crates/larql-python && uv run --no-sync maturin develop --release

python-test: python-build
	cd crates/larql-python && uv run --no-sync pytest tests/ -v

python-check:
	cargo check -p larql-python

python-clean:
	rm -rf crates/larql-python/.venv crates/larql-python/uv.lock

# Extraction
extract-test:
	cargo run --release -p larql-cli -- weight-extract google/gemma-3-4b-it \
		--layer 26 -o output/test-L26.larql.json \
		--stats output/test-L26-stats.json

extract-full:
	cargo run --release -p larql-cli -- weight-extract google/gemma-3-4b-it \
		-o output/gemma-3-4b-knowledge.larql.json \
		--stats output/gemma-3-4b-stats.json

# Inference
predict:
	cargo run --release -p larql-cli -- predict google/gemma-3-4b-it \
		--prompt "The capital of France is" -k 10

BIT_PERFECT_MODELS ?= \
	/home/arty/Documents/projects/larql/data/gemma3-4b-it-vindex \
	/home/arty/Documents/projects/larql/data/qwen3-0.6b-base-all-release.vindex
BIT_PERFECT_OUTPUT_DIR ?= docs/scientific-alignment
BIT_PERFECT_TOP_K ?= 20

bit-perfect-eraser:
	mkdir -p $(BIT_PERFECT_OUTPUT_DIR)
	for model in $(BIT_PERFECT_MODELS); do \
		name=$$(basename "$$model" | tr "/." "__"); \
		out="$(BIT_PERFECT_OUTPUT_DIR)/bit-perfect-eraser-$$name.json"; \
		echo "Running bit-perfect eraser on $$model"; \
		LARQL_VINDEX__PATH="$$model" \
		LARQL_BIT_PERFECT_OUTPUT="$$out" \
		LARQL_BIT_PERFECT_TOP_K="$(BIT_PERFECT_TOP_K)" \
		cargo run --release -p larql-inference --example bit_perfect_eraser; \
	done

bit-perfect-eraser-one:
ifndef LARQL_VINDEX__PATH
	$(error LARQL_VINDEX__PATH must be set for bit-perfect-eraser-one)
endif
	mkdir -p $(BIT_PERFECT_OUTPUT_DIR)
	name=$$(basename "$(LARQL_VINDEX__PATH)" | tr "/." "__"); \
	out="$(BIT_PERFECT_OUTPUT_DIR)/bit-perfect-eraser-$$name.json"; \
	echo "Running bit-perfect eraser on $(LARQL_VINDEX__PATH)"; \
	LARQL_BIT_PERFECT_TOP_K="$(BIT_PERFECT_TOP_K)" \
	LARQL_BIT_PERFECT_OUTPUT="$$out" \
	cargo run --release -p larql-inference --example bit_perfect_eraser

# Carbonyl runtime verification
verify-carbonyl:
	@./apps/terminal-runtime/scripts/install-carbonyl-runtime.sh
