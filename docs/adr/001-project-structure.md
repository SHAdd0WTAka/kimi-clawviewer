# ADR-001: Project Structure

## Status

Accepted

## Context

ClawViewer ist eine Rust/Tauri Desktop-App mit mehreren Crates. Eine klare Struktur ist für Enterprise-Entwicklung essentiell.

## Decision

- `project/` – Root für alle Build-relevanten Dateien
  - `src-tauri/` – Tauri-Backend (Rust)
  - `src-ui/` – React/TypeScript Frontend
  - `crates/` – Shared Rust Libraries
  - `Cargo.toml` – Workspace-Definition

## Consequences

- CI muss in `project/` arbeiten (`working-directory: ./project`)
- Alle Rust-Crates teilen eine `Cargo.lock`
- Frontend- und Backend-Code sind klar getrennt
