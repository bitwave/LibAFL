use std::{env, fs::File, io::Write, path::Path, process::Command, str};

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);
    let src_dir = Path::new("src");

    let dest_path = Path::new(&out_dir).join("clang_constants.rs");
    let mut clang_constants_file = File::create(&dest_path).expect("Could not create file");

    let llvm_config = env::var("LLVM_CONFIG").unwrap_or("llvm-config".into());
    match Command::new(&llvm_config).args(&["--bindir"]).output() {
        Ok(output) => {
            let llvm_bindir = Path::new(
                str::from_utf8(&output.stdout)
                    .expect("Invalid llvm-config output")
                    .trim(),
            );

            write!(
                &mut clang_constants_file,
                "// These constants are autogenerated by build.rs

pub const CLANG_PATH: &'static str = {:?};
pub const CLANGXX_PATH: &'static str = {:?};
        ",
                llvm_bindir.join("clang"),
                llvm_bindir.join("clang++")
            )
            .expect("Could not write file");

            println!("cargo:rerun-if-changed=src/cmplog-routines-pass.cc");

            let output = Command::new(&llvm_config)
                .args(&["--cxxflags"])
                .output()
                .expect("Failed to execute llvm-config");
            let cxxflags = str::from_utf8(&output.stdout).expect("Invalid llvm-config output");

            let output = Command::new(&llvm_config)
                .args(&["--ldflags"])
                .output()
                .expect("Failed to execute llvm-config");
            let ldflags = str::from_utf8(&output.stdout).expect("Invalid llvm-config output");

            let cxxflags: Vec<&str> = cxxflags.trim().split_whitespace().collect();
            let ldflags: Vec<&str> = ldflags.trim().split_whitespace().collect();

            let _ = Command::new(llvm_bindir.join("clang++"))
                .args(&cxxflags)
                .arg(src_dir.join("cmplog-routines-pass.cc"))
                .args(&ldflags)
                .args(&["-fPIC", "-shared", "-o"])
                .arg(out_dir.join("cmplog-routines-pass.so"))
                .status()
                .expect("Failed to compile cmplog-routines-pass.cc");
        }
        Err(_) => {
            write!(
                &mut clang_constants_file,
                "// These constants are autogenerated by build.rs

pub const CLANG_PATH: &'static str = \"clang\";
pub const CLANGXX_PATH: &'static str = \"clang++\";
        "
            )
            .expect("Could not write file");

            println!("cargo:warning=Failed to locate the LLVM path using {}, we will not build LLVM passes", llvm_config);
        }
    };

    println!("cargo:rerun-if-changed=build.rs");
}
