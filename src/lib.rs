//! Soufflerie numérique : transport d'un traceur passif autour d'un obstacle,
//! en volumes finis sur un maillage non structuré.
//!
//! Le projet est construit étape par étape pendant la formation ; voir `ETAPES.md`
//! et `docs/etapes/`. L'organisation suit le chemin de la donnée :
//!
//! ```text
//! masque ASCII ──▶ [mask] ──▶ [mesh] ──▶ [field] ──▶ [solver] ──▶ [io] ──▶ PNG / VTK
//!                                          ▲            ▲
//!                                    [velocity]      [flux]
//! ```

#![warn(missing_docs)]

pub mod app;
pub mod error;
pub mod field;
pub mod flux;
pub mod geom;
pub mod gradient;
pub mod io;
pub mod mask;
pub mod mesh;
pub mod solver;
pub mod velocity;
