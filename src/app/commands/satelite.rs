//! `SATELITE` — inserta una imagen satelital georreferenciada.
//!
//! Se marcan dos esquinas del área y se inserta la imagen de Esri World
//! Imagery ya reproyectada a UTM 16N, en la capa `SATELITE`, semitransparente
//! para poder dibujar encima. Como la imagen cae en las coordenadas del
//! plano, todo lo que se dibuje sobre ella queda georreferenciado.

use glam::DVec3;

use crate::command::{CadCommand, CmdResult, InputKind};

pub(crate) struct SateliteCommand {
    primera: Option<DVec3>,
}

impl SateliteCommand {
    pub(crate) fn new() -> Self {
        Self { primera: None }
    }
}

fn ordenar(a: f64, b: f64) -> (f64, f64) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

impl CadCommand for SateliteCommand {
    fn name(&self) -> &'static str {
        "SATELITE"
    }

    fn prompt(&self) -> String {
        if self.primera.is_none() {
            "SATELITE  Esquina 1 del área a cubrir:".to_string()
        } else {
            "SATELITE  Esquina opuesta del área:".to_string()
        }
    }

    fn input_kind(&self) -> InputKind {
        InputKind::Point
    }

    fn on_point(&mut self, pt: DVec3) -> CmdResult {
        let Some(primera) = self.primera else {
            self.primera = Some(pt);
            return CmdResult::NeedPoint;
        };
        let (min_x, max_x) = ordenar(primera.x, pt.x);
        let (min_y, max_y) = ordenar(primera.y, pt.y);
        if (max_x - min_x) < 1.0 || (max_y - min_y) < 1.0 {
            return CmdResult::ReportError(
                "SATELITE  El área es demasiado pequeña; marque al menos un metro."
                    .to_string(),
            );
        }
        // La descarga la hace el manejador: un comando no puede bloquearse.
        CmdResult::Dispatch(format!(
            "SATELITE_FETCH {min_x:.4} {min_y:.4} {max_x:.4} {max_y:.4}"
        ))
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

// ── Autocomplete registry ───────────────────────────────────────────────────
inventory::submit!(crate::command::CommandRegistration {
    names: &["SATELITE", "BASEMAP"]
});
