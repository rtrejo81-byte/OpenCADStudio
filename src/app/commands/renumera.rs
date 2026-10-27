//! `RENUMERA` — renumeración de NAPs por tecnología.
//!
//! Portación de la rutina LISP `FTTH_RENUMERA_NAP` v4.0 (TREJO):
//!
//! - Pide distrito, tecnología (HUAWEI / COMMSCOPE) y número inicial.
//! - Después se seleccionan los bloques **en el orden que quieras numerarlos**.
//! - Cada bloque cambia de definición: en HUAWEI los múltiplos de 4 pasan a
//!   `FAT-F` y el resto a `FAT-L`; en COMMSCOPE todos pasan a `NAP`.
//! - El atributo `NOMBRE` se actualiza a `DISTRITO-NAPxx` (dos cifras).
//! - El rango va de 1 a 64, igual que la rutina original.
//!
//! `RENUMERA_DISTRITO` cambia el distrito recordado, como
//! `FTTH_NAP_DISTRITO`.

use std::sync::Mutex;

use codec::{EntityType, Handle};
use glam::DVec3;

use crate::command::{CadCommand, CmdOption, CmdResult, InputKind};

/// Último número que admite la rutina (NAP 64).
const MAX_NAP: u32 = 64;

/// Distrito recordado entre ejecuciones, como el global `*FTTH-LAST-DISTRICT*`.
static DISTRITO: Mutex<String> = Mutex::new(String::new());

fn distrito_actual() -> String {
    let guard = DISTRITO.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if guard.trim().is_empty() {
        "SYP01".to_string()
    } else {
        guard.clone()
    }
}

fn fijar_distrito(valor: &str) {
    let mut guard = DISTRITO.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = valor.trim().to_string();
}

/// Bloque que corresponde a un número, según la regla de la tecnología.
pub(crate) fn bloque_destino(huawei: bool, numero: u32) -> &'static str {
    if huawei {
        if numero % 4 == 0 {
            "FAT-F"
        } else {
            "FAT-L"
        }
    } else {
        "NAP"
    }
}

