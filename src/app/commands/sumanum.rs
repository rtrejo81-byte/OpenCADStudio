//! `SUMANUM` — suma los números contenidos en los textos seleccionados.
//!
//! Equivalente a la rutina que Roberto Trejo usaba en AutoCAD: se seleccionan
//! rótulos (TEXT, MTEXT, atributos, tolerancias, multileaders…) y `Enter`
//! imprime el desglose, cuántos se procesaron y el total.
//!
//! La lectura de números tolera lo que aparece en planos reales: signo
//! negativo, coma o punto decimal, separadores de miles y códigos de formato
//! de MText (`{\fArial;…}`, `\H2.5x;`, `\P`).

use codec::Handle;
use glam::DVec3;

use crate::command::{CadCommand, CmdResult, InputKind};

pub(crate) struct SumaNumCommand {
    selected: Vec<Handle>,
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
            "SUMANUM  Seleccione los textos a sumar y presione Enter:".to_string()
        } else {
            format!(
                "SUMANUM  {} texto(s) en la selección — Enter para sumar:",
                self.selected.len()
            )
        }
    }

    fn input_kind(&self) -> InputKind {
        InputKind::Point
    }

    fn is_selection_gathering(&self) -> bool {
        true
    }

    fn on_selection_complete(&mut self, handles: Vec<Handle>) -> CmdResult {
        // El sistema de selección ya mantiene el conjunto vivo en la escena;
        // aquí solo se recuerda para saber si `Enter` tiene algo que sumar.
        self.selected = handles;
        CmdResult::NeedPoint
    }

    fn on_enter(&mut self) -> CmdResult {
        if self.selected.is_empty() {
            return CmdResult::Cancel;
        }
        // `handle_relaunch` vuelve a seleccionar estos handles antes de
        // despachar, así que `SUMANUM_EXECUTE` los encuentra en la escena.
        CmdResult::Relaunch(
            "SUMANUM_EXECUTE".to_string(),
            std::mem::take(&mut self.selected),
        )
    }

    fn on_point(&mut self, _pt: DVec3) -> CmdResult {
        CmdResult::NeedPoint
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

/// Extrae el primer número de un rótulo, o `None` si no hay ninguno.
///
/// `-12` → -12 · `12,5` → 12.5 · `1.234,56` → 1234.56 · `1,234.56` → 1234.56
/// · `12.5 m` → 12.5 · `{\fArial;N15}` → 15
pub(crate) fn parse_number(text: &str) -> Option<f64> {
    let cleaned = strip_format_codes(text);
    let token = numeric_token(&cleaned)?;
    normalize_number(&token)?.parse::<f64>().ok()
}

/// Quita los códigos de formato de MText/MTEXT: `\H2.5x;`, `\P`, `\~`, `\\`
/// y las llaves de agrupación, para que sus cifras no se confundan con datos.
fn strip_format_codes(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' => {
                // Un código con parámetros termina en `;` (`\H2.5x;`); los
                // cortos (`\P`, `\~`) no lo llevan y se saltan solos.
                let end = chars
                    .iter()
                    .skip(i + 1)
                    .take(32)
                    .position(|c| *c == ';');
                i = end.map_or(i + 1, |offset| i + 1 + offset + 1);
            }
            '{' | '}' => i += 1,
            _ => {
                out.push(chars[i]);
                i += 1;
            }
        }
    }
    out
}

/// Primer token numérico: signo opcional seguido de dígitos y separadores.
fn numeric_token(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let signed = (c == '-' || c == '+')
            && chars
                .get(i + 1)
                .is_some_and(|next| next.is_ascii_digit());
        if c.is_ascii_digit() || signed {
            let start = i;
            if signed {
                i += 1;
            }
            while i < chars.len()
                && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == ',')
            {
                i += 1;
            }
            return Some(chars[start..i].iter().collect());
        }
        i += 1;
    }
    None
}

/// Decide qué separador es decimal y cuál de miles.
///
/// Con ambos presentes manda el último (`1.234,56` y `1,234.56` son 1234.56).
/// Una sola coma con uno o dos decimales es decimal (`12,5`); con tres, miles
/// (`1,234`). El punto simple se toma siempre como decimal, la convención de
/// CAD (`12.5`).
fn normalize_number(token: &str) -> Option<String> {
    let (sign, body) = match token.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", token.strip_prefix('+').unwrap_or(token)),
    };
    let dots = body.matches('.').count();
    let commas = body.matches(',').count();
    if dots > 0 && commas > 0 {
        let decimal = if body.rfind('.').unwrap_or(0) > body.rfind(',').unwrap_or(0) {
            '.'
        } else {
            ','
        };
        let cleaned: String = body
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == decimal)
            .collect();
        return Some(format!("{sign}{}", cleaned.replace(decimal, ".")));
    }
    if commas > 0 {
        let decimals = body.rsplit(',').next().map_or(0, |group| group.len());
        if commas == 1 && decimals <= 2 {
            return Some(format!("{sign}{}", body.replacen(',', ".", 1)));
        }
        return Some(format!("{sign}{}", body.replace(',', "")));
    }
    if dots > 1 {
        // Varios puntos sin coma: separadores de miles (`1.234.567`).
        return Some(format!("{sign}{}", body.replace('.', "")));
    }
    Some(format!("{sign}{body}"))
}

// ── Autocomplete registry ───────────────────────────────────────────────────
inventory::submit!(crate::command::CommandRegistration {
    names: &["SUMANUM"]
});
