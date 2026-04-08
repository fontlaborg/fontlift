//! PyO3 bindings for `fontlift`.
//!
//! This file defines the `fontlift._native` module.
//! - The one-shot functions (`install`, `list`, `uninstall`, `remove`,
//!   `cleanup`) create a manager, do one job, and return.
//! - [`FontliftManager`] keeps a platform manager alive across calls.
//! - `FontSource` and `FontFaceInfo` expose Rust structs as Python-friendly
//!   objects and dicts.
//!
//! ## Module layout
//!
//! ```text
//! fontlift._native
//! ├── __version__          string, e.g. "5.0.12"
//! ├── FontSource           class  — where a font file lives and how it's scoped
//! ├── FontFaceInfo         class  — metadata for one face inside a font file
//! ├── FontliftManager      class  — reusable manager; create once, call many times
//! ├── install(...)         fn     — one-shot convenience: install a font file
//! ├── list()               fn     — one-shot convenience: list installed fonts
//! ├── uninstall(...)       fn     — one-shot convenience: uninstall by path or name
//! ├── remove(...)          fn     — one-shot convenience: uninstall + delete the file
//! └── cleanup(...)         fn     — one-shot convenience: prune & clear caches
//! ```
//!
//! Naming and scope match the Rust core:
//! - `uninstall` removes the OS registration and keeps the file.
//! - `remove` deregisters the font and deletes the file.
//! - Scope controls who sees the font:
//! - `"user"` — only the current user sees it; no admin rights needed.
//! - `"system"` — every user on the machine sees it; requires elevated privileges.

#![allow(non_local_definitions)]

use fontlift_core::{
    validation_ext::ValidatorConfig, FontError, FontManager, FontScope, FontliftFontFaceInfo,
    FontliftFontSource,
};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};
use pyo3::{IntoPyObject, PyErr};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(test)]
use fontlift_core::FontResult;
#[cfg(test)]
use std::collections::VecDeque;
#[cfg(test)]
use std::sync::Mutex;

pub const PYTHON_BINDINGS_ENABLED: bool = true;
const VERSION: &str = env!("GIT_VERSION");

/// Convert a Rust [`FontError`] into a Python `RuntimeError`.
///
/// Public Python entry points use this so errors read like
/// `Failed to install font: ...`.
fn py_error(action: &str, err: FontError) -> PyErr {
    PyRuntimeError::new_err(format!("Failed to {action}: {err}"))
}

/// Run cleanup against an existing manager.
///
/// Shared by `FontliftManager.cleanup()` and the module-level `cleanup()` so
/// the behavior stays identical. At least one of `prune` or `cache` must be
/// enabled.
fn cleanup_with_manager(
    manager: &Arc<dyn FontManager>,
    admin: bool,
    prune: bool,
    cache: bool,
    dry_run: bool,
) -> PyResult<()> {
    if !prune && !cache {
        return Err(PyRuntimeError::new_err(
            "cleanup requires at least one of prune or cache to be enabled",
        ));
    }

    let scope = if admin {
        FontScope::System
    } else {
        FontScope::User
    };

    if dry_run {
        return Ok(());
    }

    if prune {
        manager
            .prune_missing_fonts(scope)
            .map_err(|e| py_error("prune stale font registrations", e))?;
    }

    if cache {
        manager
            .clear_font_caches(scope)
            .map_err(|e| py_error("clear font caches", e))?;
    }

    Ok(())
}

/// Return the two scopes in fallback order, preferred scope first.
///
/// Uninstall tries the expected scope first, then the other scope.
fn scope_order(preferred: FontScope) -> [FontScope; 2] {
    match preferred {
        FontScope::User => [FontScope::User, FontScope::System],
        FontScope::System => [FontScope::System, FontScope::User],
    }
}

