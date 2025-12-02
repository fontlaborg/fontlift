//! Python bindings for fontlift
//!
//! This module provides Python bindings using PyO3, exposing fontlift's
//! cross-platform font management capabilities to Python developers.

#![allow(non_local_definitions)]

use fontlift_core::{FontInfo, FontManager, FontScope};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::path::PathBuf;
use std::sync::Arc;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Python representation of `FontInfo` returned by Rust core
#[pyclass(module = "fontlift._native", name = "FontInfo")]
#[derive(Clone)]
struct PyFontInfo {
    #[pyo3(get)]
    path: String,
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
    #[pyo3(get)]
    format: Option<String>,
    #[pyo3(get)]
    scope: Option<String>,
}

impl From<FontInfo> for PyFontInfo {
    fn from(info: FontInfo) -> Self {
        let scope = info.scope.map(|s| match s {
            FontScope::User => "user".to_string(),
            FontScope::System => "system".to_string(),
        });
        Self {
            path: info.path.to_string_lossy().into_owned(),
            postscript_name: info.postscript_name,
            full_name: info.full_name,
            family_name: info.family_name,
            style: info.style,
            weight: info.weight,
            italic: info.italic,
            format: info.format,
            scope,
        }
    }
}

#[pymethods]
impl PyFontInfo {
    /// Return a JSON/dict-friendly representation of the font
    #[pyo3(name = "dict")]
    fn dict_py(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        dict.set_item("path", &self.path)?;
        dict.set_item("postscript_name", &self.postscript_name)?;
        dict.set_item("full_name", &self.full_name)?;
        dict.set_item("family_name", &self.family_name)?;
        dict.set_item("style", &self.style)?;
        dict.set_item("weight", self.weight)?;
        dict.set_item("italic", self.italic)?;
        dict.set_item("format", &self.format)?;
        dict.set_item("scope", &self.scope)?;
        Ok(dict.into_py(py))
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "FontInfo(path='{path}', postscript_name='{ps_name}', style='{style}')",
            path = self.path,
            ps_name = self.postscript_name,
            style = self.style
        ))
    }
}

/// Python wrapper for fontlift manager
#[pyclass]
struct FontliftManager {
    manager: Arc<dyn FontManager>,
}

#[allow(non_local_definitions)]
#[pymethods]
impl FontliftManager {
    /// Create a new fontlift manager
    #[new]
    fn new() -> PyResult<Self> {
        let manager = create_platform_manager();
        Ok(Self { manager })
    }

    /// List all installed fonts
    fn list_fonts(&self, py: Python) -> PyResult<Vec<PyObject>> {
        let fonts = self
            .manager
            .list_installed_fonts()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list fonts: {}", e)))?;

        let mut result = Vec::new();
        for font in fonts {
            result.push(PyFontInfo::from(font).into_py(py));
        }

        Ok(result)
    }

    /// Install a font file
    #[pyo3(signature = (font_path, admin=false))]
    fn install_font(&self, font_path: &str, admin: bool) -> PyResult<()> {
        let path = PathBuf::from(font_path);
        let scope = if admin {
            FontScope::System
        } else {
            FontScope::User
        };

        self.manager
            .install_font(&path, scope)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to install font: {}", e)))?;

        Ok(())
    }

    /// Check if a font is installed
    fn is_font_installed(&self, font_path: &str) -> PyResult<bool> {
        let path = PathBuf::from(font_path);

        let installed = self
            .manager
            .is_font_installed(&path)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to check font: {}", e)))?;

        Ok(installed)
    }

    /// Uninstall a font file
    #[pyo3(signature = (font_path, admin=false))]
    fn uninstall_font(&self, font_path: &str, admin: bool) -> PyResult<()> {
        let path = PathBuf::from(font_path);
        let scope = if admin {
            FontScope::System
        } else {
            FontScope::User
        };

        self.manager
            .uninstall_font(&path, scope)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to uninstall font: {}", e)))?;

        Ok(())
    }

    /// Remove a font file (uninstall and delete)
    #[pyo3(signature = (font_path, admin=false))]
    fn remove_font(&self, font_path: &str, admin: bool) -> PyResult<()> {
        let path = PathBuf::from(font_path);
        let scope = if admin {
            FontScope::System
        } else {
            FontScope::User
        };

        self.manager
            .remove_font(&path, scope)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to remove font: {}", e)))?;

        Ok(())
    }

