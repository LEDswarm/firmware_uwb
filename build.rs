// Set up espidf and link Arduino SDK along with DW3000 library into our build.

use std::path::PathBuf;
use serde::Deserialize;

fn main() {
    embuild::espidf::sysenv::output();
}
