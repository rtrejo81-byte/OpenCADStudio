//! `NAPAKML` — exporta los NAPs y los polígonos del dibujo a un KML para
//! Google Earth.
//!
//! Portación de la rutina LISP `FTTH_NAPaKML-UTM` (TREJO FIBER MAPPER v3.2):
//! los bloques insertados salen como chinchetas con la letra «N», el nombre
//! tomado de sus atributos, y las polilíneas cerradas como polígonos
//! amarillos nombrados por su capa.
//!
//! Las coordenadas del dibujo se leen como **UTM zona 16N (WGS84)** y se
//! convierten a latitud/longitud con la misma fórmula de la rutina original,
//! para que el KML caiga exactamente donde caía antes.

use codec::{CadDocument, EntityType, Handle};

/// Huso UTM de El Salvador.
const ZONA: f64 = 16.0;
/// Meridiano central del huso 16.
const MERIDIANO_CENTRAL: f64 = 6.0 * ZONA - 183.0;

/// Etiquetas que se prueban, en orden, para nombrar un NAP.
const TAGS_PRIORITARIOS: [&str; 40] = [
    "NUMERA",
    "NUMERO",
    "NUMBER",
    "NAP",
    "NAP_NAME",
    "NAPNAME",
    "NAPNOMBRE",
    "NAP-NOMBRE",
    "NAP_NOMBRE",
    "NAPN",
    "CODIGO",
    "COD",
    "CODE",
    "TYPE",
    "TIPO",
    "NOMBRE",
    "NAME",
    "ID",
    "IDENTIFICADOR",
    "IDENTIFIER",
    "TAG",
    "DESIGNACION",
    "DESIGNATION",
    "CP",
    "POSTE",
    "OLT",
    "FDH",
    "ONT",
    "LABEL",
    "ETIQUETA",
    "EQUIPO",
    "EQUIPMENT",
    "DESCRIPCION",
    "DESCRIPTION",
    "REFERENCIA",
    "REFERENCE",
    "SERIE",
    "SERIAL",
    "UBICACION",
    "LOCATION",
];

/// Convierte UTM 16N a latitud/longitud (grados), con la fórmula de la rutina
/// original (`utm-to-latlon-precise`).
pub(crate) fn utm16n_a_latlon(x: f64, y: f64) -> (f64, f64) {
    const A: f64 = 6_378_137.0;
    #[allow(non_upper_case_globals)]
    const b: f64 = 6_356_752.314_245_18;
    const EP2: f64 = 0.006_739_496_7;
    const C: f64 = 6_399_593.625_758_49;
    const K0: f64 = 0.999_6;

    let _ = (A, b);
    let fi_p = (y / 6_366_197.724) * K0;
    let cos_fi = fi_p.cos();
    let ni_p = (C / (1.0 + EP2 * cos_fi * cos_fi).sqrt()) * K0;
    let a_val = (x - 500_000.0) / ni_p;

    let a1 = (2.0 * fi_p).sin();
    let a2 = a1 * cos_fi * cos_fi;
    let j2 = fi_p + a1 / 2.0;
    let j4 = (3.0 * j2 + a2) / 4.0;
    let j6 = (5.0 * j4 + a2 * cos_fi * cos_fi) / 3.0;
    let alfa = 0.75 * EP2;
    let beta = (5.0 / 3.0) * alfa * alfa;
    let gamma = (35.0 / 27.0) * alfa * alfa * alfa;
    let bfi = K0 * C * (fi_p - alfa * j2 + beta * j4 + gamma * j6);

    let b_val = (y - bfi) / ni_p;
    let zeta = (EP2 * a_val * a_val / 2.0) * cos_fi * cos_fi;
    let xi = a_val * (1.0 - zeta / 3.0);
    let eta = b_val * (1.0 - zeta) + fi_p;

    let sinh_xi = (xi.exp() - (-xi).exp()) / 2.0;
    let d_lambda = (sinh_xi / eta.cos()).atan();
    let tau = (d_lambda.cos() * eta.tan()).atan();
    let rad_corr = fi_p
        + (1.0 + EP2 * (cos_fi * cos_fi - 1.5 * EP2 * fi_p.sin() * cos_fi)) * (tau - fi_p);

    let lat = rad_corr.to_degrees();
    let lon = d_lambda.to_degrees() + MERIDIANO_CENTRAL;
    (lat, lon)
}

/// Escapa el texto para que quepa en XML.
fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Nombre del NAP a partir de sus atributos, con la lista de prioridad.
fn nombre_desde_atributos(atributos: &[(String, String)], bloque: &str) -> String {
    for tag in TAGS_PRIORITARIOS {
        if let Some((_, valor)) = atributos
            .iter()
            .find(|(nombre, _)| nombre.eq_ignore_ascii_case(tag))
        {
            if !valor.trim().is_empty() {
                return valor.clone();
            }
        }
    }
    let primero = atributos
        .iter()
        .map(|(_, valor)| valor)
        .find(|valor| !valor.trim().is_empty());
    primero.cloned().unwrap_or_else(|| bloque.to_string())
}

/// Atributos (tag, valor) de un bloque insertado. El motor los guarda dentro
/// del propio INSERT (`insert.attributes`), no como entidades sueltas.
fn atributos_de(insert: &codec::entities::Insert) -> Vec<(String, String)> {
    insert
        .attributes
        .iter()
        .map(|attribute| (attribute.tag.clone(), attribute.value.clone()))
        .collect()
}

