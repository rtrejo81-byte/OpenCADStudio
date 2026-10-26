//! Imagen satelital georreferenciada (Esri World Imagery).
//!
//! El dibujo está en UTM 16N, así que se pide la imagen **ya reproyectada a
//! EPSG:32616**: las coordenadas del ráster son las mismas del plano y la
//! imagen queda georreferenciada sin transformaciones locales.

/// Resolución objetivo, en metros por píxel. El servicio rechaza pedir más
/// de ~0.6 m/px (devuelve una respuesta vacía), así que se pide 0.62.
const METROS_POR_PIXEL: f64 = 0.62;

/// Área pedida, en coordenadas UTM.
#[derive(Clone, Debug)]
pub struct Pedido {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

/// Imagen descargada, con lo necesario para georreferenciarla.
#[derive(Clone, Debug)]
pub struct Satelite {
    pub nombre: String,
    pub bytes: Vec<u8>,
    pub min_x: f64,
    pub min_y: f64,
    pub ancho: f64,
    pub alto: f64,
    pub px_ancho: u32,
    pub px_alto: u32,
    /// Resolución efectiva de la imagen.
    pub metros_por_pixel: f64,
}

/// Lado máximo que acepta el servicio en una sola descarga.
const TESELA_PX: u32 = 1500;
/// Teselas por lado como máximo (9 descargas).
const MAX_TESELAS: u32 = 3;

/// Píxeles por lado del mosaico completo y metros por píxel efectivos.
///
/// Se apunta a 0.5 m/px; si el área es tan grande que no cabe en el máximo de
/// teselas, la resolución se degrada para no multiplicar las descargas.
pub(crate) fn plan(ancho: f64, alto: f64) -> (u32, u32, f64) {
    let objetivo = METROS_POR_PIXEL;
    let metros_max = (TESELA_PX * MAX_TESELAS) as f64 * objetivo;
    let metros_por_pixel = objetivo.max(ancho / (TESELA_PX * MAX_TESELAS) as f64)
        .max(alto / (TESELA_PX * MAX_TESELAS) as f64);
    let _ = metros_max;
    let px_ancho = ((ancho / metros_por_pixel).round() as u32).max(1);
    let px_alto = ((alto / metros_por_pixel).round() as u32).max(1);
    (px_ancho, px_alto, metros_por_pixel)
}

/// URL de UNA tesela, en UTM 16N.
pub(crate) fn url(
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    px: u32,
    py: u32,
) -> String {
    format!(
        "https://services.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/export\
         ?bbox={min_x:.4},{min_y:.4},{max_x:.4},{max_y:.4}&bboxSR=32616&imageSR=32616\
         &size={px},{py}&format=png&f=image"
    )
}

fn descargar_tesela(agente: &ureq::Agent, url: &str) -> Result<Vec<u8>, String> {
    let mut respuesta = agente
        .get(url)
        .header(
            "User-Agent",
            concat!("lgbcadstudio/", env!("OCS_APP_VERSION")),
        )
        .call()
        .map_err(|error| format!("No se pudo descargar la imagen satelital: {error}"))?;
    let bytes = respuesta
        .body_mut()
        .with_config()
        .limit(48 * 1024 * 1024)
        .read_to_vec()
        .map_err(|error| format!("La descarga se cortó: {error}"))?;
    if bytes.len() < 1024 {
        return Err("El servidor devolvió una imagen vacía.".to_string());
    }
    if bytes.starts_with(b"{") {
        return Err(format!(
            "El servidor rechazó el área pedida: {}",
            String::from_utf8_lossy(&bytes[..bytes.len().min(300)])
        ));
    }
    Ok(bytes)
}

/// Descarga el área como mosaico de teselas y lo deja en `carpeta`.
pub(crate) fn descargar(pedido: &Pedido, carpeta: &std::path::Path) -> Result<Satelite, String> {
    let ancho = pedido.max_x - pedido.min_x;
    let alto = pedido.max_y - pedido.min_y;
    if !(ancho.is_finite() && alto.is_finite()) || ancho <= 0.0 || alto <= 0.0 {
        return Err("El área marcada no es válida.".to_string());
    }
    let (px_ancho, px_alto, metros_por_pixel) = plan(ancho, alto);

    let teselas_x = ((px_ancho + TESELA_PX - 1) / TESELA_PX).max(1);
    let teselas_y = ((px_alto + TESELA_PX - 1) / TESELA_PX).max(1);
    let lado_x = (px_ancho + teselas_x - 1) / teselas_x;
    let lado_y = (px_alto + teselas_y - 1) / teselas_y;
    let ancho_final = lado_x * teselas_x;
    let alto_final = lado_y * teselas_y;
    let escala_x = ancho / ancho_final as f64;
    let escala_y = alto / alto_final as f64;

    let agente = crate::network::agent(std::time::Duration::from_secs(45));
    let mut lienzo = image::RgbaImage::new(ancho_final, alto_final);

    for fila in 0..teselas_y {
        for columna in 0..teselas_x {
            let x0 = pedido.min_x + (columna * lado_x) as f64 * escala_x;
            let y0 = pedido.min_y + (fila * lado_y) as f64 * escala_y;
            let x1 = x0 + lado_x as f64 * escala_x;
            let y1 = y0 + lado_y as f64 * escala_y;
            let bytes = descargar_tesela(&agente, &url(x0, y0, x1, y1, lado_x, lado_y))?;
            let tesela = image::load_from_memory(&bytes)
                .map_err(|error| format!("La imagen recibida no es válida: {error}"))?
                .to_rgba8();
            // La imagen llega con el norte arriba: la fila 0 del lienzo es el
            // borde superior del área.
            let destino_y = (teselas_y - 1 - fila) * lado_y;
            image::imageops::replace(&mut lienzo, &tesela, (columna * lado_x) as i64, destino_y as i64);
        }
    }

    let mut png = std::io::Cursor::new(Vec::new());
    lienzo
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|error| format!("No se pudo componer la imagen: {error}"))?;
    let bytes = png.into_inner();

