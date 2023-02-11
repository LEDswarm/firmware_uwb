main:
	./scripts/build.sh
	espflash target/xtensa-esp32-espidf/release/firmware-rust-esp32-uwb
