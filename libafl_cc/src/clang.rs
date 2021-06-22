//! LLVM compiler Wrapper from `LibAFL`

use std::{
    convert::Into,
    path::{Path, PathBuf},
    string::String,
    vec::Vec,
};

use crate::{CompilerWrapper, Error, LIB_EXT, LIB_PREFIX};

include!(concat!(env!("OUT_DIR"), "/clang_constants.rs"));

pub enum LLVMPasses {
    //CmpLogIns,
    CmpLogRtn,
}

impl LLVMPasses {
    pub fn path(&self) -> PathBuf {
        match self {
            LLVMPasses::CmpLogRtn => PathBuf::from(env!("OUT_DIR")).join("cmplog-routines-pass.so"),
        }
    }
}

/// Wrap Clang
#[allow(clippy::struct_excessive_bools)]
pub struct ClangWrapper {
    is_silent: bool,
    optimize: bool,
    wrapped_cc: String,
    wrapped_cxx: String,

    name: String,
    is_cpp: bool,
    linking: bool,
    x_set: bool,
    bit_mode: u32,

    base_args: Vec<String>,
    cc_args: Vec<String>,
    link_args: Vec<String>,
    passes: Vec<LLVMPasses>,
}

#[allow(clippy::match_same_arms)] // for the linking = false wip for "shared"
impl CompilerWrapper for ClangWrapper {
    fn from_args<S>(&mut self, args: &[S]) -> Result<&'_ mut Self, Error>
    where
        S: AsRef<str>,
    {
        let mut new_args: Vec<String> = vec![];
        if args.is_empty() {
            return Err(Error::InvalidArguments(
                "The number of arguments cannot be 0".to_string(),
            ));
        }

        if args.len() == 1 {
            return Err(Error::InvalidArguments(
                "LibAFL Compiler wrapper - no commands specified. Use me as compiler.".to_string(),
            ));
        }

        self.name = args[0].as_ref().to_string();
        // Detect C++ compiler looking at the wrapper name
        self.is_cpp = self.is_cpp || self.name.ends_with("++");

        // Sancov flag
        // new_args.push("-fsanitize-coverage=trace-pc-guard".into());

        let mut linking = true;
        // Detect stray -v calls from ./configure scripts.
        if args.len() > 1 && args[1].as_ref() == "-v" {
            linking = false;
        }

        for arg in &args[1..] {
            match arg.as_ref() {
                "-x" => self.x_set = true,
                "-m32" => self.bit_mode = 32,
                "-m64" => self.bit_mode = 64,
                "-c" | "-S" | "-E" => linking = false,
                "-shared" => linking = false, // TODO dynamic list?
                "-Wl,-z,defs" | "-Wl,--no-undefined" => continue,
                _ => (),
            };
            new_args.push(arg.as_ref().to_string());
        }
        self.linking = linking;

        if self.optimize {
            new_args.push("-g".into());
            new_args.push("-O3".into());
            new_args.push("-funroll-loops".into());
        }

        // Fuzzing define common among tools
        new_args.push("-DFUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION=1".into());

        // Libraries needed by libafl on Windows
        #[cfg(windows)]
        if linking {
            new_args.push("-lws2_32".into());
            new_args.push("-lBcrypt".into());
            new_args.push("-lAdvapi32".into());
        }

        self.base_args = new_args;
        Ok(self)
    }

    fn add_arg<S>(&mut self, arg: S) -> &'_ mut Self
    where
        S: AsRef<str>,
    {
        self.base_args.push(arg.as_ref().to_string());
        self
    }

    fn add_cc_arg<S>(&mut self, arg: S) -> &'_ mut Self
    where
        S: AsRef<str>,
    {
        self.cc_args.push(arg.as_ref().to_string());
        self
    }

    fn add_link_arg<S>(&mut self, arg: S) -> &'_ mut Self
    where
        S: AsRef<str>,
    {
        self.link_args.push(arg.as_ref().to_string());
        self
    }

    fn link_staticlib<S>(&mut self, dir: &Path, name: S) -> &'_ mut Self
    where
        S: AsRef<str>,
    {
        if cfg!(any(target_os = "macos", target_os = "ios")) {
            //self.add_link_arg("-force_load".into())?;
        } else {
            self.add_link_arg("-Wl,--whole-archive");
        }
        self.add_link_arg(
            dir.join(format!("{}{}.{}", LIB_PREFIX, name.as_ref(), LIB_EXT))
                .into_os_string()
                .into_string()
                .unwrap(),
        );
        if cfg!(any(target_os = "macos", target_os = "ios")) {
            self
        } else {
            self.add_link_arg("-Wl,-no-whole-archive")
        }
    }

    fn command(&mut self) -> Result<Vec<String>, Error> {
        let mut args = vec![];
        if self.is_cpp {
            args.push(self.wrapped_cxx.clone());
        } else {
            args.push(self.wrapped_cc.clone());
        }
        args.extend_from_slice(self.base_args.as_slice());
        for pass in &self.passes {
            args.push("-Xclang".into());
            args.push("-load".into());
            args.push("-Xclang".into());
            args.push(pass.path().into_os_string().into_string().unwrap());
        }
        if self.linking {
            if self.x_set {
                args.push("-x".into());
                args.push("none".into());
            }

            args.extend_from_slice(self.link_args.as_slice());
        } else {
            args.extend_from_slice(self.cc_args.as_slice());
        }

        Ok(args)
    }

    fn is_linking(&self) -> bool {
        self.linking
    }

    fn silence(&mut self) -> &'_ mut Self {
        self.is_silent = true;
        self
    }

    fn is_silent(&self) -> bool {
        self.is_silent
    }
}

impl ClangWrapper {
    /// Create a new Clang Wrapper
    #[must_use]
    pub fn new() -> Self {
        Self {
            optimize: true,
            wrapped_cc: CLANG_PATH.into(),
            wrapped_cxx: CLANGXX_PATH.into(),
            name: "".into(),
            is_cpp: false,
            linking: false,
            x_set: false,
            bit_mode: 0,
            base_args: vec![],
            cc_args: vec![],
            link_args: vec![],
            passes: vec![],
            is_silent: false,
        }
    }

    pub fn wrapped_cc(&mut self, cc: String) -> &'_ mut Self {
        self.wrapped_cc = cc;
        self
    }

    pub fn wrapped_cxx(&mut self, cxx: String) -> &'_ mut Self {
        self.wrapped_cxx = cxx;
        self
    }

    /// Disable optimizations
    pub fn dont_optimize(&mut self) -> &'_ mut Self {
        self.optimize = false;
        self
    }

    /// Set cpp mode
    pub fn is_cpp(&mut self, value: bool) -> &'_ mut Self {
        self.is_cpp = value;
        self
    }

    // Add LLVM pass
    pub fn add_pass(&mut self, pass: LLVMPasses) {
        self.passes.push(pass);
    }
}

#[cfg(test)]
mod tests {
    use crate::{ClangWrapper, CompilerWrapper};

    #[test]
    fn test_clang_version() {
        ClangWrapper::new("clang", "clang++")
            .from_args(&["my-clang".into(), "-v".into()])
            .unwrap()
            .run()
            .unwrap();
    }
}
