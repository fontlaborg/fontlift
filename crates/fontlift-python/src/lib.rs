//! Python bindings for fontlift
//!
//! This module provides Python bindings using PyO3, exposing fontlift's
//! cross-platform font management capabilities to Python developers.

#![allow(non_local_definitions)]

use fontlift_core::{FontManager, FontScope};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::path::PathBuf;
use std::sync::Arc;

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
            let font_dict = PyDict::new(py);
            font_dict.set_item("path", font.path.to_string_lossy())?;
            font_dict.set_item("postscript_name", font.postscript_name)?;
            font_dict.set_item("full_name", font.full_name)?;
            font_dict.set_item("family_name", font.family_name)?;
            font_dict.set_item("style", font.style)?;
            result.push(font_dict.into_py(py));
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
            let font_dict = PyDict::new(py);
            font_dict
                .set_item("path", font.path.to_string_lossy())
                .unwrap();
            font_dict
                .set_item("postscript_name", font.postscript_name)
                .unwrap();
            font_dict.set_item("full_name", font.full_name).unwrap();
            font_dict.set_item("family_name", font.family_name).unwrap();
            font_dict.set_item("style", font.style).unwrap();
            result.push(font_dict.into_py(py));
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
fn fontlift_python(py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<FontliftManager>()?;
    m.add_function(wrap_pyfunction!(install, m)?)?;
    m.add_function(wrap_pyfunction!(list, m)?)?;
    m.add_function(wrap_pyfunction!(uninstall, m)?)?;
    m.add_function(wrap_pyfunction!(remove, m)?)?;
    m.add_function(wrap_pyfunction!(cleanup, m)?)?;
    m.add("__version__", "2.0.0-dev")?;

    // Expose convenience alias matching CLI naming
    m.add("__all__", PyDict::new(py))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_creation() {
        // This test will only compile on supported platforms
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let _manager = create_platform_manager();
        }
    }
}