/// Resolve a Python font identifier to a filesystem path and scope.
///
/// Callers must pass exactly one of:
/// - `font_path`, used as-is and paired with `default_scope`
/// - `name`, matched against installed fonts by PostScript name or full name
///
/// Name-based lookup returns the scope recorded for the installed font so a
/// later uninstall or remove targets the right registry first.
fn resolve_font_target(
    manager: &Arc<dyn FontManager>,
    font_path: Option<&str>,
    name: Option<&str>,
    default_scope: FontScope,
) -> PyResult<(PathBuf, FontScope)> {
    match (font_path, name) {
        (Some(_), Some(_)) => Err(PyRuntimeError::new_err(
            "Provide either font_path or name, not both",
        )),
        (None, None) => Err(PyRuntimeError::new_err(
            "A font_path or name is required to select a font",
        )),
        (Some(path), None) => Ok((PathBuf::from(path), default_scope)),
        (None, Some(font_name)) => {
            let installed_fonts = manager
                .list_installed_fonts()
                .map_err(|e| py_error("list installed fonts", e))?;

            if let Some(font) = installed_fonts
                .iter()
                .find(|f| f.postscript_name == font_name || f.full_name == font_name)
            {
                let starting_scope = font.source.scope.unwrap_or(default_scope);
                return Ok((font.source.path.clone(), starting_scope));
            }

            Err(PyRuntimeError::new_err(format!(
                "Font not found by name: {font_name}"
            )))
        }
    }
}

/// Uninstall a known font path, trying scopes in fallback order.
///
/// `uninstall` removes the OS registration and keeps the file on disk.
/// `dry_run` returns the scope that would be tried first without changing the
/// OS.
fn uninstall_resolved(
    manager: &Arc<dyn FontManager>,
    path: &Path,
    starting_scope: FontScope,
    dry_run: bool,
) -> PyResult<FontScope> {
    if dry_run {
        return Ok(starting_scope);
    }

    let mut last_error: Option<FontError> = None;

    for scope in scope_order(starting_scope) {
        let source = FontliftFontSource::new(path.to_path_buf()).with_scope(Some(scope));
        match manager.uninstall_font(&source) {
            Ok(()) => return Ok(scope),
            Err(err) => last_error = Some(err),
        }
    }

    Err(py_error(
        "uninstall font",
        last_error.unwrap_or(FontError::RegistrationFailed(format!(
            "Failed to uninstall font {} in any scope",
            path.display()
        ))),
    ))
}

fn remove_resolved(
    manager: &Arc<dyn FontManager>,
    path: &Path,
    scope: FontScope,
    dry_run: bool,
) -> PyResult<()> {
    if dry_run {
        return Ok(());
    }

    let source = FontliftFontSource::new(path.to_path_buf()).with_scope(Some(scope));
    manager
        .remove_font(&source)
        .map_err(|e| py_error("remove font", e))
}

/// Python view of a `FontliftFontSource`.
///
/// Fields use plain Python values: strings for paths and scope, `int | None`
/// for `face_index`, and `bool | None` for `is_collection`.
#[pyclass(module = "fontlift._native", name = "FontSource")]
#[derive(Clone)]
struct PyFontSource {
    #[pyo3(get)]
    path: String,
    #[pyo3(get)]
    format: Option<String>,
    #[pyo3(get)]
    face_index: Option<u32>,
    #[pyo3(get)]
    is_collection: Option<bool>,
    #[pyo3(get)]
    scope: Option<String>,
}

impl From<FontliftFontSource> for PyFontSource {
    fn from(source: FontliftFontSource) -> Self {
        let scope = source.scope.map(|s| match s {
            FontScope::User => "user".to_string(),
            FontScope::System => "system".to_string(),
        });
        Self {
            path: source.path.to_string_lossy().into_owned(),
            format: source.format,
            face_index: source.face_index,
            is_collection: source.is_collection,
            scope,
        }
    }
}

fn source_dict<'py>(py: Python<'py>, source: &PyFontSource) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("path", &source.path)?;
    dict.set_item("format", &source.format)?;
    dict.set_item("face_index", source.face_index)?;
    dict.set_item("is_collection", source.is_collection)?;
    dict.set_item("scope", &source.scope)?;
    Ok(dict)
}

/// Python view of one installed font face.
///
/// Name fields keep the same distinctions as the Rust core:
/// `postscript_name` is the stable programmatic identifier, `full_name` is the
/// menu-facing name, `family_name` groups related faces, and `style` names the
/// variant within that family.
#[pyclass(module = "fontlift._native", name = "FontFaceInfo")]
#[derive(Clone)]
struct PyFontFaceInfo {
    #[pyo3(get)]
    source: PyFontSource,
    #[pyo3(get)]
    postscript_name: String,
    #[pyo3(get)]
    full_name: String,
    #[pyo3(get)]
    family_name: String,
    #[pyo3(get)]
    style: String,
    #[pyo3(get)]
    weight: Option<u16>,
    #[pyo3(get)]
    italic: Option<bool>,
}

