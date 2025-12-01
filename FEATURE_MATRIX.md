# FontLift Feature Matrix

This document compares the feature set between existing Swift/macOS and C++/Windows implementations with the target unified Rust implementation.

## Existing Implementations Analysis

### Swift/macOS CLI (fontlift-mac-cli)
**Version**: 2.0.0 (production ready)
**Source**: `Sources/fontlift/fontlift.swift` (1,344 lines)

**Core Features**:
- ✅ Font installation (user/system scope)
- ✅ Font uninstallation (user/system scope)
- ✅ Font removal (uninstall + delete)
- ✅ Font listing with multiple output modes
- ✅ Cache cleanup (user/system + third-party)
- ✅ Conflict detection and auto-resolution
- ✅ Safety protections for system fonts
- ✅ Comprehensive file validation
- ✅ Font metadata extraction (PostScript, family names)
- ✅ Shell-safe path handling
- ✅ Fake registration mode for testing

**Advanced Features**:
- ✅ Third-party cache clearing (Adobe, Microsoft Office)
- ✅ Missing font registration pruning
- ✅ Admin privilege handling
- ✅ Comprehensive error messages
- ✅ Simulated mode for testing
- ✅ Environment variable overrides
- ✅ Persistent fake registry

### Windows CLI (fontlift-win-cli)
**Version**: Based on C++ implementation
**Source**: `src/main.cpp` (complete implementation)

**Core Features**:
- ✅ Font installation via Windows Registry
- ✅ Font uninstallation
- ✅ Font listing
- ✅ Registry-based font management
- ✅ Windows GDI integration
- ✅ Admin privilege support

**Platform-Specific Features**:
- ✅ Windows Registry manipulation
- ✅ GDI font notifications
- ✅ Windows font directory management
- ✅ Permission handling

## Target Unified Rust Implementation

### Phase 1 Target Features (MVP Parity)
**Must Have** (Feature Parity with Existing CLIs):
- [ ] Font installation (user/system scope)
- [ ] Font uninstallation (user/system scope)
- [ ] Font removal (uninstall + delete)
- [ ] Font listing (paths, names, combined)
- [ ] Basic cache cleanup
- [ ] File validation (extension, existence, readability)
- [ ] System font protection
- [ ] Font metadata extraction
- [ ] Error handling and reporting

**Platform Implementation**:
- [ ] macOS: Core Text APIs
- [ ] Windows: Registry + GDI APIs
- [ ] Cross-platform: Trait-based abstractions

### Phase 2 Target Features (Enhanced)
**Should Have** (Quality Improvements):
- [ ] Advanced cache cleanup (third-party apps)
- [ ] Missing font registration pruning
- [ ] Conflict detection and auto-resolution
- [ ] Improved error messages and guidance
- [ ] Test mode/simulation
- [ ] Configuration file support
- [ ] Better privilege handling

**Could Have** (Nice to Have):
- [ ] Font collection support (.ttc/.otc)
- [ ] Batch operations
- [ ] Progress indicators
- [ ] JSON output mode
- [ ] Font validation reports
- [ ] Integration with fontsimi/typf

## Implementation Status Matrix

| Feature | Swift/macOS | Windows CLI | Rust Core | Rust macOS | Rust Windows | Status |
|---------|-------------|-------------|-----------|------------|--------------|---------|
| **Core Operations** |
| Font Install | ✅ | ✅ | 📝 | 📝 | 📝 | 🏗️ In Progress |
| Font Uninstall | ✅ | ✅ | 📝 | 📝 | 📝 | 🏗️ In Progress |
| Font Remove | ✅ | ✅ | 📝 | 📝 | 📝 | 🏗️ In Progress |
| Font List | ✅ | ✅ | 📝 | 📝 | 📝 | 🏗️ In Progress |
| **Platform Integration** |
| Core Text (macOS) | ✅ | N/A | 📝 | 📝 | N/A | 🏗️ In Progress |
| Registry/GDI (Win) | N/A | ✅ | 📝 | N/A | 📝 | 🏗️ In Progress |
| **Advanced Features** |
| Cache Cleanup | ✅ | ⚠️ | 📋 | 📋 | 📋 | 📋 Planned |
| System Font Protection | ✅ | ⚠️ | ✅ | 📋 | 📋 | ✅ Core Done |
| Conflict Detection | ✅ | ❌ | 📋 | 📋 | 📋 | 📋 Planned |
| Simulation Mode | ✅ | ❌ | 📋 | 📋 | 📋 | 📋 Planned |
| **Quality Features** |
| Error Handling | ✅ | ⚠️ | ✅ | 📝 | 📝 | ✅ Core Done |
| CLI Interface | ✅ | ✅ | 📝 | 📋 | 📋 | 📝 Basic Done |
| File Validation | ✅ | ⚠️ | ✅ | 📝 | 📝 | ✅ Core Done |