    /// Clear font caches
    #[pyo3(signature = (admin=false))]
    fn clear_caches(&self, admin: bool) -> PyResult<()> {
        let scope = if admin {
            FontScope::System
        } else {
            FontScope::User
        };

        self.manager
            .clear_font_caches(scope)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to clear caches: {}", e)))?;

        Ok(())
    }
}

/// Create the appropriate font manager for the current platform
fn create_platform_manager() -> Arc<dyn FontManager> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(fontlift_platform_mac::MacFontManager::new())
    }

    #[cfg(target_os = "windows")]
    {
        Arc::new(fontlift_platform_win::WinFontManager::new())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        compile_error!("Linux support not yet implemented");
    }
}

/// Fire CLI interface for fontlift
#[pyfunction]
fn install(font_path: &str, admin: bool) -> PyResult<()> {
    let manager = create_platform_manager();
    let path = PathBuf::from(font_path);
    let scope = if admin {
        FontScope::System
    } else {
        FontScope::User
    };

    manager
        .install_font(&path, scope)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to install font: {}", e)))?;

    Ok(())
}

#[pyfunction]
fn list() -> PyResult<Vec<PyObject>> {
    let manager = create_platform_manager();
    let fonts = manager
        .list_installed_fonts()
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to list fonts: {}", e)))?;

    let mut result = Vec::new();
    Python::with_gil(|py| {
        for font in fonts {
            result.push(PyFontInfo::from(font).into_py(py));
        }
    });

    Ok(result)
}

#[pyfunction]
fn uninstall(font_path: &str, admin: bool) -> PyResult<()> {
    let manager = create_platform_manager();
    let path = PathBuf::from(font_path);
    let scope = if admin {
        FontScope::System
    } else {
        FontScope::User
    };

    manager
        .uninstall_font(&path, scope)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to uninstall font: {}", e)))?;

    Ok(())
}

#[pyfunction]
fn remove(font_path: &str, admin: bool) -> PyResult<()> {
    let manager = create_platform_manager();
    let path = PathBuf::from(font_path);
    let scope = if admin {
        FontScope::System
    } else {
        FontScope::User
    };

    manager
        .remove_font(&path, scope)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to remove font: {}", e)))?;

    Ok(())
}

#[pyfunction]
fn cleanup(admin: bool) -> PyResult<()> {
    let manager = create_platform_manager();
    let scope = if admin {
        FontScope::System
    } else {
        FontScope::User
    };

    manager
        .clear_font_caches(scope)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to clear caches: {}", e)))?;

    Ok(())
}

/// Python module definition
#[pymodule]
fn _native(py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyFontInfo>()?;
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
    use pyo3::{types::PyDict, PyCell};
    use std::path::PathBuf;

    #[test]
    fn test_manager_creation() {
        // This test will only compile on supported platforms
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let _manager = create_platform_manager();
        }
    }

    #[test]
    fn py_font_info_exposes_fields_and_dict() {
        Python::with_gil(|py| {
            let font_info = FontInfo {
                path: PathBuf::from("/Library/Fonts/Example.ttf"),
                postscript_name: "ExamplePS".to_string(),
                full_name: "Example Full".to_string(),
                family_name: "Example".to_string(),
                style: "Regular".to_string(),
                weight: Some(400),
                italic: Some(false),
                format: Some("TTF".to_string()),
                scope: Some(FontScope::System),
            };

            let py_obj = PyFontInfo::from(font_info).into_py(py);
            let cell: &PyCell<PyFontInfo> = py_obj.as_ref(py).downcast().unwrap();
            let borrowed = cell.borrow();

            assert_eq!(borrowed.postscript_name, "ExamplePS");
            assert_eq!(borrowed.family_name, "Example");
            assert_eq!(borrowed.weight, Some(400));
            assert_eq!(borrowed.italic, Some(false));
            assert_eq!(borrowed.format.as_deref(), Some("TTF"));
            assert_eq!(borrowed.scope.as_deref(), Some("system"));

            let dict_obj = cell.call_method0("dict").unwrap();
            let dict: &PyDict = dict_obj.downcast().unwrap();
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
}
