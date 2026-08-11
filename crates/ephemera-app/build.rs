fn main() {
    // Bundled (not runtime-gettext) translations: the .mo files are compiled into
    // the binary at build time from ui/translations/<lang>/LC_MESSAGES/ephemera-app.po,
    // so the release binary needs no gettext runtime or external locale files on
    // any of the 5 release.yml platform targets. See `docs/10-implementation-status.md`
    // and the GAP-I18N PR for the runtime-switch API this enables
    // (`slint::select_bundled_translation`).
    let config = slint_build::CompilerConfiguration::new()
        .with_bundled_translations("ui/translations");
    slint_build::compile_with_config("ui/app.slint", config).expect("failed to compile Slint UI");
}
