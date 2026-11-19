//! Standard modules import by short name, while `std.` stays explicit.
use kestrel::modules::check_source;

fn checked(source: &str) -> kestrel::modules::ModuleCheck {
    check_source("imports.kes", source)
}

#[test]
fn short_import_example_checks() {
    let result = checked(include_str!("../testdata/language/shortimports.kes"));
    assert!(result.checked.is_ok(), "{:#?}", result.checked.diagnostics);
}

#[test]
fn math_imports_without_the_std_prefix() {
    let result = checked("import math.{max}\nfunction f() : Int = max(2, 3)\n");
    assert!(result.checked.is_ok(), "{:#?}", result.checked.diagnostics);
}

#[test]
fn dataframe_and_optimizer_modules_use_short_paths() {
    let result = checked(
        "import kesdas.{Series, DataFrame}\n\
         import kesnet.optim.{Adam}\n\
         function f() : Int = 1\n",
    );
    assert!(result.checked.is_ok(), "{:#?}", result.checked.diagnostics);
}

#[test]
fn explicit_std_still_works() {
    let result = checked("import std.math.{max}\nfunction f() : Int = max(2, 3)\n");
    assert!(result.checked.is_ok(), "{:#?}", result.checked.diagnostics);
}

#[test]
fn a_missing_short_module_is_reported() {
    let result = checked("import definitelyMissing\n");
    assert!(
        result.checked.diagnostics.iter().any(|d| {
            matches!(
                d.code,
                "E_MODULE_NOT_FOUND" | "E_IMPORT_NO_PATH" | "E_IMPORT_ROOT_REQUIRED"
            )
        }),
        "{:#?}",
        result.checked.diagnostics
    );
}
