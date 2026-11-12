fn main() {
    // screencapturekit's Swift bridge only links under the Command Line Tools (no Xcode) with the Swift
    // runtime dir on the search path, and finds the OS copy of libswift_Concurrency at run time (T50).
    let clt = "/Library/Developer/CommandLineTools/usr/lib/swift/macosx";
    if std::env::var_os("CARGO_FEATURE_RECORDING").is_some() && std::path::Path::new(clt).is_dir() {
        println!("cargo:rustc-link-search=native={clt}");
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    }
    tauri_build::build()
}
