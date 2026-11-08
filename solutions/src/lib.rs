//! Réponses en code aux « pour aller plus loin » du fil rouge, et variantes simples des
//! trous.
//!
//! Le corrigé est lui-même la solution des trous : `cargo xtask solve <n>` la remet dans
//! le dossier de travail. Ce crate contient ce qu'il ne montre pas — les extensions, et
//! les autres façons d'écrire un même trou. Tout y est compilé et testé contre le
//! corrigé, pour ne pas se périmer en silence. L'index est dans `README.md`.

#![warn(missing_docs)]

pub mod render;
pub mod simple;
pub mod stream;
