//! `NUMERA` — numeración secuencial de rótulos.
//!
//! Equivalente en la aplicación a la rutina LISP que Roberto Trejo usaba en
//! AutoCAD: cada clic coloca un `TEXT` con `prefijo + consecutivo + sufijo` y
//! avanza el contador. El comando queda activo para numerar varios puntos
//! seguidos; `Esc` o `Enter` lo terminan.
//!
//! El consecutivo vive en una variable de sesión (como el global de la rutina
//! LISP original): tras `Esc`, la siguiente ejecución continúa donde quedó.
//! La opción `Inicio` (`I`) lo fija en un número dado y `Reiniciar` (`R`) lo
//! devuelve a 1.
//!
//! Ajustes por palabra clave: `Prefijo`, `Sufijo`, `Altura`, `Inicio` y
//! `Reiniciar` (también `P`, `S`, `A`, `I`, `R`).

use std::sync::Mutex;

use codec::entities::Text;
use codec::types::Vector3;
use glam::DVec3;

use crate::command::{CadCommand, CmdOption, CmdResult, InputKind, WorkingPlane};

/// Consecutivo compartido entre invocaciones del comando, equivalente al
/// global `*numera*` que la rutina LISP guardaba durante la sesión.
static COUNTER: Mutex<f64> = Mutex::new(1.0);

/// Ajuste que el comando espera por la línea de comandos.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Prefix,
    Suffix,
    Height,
    Start,
}

pub(crate) struct NumeraCommand {
    plane: WorkingPlane,
    prefix: String,
    suffix: String,
    height: f64,
    style: String,
    /// `Some` mientras se espera el valor nuevo de un ajuste.
    field: Option<Field>,
}

/// Valor actual del consecutivo; un candado envenenado por un pánico previo no
/// debe impedir numerar, así que se recupera el valor interior.
fn counter() -> f64 {
    *COUNTER.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn set_counter(value: f64) {
    *COUNTER.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = value;
}

impl NumeraCommand {
    pub(crate) fn new(height: f64, style: String) -> Self {
        Self {
            plane: WorkingPlane::default(),
            prefix: String::new(),
            suffix: String::new(),
            height: height.max(1.0e-9),
            style,
            field: None,
        }
    }

    /// Rótulo que colocará el siguiente clic: `prefijo + n + sufijo`.
    fn label(&self) -> String {
        format!("{}{}{}", self.prefix, format_counter(counter()), self.suffix)
    }
}

/// `1.0` → `"1"`, `1.5` → `"1.5"`: sin decimales sobrantes en el consecutivo.
fn format_counter(value: f64) -> String {
    if (value - value.round()).abs() < 1.0e-9 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value}")
    }
}

impl CadCommand for NumeraCommand {
    fn name(&self) -> &'static str {
        "NUMERA"
    }

    fn prompt(&self) -> String {
        match self.field {
            Some(Field::Prefix) => {
                format!("NUMERA  Nuevo prefijo (actual '{}'):", self.prefix)
            }
            Some(Field::Suffix) => {
                format!("NUMERA  Nuevo sufijo (actual '{}'):", self.suffix)
            }
            Some(Field::Height) => format!(
                "NUMERA  Nueva altura de texto (actual {}):",
                self.height
            ),
            Some(Field::Start) => format!(
                "NUMERA  Número inicial (actual {}):",
                format_counter(counter())
            ),
            None => format!(
                "NUMERA  Punto para el número {}  [Prefijo/Sufijo/Altura/Inicio/Reiniciar]:",
                self.label()
            ),
        }
    }

    fn options(&self) -> Vec<CmdOption> {
        if self.field.is_some() {
            return Vec::new();
        }
        vec![
            CmdOption::new("Prefijo", "P"),
            CmdOption::new("Sufijo", "S"),
            CmdOption::new("Altura", "A"),
            CmdOption::new("Inicio", "I"),
            CmdOption::new("Reiniciar", "R"),
        ]
    }

    fn input_kind(&self) -> InputKind {
        match self.field {
            // El prefijo y el sufijo conservan mayúsculas y minúsculas.
            Some(Field::Prefix | Field::Suffix) => InputKind::FreeText,
            Some(Field::Height | Field::Start) => InputKind::SingleToken,
            None => InputKind::Point,
        }
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let token = text.trim();
        if let Some(field) = self.field {
            match field {
                Field::Prefix => self.prefix = token.to_string(),
                Field::Suffix => self.suffix = token.to_string(),
                Field::Height => {
                    let value = token.replace(',', ".").parse::<f64>().ok()?;
                    if !value.is_finite() || value <= 1.0e-9 {
                        return None;
                    }
                    self.height = value;
                }
                Field::Start => {
                    let value = token.replace(',', ".").parse::<f64>().ok()?;
                    if !value.is_finite() || value <= 0.0 {
                        return None;
                    }
                    set_counter(value);
                }
            }
            self.field = None;
            return Some(CmdResult::NeedPoint);
        }
        match token.to_ascii_uppercase().as_str() {
            "P" | "PREFIJO" | "PREFIX" => self.field = Some(Field::Prefix),
            "S" | "SUFIJO" | "SUFFIX" => self.field = Some(Field::Suffix),
            "A" | "ALTURA" | "HEIGHT" => self.field = Some(Field::Height),
            "I" | "INICIO" | "START" => self.field = Some(Field::Start),
            "R" | "REINICIAR" | "RESET" => set_counter(1.0),
            _ => return None,
        }
        Some(CmdResult::NeedPoint)
    }

    /// El paso de punto también acepta las letras de `P/S/A/I/R`: los dígitos
    /// siguen siendo coordenadas y las letras llegan como palabras clave.
    fn point_step_accepts_keywords(&self) -> bool {
        self.field.is_none()
    }

    fn set_working_plane(&mut self, plane: WorkingPlane) {
        self.plane = plane;
    }

    fn on_point(&mut self, point: DVec3) -> CmdResult {
        let local = self.plane.to_local(point);
        let mut text = Text::with_value(self.label(), Vector3::new(local.x, local.y, local.z))
            .with_height(self.height);
        text.style = self.style.clone();
        set_counter(counter() + 1.0);
        CmdResult::CommitEntity(codec::EntityType::Text(text))
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
    names: &["NUMERA"]
});
