main:
	./scripts/build.sh
	espflash target/riscv32imc-esp-espidf/release/firmware-rust-esp32-uwb
