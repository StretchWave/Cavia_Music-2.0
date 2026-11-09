fn main() {
    // Tauri normally embeds its own application manifest into resource.lib.
    // Adding a second /MANIFESTINPUT produces CVT1100/LNK1123 in the app,
    // while adding none makes MSVC unit-test harnesses fail with
    // STATUS_ENTRYPOINT_NOT_FOUND (ComCtl32 v5 lacks v6 entry points).
    //
    // Use Tauri's documented "without app manifest" mode and embed exactly
    // one manifest globally: the app and every test binary get the same
    // Common-Controls v6 dependency, and there is no duplicate resource.
    #[cfg(target_os = "windows")]
    {
        let manifest_content = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
</assembly>
"#;
        let manifest_path = std::env::temp_dir().join("cavia_cc6.manifest");
        let _ = std::fs::write(&manifest_path, manifest_content);
        println!("cargo:rerun-if-changed={}", manifest_path.display());

        let attributes = tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        tauri_build::try_build(attributes).expect("failed to run tauri-build");

        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg=/MANIFESTINPUT:{}",
            manifest_path.display()
        );
    }

    #[cfg(not(target_os = "windows"))]
    {
        tauri_build::build();
    }
}
