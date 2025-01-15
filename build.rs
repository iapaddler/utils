fn main() {
    println!("cargo:rustc-link-search=utils/bmp388/.");
    println!("cargo:rustc-link-lib=static=rsd");
}
