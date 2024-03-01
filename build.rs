// Set up espidf and link Arduino SDK along with DW3000 library into our build.

use std::path::PathBuf;
use serde::Deserialize;

fn main() {
    embuild::espidf::sysenv::output();

    println!("cargo:rerun-if-changed={}", CONFIG_FILE);
    let config_string = std::fs::read_to_string(CONFIG_FILE)
        .unwrap_or_else(|e| panic!("Unable to read {} file: {}", CONFIG_FILE, e));
    let config: Config = serde_yaml::from_str(&config_string)
        .unwrap_or_else(|e| panic!("Unable to parse {} file: {}", CONFIG_FILE, e));

    println!("Arduino configuration: {:#?}", config);
}
const CONFIG_FILE: &str = "arduino.yml";

#[derive(Debug, Deserialize)]
struct Config {
    pub arduino_home: String,
    pub external_libraries_home: String,
    pub core_version: String,
    pub variant: String,
    pub avr_gcc_version: String,
    pub arduino_libraries: Vec<String>,
}

impl Config {
    fn arduino_package_path(&self) -> PathBuf {
        let expanded = envmnt::expand(&self.arduino_home, None);
        let arduino_home_path = PathBuf::from(expanded);
        arduino_home_path.join("packages").join("arduino")
    }

    fn core_path(&self) -> PathBuf {
        self.arduino_package_path()
            .join("hardware")
            .join("avr")
            .join(&self.core_version)
    }

    fn avr_gcc_home(&self) -> PathBuf {
        self.arduino_package_path()
            .join("tools")
            .join("avr-gcc")
            .join(&self.avr_gcc_version)
    }

    fn avg_gcc(&self) -> PathBuf {
        self.avr_gcc_home().join("bin").join("avr-gcc")
    }

    fn arduino_core_path(&self) -> PathBuf {
        self.core_path().join("cores").join("arduino")
    }

    fn arduino_include_dirs(&self) -> Vec<PathBuf> {
        let variant_path = self.core_path().join("variants").join(&self.variant);
        let avr_gcc_include_path = self.avr_gcc_home().join("avr").join("include");
        vec![self.arduino_core_path(), variant_path, avr_gcc_include_path]
    }

    fn arduino_libraries_path(&self) -> Vec<PathBuf> {
        let library_root = self.core_path().join("libraries");
        let mut result = vec![];
        for library in &self.arduino_libraries {
            result.push(library_root.join(library).join("src"))
        }
        result
    }

    fn external_libraries_path(&self) -> Vec<PathBuf> {
        let expanded = envmnt::expand(&self.external_libraries_home, None);
        let external_library_root = PathBuf::from(expanded);
        let mut result = vec![];
        for library in &self.external_libraries {
            result.push(external_library_root.join(library))
        }
        result
    }

    fn include_dirs(&self) -> Vec<PathBuf> {
        let mut result = self.arduino_include_dirs();
        result.extend(self.arduino_libraries_path());
        result.extend(self.external_libraries_path());
        result
    }
}
