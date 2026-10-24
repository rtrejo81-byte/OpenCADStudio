use super::*;
use crate::command::{CadCommand, CmdResult, InputKind};

pub(crate) struct SumaNumCommand {
    selected: Vec<codec::Handle>,
}

impl SumaNumCommand {
    pub(crate) fn new() -> Self {
        Self {
            selected: Vec::new(),
        }
    }
}

impl CadCommand for SumaNumCommand {
    fn name(&self) -> &'static str {
        "SUMANUM"
    }

    fn prompt(&self) -> String {
        if self.selected.is_empty() {
            "SUMANUM  Seleccione los textos a sumar:".to_string()
        } else {
            format!("SUMANUM  Seleccione más textos o presione Enter para sumar ({} seleccionados):", self.selected.len())
        }
    }

    fn input_kind(&self) -> InputKind {
        InputKind::Point
    }

    fn is_selection_gathering(&self) -> bool {
        true
    }

    fn on_selection_complete(&mut self, handles: Vec<codec::Handle>) -> CmdResult {
        self.selected = handles;
        CmdResult::NeedPoint
    }

    fn on_enter(&mut self) -> CmdResult {
        // Aquí es donde hacemos la magia: procesamos los textos seleccionados.
        if self.selected.is_empty() {
            CmdResult::Cancel
        } else {
            CmdResult::Relaunch("SUMANUM_EXECUTE".to_string(), self.selected.clone())
        }
    }

    fn on_entity_pick(&mut self, _handle: codec::Handle, _pt: glam::DVec3) -> CmdResult {
        CmdResult::NeedPoint
    }

    fn on_point(&mut self, _pt: glam::DVec3) -> CmdResult {
        CmdResult::NeedPoint
    }
}