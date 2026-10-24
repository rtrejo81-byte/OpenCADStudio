use super::*;
use crate::command::{CadCommand, CmdResult, InputKind};

pub(crate) struct NumeraCommand;

impl NumeraCommand {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl CadCommand for NumeraCommand {
    fn name(&self) -> &'static str {
        "NUMERA"
    }

    fn prompt(&self) -> String {
        "NUMERA  Especifique el punto:".to_string()
    }

    fn input_kind(&self) -> InputKind {
        InputKind::Point
    }

    fn on_point(&mut self, _pt: glam::DVec3) -> CmdResult {
        // No podemos crear la entidad aquí, así que cancelamos y dejamos
        // que view.rs se encargue de todo con el comando TEXT.
        CmdResult::Cancel
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn on_text_input(&mut self, _text: &str) -> Option<CmdResult> {
        None
    }
}