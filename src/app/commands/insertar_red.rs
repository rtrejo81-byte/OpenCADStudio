//! `INSERTAR_RED` — traza un tramo de red insertando bloques conectados.
//!
//! Portación de la rutina LISP `C:INSERTAR_RED` v4.0 de Roberto Trejo. Recorre
//! una red de postes/pozos/NAPs: en cada clic inserta el bloque elegido y, si
//! ya hay un punto anterior, dibuja la conexión y rotula la distancia en el
//! punto medio.
//!
//! - Conexiones en `SALVADOR_STRAND` y rótulos en `SALVADOR_STRANDDISTANCE`.
//! - Si alguno de los dos extremos es `POZO ACERA` / `POZO CALLE`, la conexión
//!   va por `SALVADOR_DUCT` y el rótulo por `SALVADOR_DUCTDISTANCE`.
//! - Los bloques y las capas ya vienen en el plano/plantilla: aquí no se crean.
//!
//! Modos de inicio: `Cero` (primer bloque), `Existente` (arranca en un bloque
//! ya dibujado) y `Unir` (conecta dos bloques existentes).

use codec::entities::{Insert, Line, Text, TextHorizontalAlignment as HA, TextVerticalAlignment as VA};
use codec::types::Vector3;
use codec::EntityType;
use glam::DVec3;

use crate::command::{CadCommand, CmdOption, CmdResult, InputKind};
use crate::scene::model::wire_model::WireModel;

pub(crate) const LAYER_BLOCK: &str = "SALVADOR_POLEPED";
const LAYER_STRAND: &str = "SALVADOR_STRAND";
const LAYER_DUCT: &str = "SALVADOR_DUCT";
const LAYER_STRAND_DIST: &str = "SALVADOR_STRANDDISTANCE";
const LAYER_DUCT_DIST: &str = "SALVADOR_DUCTDISTANCE";

/// Altura del rótulo de distancia, igual que la rutina original.
const TEXT_HEIGHT: f64 = 1.75;
/// Distancia de referencia con la que arranca el comando.
const DEFAULT_DISTANCE: f64 = 5.0;

const YELLOW: [f32; 4] = [1.00, 0.85, 0.20, 1.0];
const GREEN: [f32; 4] = [0.20, 1.00, 0.35, 1.0];

