# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Fix `ansi-to-tui` git dependency URL (moved to `ratatui/ansi-to-tui`), which broke `cargo install lazyjj` and source builds

### Added

- Details panel responds to mouse scroll in all tabs
- Details panel sets COLUMNS to allow jj diff tool to fit window
