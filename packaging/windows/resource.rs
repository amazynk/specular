// The Windows resource of Specular's executables, shared by app/specular/build.rs and
// crates/specular-decode-host/build.rs (both `include!` this file; it is not a module).

/// Compiles a Windows resource into the crate's executables: the Specular icon as resource 1
/// (Explorer shows it; the viewer also loads it for its windows and taskbar button), version
/// information (file properties, Task Manager's description) and the application manifest
/// ([`manifest`]). `windowed`: the program shows windows (the viewer, not the decode host).
/// Nothing for other targets.
fn windows_resource(description: &str, file_name: &str, windowed: bool) {
    let crate_dir = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let icon = crate_dir.join("../../packaging/windows/specular.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    println!(
        "cargo:rerun-if-changed={}",
        crate_dir
            .join("../../packaging/windows/resource.rs")
            .display()
    );
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    // FILEVERSION wants four numbers: major, minor, patch and 0.
    let mut n: Vec<u16> = version
        .split(['.', '-', '+'])
        .take(3)
        .map(|p| p.parse().unwrap_or(0))
        .collect();
    n.resize(4, 0);
    let numeric = format!("{},{},{},{}", n[0], n[1], n[2], n[3]);
    let stem = file_name.trim_end_matches(".exe");
    let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let manifest_file = out_dir.join("specular.manifest");
    let dotted = format!("{}.{}.{}.{}", n[0], n[1], n[2], n[3]);
    std::fs::write(&manifest_file, manifest(stem, &dotted, windowed)).unwrap();
    // Resource scripts take C string literals: backslashes doubled.
    let c_path = |p: &std::path::Path| p.display().to_string().replace('\\', "\\\\");
    let icon = c_path(&icon);
    let manifest_file = c_path(&manifest_file);
    // 24 is RT_MANIFEST; ID 1 is the manifest Windows reads when it starts an executable.
    let rc = format!(
        r#"1 ICON "{icon}"

1 24 "{manifest_file}"

1 VERSIONINFO
FILEVERSION {numeric}
PRODUCTVERSION {numeric}
FILEFLAGSMASK 0x3F
FILEFLAGS 0x0
FILEOS 0x40004
FILETYPE 0x1
FILESUBTYPE 0x0
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "The Specular authors"
      VALUE "FileDescription", "{description}"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "{stem}"
      VALUE "OriginalFilename", "{file_name}"
      VALUE "ProductName", "Specular"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    );
    let out = out_dir.join("specular.rc");
    std::fs::write(&out, rc).unwrap();
    match embed_resource::compile(&out, embed_resource::NONE) {
        embed_resource::CompilationResult::Ok | embed_resource::CompilationResult::NotWindows => {}
        // No resource compiler (some cross-compiling setups): a build without the icon.
        embed_resource::CompilationResult::NotAttempted(why) => {
            println!("cargo:warning=no Windows resource compiler ({why}): {file_name} has no icon");
        }
        failed => panic!("{file_name}: Windows resource: {failed}"),
    }
}

/// The application manifest. Every executable runs as the user who started it (no elevation
/// prompt), declares Windows 10 and 11 (so version checks see the real system), may use paths
/// longer than 260 characters (where the system allows them: the LongPathsEnabled policy) and
/// has UTF-8 as its code page, so C libraries that take `char` paths open any file name.
/// `windowed` adds per-monitor DPI awareness (sharp on every display, also before winit sets
/// it) and the version 6 common controls (today's look for message boxes). The decode host
/// gets neither: it runs without user32, so it must not ask for windowing libraries.
fn manifest(stem: &str, version: &str, windowed: bool) -> String {
    let name = format!("Specular.{}", stem.replace('-', "."));
    let (dpi, controls) = if windowed {
        (
            r#"
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2, PerMonitor</dpiAwareness>"#,
            r#"
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>"#,
        )
    } else {
        ("", "")
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="{name}" version="{version}"/>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <!-- Windows 10 and Windows 11 -->
      <supportedOS Id="{{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware>
      <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>{dpi}
    </windowsSettings>
  </application>{controls}
</assembly>
"#
    )
}
