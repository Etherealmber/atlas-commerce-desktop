//! Compiles the gettext catalogs (French default, English source) into
//! `$OUT_DIR`; no PO parsing happens at runtime.

fn main() {
    fastframe_i18n::build::compile_catalogs("assets/i18n");
}
