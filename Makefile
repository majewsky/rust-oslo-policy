testloop: FORCE
	@# I like to have this run in a second terminal window while I work on the code.
	cargo watch -s 'clear && cargo doc && cargo test -q && cargo clippy'

.PHONY: FORCE
