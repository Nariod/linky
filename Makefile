# Makefile pour Linky C2

.PHONY: build test check clean run podman

build:
	cargo build --release

test:
	cargo test

check:
	cargo check
	cargo clippy

podman:
	podman build -t linky-c2 .

run:
	cargo run --release --bin linky

clean:
	cargo clean
	rm -rf target

# Test complet
test-full: build test check podman
	@echo "✅ Tous les tests réussis"

# Exécuter le script de test
test-script:
	./test_linky.sh