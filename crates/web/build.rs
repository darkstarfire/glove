// O motor (libcef.so) fica ao lado do executável, aqui e instalado.
fn main() {
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
}
