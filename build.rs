use slint_build;

fn main() {
    slint_build::compile("src/gui/slint/module-metadata.slint").unwrap();
    slint_build::compile("src/gui/slint/main.slint").unwrap();
}
