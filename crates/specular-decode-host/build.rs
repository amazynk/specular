//! Windows: the icon, version information and manifest of specular-decode-host.exe
//! (packaging/windows/resource.rs).
//!
//! Unix: the OpenCASCADE bridge (`occt/specular_occt.cpp` → `libspecular-occt.so` /
//! `.dylib`), when OCCT's headers and libraries are found: `SPECULAR_OCCT_DIR` (an install
//! prefix: `include/opencascade`, `lib`), else the system's usual places. The bridge links
//! OCCT dynamically (LGPL) and is put next to the host binary; without OCCT nothing is built
//! and the host says STEP and IGES need it. `SPECULAR_NO_OCCT_BRIDGE=1` skips it.
//!
//! Unix: the libCZI bridge (`czi/specular_czi.cpp` → `libspecular-czi.so` / `.dylib`), when
//! libCZI's headers and library are found: `SPECULAR_LIBCZI_DIR` (an install prefix:
//! `include/libCZI`, `lib`), else the system's usual places. The bridge links libCZI
//! dynamically (LGPL) and is put next to the host binary; without libCZI nothing is built and
//! the host says Zeiss CZI files need it. `SPECULAR_NO_CZI_BRIDGE=1` skips it.

include!("../../packaging/windows/resource.rs");

fn main() {
    windows_resource("Specular decode host", "specular-decode-host.exe", false);
    occt::bridge();
    czi::bridge();
}

mod occt {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    /// OCCT's toolkits the bridge uses, in link order (each present one is linked; OCCT
    /// before 7.8 splits the STEP and IGES readers into more toolkits).
    const TOOLKITS: &[&str] = &[
        "TKDESTEP",
        "TKDEIGES",
        "TKXDESTEP",
        "TKSTEP",
        "TKSTEPAttr",
        "TKSTEPBase",
        "TKSTEP209",
        "TKXDEIGES",
        "TKIGES",
        "TKDE",
        "TKXCAF",
        "TKVCAF",
        "TKV3d",
        "TKService",
        "TKCAF",
        "TKLCAF",
        "TKCDF",
        "TKXSBase",
        "TKMesh",
        "TKShHealing",
        "TKTopAlgo",
        "TKGeomAlgo",
        "TKBRep",
        "TKGeomBase",
        "TKG3d",
        "TKG2d",
        "TKMath",
        "TKernel",
    ];

    struct Install {
        include: PathBuf,
        lib: PathBuf,
        system: bool,
    }

    fn find(target_os: &str) -> Option<Install> {
        let lib_of = |prefix: &Path| {
            [
                "lib",
                "lib64",
                "lib/x86_64-linux-gnu",
                "lib/aarch64-linux-gnu",
            ]
            .iter()
            .map(|l| prefix.join(l))
            .find(|d| has_lib(d, "TKernel", target_os))
        };
        if let Some(p) = std::env::var_os("SPECULAR_OCCT_DIR") {
            let prefix = PathBuf::from(p);
            let include = prefix.join("include/opencascade");
            return Some(Install {
                lib: lib_of(&prefix)?,
                include: include.is_dir().then_some(include)?,
                system: false,
            });
        }
        let prefixes: &[&str] = if target_os == "macos" {
            &["/opt/homebrew", "/usr/local"]
        } else {
            &["/usr", "/usr/local"]
        };
        prefixes.iter().find_map(|p| {
            let prefix = Path::new(p);
            let include = prefix.join("include/opencascade");
            Some(Install {
                lib: lib_of(prefix)?,
                include: include
                    .join("Standard_Version.hxx")
                    .is_file()
                    .then_some(include)?,
                system: matches!(*p, "/usr"),
            })
        })
    }

    fn has_lib(dir: &Path, name: &str, target_os: &str) -> bool {
        let file = if target_os == "macos" {
            format!("lib{name}.dylib")
        } else {
            format!("lib{name}.so")
        };
        dir.join(file).exists()
    }

    /// "7.9" from Standard_Version.hxx.
    fn version(include: &Path) -> Option<String> {
        let text = std::fs::read_to_string(include.join("Standard_Version.hxx")).ok()?;
        let num = |key: &str| {
            text.lines().find_map(|l| {
                let rest = l.trim().strip_prefix("#define")?.trim();
                let v = rest.strip_prefix(key)?;
                v.trim().parse::<u32>().ok()
            })
        };
        Some(format!(
            "{}.{}",
            num("OCC_VERSION_MAJOR")?,
            num("OCC_VERSION_MINOR")?
        ))
    }