/// Tabla de bloques de la rutina original: (nombre en el dibujo, etiqueta).
pub(crate) const BLOCKS: [(&str, &str); 25] = [
    ("PC8_E_42", "Poste 8m existente"),
    ("PC8_N", "Poste 8m nuevo"),
    ("PC6_E", "Poste 6m existente"),
    ("PC6_N", "Poste 6m nuevo"),
    ("PTT_8", "Poste metálico 8m existente"),
    ("PTT_8N", "Poste metálico 8m nuevo"),
    ("PT6", "Poste tubo 6m existente"),
    ("PT_R", "Poste tubo 6m nuevo"),
    ("P_ELEC", "Poste eléctrico"),
    ("PC10_E", "Poste 10m existente"),
    ("PC10_N", "Poste 10m nuevo"),
    ("PC15_E", "Poste 15m existente"),
    ("PC15_N", "Poste 15m nuevo"),
    ("PM_E", "Poste madera"),
    ("P_NUL_7", "P_NUL_7"),
    ("POZO ACERA", "Pozo en acera"),
    ("POZO CALLE", "Pozo en calle"),
    ("ODF", "ODF"),
    ("C-12", "C-12"),
    ("HUB BOX", "HUB BOX"),
    ("NAP", "NAP"),
    ("FAT-L", "FAT-L"),
    ("FAT-F", "FAT-F"),
    ("EMPALME", "Empalme"),
    ("BTS", "BTS"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    /// Cómo empezar: Cero / Existente / Unir.
    Mode,
    /// Elegir bloque por número.
    Block,
    /// Distancia de referencia para el círculo guía.
    Distance,
    /// Clic de inserción.
    Point,
    /// Después de colocar: Mismo / Cambiar / Terminar.
    Next,
    /// Seleccionar un bloque existente (inicio o terminación).
    PickStart,
    /// Unir: primer bloque.
    PickFirst,
    /// Unir: segundo bloque.
    PickSecond,
}

pub(crate) struct InsertarRedCommand {
    step: Step,
    /// Bloques presentes en el dibujo, en mayúsculas.
    available: Vec<String>,
    block: Option<String>,
    previous: Option<DVec3>,
    previous_block: Option<String>,
    distance: f64,
    /// Primer bloque del modo Unir.
    first_pick: Option<(DVec3, String)>,
    /// Entidad que el host inyecta justo antes del pick.
    picked: Option<(DVec3, String)>,
    /// Cuántos bloques se han colocado en esta ejecución.
    placed: usize,
}

impl InsertarRedCommand {
    pub(crate) fn new(available: Vec<String>) -> Self {
        Self {
            step: Step::Mode,
            available,
            block: None,
            previous: None,
            previous_block: None,
            distance: DEFAULT_DISTANCE,
            first_pick: None,
            picked: None,
            placed: 0,
        }
    }

    fn has_block(&self, name: &str) -> bool {
        self.available
            .iter()
            .any(|block| block.eq_ignore_ascii_case(name))
    }

    /// Nombre del bloque `n` (1..=25) o `None` si está fuera de la tabla.
    fn select_block(&mut self, n: u32) -> CmdResult {
        let Some(name) = BLOCKS.get(n.saturating_sub(1) as usize).map(|(name, _)| *name) else {
            return CmdResult::ReportError("Número de bloque fuera de la tabla (1-25).".to_string());
        };
        if !self.has_block(name) {
            return CmdResult::ReportError(format!(
                "El bloque '{name}' no está en el dibujo."
            ));
        }
        self.block = Some(name.to_string());
        self.step = Step::Distance;
        CmdResult::NeedPoint
    }

    /// Inserta el bloque con la capa de postes.
    fn block_entity(name: &str, pt: DVec3) -> EntityType {
        let insert = Insert::new(name, Vector3::new(pt.x, pt.y, pt.z));
        let mut entity = EntityType::Insert(insert);
        entity.common_mut().layer = LAYER_BLOCK.to_string();
        entity
    }

    /// Línea de conexión + rótulo de distancia entre dos bloques.
    fn connection(from: DVec3, from_block: &str, to: DVec3, to_block: &str) -> Vec<EntityType> {
        let duct = is_pozo(from_block) || is_pozo(to_block);
        let (line_layer, text_layer) = if duct {
            (LAYER_DUCT, LAYER_DUCT_DIST)
        } else {
            (LAYER_STRAND, LAYER_STRAND_DIST)
        };

        let mut line = Line::from_points(vec3(from), vec3(to));
        line.common.layer = line_layer.to_string();
        let line = EntityType::Line(line);

        let distance = (to - from).length();
        let mut angle = (to.y - from.y).atan2(to.x - from.x);
        // El rótulo se voltea para no quedar boca abajo, como en la rutina.
        if angle > std::f64::consts::FRAC_PI_2 && angle < 1.5 * std::f64::consts::PI {
            angle += std::f64::consts::PI;
        }
        let mid = (from + to) * 0.5;
        let mut text =
            Text::with_value(format!("{distance:.1}m"), vec3(mid)).with_height(TEXT_HEIGHT);
        text.rotation = angle;
        text.horizontal_alignment = HA::Center;
        text.vertical_alignment = VA::Middle;
        text.alignment_point = Some(vec3(mid));
        text.common.layer = text_layer.to_string();

        vec![line, EntityType::Text(text)]
    }

    fn place(&mut self, pt: DVec3) -> CmdResult {
        let Some(block) = self.block.clone() else {
            return CmdResult::Cancel;
        };
        let mut entities = vec![Self::block_entity(&block, pt)];
        if let (Some(previous), Some(previous_block)) = (self.previous, self.previous_block.clone()) {
            entities.extend(Self::connection(previous, &previous_block, pt, &block));
        }
        self.previous = Some(pt);
        self.previous_block = Some(block);
        self.placed += 1;
        self.step = Step::Next;
        CmdResult::CommitEntities(entities)
    }

    /// Cierra el tramo contra un bloque ya dibujado.
    fn finish_at(&mut self, point: DVec3, block: &str) -> CmdResult {
        let (Some(previous), Some(previous_block)) =
            (self.previous, self.previous_block.clone())
        else {
            return CmdResult::Cancel;
        };
        if (point - previous).length() < 1.0e-9 {
            return CmdResult::ReportError(
                "El bloque de terminación está en el mismo punto que el último.".to_string(),
            );
        }
        CmdResult::CommitEntitiesAndExit(Self::connection(previous, &previous_block, point, block))
    }
}

fn vec3(point: DVec3) -> Vector3 {
    Vector3::new(point.x, point.y, point.z)
}

fn is_pozo(name: &str) -> bool {
    name.eq_ignore_ascii_case("POZO ACERA") || name.eq_ignore_ascii_case("POZO CALLE")
}

/// El host solo inyecta entidades si el comando lo pide explícitamente.
fn is_insert(entity: &EntityType) -> bool {
    matches!(entity, EntityType::Insert(_))
}

impl CadCommand for InsertarRedCommand {
    fn name(&self) -> &'static str {
        "INSERTAR_RED"
    }

    fn prompt(&self) -> String {
        match self.step {
            Step::Mode => "INSERTAR_RED  ¿Cómo desea comenzar? [Cero/Existente/Unir] <Cero>:"
                .to_string(),
            Step::Block => {
                "INSERTAR_RED  Tipo de bloque [1-25] o T para terminar en uno existente:"
                    .to_string()
            }
            Step::Distance => format!(
                "INSERTAR_RED  Distancia de referencia para el círculo guía <{:.2}>:",
                self.distance
            ),
            Step::Point => {
                let block = self.block.as_deref().unwrap_or("?");
                match self.previous {
                    Some(_) => format!(
                        "INSERTAR_RED  [{block}] Punto de inserción · guía {:.2} m:",
                        self.distance
                    ),
                    None => format!("INSERTAR_RED  [{block}] Punto de inserción:"),
                }
            }
            Step::Next => format!(
                "INSERTAR_RED  {} bloque(s) colocado(s) · [Mismo/Cambiar/Terminar] <Mismo>:",
                self.placed
            ),
            Step::PickStart => {
                if self.previous.is_some() {
                    "INSERTAR_RED  Seleccione el bloque donde terminar:".to_string()
                } else {
                    "INSERTAR_RED  Seleccione el bloque de inicio:".to_string()
                }
            }
            Step::PickFirst => "INSERTAR_RED  Unir · seleccione el PRIMER bloque:".to_string(),
            Step::PickSecond => "INSERTAR_RED  Unir · seleccione el SEGUNDO bloque:".to_string(),
        }
    }

    fn options(&self) -> Vec<CmdOption> {
        match self.step {
            Step::Mode => vec![
                CmdOption::new("Cero", "C"),
                CmdOption::new("Existente", "E"),
                CmdOption::new("Unir", "U"),
            ],
            Step::Next => vec![
                CmdOption::new("Mismo", "M"),
                CmdOption::new("Cambiar", "C"),
                CmdOption::new("Terminar", "T"),
            ],
            _ => Vec::new(),
        }
    }

    fn input_kind(&self) -> InputKind {
        match self.step {
            Step::Point | Step::PickStart | Step::PickFirst | Step::PickSecond => InputKind::Point,
            _ => InputKind::SingleToken,
        }
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let token = text.trim().to_ascii_uppercase();
        match self.step {
            Step::Mode => {
                match token.as_str() {
                    "" | "C" | "CERO" => self.step = Step::Block,
                    "E" | "EXISTENTE" => self.step = Step::PickStart,
                    "U" | "UNIR" => self.step = Step::PickFirst,
                    _ => return None,
                }
                Some(CmdResult::NeedPoint)
            }
            Step::Block => {
                if token.is_empty() {
                    return None;
                }
                if matches!(token.as_str(), "T" | "TERMINAR") {
                    if self.previous.is_none() {
                        return Some(CmdResult::ReportError(
                            "Aún no hay un bloque colocado desde el cual terminar.".to_string(),
                        ));
                    }
                    self.step = Step::PickStart;
                    return Some(CmdResult::NeedPoint);
                }
                let Ok(number) = token.parse::<u32>() else {
                    return Some(CmdResult::ReportError(
                        "Escriba un número de bloque (1-25) o T.".to_string(),
                    ));
                };
                Some(self.select_block(number))
            }
            Step::Distance => {
                if token.is_empty() {
                    self.step = Step::Point;
                    return Some(CmdResult::NeedPoint);
                }
                let Ok(value) = token.replace(',', ".").parse::<f64>() else {
                    return Some(CmdResult::ReportError(
                        "Distancia inválida: escriba un número mayor que cero.".to_string(),
                    ));
                };
                if !value.is_finite() || value <= 0.0 {
                    return Some(CmdResult::ReportError(
                        "La distancia debe ser mayor que cero.".to_string(),
                    ));
                }
                self.distance = value;
                self.step = Step::Point;
                Some(CmdResult::NeedPoint)
            }
            Step::Next => {
                match token.as_str() {
                    "" | "M" | "MISMO" => self.step = Step::Distance,
                    "C" | "CAMBIAR" => self.step = Step::Block,
                    "T" | "TERMINAR" => self.step = Step::PickStart,
                    _ => return None,
                }
                Some(CmdResult::NeedPoint)
            }
            _ => None,
        }
    }

    fn on_point(&mut self, point: DVec3) -> CmdResult {
        if self.step == Step::Point {
            self.place(point)
        } else {
            CmdResult::NeedPoint
        }
    }

    fn on_enter(&mut self) -> CmdResult {
        match self.step {
            // Enter en un punto elige el modo por defecto.
            Step::Mode => {
                self.step = Step::Block;
                CmdResult::NeedPoint
            }
            // Enter acepta la distancia anterior.
            Step::Distance => {
                self.step = Step::Point;
                CmdResult::NeedPoint
            }
            // Enter tras colocar = mismo bloque.
            Step::Next => {
                self.step = Step::Distance;
                CmdResult::NeedPoint
            }
            _ => CmdResult::Cancel,
        }
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn needs_entity_pick(&self) -> bool {
        matches!(self.step, Step::PickStart | Step::PickFirst | Step::PickSecond)
    }

    fn selection_entity_filter(&self) -> Option<fn(&EntityType) -> bool> {
        Some(is_insert)
    }

    fn inject_before_entity_pick(&self) -> bool {
        true
    }

    fn inject_picked_entity(&mut self, entity: EntityType) {
        if let EntityType::Insert(insert) = entity {
            self.picked = Some((
                DVec3::new(
                    insert.insert_point.x,
                    insert.insert_point.y,
                    insert.insert_point.z,
                ),
                insert.block_name,
            ));
        }
    }

    fn on_entity_pick(&mut self, _handle: codec::Handle, _pt: DVec3) -> CmdResult {
        let Some((point, block)) = self.picked.take() else {
            return CmdResult::ReportError("Seleccione un bloque (INSERT).".to_string());
        };
        match self.step {
            Step::PickStart => {
                if self.previous.is_some() {
                    self.finish_at(point, &block)
                } else {
                    self.previous = Some(point);
                    self.previous_block = Some(block.clone());
                    self.block = Some(block);
                    self.step = Step::Next;
                    CmdResult::NeedPoint
                }
            }
            Step::PickFirst => {
                self.first_pick = Some((point, block));
                self.step = Step::PickSecond;
                CmdResult::NeedPoint
            }
            Step::PickSecond => {
                let Some((first_point, first_block)) = self.first_pick.take() else {
                    return CmdResult::Cancel;
                };
                if (point - first_point).length() < 1.0e-9 {
                    return CmdResult::ReportError(
                        "Ha seleccionado el mismo bloque dos veces.".to_string(),
                    );
                }
                CmdResult::CommitEntitiesAndExit(Self::connection(
                    first_point,
                    &first_block,
                    point,
                    &block,
                ))
            }
            _ => CmdResult::NeedPoint,
        }
    }

    fn on_preview_wires(&mut self, pt: DVec3) -> Vec<WireModel> {
        if self.step != Step::Point {
            return Vec::new();
        }
        let mut wires = Vec::new();
        if let Some(previous) = self.previous {
            // Línea elástica al cursor.
            wires.push(WireModel::solid_f64(
                "insertar_red_rubber".to_string(),
                vec![
                    [previous.x, previous.y, previous.z],
                    [pt.x, pt.y, pt.z],
                ],
                WireModel::CYAN,
                false,
            ));
            // Círculo guía con la distancia configurada.
            if self.distance > 0.0 {
                let mut ring = Vec::with_capacity(33);
                for i in 0..=32 {
                    let angle = i as f64 * std::f64::consts::TAU / 32.0;
                    let x = previous.x + angle.cos() * self.distance;
                    let y = previous.y + angle.sin() * self.distance;
                    ring.push([x, y, previous.z]);
                }
                wires.push(WireModel::solid_f64(
                    "insertar_red_guide".to_string(),
                    ring,
                    YELLOW,
                    true,
                ));
            }
        }
        // Cruz en el cursor.
        let size = 0.5;
        wires.push(WireModel::solid_f64(
            "insertar_red_cross_a".to_string(),
            vec![
                [pt.x - size, pt.y - size, pt.z],
                [pt.x + size, pt.y + size, pt.z],
            ],
            GREEN,
            false,
        ));
        wires.push(WireModel::solid_f64(
            "insertar_red_cross_b".to_string(),
            vec![
                [pt.x - size, pt.y + size, pt.z],
                [pt.x + size, pt.y - size, pt.z],
            ],
            GREEN,
            false,
        ));
        wires
    }

    fn has_measurement_readout(&self) -> bool {
        true
    }

    fn measurement_readout(&self, cursor: DVec3) -> Option<String> {
        let previous = self.previous?;
        let distance = (cursor - previous).length();
        distance.is_finite().then(|| {
            format!(
                "Distancia al cursor: {:.2} m · configuración: {:.2} m",
                distance, self.distance
            )
        })
    }

    fn preserve_commit_layer(&self) -> bool {
        true
    }

    fn preserve_commit_style(&self) -> bool {
        true
    }
}

// ── Autocomplete registry ───────────────────────────────────────────────────
inventory::submit!(crate::command::CommandRegistration {
    names: &["INSERTAR_RED"]
});