impl From<FontliftFontFaceInfo> for PyFontFaceInfo {
    fn from(info: FontliftFontFaceInfo) -> Self {
        let source = PyFontSource::from(info.source.clone());

        Self {
            source,
            postscript_name: info.postscript_name,
            full_name: info.full_name,
            family_name: info.family_name,
            style: info.style,
            weight: info.weight,
            italic: info.italic,
        }
    }
}

#[pymethods]
impl PyFontFaceInfo {
    /// Return a plain `dict` for JSON and interop.
    ///
    /// Keys mirror the object fields. For backward compatibility, `path`,
    /// `format`, and `scope` are duplicated at the top level as well as inside
    /// the nested `source` dict.
    #[pyo3(name = "dict")]
    fn dict_py<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("source", source_dict(py, &self.source)?)?;
        dict.set_item("path", &self.source.path)?; // legacy top-level path
        dict.set_item("postscript_name", &self.postscript_name)?;
        dict.set_item("full_name", &self.full_name)?;
        dict.set_item("family_name", &self.family_name)?;
        dict.set_item("style", &self.style)?;
        dict.set_item("weight", self.weight)?;
        dict.set_item("italic", self.italic)?;
        dict.set_item("format", &self.source.format)?;
        dict.set_item("scope", &self.source.scope)?;
        Ok(dict)
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "FontFaceInfo(path='{path}', postscript_name='{ps_name}', style='{style}')",
            path = self.source.path,
            ps_name = self.postscript_name,
            style = self.style
        ))
    }
}

/// Reusable Python font manager.
///
/// Use this when you want one object that can perform several operations in a
/// row. For quick scripts, the module-level one-shot helpers are usually enough.
///
/// ```python
/// from fontlift import FontliftManager
///
/// mgr = FontliftManager()
/// mgr.install_font("/tmp/MyFont.ttf")
/// faces = mgr.list_fonts()
/// for face in faces:
///     print(face.postscript_name, face.source.path)
/// mgr.cleanup(prune=True, cache=True)
/// ```
#[pyclass]
struct FontliftManager {
    manager: Arc<dyn FontManager>,
}

#[allow(non_local_definitions)]
#[pymethods]
impl FontliftManager {
    /// Create a manager backed by the current platform.
    #[new]
    fn new() -> PyResult<Self> {
        let manager = create_platform_manager();
        Ok(Self { manager })
    }

    /// Return one `FontFaceInfo` object per installed face.
    ///
    /// Collection files produce multiple entries. Results are not limited to
    /// fonts installed by `fontlift`.
    fn list_fonts(&self, py: Python) -> PyResult<Vec<PyObject>> {
        let fonts = self
            .manager
            .list_installed_fonts()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list fonts: {}", e)))?;

        let mut result = Vec::new();
        for font in fonts {
            let obj = PyFontFaceInfo::from(font)
                .into_pyobject(py)?
                .unbind()
                .into_any();
            result.push(obj);
        }

        Ok(result)
    }

    #[pyo3(signature = (font_path, admin=false, strict=false))]
    fn install_font(&self, font_path: &str, admin: bool, strict: bool) -> PyResult<()> {
        let path = PathBuf::from(font_path);
        let scope = if admin {
            FontScope::System
        } else {
            FontScope::User
        };
        let source = FontliftFontSource::new(path).with_scope(Some(scope));

        // Use validating manager if strict mode requested
        let manager: Arc<dyn FontManager> = if strict {
            create_platform_manager_with_validation(Some(ValidatorConfig::default()))
        } else {
            self.manager.clone()
        };

        manager
            .install_font(&source)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to install font: {}", e)))?;

        Ok(())
    }

    /// Return whether the OS currently has a registration for `font_path`.
    fn is_font_installed(&self, font_path: &str) -> PyResult<bool> {
        let path = PathBuf::from(font_path);
        let source = FontliftFontSource::new(path);

        let installed = self
            .manager
            .is_font_installed(&source)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to check font: {}", e)))?;

        Ok(installed)
    }