    pub fn bridge() {
        println!("cargo:rerun-if-changed=occt/specular_occt.cpp");
        println!("cargo:rerun-if-env-changed=SPECULAR_OCCT_DIR");
        println!("cargo:rerun-if-env-changed=SPECULAR_NO_OCCT_BRIDGE");
        println!("cargo:rerun-if-env-changed=CXX");
        let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
        let (host, target) = (
            std::env::var("HOST").unwrap_or_default(),
            std::env::var("TARGET").unwrap_or_default(),
        );
        // Unix only, and not when cross-compiling (the bridge links the build machine's OCCT).
        if std::env::var_os("SPECULAR_NO_OCCT_BRIDGE").is_some_and(|v| v == "1")
            || !matches!(target_os.as_str(), "linux" | "macos")
            || host != target
        {
            return;
        }
        let Some(occt) = find(&target_os) else {
            return;
        };
        let Some(ver) = version(&occt.include) else {
            println!(
                "cargo:warning=OpenCASCADE headers at {} have no version; bridge not built",
                occt.include.display()
            );
            return;
        };
        let name = if target_os == "macos" {
            "libspecular-occt.dylib"
        } else {
            "libspecular-occt.so"
        };
        let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default());
        let built = out_dir.join(name);
        let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".into());
        let mut cmd = Command::new(&cxx);
        cmd.args([
            "-std=c++17",
            "-O2",
            "-fPIC",
            "-shared",
            "-fvisibility=hidden",
            "-w",
        ])
        .arg("-I")
        .arg(&occt.include)
        .arg("occt/specular_occt.cpp")
        .arg("-o")
        .arg(&built)
        .arg("-L")
        .arg(&occt.lib);
        if target_os == "macos" {
            cmd.arg(format!("-Wl,-install_name,@rpath/{name}"));
        }
        if !occt.system {
            // A private OCCT (a prefix of its own): find it there at run time too, OCCT's
            // own dependencies on each other included (an old-style RPATH applies to them,
            // a RUNPATH would not).
            if target_os == "linux" {
                cmd.arg("-Wl,--disable-new-dtags");
            }
            cmd.arg(format!("-Wl,-rpath,{}", occt.lib.display()));
        }
        for t in TOOLKITS {
            if has_lib(&occt.lib, t, &target_os) {
                cmd.arg(format!("-l{t}"));
            }
        }
        match cmd.status() {
            Ok(s) if s.success() => {}
            other => {
                println!(
                    "cargo:warning=OpenCASCADE {ver} found at {}, but the bridge did not \
                     build ({other:?}); STEP and IGES will be refused",
                    occt.lib.display()
                );
                return;
            }
        }
        println!("cargo:rustc-env=SPECULAR_OCCT_BUILT_FOR={ver}");
        // Next to the host binary: OUT_DIR is target/<profile>/build/<pkg>/out.
        if let Some(profile) = out_dir.ancestors().nth(3) {
            let next_to_host = profile.join(name);
            if std::fs::copy(&built, &next_to_host).is_ok() {
                println!(
                    "cargo:rustc-env=SPECULAR_OCCT_BRIDGE_BUILT={}",
                    next_to_host.display()
                );
                return;
            }
        }
        println!(
            "cargo:rustc-env=SPECULAR_OCCT_BRIDGE_BUILT={}",
            built.display()
        );
    }
}

mod czi {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    struct Install {
        include: PathBuf,
        lib: PathBuf,
        system: bool,
    }