    std::fs::create_dir_all(carpeta)
        .map_err(|error| format!("No se pudo preparar {}: {error}", carpeta.display()))?;
    let sello = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |transcurrido| transcurrido.as_millis());
    let ruta = carpeta.join(format!("satelite-{sello}.png"));
    std::fs::write(&ruta, &bytes)
        .map_err(|error| format!("No se pudo guardar {}: {error}", ruta.display()))?;

    Ok(Satelite {
        nombre: ruta.to_string_lossy().into_owned(),
        bytes,
        min_x: pedido.min_x,
        min_y: pedido.min_y,
        ancho: ancho_final as f64 * escala_x,
        alto: alto_final as f64 * escala_y,
        px_ancho: ancho_final,
        px_alto: alto_final,
        metros_por_pixel,
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_keeps_the_target_scale_for_small_areas() {
        // 400 m de ancho a 0.5 m/px = 800 px.
        let (px, py, escala) = plan(400.0, 200.0);
        assert_eq!((px, py), (800, 400));
        assert!((escala - 0.5).abs() < 1e-9);
    }

    #[test]
    fn plan_degrades_the_scale_instead_of_exceeding_the_tile_budget() {
        // 30 km: con 3×3 teselas de 1500 px el tope son 4500 px por lado.
        let (px, py, escala) = plan(30_000.0, 15_000.0);
        assert!(px <= 4500 && py <= 4500);
        assert!(escala > 0.5);
    }

    #[test]
    fn url_requests_utm_16n_on_both_sides() {
        let texto = url(470_000.0, 1_510_000.0, 470_200.0, 1_510_100.0, 400, 200);
        assert!(texto.contains("bboxSR=32616"));
        assert!(texto.contains("imageSR=32616"));
        assert!(texto.contains("size=400,200"));
        assert!(texto.contains("470000.0000,1510000.0000"));
    }
}