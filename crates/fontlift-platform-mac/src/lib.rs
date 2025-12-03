//! macOS platform implementation for fontlift
//!
//! This module provides macOS-specific font management using Core Text APIs,
//! implementing the same functionality as the Swift CLI but in Rust.

use fontlift_core::{
    protection, validation, FontError, FontManager, FontResult, FontScope, FontliftFontFaceInfo,
    FontliftFontSource,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use core_foundation::array::CFArray;
use core_foundation::base::TCFType;
use core_foundation::dictionary::CFDictionary;
use core_foundation::error::{CFError, CFErrorRef};
use core_foundation::string::CFString;
use core_foundation::url::{CFURLRef, CFURL};
use core_text::font_descriptor::{
    self as ct_font_descriptor, CTFontDescriptor, CTFontFormat, SymbolicTraitAccessors,
    TraitAccessors,
};
use core_text::font_manager as ct_font_manager;

type CTFontManagerScope = u32;
const K_CT_FONT_MANAGER_SCOPE_PERSISTENT: CTFontManagerScope = 2; // aligns with user scope
const K_CT_FONT_MANAGER_SCOPE_USER: CTFontManagerScope = 2;
const K_CT_FONT_MANAGER_ERROR_ALREADY_REGISTERED: isize = 105;
const K_CT_FONT_MANAGER_ERROR_DUPLICATED_NAME: isize = 305;

fn test_cache_root() -> Option<PathBuf> {
    env::var_os("FONTLIFT_TEST_CACHE_ROOT").map(PathBuf::from)
}

fn user_home(test_root: &Option<PathBuf>) -> FontResult<PathBuf> {
    if let Some(root) = test_root.clone() {
        return Ok(root);
    }

    env::var("HOME").map(PathBuf::from).map_err(|_| {
        FontError::UnsupportedOperation(
            "Unable to resolve HOME directory for cache cleanup".to_string(),
        )
    })
}

fn fake_registry_root() -> Option<PathBuf> {
    env::var_os("FONTLIFT_FAKE_REGISTRY_ROOT").map(PathBuf::from)
}

#[allow(dead_code)]
fn fake_registry_target(root: &Path, scope: FontScope, source: &Path) -> FontResult<PathBuf> {
    let file_name = source.file_name().ok_or_else(|| {
        FontError::InvalidFormat("Font path must include a file name".to_string())
    })?;

    let base = match scope {
        FontScope::User => root.join("Library/Fonts"),
        FontScope::System => root.join("System/Library/Fonts"),
    };

    Ok(base.join(file_name))
}

fn delete_matching_files(root: &Path, predicate: impl Fn(&Path) -> bool) -> FontResult<usize> {
    if !root.exists() {
        return Ok(0);
    }

    let mut removed = 0usize;
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(FontError::IoError(err)),
        };

        for entry in entries {
            let entry = entry.map_err(FontError::IoError)?;
            let path = entry.path();

            if path.is_dir() {
                stack.push(path);
                continue;
            }

            if predicate(&path) {
                match fs::remove_file(&path) {
                    Ok(_) => removed += 1,
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                    Err(err) => return Err(FontError::IoError(err)),
                }
            }
        }
    }

    Ok(removed)
}

fn purge_directory_contents(root: &Path) -> FontResult<usize> {
    if !root.exists() {
        return Ok(0);
    }

    let mut removed = 0usize;
    let entries = fs::read_dir(root).map_err(FontError::IoError)?;

    for entry in entries {
        let entry = entry.map_err(FontError::IoError)?;
        let path = entry.path();

        if path.is_dir() {
            removed += purge_directory_contents(&path)?;
            match fs::remove_dir(&path) {
                Ok(_) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(FontError::IoError(err)),
            }
        } else {
            fs::remove_file(&path).map_err(FontError::IoError)?;
            removed += 1;
        }
    }

    Ok(removed)
}

fn clear_adobe_font_caches(home: &Path) -> FontResult<usize> {
    // Adobe Font cache manifests (AdobeFnt*.lst) live under TypeSupport; remove them recursively
    let type_support = home.join("Library/Application Support/Adobe/TypeSupport");
    let removed_lists = delete_matching_files(&type_support, |path| {
        path.file_name()
            .and_then(|n| n.to_str())
            .map(|name| name.starts_with("AdobeFnt") && name.ends_with(".lst"))
            .unwrap_or(false)
    })?;

    // Adobe font cache files under Caches/Adobe/Fonts
    let fonts_cache = home.join("Library/Caches/Adobe/Fonts");
    let removed_cache = purge_directory_contents(&fonts_cache)?;

    Ok(removed_lists + removed_cache)
}

fn clear_office_font_cache(home: &Path) -> FontResult<usize> {
    // Microsoft Office font cache storage used by Office apps
    let office_cache = home.join("Library/Group Containers/UBF8T346G9.Office/FontCache");
    purge_directory_contents(&office_cache)
}

