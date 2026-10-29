use std::{env, fs, path::PathBuf, process::Command};

fn msvc_resource_compiler() -> PathBuf {
    if let Some(path) = env::var_os("RC") {
        return path.into();
    }
    let mut roots = Vec::new();
    if let Some(path) = env::var_os("WindowsSdkDir") {
        roots.push(PathBuf::from(path));
    }
    if let Some(path) = env::var_os("ProgramFiles(x86)") {
        roots.push(PathBuf::from(path).join("Windows Kits/10"));
    }
    for root in roots {
        let bin = root.join("bin");
        let mut versions = fs::read_dir(&bin)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect::<Vec<_>>();
        versions.sort();
        versions.reverse();
        versions.push(bin);
        for version in versions {
            for host in ["x64", "x86", "arm64"] {
                let candidate = version.join(host).join("rc.exe");
                if candidate.is_file() {
                    return candidate;
                }
            }
        }
    }
    "rc.exe".into()
}
fn main() {
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=assets/app-icon.ico");
    println!("cargo:rerun-if-changed=assets/app-icon.rc");
    for name in ["RC", "WINDRES", "WindowsSdkDir", "ProgramFiles(x86)"] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let msvc = env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    let root =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo provides the package root"));
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo provides OUT_DIR"));
    let version = [
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR"),
        env!("CARGO_PKG_VERSION_PATCH"),
    ]
    .map(|part| {
        part.parse::<u16>()
            .expect("Windows version components must fit in 16 bits")
    });
    let source = out.join("file-beholder.rc");
    let template = fs::read_to_string(root.join("assets/app-icon.rc"))
        .expect("Application resource template is readable");
    fs::write(
        &source,
        template
            .replace(
                "@VERSION_NUMBERS@",
                &format!("{},{},{},0", version[0], version[1], version[2]),
            )
            .replace("@PACKAGE_VERSION@", env!("CARGO_PKG_VERSION")),
    )
    .expect("Application resources can be written to OUT_DIR");
    let resource = out.join(if msvc {
        "file-beholder.res"
    } else {
        "file-beholder.o"
    });
    let mut command = if msvc {
        let mut command = Command::new(msvc_resource_compiler());
        command
            .arg("/nologo")
            .arg(format!("/i{}", root.display()))
            .arg(format!("/fo{}", resource.display()))
            .arg(&source);
        command
    } else {
        let mut command = Command::new(env::var_os("WINDRES").unwrap_or_else(|| "windres".into()));
        command
            .arg("-I")
            .arg(&root)
            .arg("-i")
            .arg(&source)
            .arg("-o")
            .arg(&resource);
        command
    };
    let status = command
        .current_dir(&root)
        .status()
        .expect("Install Windows SDK rc.exe (MSVC) or MinGW windres (GNU), or set RC/WINDRES");
    assert!(
        status.success(),
        "Failed to compile the application icon and version resources"
    );
    println!("cargo:rustc-link-arg={}", resource.display());
}