    /// Uninstall by `font_path` or `name`.
    ///
    /// `name` matches either a PostScript name or a full name. The file stays
    /// on disk. `dry_run=True` resolves the target and scope without changing
    /// the OS.
    #[pyo3(signature = (font_path=None, name=None, admin=false, dry_run=false))]
    fn uninstall_font(
        &self,
        font_path: Option<&str>,
        name: Option<&str>,
        admin: bool,
        dry_run: bool,
    ) -> PyResult<()> {
        let default_scope = if admin {
            FontScope::System
        } else {
            FontScope::User
        };

        let (path, starting_scope) =
            resolve_font_target(&self.manager, font_path, name, default_scope)?;

        uninstall_resolved(&self.manager, &path, starting_scope, dry_run).map(|_| ())
    }

    #[pyo3(signature = (font_path=None, name=None, admin=false, dry_run=false))]
    fn remove_font(
        &self,
        font_path: Option<&str>,
        name: Option<&str>,
        admin: bool,
        dry_run: bool,
    ) -> PyResult<()> {
        let default_scope = if admin {
            FontScope::System
        } else {
            FontScope::User
        };

        let (path, scope) = resolve_font_target(&self.manager, font_path, name, default_scope)?;

        remove_resolved(&self.manager, &path, scope, dry_run)
    }

    /// Prune stale registrations, clear caches, or both.
    #[pyo3(signature = (admin=false, prune=true, cache=true, dry_run=false))]
    fn cleanup(&self, admin: bool, prune: bool, cache: bool, dry_run: bool) -> PyResult<()> {
        cleanup_with_manager(&self.manager, admin, prune, cache, dry_run)
    }

    /// Clear caches only.
    ///
    /// Compatibility wrapper for `cleanup(prune=False, cache=True)`.
    #[pyo3(signature = (admin=false))]
    fn clear_caches(&self, admin: bool) -> PyResult<()> {
        cleanup_with_manager(&self.manager, admin, false, true, false)
    }
}

fn create_platform_manager() -> Arc<dyn FontManager> {
    create_platform_manager_with_validation(None)
}