/// `SYP01-NAP05`: el número se rellena a dos cifras como en la rutina.
pub(crate) fn nombre_nap(distrito: &str, numero: u32) -> String {
    format!("{distrito}-NAP{numero:02}")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Paso {
    Distrito,
    Tecnologia,
    Inicio,
    Bloques,
}

pub(crate) struct RenumeraCommand {
    paso: Paso,
    distrito: String,
    huawei: bool,
    siguiente: u32,
    /// Bloques definidos en el dibujo, en mayúsculas.
    disponibles: Vec<String>,
    /// Entidad que el host inyecta justo antes del pick.
    recogido: Option<EntityType>,
    /// Bloques ya elegidos, con el número que les toca. Se aplican todos al
    /// final: una sola instantánea de undo en vez de una por clic.
    cola: Vec<(Handle, EntityType, u32)>,
    /// Resumen del último bloque elegido, para mostrarlo en el prompt.
    ultimo: Option<String>,
}

impl RenumeraCommand {
    pub(crate) fn new(disponibles: Vec<String>) -> Self {
        Self {
            paso: Paso::Distrito,
            distrito: distrito_actual(),
            huawei: true,
            siguiente: 1,
            disponibles,
            recogido: None,
            cola: Vec::new(),
            ultimo: None,
        }
    }

    fn tiene_bloque(&self, nombre: &str) -> bool {
        self.disponibles
            .iter()
            .any(|bloque| bloque.eq_ignore_ascii_case(nombre))
    }

    /// Encola el bloque recogido con su número. La mutación del dibujo se
    /// difiere al final: cada clic aquí no guarda nada en el undo.
    fn aplicar(&mut self, handle: Handle) -> CmdResult {
        let Some(entity) = self.recogido.take() else {
            return CmdResult::ReportError("Seleccione un bloque.".to_string());
        };
        if !matches!(entity, EntityType::Insert(_)) {
            return CmdResult::ReportError("Ese objeto no es un bloque.".to_string());
        }
        let numero = self.siguiente;
        if numero > MAX_NAP {
            return CmdResult::ReportError(
                "Ya se alcanzó el NAP 64. Presione Enter para aplicar.".to_string(),
            );
        }
        let destino = bloque_destino(self.huawei, numero);
        if !self.tiene_bloque(destino) {
            return CmdResult::ReportError(format!(
                "El bloque '{destino}' no existe en el dibujo."
            ));
        }
        self.ultimo = Some(format!(
            "en cola NAP{numero:02} → {destino} · {}",
            nombre_nap(&self.distrito, numero)
        ));
        self.cola.push((handle, entity, numero));
        self.siguiente += 1;
        CmdResult::NeedPoint
    }

    /// Entrega la cola al manejador, que escribe los cambios directamente en
    /// el documento en un solo paso de undo. Se codifica el estado en el
    /// propio comando para no perderlo al cerrar este comando interactivo.
    fn aplicar_cola(&mut self) -> CmdResult {
        if self.cola.is_empty() {
            return CmdResult::Cancel;
        }
        CmdResult::RenumeraApply {
            distrito: self.distrito.clone(),
            huawei: self.huawei,
            inicio: self.siguiente.saturating_sub(self.cola.len() as u32),
            handles: self.cola.iter().map(|(handle, _, _)| *handle).collect(),
        }
    }
}
impl CadCommand for RenumeraCommand {
    fn name(&self) -> &'static str {
        "RENUMERA"
    }

    fn prompt(&self) -> String {
        match self.paso {
            Paso::Distrito => format!(
                "RENUMERA  Distrito <{}>:",
                self.distrito
            ),
            Paso::Tecnologia => format!(
                "RENUMERA  Tecnología [COMMSCOPE/HUAWEI] <{}>:",
                if self.huawei { "HUAWEI" } else { "COMMSCOPE" }
            ),
            Paso::Inicio => "RENUMERA  Número de inicio (1-64) <1>:".to_string(),
            Paso::Bloques => match &self.ultimo {
                Some(ultimo) => format!(
                    "RENUMERA  {} en cola ({ultimo}) · Enter para APLICAR · siguiente #{}:",
                    self.cola.len(),
                    self.siguiente
                ),
                None => format!(
                    "RENUMERA  #{} → {} · {} · seleccione el bloque:",
                    self.siguiente,
                    bloque_destino(self.huawei, self.siguiente),
                    nombre_nap(&self.distrito, self.siguiente)
                ),
            },
        }
    }

    fn options(&self) -> Vec<CmdOption> {
        match self.paso {
            Paso::Tecnologia => vec![
                CmdOption::new("HUAWEI", "HUAWEI"),
                CmdOption::new("COMMSCOPE", "COMMSCOPE"),
            ],
            _ => Vec::new(),
        }
    }

    fn input_kind(&self) -> InputKind {
        match self.paso {
            // El distrito conserva mayúsculas y minúsculas.
            Paso::Distrito => InputKind::FreeText,
            Paso::Tecnologia | Paso::Inicio => InputKind::SingleToken,
            Paso::Bloques => InputKind::Point,
        }
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let token = text.trim();
        match self.paso {
            Paso::Distrito => {
                if !token.is_empty() {
                    self.distrito = token.to_string();
                    fijar_distrito(token);
                }
                self.paso = Paso::Tecnologia;
                Some(CmdResult::NeedPoint)
            }
            Paso::Tecnologia => {
                match token.to_ascii_uppercase().as_str() {
                    "" | "H" | "HUAWEI" => self.huawei = true,
                    "C" | "COMMSCOPE" => self.huawei = false,
                    _ => return None,
                }
                self.paso = Paso::Inicio;
                Some(CmdResult::NeedPoint)
            }
            Paso::Inicio => {
                if token.is_empty() {
                    self.siguiente = 1;
                } else {
                    let Ok(numero) = token.replace(',', ".").parse::<u32>() else {
                        return Some(CmdResult::ReportError(
                            "Escriba un número entre 1 y 64.".to_string(),
                        ));
                    };
                    if !(1..=MAX_NAP).contains(&numero) {
                        return Some(CmdResult::ReportError(
                            "El número debe estar entre 1 y 64.".to_string(),
                        ));
                    }
                    self.siguiente = numero;
                }
                self.paso = Paso::Bloques;
                Some(CmdResult::NeedPoint)
            }
            Paso::Bloques => None,
        }
    }

    fn on_enter(&mut self) -> CmdResult {
        match self.paso {
            Paso::Distrito => {
                self.paso = Paso::Tecnologia;
                CmdResult::NeedPoint
            }
            Paso::Tecnologia => {
                self.paso = Paso::Inicio;
                CmdResult::NeedPoint
            }
            Paso::Inicio => {
                self.paso = Paso::Bloques;
                CmdResult::NeedPoint
            }
            Paso::Bloques => self.aplicar_cola(),
        }
    }

    fn on_escape(&mut self) -> CmdResult {
        // Esc también aplica lo encolado: perder veinte clics por salir no
        // es aceptable, y el comando ya no tiene nada más que hacer.
        if self.cola.is_empty() {
            CmdResult::Cancel
        } else {
            self.aplicar_cola()
        }
    }

    fn needs_entity_pick(&self) -> bool {
        self.paso == Paso::Bloques
    }

    fn inject_before_entity_pick(&self) -> bool {
        true
    }

    fn inject_picked_entity(&mut self, entity: EntityType) {
        self.recogido = Some(entity);
    }

    fn on_entity_pick(&mut self, handle: Handle, _pt: DVec3) -> CmdResult {
        self.aplicar(handle)
    }

    fn on_point(&mut self, _pt: DVec3) -> CmdResult {
        CmdResult::NeedPoint
    }

    fn selection_entity_filter(&self) -> Option<fn(&EntityType) -> bool> {
        Some(|entity| matches!(entity, EntityType::Insert(_)))
    }
}

// ── RENUMERA_DISTRITO ───────────────────────────────────────────────────────

/// Cambia el distrito recordado, como `FTTH_NAP_DISTRITO`.
pub(crate) struct RenumeraDistritoCommand {
    distrito: String,
}

impl RenumeraDistritoCommand {
    pub(crate) fn new() -> Self {
        Self {
            distrito: distrito_actual(),
        }
    }
}

impl CadCommand for RenumeraDistritoCommand {
    fn name(&self) -> &'static str {
        "RENUMERA_DISTRITO"
    }

    fn prompt(&self) -> String {
        format!("RENUMERA_DISTRITO  Distrito actual '{}' — nuevo (Enter para dejarlo):", self.distrito)
    }

    fn input_kind(&self) -> InputKind {
        InputKind::FreeText
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let token = text.trim();
        if !token.is_empty() {
            self.distrito = token.to_string();
            fijar_distrito(token);
        }
        Some(CmdResult::Cancel)
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn on_point(&mut self, _pt: DVec3) -> CmdResult {
        CmdResult::Cancel
    }
}

// ── Autocomplete registry ───────────────────────────────────────────────────
inventory::submit!(crate::command::CommandRegistration {
    names: &["RENUMERA", "RENUMERA_DISTRITO"]
});
