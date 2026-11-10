//! Windows: the icon, version information and manifest of specular.exe
//! (packaging/windows/resource.rs).

include!("../../packaging/windows/resource.rs");

fn main() {
    windows_resource("Specular", "specular.exe", true);
}