    fn lib_file(target_os: &str) -> &'static str {
        if target_os == "macos" {
            "liblibCZI.dylib"
        } else {
            "liblibCZI.so"
        }
    }

    fn find(target_os: &str) -> Option<Install> {
        let lib_of = |prefix: &Path| {
            [
                "lib",
                "lib64",
                "lib/x86_64-linux-gnu",
                "lib/aarch64-linux-gnu",
            ]
            .iter()
            .map(|l| prefix.join(l))
            .find(|d| d.join(lib_file(target_os)).exists())
        };
        let header = |prefix: &Path| {
            let include = prefix.join("include");
            include.join("libCZI/libCZI.h").is_file().then_some(include)
        };
        if let Some(p) = std::env::var_os("SPECULAR_LIBCZI_DIR") {
            let prefix = PathBuf::from(p);
            return Some(Install {
                lib: lib_of(&prefix)?,
                include: header(&prefix)?,
                system: false,
            });
        }
        let prefixes: &[&str] = if target_os == "macos" {
            &["/opt/homebrew", "/usr/local"]
        } else {
            &["/usr", "/usr/local"]
        };
        prefixes.iter().find_map(|p| {
            let prefix = Path::new(p);
            Some(Install {
                lib: lib_of(prefix)?,
                include: header(prefix)?,
                system: matches!(*p, "/usr"),
            })
        })
    }

    /// "0.70.0" from libCZI_Config.h.
    fn version(include: &Path) -> Option<String> {
        let text = std::fs::read_to_string(include.join("libCZI/libCZI_Config.h")).ok()?;
        let num = |key: &str| {
            text.lines().find_map(|l| {
                let rest = l.trim().strip_prefix("#define")?.trim();
                let v = rest.strip_prefix(key)?.trim().trim_matches('"');
                v.parse::<u32>().ok()
            })
        };
        Some(format!(
            "{}.{}.{}",
            num("LIBCZI_VERSION_MAJOR")?,
            num("LIBCZI_VERSION_MINOR")?,
            num("LIBCZI_VERSION_PATCH").unwrap_or(0)
        ))
    }

    pub fn bridge() {
        println!("cargo:rerun-if-changed=czi/specular_czi.cpp");
        println!("cargo:rerun-if-env-changed=SPECULAR_LIBCZI_DIR");
        println!("cargo:rerun-if-env-changed=SPECULAR_NO_CZI_BRIDGE");
        println!("cargo:rerun-if-env-changed=CXX");
        let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
        let (host, target) = (
            std::env::var("HOST").unwrap_or_default(),
            std::env::var("TARGET").unwrap_or_default(),
        );
        // Unix only, and not when cross-compiling (the bridge links the build machine's
        // libCZI).
        if std::env::var_os("SPECULAR_NO_CZI_BRIDGE").is_some_and(|v| v == "1")
            || !matches!(target_os.as_str(), "linux" | "macos")
            || host != target
        {
            return;
        }
        let Some(czi) = find(&target_os) else {
            return;
        };
        let Some(ver) = version(&czi.include) else {
            println!(
                "cargo:warning=libCZI headers at {} have no version; bridge not built",
                czi.include.display()
            );
            return;
        };
        let name = if target_os == "macos" {
            "libspecular-czi.dylib"
        } else {
            "libspecular-czi.so"
        };
        let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default());
        let built = out_dir.join(name);
        let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".into());
        let mut cmd = Command::new(&cxx);
        cmd.args([
            "-std=c++17",
            "-O2",
            "-fPIC",
            "-shared",
            "-fvisibility=hidden",
            "-w",
        ])
        .arg("-I")
        .arg(&czi.include)
        .arg("czi/specular_czi.cpp")
        .arg("-o")
        .arg(&built)
        .arg("-L")
        .arg(&czi.lib);
        if target_os == "macos" {
            cmd.arg(format!("-Wl,-install_name,@rpath/{name}"));
        }
        if !czi.system {
            // A private libCZI (a prefix of its own): find it there at run time too.
            if target_os == "linux" {
                cmd.arg("-Wl,--disable-new-dtags");
            }
            cmd.arg(format!("-Wl,-rpath,{}", czi.lib.display()));
        }
        cmd.arg("-llibCZI");
        match cmd.status() {
            Ok(s) if s.success() => {}
            other => {
                println!(
                    "cargo:warning=libCZI {ver} found at {}, but the bridge did not build \
                     ({other:?}); Zeiss CZI files will be refused",
                    czi.lib.display()
                );
                return;
            }
        }
        println!("cargo:rustc-env=SPECULAR_CZI_BUILT_FOR={ver}");
        // Next to the host binary: OUT_DIR is target/<profile>/build/<pkg>/out.
        if let Some(profile) = out_dir.ancestors().nth(3) {
            let next_to_host = profile.join(name);
            if std::fs::copy(&built, &next_to_host).is_ok() {
                println!(
                    "cargo:rustc-env=SPECULAR_CZI_BRIDGE_BUILT={}",
                    next_to_host.display()
                );
                return;
            }
        }
        println!(
            "cargo:rustc-env=SPECULAR_CZI_BRIDGE_BUILT={}",
            built.display()
        );
    }
}
