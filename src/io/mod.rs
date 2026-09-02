//! Écriture des résultats.
//!
//! Deux sorties, pour deux usages : le VTK s'ouvre dans Paraview ou Tecplot et se
//! branche sur une chaîne de post-traitement existante ; le PNG ne demande rien à
//! personne et donne une image à regarder tout de suite.

pub mod png;
pub mod vtk;
