//! Generador mínimo de metarchivos EMF (Enhanced Metafile) para incrustar una
//! imagen en un objeto OLE que AutoCAD y nanoCAD dibujan.
//!
//! Un objeto OLE "Picture (Enhanced Metafile)" guarda en su flujo `CONTENTS`
//! un EMF, no un PNG: es lo que comprobamos contra el archivo de nanoCAD. Este
//! módulo produce el EMF más simple que muestra la imagen: un `EMR_HEADER`, un
//! `EMR_STRETCHDIBITS` con la imagen como DIB de 32 bits y un `EMR_EOF`.

/// Firma del encabezado EMF (" EMF").
const EMF_SIGNATURE: u32 = 0x464D_4520;
const EMR_HEADER: u32 = 1;
const EMR_STRETCHDIBITS: u32 = 81;
const EMR_EOF: u32 = 14;
/// ROP: copiar el bitmap tal cual.
const SRCCOPY: u32 = 0x00CC_0020;

/// Convierte píxeles RGBA a un EMF que los dibuja a `width` × `height` píxeles.
pub fn emf_from_rgba(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let ancho = width.max(1);
    let alto = height.max(1);
    // DIB de 32 bits en orden B,G,R,X (BI_RGB), de arriba hacia abajo.
    let mut bits = Vec::with_capacity((ancho * alto * 4) as usize);
    for y in 0..alto {
        for x in 0..ancho {
            let i = (((y * ancho) + x) * 4) as usize;
            let (r, g, b) = if i + 3 < rgba.len() {
                (rgba[i], rgba[i + 1], rgba[i + 2])
            } else {
                (255, 255, 255)
            };
            bits.extend_from_slice(&[b, g, r, 0]);
        }
    }

    // BITMAPINFOHEADER: alto negativo para que las filas vayan de arriba abajo.
    let mut bmi = Vec::with_capacity(40);
    bmi.extend_from_slice(&40u32.to_le_bytes());
    bmi.extend_from_slice(&(ancho as i32).to_le_bytes());
    bmi.extend_from_slice(&(-(alto as i32)).to_le_bytes());
    bmi.extend_from_slice(&1u16.to_le_bytes()); // planos
    bmi.extend_from_slice(&32u16.to_le_bytes()); // bits por pixel
    bmi.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    bmi.extend_from_slice(&((ancho * alto * 4) as u32).to_le_bytes());
    bmi.extend_from_slice(&[0u8; 16]); // resolucion y colores

    // EMR_STRETCHDIBITS: cabecera fija de 80 bytes + BMI + bits.
    let tam_registro = 80 + bmi.len() + bits.len();
    let mut dib = Vec::with_capacity(tam_registro);
    dib.extend_from_slice(&EMR_STRETCHDIBITS.to_le_bytes());
    dib.extend_from_slice(&(tam_registro as u32).to_le_bytes());
    // rclBounds (0,0)-(ancho,alto) como i32.
    for valor in [0i32, 0, ancho as i32, alto as i32] {
        dib.extend_from_slice(&valor.to_le_bytes());
    }
    // xDest, yDest, xSrc, ySrc, cxSrc, cySrc
    for valor in [0i32, 0, 0, 0, ancho as i32, alto as i32] {
        dib.extend_from_slice(&valor.to_le_bytes());
    }
    let off_bmi = 80u32;
    let off_bits = 80 + bmi.len() as u32;
    dib.extend_from_slice(&off_bmi.to_le_bytes());
    dib.extend_from_slice(&(bmi.len() as u32).to_le_bytes());
    dib.extend_from_slice(&off_bits.to_le_bytes());
    dib.extend_from_slice(&(bits.len() as u32).to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes()); // DIB_RGB_COLORS
    dib.extend_from_slice(&SRCCOPY.to_le_bytes());
    dib.extend_from_slice(&(ancho as i32).to_le_bytes()); // cxDest
    dib.extend_from_slice(&(alto as i32).to_le_bytes()); // cyDest
    dib.extend_from_slice(&bmi);
    dib.extend_from_slice(&bits);

    // EMR_EOF
    let mut eof = Vec::with_capacity(20);
    eof.extend_from_slice(&EMR_EOF.to_le_bytes());
    eof.extend_from_slice(&20u32.to_le_bytes());
    eof.extend_from_slice(&0u32.to_le_bytes());
    eof.extend_from_slice(&16u32.to_le_bytes());
    eof.extend_from_slice(&20u32.to_le_bytes());

    // Encabezado: 88 bytes (sin formato de pixel extendido).
    let total = 88 + dib.len() + eof.len();
    let mut cabecera = Vec::with_capacity(88);
    cabecera.extend_from_slice(&EMR_HEADER.to_le_bytes());
    cabecera.extend_from_slice(&88u32.to_le_bytes());
    for valor in [0i32, 0, ancho as i32, alto as i32] {
        cabecera.extend_from_slice(&valor.to_le_bytes()); // rclBounds
    }
    // rclFrame en unidades de 0,01 mm: 1 px = 0,264583 mm = 26,4583 unidades.
    let ancho_mm = (ancho as f64 * 26.4583) as i32;
    let alto_mm = (alto as f64 * 26.4583) as i32;
    for valor in [0i32, 0, ancho_mm, alto_mm] {
        cabecera.extend_from_slice(&valor.to_le_bytes());
    }
    cabecera.extend_from_slice(&EMF_SIGNATURE.to_le_bytes());
    cabecera.extend_from_slice(&0x0001_0000u32.to_le_bytes()); // version 1.0
    cabecera.extend_from_slice(&(total as u32).to_le_bytes());
    cabecera.extend_from_slice(&3u32.to_le_bytes()); // registros
    cabecera.extend_from_slice(&0u16.to_le_bytes()); // handles
    cabecera.extend_from_slice(&0u16.to_le_bytes()); // reservado
    cabecera.extend_from_slice(&0u32.to_le_bytes()); // descripcion
    cabecera.extend_from_slice(&0u32.to_le_bytes());
    cabecera.extend_from_slice(&0u32.to_le_bytes()); // paleta
    cabecera.extend_from_slice(&(ancho as i32).to_le_bytes()); // dispositivo
    cabecera.extend_from_slice(&(alto as i32).to_le_bytes());
    cabecera.extend_from_slice(&ancho_mm.to_le_bytes()); // milimetros
    cabecera.extend_from_slice(&alto_mm.to_le_bytes());

    let mut salida = Vec::with_capacity(total);
    salida.extend_from_slice(&cabecera);
    salida.extend_from_slice(&dib);
    salida.extend_from_slice(&eof);
    salida
}