#[link(name = "CoreText", kind = "framework")]
extern "C" {
    fn CTFontManagerRegisterFontsForURL(
        font_url: CFURLRef,
        scope: CTFontManagerScope,
        error: *mut CFErrorRef,
    ) -> bool;

    fn CTFontManagerUnregisterFontsForURL(
        font_url: CFURLRef,
        scope: CTFontManagerScope,
        error: *mut CFErrorRef,
    ) -> bool;
}

fn ct_scope(scope: FontScope) -> CTFontManagerScope {
    match scope {
        FontScope::User => K_CT_FONT_MANAGER_SCOPE_USER,
        FontScope::System => K_CT_FONT_MANAGER_SCOPE_PERSISTENT,
    }
}

fn cf_error_to_string(err: CFErrorRef) -> String {
    if err.is_null() {
        return "unknown CoreText error".to_string();
    }

    let cf_err = unsafe { CFError::wrap_under_get_rule(err) };
    cf_err.description().to_string()
}

fn scope_from_path(path: &Path) -> FontScope {
    if let Some(fake_root) = fake_registry_root() {
        let user_fonts = fake_root.join("Library/Fonts");
        let system_fonts = fake_root.join("System/Library/Fonts");

        if path.starts_with(&user_fonts) {
            return FontScope::User;
        }

        if path.starts_with(&system_fonts) {
            return FontScope::System;
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let user_fonts = PathBuf::from(home).join("Library/Fonts");
        if path.starts_with(&user_fonts) {
            return FontScope::User;
        }
    }

    if path.starts_with("/System/Library/Fonts") || path.starts_with("/Library/Fonts") {
        FontScope::System
    } else {
        // Default to user to avoid over-reporting system scope for custom paths
        FontScope::User
    }
}

fn normalize_path(path: &Path) -> String {
    let mut normalized = path.to_string_lossy().replace('\\', "/").to_lowercase();

    while normalized.contains("//") {
        normalized = normalized.replace("//", "/");
    }

    normalized
}

fn font_format_to_string(format: CTFontFormat) -> Option<String> {
    match format {
        ct_font_descriptor::kCTFontFormatOpenTypePostScript => {
            Some("OpenTypePostScript".to_string())
        }
        ct_font_descriptor::kCTFontFormatOpenTypeTrueType => Some("OpenTypeTrueType".to_string()),
        ct_font_descriptor::kCTFontFormatTrueType => Some("TrueType".to_string()),
        ct_font_descriptor::kCTFontFormatPostScript => Some("PostScript".to_string()),
        ct_font_descriptor::kCTFontFormatBitmap => Some("Bitmap".to_string()),
        _ => None,
    }
}

fn is_conflict_error(err: &CFError) -> bool {
    let domain = err.domain().to_string();
    if !domain.contains("CTFontManagerErrorDomain") {
        return false;
    }

    matches!(
        err.code(),
        K_CT_FONT_MANAGER_ERROR_ALREADY_REGISTERED | K_CT_FONT_MANAGER_ERROR_DUPLICATED_NAME
    )
}

fn descriptor_to_font_face_info(descriptor: &CTFontDescriptor) -> Option<FontliftFontFaceInfo> {
    let path = descriptor.font_path()?;
    let postscript_name = descriptor.font_name();
    let display_name = descriptor.display_name();
    let family_name = descriptor.family_name();
    let style_name = descriptor.style_name();

    let mut source = FontliftFontSource::new(path.clone()).with_scope(Some(scope_from_path(&path)));

    if let Some(format) = descriptor.font_format() {
        source = source.with_format(font_format_to_string(format));
    }

    let mut info = FontliftFontFaceInfo::new(
        source,
        postscript_name.to_string(),
        display_name.to_string(),
        family_name.to_string(),
        style_name.to_string(),
    );

    if let Some(traits) = descriptor
        .attributes()
        .find(unsafe { CFString::wrap_under_get_rule(ct_font_descriptor::kCTFontTraitsAttribute) })
        .and_then(|traits_cf| {
            if traits_cf.instance_of::<CFDictionary>() {
                Some(unsafe {
                    ct_font_descriptor::CTFontTraits::wrap_under_get_rule(
                        traits_cf.as_CFTypeRef() as _
                    )
                })
            } else {
                None
            }
        })
    {
        let symbolic = traits.symbolic_traits();
        info.italic = Some(symbolic.is_italic());

        let weight = (traits.normalized_weight() * 400.0 + 500.0).round();
        if weight.is_finite() {
            let clamped = weight.clamp(1.0, 1000.0) as u16;
            info.weight = Some(clamped);
        }
    }

    Some(info)
}

/// macOS font manager using Core Text APIs
pub struct MacFontManager {
    fake_root: Option<PathBuf>,
}

impl MacFontManager {
    /// Create a new macOS font manager
    pub fn new() -> Self {
        let fake_root = std::env::var_os("FONTLIFT_FAKE_REGISTRY_ROOT").map(PathBuf::from);
        Self { fake_root }
    }

    /// Whether the manager should avoid CoreText and operate against a fake registry root
    pub fn is_fake_registry_enabled(&self) -> bool {
        self.fake_root.is_some()
    }

    fn target_directory(&self, scope: FontScope) -> FontResult<PathBuf> {
        if let Some(root) = &self.fake_root {
            let dir = match scope {
                FontScope::User => root.join("Library/Fonts"),
                FontScope::System => root.join("System/Library/Fonts"),
            };
            return Ok(dir);
        }

        let target_dir = match scope {
            FontScope::User => {
                let home_dir = std::env::var("HOME").map_err(|_| {
                    FontError::PermissionDenied("Cannot determine home directory".to_string())
                })?;
                PathBuf::from(home_dir).join("Library/Fonts")
            }
            FontScope::System => PathBuf::from("/Library/Fonts"),
        };

        Ok(target_dir)
    }

    fn installed_target_path(
        &self,
        source: &FontliftFontSource,
        scope: FontScope,
    ) -> FontResult<PathBuf> {
        let file_name = source.path.file_name().ok_or_else(|| {
            FontError::InvalidFormat("Font path must include a file name".to_string())
        })?;

        Ok(self.target_directory(scope)?.join(file_name))
    }

    /// Extract font information using basic filename parsing as fallback
    fn get_font_info_from_path(&self, path: &Path) -> FontResult<FontliftFontFaceInfo> {
        validation::validate_font_file(path)?;

        let mut info = validation::extract_basic_info_from_path(path);
        info.source.scope = Some(scope_from_path(path));
        Ok(info)
    }

    /// Check if path is in system font directory
    fn is_system_font_path(&self, path: &Path) -> bool {
        protection::is_protected_system_font_path(path)
    }

    /// Check if current user has admin privileges
    fn has_admin_privileges(&self) -> bool {
        unsafe { libc::geteuid() == 0 }
    }

    /// Copy font to target directory based on scope
    fn copy_font_to_target_directory(
        &self,
        source_path: &Path,
        scope: FontScope,
        replace_existing: bool,
    ) -> FontResult<PathBuf> {
        let target_dir = self.target_directory(scope)?;
        let file_name = source_path.file_name().ok_or_else(|| {
            FontError::InvalidFormat("Font path must include a file name".to_string())
        })?;

        if scope == FontScope::System
            && !self.is_fake_registry_enabled()
            && !self.has_admin_privileges()
        {
            return Err(FontError::PermissionDenied(
                "System-level font installation requires administrator privileges. Run with --admin or use sudo.".to_string(),
            ));
        }

        // Create target directory if it doesn't exist
        if !target_dir.exists() {
            fs::create_dir_all(&target_dir).map_err(FontError::IoError)?;
        }

        // Check if font already exists in target location
        let target_path = target_dir.join(file_name);
        if target_path.exists() {
            if replace_existing {
                fs::remove_file(&target_path).map_err(FontError::IoError)?;
            } else {
                return Err(FontError::AlreadyInstalled(target_path));
            }
        }

        // Copy font file
        fs::copy(source_path, &target_path).map_err(FontError::IoError)?;

        Ok(target_path)
    }

    fn install_font_core_text(&self, path: &Path, scope: FontScope) -> FontResult<()> {
        // Validate the font prior to registration
        validation::validate_font_file(path)?;

        // Convert path to CFURL for Core Text
        let cf_url = match CFURL::from_path(path, false) {
            Some(url) => url,
            None => {
                return Err(FontError::InvalidFormat(format!(
                    "Cannot create CFURL from path: {}",
                    path.display()
                )))
            }
        };

        let mut error: CFErrorRef = std::ptr::null_mut();
        let result = unsafe {
            CTFontManagerRegisterFontsForURL(
                cf_url.as_concrete_TypeRef(),
                ct_scope(scope),
                &mut error,
            )
        };

        if result {
            return Ok(());
        }

        if error.is_null() {
            return Err(FontError::RegistrationFailed(format!(
                "Core Text failed to register font {}",
                path.display()
            )));
        }

        let conflict_error = unsafe { CFError::wrap_under_get_rule(error) };
        if is_conflict_error(&conflict_error) {
            let mut unregister_error: CFErrorRef = std::ptr::null_mut();
            let unregistered = unsafe {
                CTFontManagerUnregisterFontsForURL(
                    cf_url.as_concrete_TypeRef(),
                    ct_scope(scope),
                    &mut unregister_error,
                )
            };

            if !unregistered {
                return Err(FontError::RegistrationFailed(format!(
                    "Existing font conflict could not be resolved for {}: {}",
                    path.display(),
                    cf_error_to_string(unregister_error)
                )));
            }

            let mut retry_error: CFErrorRef = std::ptr::null_mut();
            let retry = unsafe {
                CTFontManagerRegisterFontsForURL(
                    cf_url.as_concrete_TypeRef(),
                    ct_scope(scope),
                    &mut retry_error,
                )
            };

            if retry {
                return Ok(());
            }

            return Err(FontError::RegistrationFailed(format!(
                "Core Text failed to register font {} after resolving conflict: {}",
                path.display(),
                cf_error_to_string(retry_error)
            )));
        }

        Err(FontError::RegistrationFailed(format!(
            "Core Text failed to register font {}: {}",
            path.display(),
            conflict_error.description()
        )))
    }

    fn install_font_fake(&self, source: &FontliftFontSource, scope: FontScope) -> FontResult<()> {
        let path = &source.path;
        self.copy_font_to_target_directory(path, scope, true)?;
        Ok(())
    }

    fn uninstall_font_fake(&self, source: &FontliftFontSource, scope: FontScope) -> FontResult<()> {
        let target_path = self.installed_target_path(source, scope)?;
        if target_path.exists() {
            std::fs::remove_file(&target_path).map_err(FontError::IoError)?;
            Ok(())
        } else {
            Err(FontError::FontNotFound(target_path))
        }
    }

    #[allow(dead_code)]
    fn list_installed_fonts_fake(&self) -> FontResult<Vec<FontliftFontFaceInfo>> {
        let mut fonts = Vec::new();

        for scope in [FontScope::User, FontScope::System] {
            let dir = self.target_directory(scope)?;
            if !dir.exists() {
                continue;
            }

            for entry in fs::read_dir(&dir).map_err(FontError::IoError)? {
                let entry = entry.map_err(FontError::IoError)?;
                let path = entry.path();

                if !validation::is_valid_font_extension(&path) {
                    continue;
                }

                match self.get_font_info_from_path(&path) {
                    Ok(font) => fonts.push(font.with_scope(Some(scope))),
                    Err(_) => continue,
                }
            }
        }

        Ok(protection::dedupe_fonts(fonts))
    }

    /// Validate system operation permissions
    fn validate_system_operation(&self, scope: FontScope) -> FontResult<()> {
        if scope == FontScope::System
            && !self.is_fake_registry_enabled()
            && !self.has_admin_privileges()
        {
            return Err(FontError::PermissionDenied(
                "System-level font operations require administrator privileges. Run with --admin or use sudo.".to_string()
            ));
        }
        Ok(())
    }
}

impl Default for MacFontManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FontManager for MacFontManager {
    fn install_font(&self, source: &FontliftFontSource) -> FontResult<()> {
        let scope = source.scope.unwrap_or(FontScope::User);
        let path = &source.path;
        // Validate inputs
        validation::validate_font_file(path)?;
        self.validate_system_operation(scope)?;

        if self.is_system_font_path(path) && !self.is_fake_registry_enabled() {
            return Err(FontError::SystemFontProtection(path.to_path_buf()));
        }

        let target_path = self.installed_target_path(source, scope)?;
        let replace_existing = self.is_fake_registry_enabled() || scope == FontScope::User;

        if self.is_fake_registry_enabled() {
            return self.install_font_fake(source, scope);
        }

        let (target_path, created_copy) = if target_path == *path {
            (target_path, false)
        } else {
            (
                self.copy_font_to_target_directory(path, scope, replace_existing)?,
                true,
            )
        };

        let result = self.install_font_core_text(&target_path, scope);
        if result.is_err() && created_copy {
            let _ = fs::remove_file(&target_path);
        }
        result
    }

    fn uninstall_font(&self, source: &FontliftFontSource) -> FontResult<()> {
        let scope = source.scope.unwrap_or(FontScope::User);
        self.validate_system_operation(scope)?;

        let target_path = self.installed_target_path(source, scope)?;

        if self.is_fake_registry_enabled() {
            return self.uninstall_font_fake(source, scope);
        }

        if !target_path.exists() {
            return Err(FontError::FontNotFound(target_path));
        }

        // Convert path to CFURL for Core Text
        let cf_url = match CFURL::from_path(&target_path, false) {
            Some(url) => url,
            None => {
                return Err(FontError::InvalidFormat(format!(
                    "Cannot create CFURL from path: {}",
                    target_path.display()
                )))
            }
        };

        let mut error: CFErrorRef = std::ptr::null_mut();
        let result = unsafe {
            CTFontManagerUnregisterFontsForURL(
                cf_url.as_concrete_TypeRef(),
                ct_scope(scope),
                &mut error,
            )
        };

        if result {
            Ok(())
        } else {
            let message = cf_error_to_string(error);
            Err(FontError::RegistrationFailed(format!(
                "Core Text failed to unregister font {}: {}",
                target_path.display(),
                message
            )))
        }
    }

    fn remove_font(&self, source: &FontliftFontSource) -> FontResult<()> {
        let scope = source.scope.unwrap_or(FontScope::User);
        let target_path = self.installed_target_path(source, scope)?;
        let installed_source = FontliftFontSource::new(target_path.clone()).with_scope(Some(scope));

        self.uninstall_font(&installed_source)?;

        if self.is_system_font_path(&target_path) && !self.is_fake_registry_enabled() {
            return Err(FontError::SystemFontProtection(target_path));
        }

        if target_path.exists() {
            std::fs::remove_file(&target_path).map_err(FontError::IoError)?;
        }

        Ok(())
    }

    fn is_font_installed(&self, source: &FontliftFontSource) -> FontResult<bool> {
        let scope = source.scope.unwrap_or(FontScope::User);
        let target_path = self.installed_target_path(source, scope)?;

        if self.is_fake_registry_enabled() {
            return Ok(target_path.exists());
        }

        if target_path.exists() {
            return Ok(true);
        }

        let font_urls = unsafe { ct_font_manager::CTFontManagerCopyAvailableFontURLs() };
        if font_urls.is_null() {
            return Ok(false);
        }

        let font_array: CFArray<CFURL> = unsafe { CFArray::wrap_under_get_rule(font_urls) };
        let normalized_target = normalize_path(&target_path);

        for i in 0..font_array.len() {
            if let Some(cf_url) = font_array.get(i) {
                if let Some(path) = cf_url.to_path() {
                    if normalize_path(&path) == normalized_target {
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    fn list_installed_fonts(&self) -> FontResult<Vec<FontliftFontFaceInfo>> {
        if self.is_fake_registry_enabled() {
            return self.list_installed_fonts_fake();
        }
        // Get all available font URLs from Core Text
        let font_urls = unsafe { ct_font_manager::CTFontManagerCopyAvailableFontURLs() };

        if font_urls.is_null() {
            return Ok(Vec::new());
        }

        let font_array: CFArray<CFURL> = unsafe { CFArray::wrap_under_get_rule(font_urls) };
        let mut fonts = Vec::new();

        for i in 0..font_array.len() {
            if let Some(cf_url) = font_array.get(i) {
                // Try to pull rich metadata via font descriptors first
                let descriptors = unsafe {
                    ct_font_manager::CTFontManagerCreateFontDescriptorsFromURL(
                        cf_url.as_concrete_TypeRef(),
                    )
                };

                if !descriptors.is_null() {
                    let descriptor_array: CFArray<CTFontDescriptor> =
                        unsafe { CFArray::wrap_under_create_rule(descriptors) };

                    for idx in 0..descriptor_array.len() {
                        if let Some(descriptor) = descriptor_array.get(idx) {
                            if let Some(info) = descriptor_to_font_face_info(&descriptor) {
                                fonts.push(info);
                                continue;
                            }
                        }
                    }
                }

                // Fallback: basic info from path
                if let Some(path) = cf_url.to_path() {
                    if !path.exists() || !validation::is_valid_font_extension(&path) {
                        continue;
                    }

                    match self.get_font_info_from_path(&path) {
                        Ok(mut font_info) => {
                            font_info.source.scope = Some(scope_from_path(&path));
                            fonts.push(font_info);
                        }
                        Err(_) => {
                            // Skip fonts we can't read, but don't fail the entire operation
                            continue;
                        }
                    }
                }
            }
        }

        Ok(protection::dedupe_fonts(fonts))
    }

    fn prune_missing_fonts(&self, scope: FontScope) -> FontResult<usize> {
        if self.is_fake_registry_enabled() {
            return Ok(0);
        }

        let font_urls = unsafe { ct_font_manager::CTFontManagerCopyAvailableFontURLs() };

        if font_urls.is_null() {
            return Ok(0);
        }

        let font_array: CFArray<CFURL> = unsafe { CFArray::wrap_under_get_rule(font_urls) };
        let mut pruned = 0usize;
        let mut failures = Vec::new();

        for i in 0..font_array.len() {
            if let Some(cf_url) = font_array.get(i) {
                let path = cf_url.to_path();

                if let Some(existing_path) = path.as_ref() {
                    if scope_from_path(existing_path) != scope {
                        continue;
                    }

                    // Skip registrations that still have a backing file
                    if existing_path.exists() {
                        continue;
                    }
                } else if scope == FontScope::System && !self.has_admin_privileges() {
                    // Don't attempt system pruning without privileges
                    continue;
                }

                let mut error: CFErrorRef = std::ptr::null_mut();
                let ok = unsafe {
                    CTFontManagerUnregisterFontsForURL(
                        cf_url.as_concrete_TypeRef(),
                        ct_scope(scope),
                        &mut error,
                    )
                };

                if ok {
                    pruned += 1;
                } else {
                    failures.push(cf_error_to_string(error));
                }
            }
        }

        if failures.is_empty() {
            Ok(pruned)
        } else {
            Err(FontError::RegistrationFailed(format!(
                "Failed to prune some font registrations: {}",
                failures.join("; ")
            )))
        }
    }

    fn clear_font_caches(&self, scope: FontScope) -> FontResult<()> {
        if self.is_fake_registry_enabled() {
            return Ok(());
        }

        let test_root = test_cache_root();
        let home = user_home(&test_root)?;
        let should_touch_system = test_root.is_none();

        match scope {
            FontScope::User => {
                if should_touch_system {
                    // Clear user font cache using atsutil
                    let output = std::process::Command::new("atsutil")
                        .args(["databases", "-removeUser"])
                        .output()
                        .map_err(FontError::IoError)?;

                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Err(FontError::RegistrationFailed(format!(
                            "Failed to clear user font cache: {}",
                            stderr
                        )));
                    }

                    // Restart ATS server for user session
                    let _ = std::process::Command::new("atsutil")
                        .args(["server", "-shutdown"])
                        .output();

                    let _ = std::process::Command::new("atsutil")
                        .args(["server", "-ping"])
                        .output();
                }

                // Vendor caches (Adobe/Microsoft) are per-user; remove safely under the resolved home dir
                clear_adobe_font_caches(&home)?;
                clear_office_font_cache(&home)?;
            }
            FontScope::System => {
                if should_touch_system {
                    // System cache clearing requires admin privileges
                    if !self.has_admin_privileges() {
                        return Err(FontError::PermissionDenied(
                            "System cache clearing requires administrator privileges".to_string(),
                        ));
                    }

                    // Clear system font cache using atsutil
                    let output = std::process::Command::new("atsutil")
                        .args(["databases", "-remove"])
                        .output()
                        .map_err(FontError::IoError)?;

                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Err(FontError::RegistrationFailed(format!(
                            "Failed to clear system font cache: {}",
                            stderr
                        )));
                    }

                    // Restart ATS server for system
                    let _ = std::process::Command::new("atsutil")
                        .args(["server", "-shutdown"])
                        .output();

                    let _ = std::process::Command::new("atsutil")
                        .args(["server", "-ping"])
                        .output();
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_foundation::url::CFURL;
    use core_text::font_descriptor as ct_font_descriptor;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};

    fn fake_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn test_mac_font_manager_creation() {
        std::env::remove_var("FONTLIFT_FAKE_REGISTRY_ROOT");
        let manager = MacFontManager::new();
        assert!(!manager.is_fake_registry_enabled());
    }

    #[test]
    fn test_system_font_detection() {
        let manager = MacFontManager::new();

        // Test system font paths
        let system_path = PathBuf::from("/System/Library/Fonts/Arial.ttf");
        assert!(manager.is_system_font_path(&system_path));

        let library_path = PathBuf::from("/Library/Fonts/Helvetica.ttc");
        assert!(manager.is_system_font_path(&library_path));

        // Test user font paths
        let user_path = PathBuf::from("/Users/test/Library/Fonts/Custom.otf");
        assert!(!manager.is_system_font_path(&user_path));

        let temp_path = PathBuf::from("/tmp/test.ttf");
        assert!(!manager.is_system_font_path(&temp_path));
    }

    #[test]
    fn test_admin_detection() {
        let manager = MacFontManager::new();
        // This test will typically fail unless run as root
        let is_admin = manager.has_admin_privileges();
        // We can't assert a specific value as it depends on execution context
        println!("Running as admin: {}", is_admin);
    }

    #[test]
    fn test_font_validation() {
        let _manager = MacFontManager::new();

        // Test valid font file paths (these may not exist, just testing validation logic)
        let valid_path = PathBuf::from("/tmp/test.ttf");
        if validation::is_valid_font_extension(&valid_path) {
            // Test would pass if file existed
        }

        let invalid_path = PathBuf::from("/tmp/test.txt");
        assert!(!validation::is_valid_font_extension(&invalid_path));
    }

    #[test]
    fn descriptor_metadata_is_preferred_over_filename() {
        let path = PathBuf::from("/Library/Fonts/TestSans-Bold.ttf");
        let cf_url = CFURL::from_path(&path, false).expect("url");

        let name_key =
            unsafe { CFString::wrap_under_get_rule(ct_font_descriptor::kCTFontNameAttribute) };
        let display_key = unsafe {
            CFString::wrap_under_get_rule(ct_font_descriptor::kCTFontDisplayNameAttribute)
        };
        let family_key = unsafe {
            CFString::wrap_under_get_rule(ct_font_descriptor::kCTFontFamilyNameAttribute)
        };
        let style_key =
            unsafe { CFString::wrap_under_get_rule(ct_font_descriptor::kCTFontStyleNameAttribute) };
        let url_key =
            unsafe { CFString::wrap_under_get_rule(ct_font_descriptor::kCTFontURLAttribute) };
        let format_key =
            unsafe { CFString::wrap_under_get_rule(ct_font_descriptor::kCTFontFormatAttribute) };
        let traits_key =
            unsafe { CFString::wrap_under_get_rule(ct_font_descriptor::kCTFontTraitsAttribute) };
        let symbolic_key =
            unsafe { CFString::wrap_under_get_rule(ct_font_descriptor::kCTFontSymbolicTrait) };
        let weight_key =
            unsafe { CFString::wrap_under_get_rule(ct_font_descriptor::kCTFontWeightTrait) };

        let traits_dict: CFDictionary<CFString, core_foundation::base::CFType> =
            CFDictionary::from_CFType_pairs(&[
                (symbolic_key, CFNumber::from(0).as_CFType()),
                (weight_key, CFNumber::from(0).as_CFType()),
            ]);

        let attrs: CFDictionary<CFString, core_foundation::base::CFType> =
            CFDictionary::from_CFType_pairs(&[
                (url_key, cf_url.as_CFType()),
                (name_key, CFString::new("TestSans-Bold").as_CFType()),
                (display_key, CFString::new("Test Sans Bold").as_CFType()),
                (family_key, CFString::new("Test Sans").as_CFType()),
                (style_key, CFString::new("Bold").as_CFType()),
                (
                    format_key,
                    CFNumber::from(ct_font_descriptor::kCTFontFormatOpenTypeTrueType as i32)
                        .as_CFType(),
                ),
                (traits_key, traits_dict.as_CFType()),
            ]);

        let descriptor = ct_font_descriptor::new_from_attributes(&attrs);
        let info = descriptor_to_font_face_info(&descriptor).expect("font info");

        assert_eq!(info.postscript_name, "TestSans-Bold");
        assert_eq!(info.full_name, "Test Sans Bold");
        assert_eq!(info.family_name, "Test Sans");
        assert_eq!(info.style, "Bold");
        assert_eq!(info.source.format.as_deref(), Some("OpenTypeTrueType"));
        assert_eq!(info.source.scope, Some(FontScope::System));
    }

    #[test]
    fn scope_detection_maps_user_and_system_paths() {
        let user_path = PathBuf::from("/Users/demo/Library/Fonts/Custom.otf");
        let system_path = PathBuf::from("/Library/Fonts/SystemFont.ttf");
        let other_path = PathBuf::from("/tmp/random-font.ttf");

        assert_eq!(scope_from_path(&user_path), FontScope::User);
        assert_eq!(scope_from_path(&system_path), FontScope::System);
        assert_eq!(scope_from_path(&other_path), FontScope::User);
    }

    #[test]
    fn fake_registry_install_list_uninstall_round_trip() {
        let _env_lock = fake_env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let fake_root = temp.path().join("fake-root");
        std::env::set_var("FONTLIFT_FAKE_REGISTRY_ROOT", &fake_root);

        let manager = MacFontManager::new();
        let source_font = temp.path().join("DemoFake.ttf");
        fs::write(&source_font, b"dummy font").expect("write font");

        let source = FontliftFontSource::new(source_font.clone()).with_scope(Some(FontScope::User));

        manager
            .install_font(&source)
            .expect("install in fake registry");

        let installed_path = fake_root.join("Library/Fonts/DemoFake.ttf");
        assert!(
            installed_path.exists(),
            "font should be copied into fake registry"
        );

        let listed = manager.list_installed_fonts().expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].source.scope, Some(FontScope::User));

        let installed_source =
            FontliftFontSource::new(installed_path.clone()).with_scope(Some(FontScope::User));

        manager
            .uninstall_font(&installed_source)
            .expect("uninstall from fake registry");
        assert!(
            !installed_path.exists(),
            "font file should be removed in fake registry"
        );

        std::env::remove_var("FONTLIFT_FAKE_REGISTRY_ROOT");
    }

    #[test]
    fn fake_registry_allows_system_scope_without_admin() {
        let _env_lock = fake_env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let fake_root = temp.path().join("fake-root");
        std::env::set_var("FONTLIFT_FAKE_REGISTRY_ROOT", &fake_root);

        let manager = MacFontManager::new();
        let source_font = temp.path().join("DemoSystem.ttf");
        fs::write(&source_font, b"dummy font").expect("write font");

        let source =
            FontliftFontSource::new(source_font.clone()).with_scope(Some(FontScope::System));

        manager
            .install_font(&source)
            .expect("system install should bypass admin in fake mode");

        let installed_path = fake_root.join("System/Library/Fonts/DemoSystem.ttf");
        assert!(installed_path.exists());

        std::env::remove_var("FONTLIFT_FAKE_REGISTRY_ROOT");
    }

    #[test]
    fn is_font_installed_tracks_fake_registry_state() {
        struct EnvGuard;
        impl Drop for EnvGuard {
            fn drop(&mut self) {
                std::env::remove_var("FONTLIFT_FAKE_REGISTRY_ROOT");
            }
        }

        let _lock = fake_env_lock().lock().expect("env lock");
        let _guard = EnvGuard;
        let temp = tempfile::tempdir().expect("tempdir");
        let fake_root = temp.path().join("fake-root");
        std::env::set_var("FONTLIFT_FAKE_REGISTRY_ROOT", &fake_root);

        let manager = MacFontManager::new();
        let source_font = temp.path().join("DemoFake.ttf");
        fs::write(&source_font, b"dummy font").expect("write font");

        let source = FontliftFontSource::new(source_font.clone()).with_scope(Some(FontScope::User));

        assert!(
            !manager
                .is_font_installed(&source)
                .expect("check before install"),
            "font should not be marked installed before copying"
        );

        manager
            .install_font(&source)
            .expect("install in fake registry");
        assert!(
            manager
                .is_font_installed(&source)
                .expect("check after install"),
            "font should be marked installed after copy"
        );

        let installed_source =
            FontliftFontSource::new(fake_root.join("Library/Fonts/DemoFake.ttf"))
                .with_scope(Some(FontScope::User));

        manager
            .uninstall_font(&installed_source)
            .expect("uninstall from fake registry");

        assert!(
            !manager
                .is_font_installed(&source)
                .expect("check after uninstall"),
            "font should be absent after uninstall"
        );
    }

    #[test]
    fn reinstall_overwrites_existing_file_in_fake_registry() {
        struct EnvGuard;
        impl Drop for EnvGuard {
            fn drop(&mut self) {
                std::env::remove_var("FONTLIFT_FAKE_REGISTRY_ROOT");
            }
        }

        let _lock = fake_env_lock().lock().expect("env lock");
        let _guard = EnvGuard;
        let temp = tempfile::tempdir().expect("tempdir");
        let fake_root = temp.path().join("fake-root");
        std::env::set_var("FONTLIFT_FAKE_REGISTRY_ROOT", &fake_root);

        let manager = MacFontManager::new();
        let source_font = temp.path().join("DemoFake.ttf");

        fs::write(&source_font, b"version-one").expect("write v1");
        let source = FontliftFontSource::new(source_font.clone()).with_scope(Some(FontScope::User));
        manager.install_font(&source).expect("initial install");

        fs::write(&source_font, b"version-two").expect("write v2");
        manager
            .install_font(&source)
            .expect("reinstall should replace");

        let installed_path = fake_root.join("Library/Fonts/DemoFake.ttf");
        let contents = fs::read(&installed_path).expect("read installed copy");
        assert_eq!(
            contents, b"version-two",
            "reinstall should replace the existing file in fake registry"
        );
    }

    #[test]
    fn clear_font_caches_removes_vendor_caches_under_override_root() {
        use std::env;

        struct EnvGuard;
        impl Drop for EnvGuard {
            fn drop(&mut self) {
                env::remove_var("FONTLIFT_FAKE_REGISTRY_ROOT");
                env::remove_var("FONTLIFT_TEST_CACHE_ROOT");
            }
        }

        let _lock = fake_env_lock().lock().expect("env lock");
        let _guard = EnvGuard;
        env::remove_var("FONTLIFT_FAKE_REGISTRY_ROOT");

        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();

        let adobe_type_support = root.join("Library/Application Support/Adobe/TypeSupport/More");
        fs::create_dir_all(&adobe_type_support).expect("adobe type support dir");
        let adobe_list = adobe_type_support.join("AdobeFnt11.lst");
        fs::write(&adobe_list, b"cache").expect("adobe list");

        let adobe_cache_dir = root.join("Library/Caches/Adobe/Fonts");
        fs::create_dir_all(&adobe_cache_dir).expect("adobe cache dir");
        let adobe_cache_file = adobe_cache_dir.join("fonts.bin");
        fs::write(&adobe_cache_file, b"cache").expect("adobe cache");

        let office_cache_dir = root.join("Library/Group Containers/UBF8T346G9.Office/FontCache");
        fs::create_dir_all(&office_cache_dir).expect("office cache dir");
        let office_cache_file = office_cache_dir.join("fontcache.dat");
        fs::write(&office_cache_file, b"cache").expect("office cache");

        env::set_var("FONTLIFT_TEST_CACHE_ROOT", root);
        let manager = MacFontManager::new();
        manager
            .clear_font_caches(FontScope::User)
            .expect("clear caches");

        assert!(
            !adobe_list.exists(),
            "Adobe font list cache should be removed"
        );
        assert!(
            !adobe_cache_file.exists(),
            "Adobe font cache file should be removed"
        );
        assert!(
            fs::read_dir(&office_cache_dir)
                .expect("office dir")
                .next()
                .is_none(),
            "Office font cache directory should be emptied"
        );
    }
}