## Legacy CLI Parity Checklist (commands & flags)

| Item | Swift/macOS CLI | Windows CLI | Rust CLI (current) | Gap / Notes |
|------|-----------------|-------------|--------------------|-------------|
| Commands present | `list`, `install`, `uninstall`, `remove`, `cleanup` | same | same | core surface matches |
| Aliases | `l`, `i`, `u`, `rm`; cleanup exposed as `cleanup` only | `l`, `i`, `u`, `rm`, `c` | none | add aliases for parity |
| Path flag (`-p/--path`) | yes (list/install/uninstall/remove) | yes (list/install/uninstall/remove) | list only; other commands take positional path | add `-p/--path` for operations |
| Name flag (`-n/--name`) | list/uninstall/remove support | list/uninstall/remove support | uninstall/remove; list supports | align across commands |
| Sorted flag (`-s/--sorted`) | yes (list) | yes (list) | yes (list) | parity achieved |
| Scope flag (`--admin/-a`) | install/uninstall/remove/cleanup | all commands | all commands | parity achieved |
| Cleanup toggles | `--prune-only`, `--cache-only`; clears Adobe/Microsoft caches | none; cleanup always prunes + clears caches (user, `--admin` for system) | none; simple cache clear | add prune/cache toggles + third-party cache handling |
| Conflict handling | detection + auto-resolve; fake registry mode for tests | auto-removes existing family on install | basic validation only | add detection/auto-resolve + fake registry hooks |
| Batch/collection handling | `.ttc/.otc`; docs encourage directory loops | `.ttc/.otc`; no directory helper | `.ttc/.otc` accepted; no directory helper | add batch file/dir handling |
| Output modes | path, name, `path::name`; shell-safe escaping | path, name, both; sorted option | path default; optional name + sorted | add deterministic combined output + escaping |
| Exit codes | `0` success, `1` failure | `0` success, `1` error, `2` permission denied | `0` success, `1` error | add permission-denied exit code |
| Simulation/dry-run | env-driven fake registry + dry-run guidance | none | none | add dry-run/simulation hooks |

**Legend**:
- ✅ Complete/Implemented
- 📝 In Progress/Partially Implemented
- 📋 Planned/Designed
- ❌ Not Available
- ⚠️ Limited Implementation
- N/A Not Applicable

## Success Metrics

### Functional Metrics
- ✅ **API Parity**: 100% of core operations available across platforms
- ✅ **Feature Coverage**: All essential features from existing CLIs implemented
- ✅ **Cross-Platform**: Identical behavior on macOS and Windows
- ✅ **Integration**: Seamless integration with typf/testypf ecosystems

### Quality Metrics
- ✅ **Test Coverage**: >90% test coverage for all functionality
- ✅ **Error Handling**: Comprehensive error messages with actionable guidance
- ✅ **Performance**: Operations complete within acceptable timeframes
- ✅ **Safety**: No accidental system font modifications
- ✅ **Documentation**: Complete API and usage documentation

### Integration Metrics
- ✅ **CLI Consistency**: Same command structure across platforms
- ✅ **Library Integration**: Clean Rust API for other projects
- ✅ **Bindings Support**: Python bindings ready for fontsimi
- ✅ **Future Compatibility**: Extensible architecture for new platforms

## Implementation Roadmap

### Phase 0: Alignment ✅ COMPLETE
- [x] Analyze existing implementations
- [x] Create feature matrix
- [x] Define success metrics
- [x] Document architectural decisions

### Phase 1: Architectural Foundations (Current)
- [x] Design crate structure
- [x] Specify FontManager trait
- [x] Define error handling strategy
- [ ] Implement core validation utilities
- [ ] Complete platform-specific implementations
- [ ] Basic CLI with essential commands

### Phase 2: Feature Completion
- [ ] Complete all core operations
- [ ] Implement advanced cache management
- [ ] Add conflict detection
- [ ] Enhance error messages
- [ ] Add simulation/testing mode

### Phase 3: Integration & Polish
- [ ] Python bindings integration
- [ ] CLI feature completeness
- [ ] Documentation completion
- [ ] Performance optimization
- [ ] Cross-platform testing

### Phase 4: Production Ready
- [ ] Comprehensive test suite
- [ ] Release documentation
- [ ] Integration testing with typf/testypf
- [ ] Performance benchmarks
- [ ] Security audit

---

**Status**: Phase 0 Complete, Phase 1 In Progress
**Next**: Complete core platform implementations and CLI functionality