fn create_platform_manager_with_validation(
    validation_config: Option<ValidatorConfig>,
) -> Arc<dyn FontManager> {
    #[cfg(target_os = "macos")]
    {
        if let Some(config) = validation_config {
            Arc::new(fontlift_platform_mac::MacFontManager::with_validation(
                config,
            ))
        } else {
            Arc::new(fontlift_platform_mac::MacFontManager::new())
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(config) = validation_config {
            Arc::new(fontlift_platform_win::WinFontManager::with_validation(
                config,
            ))
        } else {
            Arc::new(fontlift_platform_win::WinFontManager::new())
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        compile_error!("Linux support not yet implemented");
    }
}

#[pyfunction]
#[pyo3(signature = (font_path, admin=false, strict=false))]
fn install(font_path: &str, admin: bool, strict: bool) -> PyResult<()> {
    let validation_config = if strict {
        Some(ValidatorConfig::default())
    } else {
        None
    };
    let manager = create_platform_manager_with_validation(validation_config);
    let path = PathBuf::from(font_path);
    let scope = if admin {
        FontScope::System
    } else {
        FontScope::User
    };
    let source = FontliftFontSource::new(path).with_scope(Some(scope));

    manager
        .install_font(&source)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to install font: {}", e)))?;

    Ok(())
}

#[pyfunction]
fn list() -> PyResult<Vec<PyObject>> {
    let manager = create_platform_manager();
    let fonts = manager
        .list_installed_fonts()
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to list fonts: {}", e)))?;

    Python::with_gil(|py| {
        let mut result = Vec::with_capacity(fonts.len());
        for font in fonts {
            let obj = PyFontFaceInfo::from(font)
                .into_pyobject(py)?
                .unbind()
                .into_any();
            result.push(obj);
        }
        Ok(result)
    })
}

#[pyfunction]
#[pyo3(signature = (font_path=None, name=None, admin=false, dry_run=false))]
fn uninstall(
    font_path: Option<&str>,
    name: Option<&str>,
    admin: bool,
    dry_run: bool,
) -> PyResult<()> {
    let manager = create_platform_manager();
    let default_scope = if admin {
        FontScope::System
    } else {
        FontScope::User
    };

    let (path, starting_scope) = resolve_font_target(&manager, font_path, name, default_scope)?;
    uninstall_resolved(&manager, &path, starting_scope, dry_run).map(|_| ())
}

#[pyfunction]
#[pyo3(signature = (font_path=None, name=None, admin=false, dry_run=false))]
fn remove(font_path: Option<&str>, name: Option<&str>, admin: bool, dry_run: bool) -> PyResult<()> {
    let manager = create_platform_manager();
    let default_scope = if admin {
        FontScope::System
    } else {
        FontScope::User
    };

    let (path, scope) = resolve_font_target(&manager, font_path, name, default_scope)?;
    remove_resolved(&manager, &path, scope, dry_run)
}

#[pyfunction]
#[pyo3(signature = (admin=false, prune=true, cache=true, dry_run=false))]
fn cleanup(admin: bool, prune: bool, cache: bool, dry_run: bool) -> PyResult<()> {
    let manager = create_platform_manager();
    cleanup_with_manager(&manager, admin, prune, cache, dry_run)
}

#[pymodule]
fn _native(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFontSource>()?;
    m.add_class::<PyFontFaceInfo>()?;
    m.add_class::<FontliftManager>()?;
    m.add_function(wrap_pyfunction!(install, m)?)?;
    m.add_function(wrap_pyfunction!(list, m)?)?;
    m.add_function(wrap_pyfunction!(uninstall, m)?)?;
    m.add_function(wrap_pyfunction!(remove, m)?)?;
    m.add_function(wrap_pyfunction!(cleanup, m)?)?;
    m.add("__version__", VERSION)?;

    // Expose convenience alias matching CLI naming
    m.add("__all__", PyDict::new(py))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "python-bindings")]
    use pyo3::types::PyDict;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[derive(Default)]
    struct FakeManager {
        prune_calls: Mutex<VecDeque<FontScope>>,
        cache_calls: Mutex<VecDeque<FontScope>>,
    }

    impl FakeManager {
        fn prune_calls(&self) -> Vec<FontScope> {
            self.prune_calls
                .lock()
                .expect("prune lock")
                .iter()
                .copied()
                .collect()
        }

        fn cache_calls(&self) -> Vec<FontScope> {
            self.cache_calls
                .lock()
                .expect("cache lock")
                .iter()
                .copied()
                .collect()
        }
    }

    impl FontManager for FakeManager {
        fn install_font(&self, _source: &FontliftFontSource) -> FontResult<()> {
            Err(FontError::UnsupportedOperation(
                "install unused in fake manager".to_string(),
            ))
        }

        fn uninstall_font(&self, _source: &FontliftFontSource) -> FontResult<()> {
            Err(FontError::UnsupportedOperation(
                "uninstall unused in fake manager".to_string(),
            ))
        }

        fn remove_font(&self, _source: &FontliftFontSource) -> FontResult<()> {
            Err(FontError::UnsupportedOperation(
                "remove unused in fake manager".to_string(),
            ))
        }

        fn is_font_installed(&self, _source: &FontliftFontSource) -> FontResult<bool> {
            Ok(false)
        }

        fn list_installed_fonts(&self) -> FontResult<Vec<FontliftFontFaceInfo>> {
            Ok(Vec::new())
        }

        fn clear_font_caches(&self, scope: FontScope) -> FontResult<()> {
            self.cache_calls
                .lock()
                .expect("cache lock")
                .push_back(scope);
            Ok(())
        }

        fn prune_missing_fonts(&self, scope: FontScope) -> FontResult<usize> {
            self.prune_calls
                .lock()
                .expect("prune lock")
                .push_back(scope);
            Ok(1)
        }
    }

    #[test]
    fn test_manager_creation() {
        // This test will only compile on supported platforms
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let _manager = create_platform_manager();
        }
    }

    // This test requires linking to libpython and only runs when the
    // `python-bindings` feature is enabled (maturin builds).
    #[test]
    #[cfg(feature = "python-bindings")]
    fn py_font_info_exposes_fields_and_dict() {
        Python::with_gil(|py| {
            let font_info = FontliftFontFaceInfo::new(
                FontliftFontSource::new(PathBuf::from("/Library/Fonts/Example.ttf"))
                    .with_format(Some("TTF".to_string()))
                    .with_scope(Some(FontScope::System)),
                "ExamplePS".to_string(),
                "Example Full".to_string(),
                "Example".to_string(),
                "Regular".to_string(),
            );

            let bound = PyFontFaceInfo::from(font_info)
                .into_pyobject(py)
                .expect("convert font info to PyObject");
            let borrowed = bound.borrow();

            assert_eq!(borrowed.postscript_name, "ExamplePS");
            assert_eq!(borrowed.family_name, "Example");
            assert_eq!(borrowed.weight, Some(400));
            assert_eq!(borrowed.italic, Some(false));
            assert_eq!(borrowed.source.format.as_deref(), Some("TTF"));
            assert_eq!(borrowed.source.scope.as_deref(), Some("system"));

            let dict_obj = bound.call_method0("dict").unwrap();
            let dict = dict_obj.downcast::<PyDict>().unwrap();
            let style: String = dict
                .get_item("style")
                .unwrap()
                .expect("missing style")
                .extract()
                .unwrap();
            let path: String = dict
                .get_item("path")
                .unwrap()
                .expect("missing path")
                .extract()
                .unwrap();
            let weight: u16 = dict
                .get_item("weight")
                .unwrap()
                .expect("missing weight")
                .extract()
                .unwrap();

            assert_eq!(style, "Regular");
            assert_eq!(path, "/Library/Fonts/Example.ttf");
            assert_eq!(weight, 400);
        });
    }

    #[test]
    fn cleanup_runs_selected_operations() {
        let manager = Arc::new(FakeManager::default());
        let dyn_manager: Arc<dyn FontManager> = manager.clone();

        cleanup_with_manager(&dyn_manager, false, true, true, false).expect("cleanup");

        assert_eq!(manager.prune_calls(), vec![FontScope::User]);
        assert_eq!(manager.cache_calls(), vec![FontScope::User]);
    }

    #[test]
    fn cleanup_respects_action_flags_and_scopes() {
        let manager = Arc::new(FakeManager::default());
        let dyn_manager: Arc<dyn FontManager> = manager.clone();

        cleanup_with_manager(&dyn_manager, false, true, false, false).expect("prune only");
        cleanup_with_manager(&dyn_manager, true, false, true, false).expect("cache only admin");

        assert_eq!(manager.prune_calls(), vec![FontScope::User]);
        assert_eq!(manager.cache_calls(), vec![FontScope::System]);
    }

    #[test]
    fn cleanup_supports_dry_run_and_requires_actions() {
        let manager = Arc::new(FakeManager::default());
        let dyn_manager: Arc<dyn FontManager> = manager.clone();

        cleanup_with_manager(&dyn_manager, false, true, true, true).expect("dry run");
        assert!(manager.prune_calls().is_empty());
        assert!(manager.cache_calls().is_empty());

        let err = cleanup_with_manager(&dyn_manager, false, false, false, false)
            .expect_err("at least one action required");
        assert!(
            err.to_string().contains("cleanup requires"),
            "message preserved"
        );
    }

    #[derive(Default)]
    struct RecordingManager {
        installed_fonts: Vec<FontliftFontFaceInfo>,
        uninstall_calls: Mutex<Vec<FontScope>>,
        remove_calls: Mutex<Vec<FontScope>>,
        fail_uninstall_scopes: Mutex<Vec<FontScope>>,
    }

    impl RecordingManager {
        fn with_fonts(fonts: Vec<FontliftFontFaceInfo>) -> Self {
            Self {
                installed_fonts: fonts,
                uninstall_calls: Mutex::new(Vec::new()),
                remove_calls: Mutex::new(Vec::new()),
                fail_uninstall_scopes: Mutex::new(Vec::new()),
            }
        }

        fn with_failures(self, scopes: Vec<FontScope>) -> Self {
            *self.fail_uninstall_scopes.lock().expect("fail scope lock") = scopes;
            self
        }

        fn uninstall_scopes(&self) -> Vec<FontScope> {
            self.uninstall_calls
                .lock()
                .expect("uninstall lock")
                .iter()
                .copied()
                .collect()
        }

        fn remove_scopes(&self) -> Vec<FontScope> {
            self.remove_calls
                .lock()
                .expect("remove lock")
                .iter()
                .copied()
                .collect()
        }
    }

    impl FontManager for RecordingManager {
        fn install_font(&self, _source: &FontliftFontSource) -> FontResult<()> {
            Ok(())
        }

        fn uninstall_font(&self, source: &FontliftFontSource) -> FontResult<()> {
            let scope = source.scope.unwrap_or(FontScope::User);
            self.uninstall_calls
                .lock()
                .expect("uninstall lock")
                .push(scope);

            let mut failures = self.fail_uninstall_scopes.lock().expect("failure lock");
            if let Some(pos) = failures.iter().position(|s| *s == scope) {
                failures.remove(pos);
                return Err(FontError::PermissionDenied(format!(
                    "forced uninstall failure in {:?} scope",
                    scope
                )));
            }

            Ok(())
        }

        fn remove_font(&self, source: &FontliftFontSource) -> FontResult<()> {
            let scope = source.scope.unwrap_or(FontScope::User);
            self.remove_calls.lock().expect("remove lock").push(scope);
            Ok(())
        }

        fn is_font_installed(&self, _source: &FontliftFontSource) -> FontResult<bool> {
            Ok(false)
        }

        fn list_installed_fonts(&self) -> FontResult<Vec<FontliftFontFaceInfo>> {
            Ok(self.installed_fonts.clone())
        }

        fn clear_font_caches(&self, _scope: FontScope) -> FontResult<()> {
            Ok(())
        }
    }

    #[test]
    fn resolve_font_by_name_uses_scope_and_falls_back_on_error() {
        let font = FontliftFontFaceInfo::new(
            FontliftFontSource::new(PathBuf::from("/fonts/Example.ttf"))
                .with_scope(Some(FontScope::System)),
            "ExamplePS".to_string(),
            "Example Full".to_string(),
            "Example".to_string(),
            "Regular".to_string(),
        );

        let manager = Arc::new(
            RecordingManager::with_fonts(vec![font]).with_failures(vec![FontScope::System]),
        );
        let dyn_manager: Arc<dyn FontManager> = manager.clone();

        let (path, starting_scope) =
            resolve_font_target(&dyn_manager, None, Some("ExamplePS"), FontScope::User)
                .expect("resolved font by name");

        assert_eq!(starting_scope, FontScope::System);

        let used_scope =
            uninstall_resolved(&dyn_manager, &path, starting_scope, false).expect("uninstall");

        assert_eq!(used_scope, FontScope::User);
        assert_eq!(
            manager.uninstall_scopes(),
            vec![FontScope::System, FontScope::User]
        );
    }

    #[test]
    fn resolve_font_target_requires_identifier() {
        let manager = Arc::new(RecordingManager::default());
        let dyn_manager: Arc<dyn FontManager> = manager.clone();

        let err = resolve_font_target(&dyn_manager, None, None, FontScope::User)
            .expect_err("identifier required");

        assert!(err.to_string().contains("font_path or name is required"));
    }

    #[test]
    fn remove_by_name_uses_font_scope_and_supports_dry_run() {
        let font = FontliftFontFaceInfo::new(
            FontliftFontSource::new(PathBuf::from("/fonts/Remove.ttf"))
                .with_scope(Some(FontScope::User)),
            "RemovePS".to_string(),
            "Remove Full".to_string(),
            "Remove".to_string(),
            "Regular".to_string(),
        );

        let manager = Arc::new(RecordingManager::with_fonts(vec![font]));
        let dyn_manager: Arc<dyn FontManager> = manager.clone();

        let (path, scope) =
            resolve_font_target(&dyn_manager, None, Some("RemovePS"), FontScope::System)
                .expect("resolved font by name");

        remove_resolved(&dyn_manager, &path, scope, true).expect("dry run remove");
        assert!(manager.remove_scopes().is_empty());

        remove_resolved(&dyn_manager, &path, scope, false).expect("remove executes");
        assert_eq!(manager.remove_scopes(), vec![FontScope::User]);
    }
}