/// Construye el KML de los handles indicados. Devuelve `(kml, nAPs, polígonos)`.
pub(crate) fn exportar(doc: &CadDocument, handles: &[Handle]) -> (String, usize, usize) {
    let mut kml = String::with_capacity(4096);
    kml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    kml.push_str("<kml xmlns=\"http://www.opengis.net/kml/2.2\">\n<Document>\n");
    kml.push_str("  <name>Exportacion LGBCADStudio by Trejo</name>\n");
    kml.push_str(concat!(
        "  <Style id=\"yellowLine\">\n",
        "    <LineStyle><color>ff00ffff</color><width>2</width></LineStyle>\n",
        "    <PolyStyle><color>0000ffff</color><fill>0</fill><outline>1</outline></PolyStyle>\n",
        "  </Style>\n",
        "  <Style id=\"napPaddle\">\n",
        "    <IconStyle>\n      <scale>1.2</scale>\n      <Icon>\n",
        "        <href>http://maps.google.com/mapfiles/kml/paddle/N.png</href>\n",
        "      </Icon>\n    </IconStyle>\n",
        "    <LabelStyle>\n      <scale>1.0</scale>\n      <color>ffffffff</color>\n",
        "    </LabelStyle>\n  </Style>\n",
    ));

    let mut naps = 0usize;
    let mut poligonos = 0usize;

    for handle in handles {
        match doc.get_entity(*handle) {
            Some(EntityType::Insert(insert)) => {
                let (lat, lon) =
                    utm16n_a_latlon(insert.insert_point.x, insert.insert_point.y);
                let atributos = atributos_de(insert);
                let nombre = nombre_desde_atributos(&atributos, &insert.block_name);
                kml.push_str("  <Placemark>\n");
                kml.push_str(&format!("    <name>{}</name>\n", xml_escape(&nombre)));
                kml.push_str("    <styleUrl>#napPaddle</styleUrl>\n    <Point>\n");
                kml.push_str(&format!(
                    "      <coordinates>{lon:.8},{lat:.8},0</coordinates>\n"
                ));
                kml.push_str("    </Point>\n  </Placemark>\n");
                naps += 1;
            }
            Some(EntityType::LwPolyline(polyline)) if polyline.is_closed => {
                let puntos: Vec<(f64, f64)> = polyline
                    .vertices
                    .iter()
                    .map(|vertice| (vertice.location.x, vertice.location.y))
                    .collect();
                if puntos.len() < 3 {
                    continue;
                }
                let capa = if polyline.common.layer.trim().is_empty() {
                    "MICROZONA"
                } else {
                    polyline.common.layer.as_str()
                };
                kml.push_str("  <Placemark>\n");
                kml.push_str(&format!("    <name>{}</name>\n", xml_escape(capa)));
                kml.push_str("    <styleUrl>#yellowLine</styleUrl>\n    <Polygon>\n");
                kml.push_str("      <outerBoundaryIs>\n        <LinearRing>\n");
                kml.push_str("          <coordinates>\n");
                let mut primero = None;
                for (x, y) in &puntos {
                    let (lat, lon) = utm16n_a_latlon(*x, *y);
                    primero.get_or_insert((lat, lon));
                    kml.push_str(&format!("            {lon:.8},{lat:.8},0\n"));
                }
                // El anillo debe cerrarse en el primer vértice.
                if let Some((lat, lon)) = primero {
                    kml.push_str(&format!("            {lon:.8},{lat:.8},0\n"));
                }
                kml.push_str(concat!(
                    "          </coordinates>\n",
                    "        </LinearRing>\n",
                    "      </outerBoundaryIs>\n",
                    "    </Polygon>\n",
                    "  </Placemark>\n",
                ));
                poligonos += 1;
            }
            _ => {}
        }
    }

    kml.push_str("</Document>\n</kml>\n");
    (kml, naps, poligonos)
}

// ── Comando interactivo ─────────────────────────────────────────────────────

use crate::command::{CadCommand, CmdResult, InputKind};
use glam::DVec3;

pub(crate) struct NapaKmlCommand {
    seleccion: Vec<Handle>,
}

impl NapaKmlCommand {
    pub(crate) fn new() -> Self {
        Self {
            seleccion: Vec::new(),
        }
    }
}

impl CadCommand for NapaKmlCommand {
    fn name(&self) -> &'static str {
        "NAPAKML"
    }

    fn prompt(&self) -> String {
        if self.seleccion.is_empty() {
            "NAPAKML  Seleccione bloques NAP y polilíneas cerradas, Enter para exportar:"
                .to_string()
        } else {
            format!(
                "NAPAKML  {} objeto(s) seleccionado(s) — Enter para exportar a KML:",
                self.seleccion.len()
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
        self.seleccion = handles;
        CmdResult::NeedPoint
    }

    fn on_enter(&mut self) -> CmdResult {
        if self.seleccion.is_empty() {
            return CmdResult::Cancel;
        }
        CmdResult::Relaunch(
            "NAPAKML_EXEC".to_string(),
            std::mem::take(&mut self.seleccion),
        )
    }

    fn on_point(&mut self, _pt: DVec3) -> CmdResult {
        CmdResult::NeedPoint
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

// ── Autocomplete registry ───────────────────────────────────────────────────
inventory::submit!(crate::command::CommandRegistration {
    names: &["NAPAKML", "NAPKML"]
});
